//! `projects::project` part — business logic. Owns DB access for the `projects` table, plus
//! the live git-status reads that enrich each project (delegated to [`super::git`]).

use std::collections::{HashMap, HashSet};
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
    /// Projects with a `git pull --rebase` currently in flight, by id. Its own guard (separate
    /// from `refreshing`, which is git-status fetches) so a pull's loading state can't be confused
    /// with a refresh, and — critically — so a second Pull click on the same project while one is
    /// running is refused at the source ([`begin_pull`](Self::begin_pull)): never two concurrent
    /// pulls on one repo. Surfaced per-card in the `View`. `Arc` so all `Backend` clones share it.
    pulling: Arc<Mutex<HashSet<Uuid>>>,
}

impl ProjectService {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            status_cache: Arc::new(Mutex::new(HashMap::new())),
            refreshing: Arc::new(AtomicBool::new(false)),
            pulling: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    /// All projects in a profile, newest first (most-recently added on top).
    pub async fn list(&self, profile_id: Uuid) -> Result<Vec<Project>, DbError> {
        sqlx::query_as::<_, Project>(
            "SELECT id, profile_id, name, path, setup_script, created_at, updated_at FROM projects \
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
             RETURNING id, profile_id, name, path, setup_script, created_at, updated_at",
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

    /// One-click **Pull** for a project: `git pull --rebase origin <branch>`, then refetch ONLY
    /// this project's git status into the cache (AGENTS.md §10 — the owner asked to pull this repo
    /// specifically, so we don't re-fetch the whole grid). Guarded here, not just in the UI: the
    /// branch is re-read live and refused unless it's a shared branch (`main`/`develop`), so a
    /// stale card can't drive a pull on a feature branch.
    pub async fn pull(&self, id: Uuid) -> Result<(), AppError> {
        let path = self.path_of(id).await?;
        let branch = git::current_branch(&path)
            .await
            .filter(|b| GitStatus::is_pullable_branch(b))
            .ok_or_else(|| ProjectError::NotPullable { path: path.clone() })?;
        git::pull_rebase(&path, &branch).await?;
        // Refetch just this repo so its card reflects the post-pull state (in sync, clean).
        self.refresh_statuses(vec![path]).await;
        Ok(())
    }

    /// Try to claim the "pulling" guard for a project. Returns `true` if this call started the
    /// pull, or `false` if one was already in flight for that project — so callers skip spawning a
    /// duplicate `git pull --rebase` on the same repo (the system-level dedupe).
    pub fn begin_pull(&self, id: Uuid) -> bool {
        self.lock_pulling().insert(id)
    }

    /// Release a project's "pulling" guard once its pull finishes (success or failure).
    pub fn end_pull(&self, id: Uuid) {
        self.lock_pulling().remove(&id);
    }

    /// The set of projects with a pull in flight (drives each card's pulling state in the `View`).
    pub fn pulling_ids(&self) -> HashSet<Uuid> {
        self.lock_pulling().clone()
    }

    /// Lock the pulling set, recovering a poisoned guard (a poisoned mutex must not crash the app
    /// — §3 bans `unwrap`/`expect`), same policy as [`lock_cache`](Self::lock_cache).
    fn lock_pulling(&self) -> std::sync::MutexGuard<'_, HashSet<Uuid>> {
        self.pulling
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// A project's repository path on disk, or a typed "not found" if the row is gone.
    async fn path_of(&self, id: Uuid) -> Result<String, AppError> {
        sqlx::query_scalar::<_, String>("SELECT path FROM projects WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|source| DbError::Query {
                context: "load project path",
                source,
            })?
            .ok_or_else(|| {
                DbError::NotFound {
                    entity: "project",
                    id: id.to_string(),
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

    /// Set (or clear, when empty) a project's setup script — the bash run inside each new worktree
    /// (AGENTS.md §10). Stored verbatim; it's only ever executed on worktree creation, never here.
    pub async fn set_setup_script(&self, id: Uuid, script: &str) -> Result<(), AppError> {
        let affected =
            sqlx::query("UPDATE projects SET setup_script = $2, updated_at = now() WHERE id = $1")
                .bind(id)
                .bind(script)
                .execute(&self.pool)
                .await
                .map_err(|source| DbError::Query {
                    context: "update project setup script",
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
