//! `projects::project` part — business logic. Owns DB access for the `projects` table, plus
//! the live git-status reads that enrich each project (delegated to [`super::git`]).

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use chrono::Utc;
use sqlx::postgres::PgPool;
use uuid::Uuid;

use crate::domain::projects::{GitStatus, Project};
use crate::error::{AppError, DbError, ProjectError};

use super::git;

/// Git status per repo path, refreshed together and stamped with one check time.
type StatusCache = HashMap<String, GitStatus>;

#[derive(Clone)]
pub struct ProjectService {
    pool: PgPool,
    /// Session cache of git status per repo path. Git is a (possibly network-bound) shell-out,
    /// so it runs only on open + on explicit refresh (AGENTS.md §10); every other snapshot reads
    /// this cache instead of re-fetching. `Arc` so all `Backend` clones share one cache.
    status_cache: Arc<Mutex<StatusCache>>,
    /// Whether a git refresh is in flight. Surfaced in the `View` so the projects tab can show a
    /// loading state; a CAS guard (see [`begin_refresh`](Self::begin_refresh)) keeps concurrent
    /// refreshes from piling up. `Arc` so all `Backend` clones observe the same flag.
    refreshing: Arc<AtomicBool>,
}

impl ProjectService {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            status_cache: Arc::new(Mutex::new(HashMap::new())),
            refreshing: Arc::new(AtomicBool::new(false)),
        }
    }

    /// All projects in a profile, newest first (most-recently added on top).
    pub async fn list(&self, profile_id: Uuid) -> Result<Vec<Project>, DbError> {
        sqlx::query_as::<_, Project>(
            "SELECT id, profile_id, name, path, created_at, updated_at FROM projects \
             WHERE profile_id = $1 ORDER BY created_at DESC",
        )
        .bind(profile_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|source| DbError::Query {
            context: "list projects",
            source,
        })
    }

    /// Register an existing local repository as a project. Validates that the path exists and
    /// is a git repo — this tool points at repos on disk, it never clones (AGENTS.md §10).
    pub async fn create(
        &self,
        profile_id: Uuid,
        name: &str,
        path: &str,
    ) -> Result<Project, AppError> {
        let name = name.trim();
        let path = path.trim();
        if name.is_empty() {
            return Err(ProjectError::Empty {
                field: "project name",
            }
            .into());
        }
        if path.is_empty() {
            return Err(ProjectError::Empty {
                field: "repository path",
            }
            .into());
        }
        if !Path::new(path).is_dir() {
            return Err(ProjectError::PathMissing {
                path: path.to_owned(),
            }
            .into());
        }
        if !git::status(path).await.is_repo {
            return Err(ProjectError::NotARepo {
                path: path.to_owned(),
            }
            .into());
        }

        sqlx::query_as::<_, Project>(
            "INSERT INTO projects (id, profile_id, name, path) VALUES ($1, $2, $3, $4) \
             RETURNING id, profile_id, name, path, created_at, updated_at",
        )
        .bind(Uuid::new_v4())
        .bind(profile_id)
        .bind(name)
        .bind(path)
        .fetch_one(&self.pool)
        .await
        .map_err(|source| {
            DbError::Query {
                context: "create project",
                source,
            }
            .into()
        })
    }

    /// Hard-delete a project (its worktree rows cascade). The on-disk repo is untouched — we
    /// only forget about it here.
    pub async fn delete(&self, id: Uuid) -> Result<(), AppError> {
        let affected = sqlx::query("DELETE FROM projects WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|source| DbError::Query {
                context: "delete project",
                source,
            })?
            .rows_affected();

        if affected == 0 {
            return Err(DbError::NotFound {
                entity: "project",
                id: id.to_string(),
            }
            .into());
        }
        Ok(())
    }

    /// The cached git status for each path, order-preserving. Never shells out — returns the
    /// last [`refresh_statuses`](Self::refresh_statuses) result, or an unchecked default
    /// (`checked_at = None`) for a path not refreshed yet this session. This is what every
    /// snapshot uses, so a mutation doesn't pay for a `git fetch`.
    pub fn cached_statuses(&self, paths: &[String]) -> Vec<GitStatus> {
        let cache = self.lock_cache();
        paths
            .iter()
            .map(|p| cache.get(p).cloned().unwrap_or_default())
            .collect()
    }

    /// Refetch live git status for these paths — the (network) `git fetch` + reads — concurrently
    /// and order-preserving, stamp each with the check time, and update the cache. The ONLY path
    /// that shells out for status; called on open and on explicit refresh (AGENTS.md §10). Never
    /// errors (see [`git::status`]).
    pub async fn refresh_statuses(&self, paths: Vec<String>) {
        let now = Utc::now();
        let statuses = git::statuses(paths.clone()).await;
        let mut cache = self.lock_cache();
        for (path, mut status) in paths.into_iter().zip(statuses) {
            status.checked_at = Some(now);
            cache.insert(path, status);
        }
    }

    /// Whether a git refresh is currently in flight (drives the projects tab's loading state).
    pub fn is_refreshing(&self) -> bool {
        self.refreshing.load(Ordering::Acquire)
    }

    /// Try to claim the "refreshing" flag. Returns `true` if this call started a refresh, or
    /// `false` if one was already running — so callers can skip spawning a duplicate fetch.
    pub fn begin_refresh(&self) -> bool {
        self.refreshing
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }

    /// Release the "refreshing" flag once a refresh finishes.
    pub fn end_refresh(&self) {
        self.refreshing.store(false, Ordering::Release);
    }

    /// Lock the status cache, recovering the guard if a previous holder panicked (a poisoned
    /// mutex must not take the whole app down — §3 bans `unwrap`/`expect` in app code).
    fn lock_cache(&self) -> std::sync::MutexGuard<'_, StatusCache> {
        self.status_cache
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}
