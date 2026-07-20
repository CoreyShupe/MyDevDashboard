//! `projects::worktree` part — business logic. Owns DB access for the `worktrees` table and
//! coordinates the on-disk git worktree with the row that tracks it.
//!
//! Invariants enforced here (AGENTS.md §10):
//! - A ticket has at most one LIVE worktree per project (`create` rejects a duplicate).
//! - A ticket's branch is chosen once and SHARED across all its worktrees (`create` reuses the
//!   ticket's existing branch, ignoring the requested one, when there already is one).
//! - Removal leaves a marker (`removed_at` set, folder cleaned); recreation revives the marker.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use sqlx::postgres::PgPool;
use uuid::Uuid;

use crate::domain::projects::worktree::{checked_worktree_path, worktree_path};
use crate::domain::projects::{Worktree, WorktreeBusy};
use crate::error::{AppError, DbError, ProjectError};

use super::git;

#[derive(Clone)]
pub struct WorktreeService {
    pool: PgPool,
    /// Worktrees currently being provisioned, keyed by `(project_id, ticket_id)`. Provisioning
    /// shells out to git AND runs the project's (possibly slow) setup script, so it's spawned off
    /// the worker loop and this guard surfaces the in-flight state in the `View` — the ticket/
    /// project detail shows a loading row until it lands, and the worktree isn't presented as ready
    /// until setup has finished (AGENTS.md §10). `Arc` so all `Backend` clones share one set; the
    /// guard also dedupes a double-click into a single provision.
    creating: Arc<Mutex<HashSet<(Uuid, Uuid)>>>,
    /// Existing worktrees with a slow action in flight, keyed by worktree id → what it's doing
    /// (`Remove`/`Open`). Both shell out (git / the editor launcher), so they're spawned off the
    /// worker loop and this guard surfaces the "waiting" state in the `View` — the worktree row
    /// swaps its buttons for a spinner until it lands (AGENTS.md §10). Keyed by worktree id (unlike
    /// `creating`, which has none yet). `Arc` so all `Backend` clones share it; the guard also
    /// dedupes a double-click into a single run.
    busy: Arc<Mutex<HashMap<Uuid, WorktreeBusy>>>,
}

impl WorktreeService {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            creating: Arc::new(Mutex::new(HashSet::new())),
            busy: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Try to claim the "creating" guard for a `(project, ticket)` worktree. Returns `true` if this
    /// call started the provision, or `false` if one was already in flight — so callers skip
    /// spawning a duplicate `git worktree add` + setup run for the same worktree.
    pub fn begin_create(&self, project_id: Uuid, ticket_id: Uuid) -> bool {
        self.lock_creating().insert((project_id, ticket_id))
    }

    /// Release a worktree's "creating" guard once its provision finishes (success or failure).
    pub fn end_create(&self, project_id: Uuid, ticket_id: Uuid) {
        self.lock_creating().remove(&(project_id, ticket_id));
    }

    /// The `(project_id, ticket_id)` pairs whose worktree is being provisioned right now (drives
    /// the loading rows in the ticket + project detail views).
    pub fn creating_ids(&self) -> HashSet<(Uuid, Uuid)> {
        self.lock_creating().clone()
    }

    /// Lock the creating set, recovering a poisoned guard (a poisoned mutex must not crash the app
    /// — §3 bans `unwrap`/`expect`), same policy as the project service's caches.
    fn lock_creating(&self) -> std::sync::MutexGuard<'_, HashSet<(Uuid, Uuid)>> {
        self.creating
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Try to claim the "busy" guard for a worktree with `action`. Returns `true` if this call
    /// started the action, or `false` if one was already in flight for that worktree — so callers
    /// skip spawning a duplicate remove/open for the same worktree.
    pub fn begin_busy(&self, id: Uuid, action: WorktreeBusy) -> bool {
        self.lock_busy().insert(id, action).is_none()
    }

    /// Release a worktree's "busy" guard once its action finishes (success or failure).
    pub fn end_busy(&self, id: Uuid) {
        self.lock_busy().remove(&id);
    }

    /// The worktrees with a slow action in flight right now (drives their loading rows).
    pub fn busy_ids(&self) -> HashMap<Uuid, WorktreeBusy> {
        self.lock_busy().clone()
    }

    /// Lock the busy map, recovering a poisoned guard (same no-panic policy as `lock_creating`).
    fn lock_busy(&self) -> std::sync::MutexGuard<'_, HashMap<Uuid, WorktreeBusy>> {
        self.busy
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Every worktree (live AND historical markers) across a profile's projects, oldest first.
    /// The UI filters these by project or ticket in memory.
    pub async fn list_for_profile(&self, profile_id: Uuid) -> Result<Vec<Worktree>, DbError> {
        sqlx::query_as::<_, Worktree>(
            "SELECT w.id, w.project_id, w.ticket_id, w.name, w.branch, w.removed_at, w.created_at \
             FROM worktrees w JOIN projects p ON w.project_id = p.id \
             WHERE p.profile_id = $1 ORDER BY w.created_at ASC",
        )
        .bind(profile_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|source| DbError::Query {
            context: "list worktrees",
            source,
        })
    }

    /// The branch a ticket's worktrees share, if it already has one (any row, live or marker).
    pub async fn branch_for_ticket(&self, ticket_id: Uuid) -> Result<Option<String>, DbError> {
        sqlx::query_scalar::<_, String>(
            "SELECT branch FROM worktrees WHERE ticket_id = $1 ORDER BY created_at ASC LIMIT 1",
        )
        .bind(ticket_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|source| DbError::Query {
            context: "load ticket worktree branch",
            source,
        })
    }

    /// Create a worktree for a ticket in a project. `requested_branch` is used ONLY when the
    /// ticket has no worktree yet; otherwise the ticket's existing (shared) branch wins.
    ///
    /// Returns the worktree AND whether it was *freshly* provisioned (`true`) vs. an existing
    /// folder adopted (`false`) — the caller runs [`run_setup`](Self::run_setup) only when fresh.
    /// Setup is deliberately NOT run here so a failing setup script can't undo an
    /// already-created worktree (AGENTS.md §10).
    pub async fn create(
        &self,
        project_id: Uuid,
        ticket_id: Uuid,
        requested_branch: &str,
    ) -> Result<(Worktree, bool), AppError> {
        // Branch is a ticket-level choice, shared across projects (AGENTS.md §10).
        let branch = match self.branch_for_ticket(ticket_id).await? {
            Some(existing) => existing,
            None => {
                let requested = requested_branch.trim();
                if requested.is_empty() {
                    return Err(ProjectError::Empty {
                        field: "branch name",
                    }
                    .into());
                }
                requested.to_owned()
            }
        };

        // Enforce one live worktree per (project, ticket).
        if let Some(existing) = self.row_for(project_id, ticket_id).await?
            && existing.is_live()
        {
            return Err(ProjectError::WorktreeExists {
                project: self.project_name(project_id).await?,
            }
            .into());
        }

        // The folder name IS the branch now: worktrees nest under `.dev-dash/worktrees/{repo}/`,
        // and a valid git branch name is already a valid (possibly nested) relative path (§10).
        self.provision(project_id, ticket_id, &branch, &branch)
            .await
    }

    /// Recreate a previously-removed worktree from its historical marker (same branch + folder).
    /// Since removal deleted the folder, this provisions afresh (returns `fresh = true`), so the
    /// caller re-runs the project's setup script, just like a first creation (AGENTS.md §10).
    pub async fn recreate(&self, id: Uuid) -> Result<(Worktree, bool), AppError> {
        let row = self.row(id).await?;
        self.provision(row.project_id, row.ticket_id, &row.name, &row.branch)
            .await
    }

    /// Run the project's setup script inside a worktree's folder (e.g. `bun install`), if the
    /// project has one (an empty script is a no-op). Called by the app layer AFTER a fresh
    /// provision, as a SEPARATE step so its failure surfaces to the owner without undoing the
    /// already-created worktree — the worktree still "succeeds", the error is just shown so they
    /// can fix it and re-run (AGENTS.md §10).
    pub async fn run_setup(&self, worktree: &Worktree) -> Result<(), AppError> {
        let (repo, script) = self.project_path_and_script(worktree.project_id).await?;
        if script.trim().is_empty() {
            return Ok(());
        }
        git::run_setup_script(&worktree_path(&repo, &worktree.name), &script).await?;
        Ok(())
    }

    /// The `(project_id, ticket_id)` a worktree row belongs to — used to key the in-flight
    /// "creating" guard when recreating (which only has the worktree id to start from).
    pub async fn ids_of(&self, id: Uuid) -> Result<(Uuid, Uuid), AppError> {
        let row = self.row(id).await?;
        Ok((row.project_id, row.ticket_id))
    }

    /// Remove a live worktree: clean the folder via git, then leave the row as a marker. If git
    /// refuses (uncommitted changes) the error surfaces and the row stays live (AGENTS.md §10).
    pub async fn remove(&self, id: Uuid) -> Result<(), AppError> {
        let row = self.row(id).await?;
        if row.is_live() {
            let repo = self.project_path(row.project_id).await?;
            git::worktree_remove(&repo, &worktree_path(&repo, &row.name)).await?;
        }
        sqlx::query("UPDATE worktrees SET removed_at = now() WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|source| DbError::Query {
                context: "mark worktree removed",
                source,
            })?;
        tracing::info!("removed worktree '{}' (kept as a marker)", row.name);
        Ok(())
    }

    /// Open a worktree's folder in VS Code.
    pub async fn open_in_editor(&self, id: Uuid) -> Result<(), AppError> {
        let row = self.row(id).await?;
        let repo = self.project_path(row.project_id).await?;
        git::open_in_vscode(&worktree_path(&repo, &row.name)).await?;
        Ok(())
    }

    /// Best-effort cleanup of a ticket's live worktree folders before the ticket (and, via
    /// cascade, these rows) is deleted. Cross-feature entry point: `tasks::ticket` delete calls
    /// this so deleting a ticket doesn't orphan folders on disk. A folder git won't remove
    /// (uncommitted work) is logged and left behind rather than blocking the ticket delete.
    pub async fn remove_all_for_ticket(&self, ticket_id: Uuid) -> Result<(), AppError> {
        let live = sqlx::query_as::<_, (String, String)>(
            "SELECT w.name, p.path FROM worktrees w JOIN projects p ON w.project_id = p.id \
             WHERE w.ticket_id = $1 AND w.removed_at IS NULL",
        )
        .bind(ticket_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|source| DbError::Query {
            context: "list ticket worktrees for cleanup",
            source,
        })?;

        for (name, repo) in live {
            if let Err(e) = git::worktree_remove(&repo, &worktree_path(&repo, &name)).await {
                tracing::warn!(
                    error = %e,
                    worktree = %name,
                    "leaving an orphaned worktree folder while deleting its ticket"
                );
            }
        }
        Ok(())
    }

    /// Reconcile the DB against disk: any live worktree whose folder has vanished (e.g. the
    /// owner deleted it outside the app) becomes a marker, so counts stay accurate and it can
    /// be recreated later (AGENTS.md §10). Run before building a projects snapshot.
    pub async fn reconcile(&self, profile_id: Uuid) -> Result<(), AppError> {
        let rows = sqlx::query_as::<_, (Uuid, String, String)>(
            "SELECT w.id, w.name, p.path FROM worktrees w JOIN projects p ON w.project_id = p.id \
             WHERE p.profile_id = $1 AND w.removed_at IS NULL",
        )
        .bind(profile_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|source| DbError::Query {
            context: "list worktrees for reconcile",
            source,
        })?;

        for (id, name, repo) in rows {
            if !worktree_path(&repo, &name).exists() {
                sqlx::query(
                    "UPDATE worktrees SET removed_at = now() WHERE id = $1 AND removed_at IS NULL",
                )
                .bind(id)
                .execute(&self.pool)
                .await
                .map_err(|source| DbError::Query {
                    context: "reconcile vanished worktree",
                    source,
                })?;
            }
        }
        Ok(())
    }

    /// Create the on-disk worktree (new branch or existing) and upsert its row (reviving a marker
    /// if one exists for this project+ticket). Git runs FIRST: a git failure leaves no row, so the
    /// DB never claims a worktree that isn't there. Returns the row plus whether it was *freshly*
    /// created (`true`) vs. an existing folder adopted (`false`) so the caller knows whether to run
    /// the setup script. Setup is intentionally NOT run here (see [`create`](Self::create)).
    async fn provision(
        &self,
        project_id: Uuid,
        ticket_id: Uuid,
        name: &str,
        branch: &str,
    ) -> Result<(Worktree, bool), AppError> {
        let repo = self.project_path(project_id).await?;
        // Resolve the on-disk path and REFUSE a name that doesn't land exactly where we assume it
        // will (a `..` traversal / absolute component escaping the worktree root) — before we
        // touch git or the filesystem. Everything below then operates on a path we've proven stays
        // inside `{repo}/.dev-dash/worktrees/{repo}/` (AGENTS.md §10).
        let path =
            checked_worktree_path(&repo, name).ok_or_else(|| ProjectError::InvalidBranch {
                branch: branch.to_owned(),
            })?;
        // If the target folder already exists, adopt it as-is: skip `git worktree add` (and branch
        // creation) entirely. This happens when a folder was left on disk after its rows were
        // cascade-deleted (deleting a profile or project drops worktree ROWS but not folders —
        // §9, §10); git would otherwise error on the existing path. We trust the structure is set
        // up for us and just (re)create the tracking row below.
        let fresh = !path.exists();
        if fresh {
            let new_branch = !git::branch_exists(&repo, branch).await;
            tracing::info!(
                "provisioning fresh worktree at {} (branch {branch}, new_branch={new_branch})",
                path.display()
            );
            git::worktree_add(&repo, &path, branch, new_branch).await?;
        } else {
            // Adopting an existing folder runs NO git command, so log it explicitly — otherwise a
            // "why didn't the setup script run?" would have no trace (adopted folders skip setup).
            tracing::info!(
                "adopting existing worktree folder at {} (branch {branch}) — skipping git add + setup",
                path.display()
            );
        }

        let worktree = sqlx::query_as::<_, Worktree>(
            "INSERT INTO worktrees (id, project_id, ticket_id, name, branch) \
             VALUES ($1, $2, $3, $4, $5) \
             ON CONFLICT (project_id, ticket_id) \
             DO UPDATE SET name = EXCLUDED.name, branch = EXCLUDED.branch, removed_at = NULL \
             RETURNING id, project_id, ticket_id, name, branch, removed_at, created_at",
        )
        .bind(Uuid::new_v4())
        .bind(project_id)
        .bind(ticket_id)
        .bind(name)
        .bind(branch)
        .fetch_one(&self.pool)
        .await
        .map_err(|source| DbError::Query {
            context: "upsert worktree",
            source,
        })?;

        Ok((worktree, fresh))
    }

    /// Load one worktree row by id, or a typed "missing" error.
    async fn row(&self, id: Uuid) -> Result<Worktree, AppError> {
        sqlx::query_as::<_, Worktree>(
            "SELECT id, project_id, ticket_id, name, branch, removed_at, created_at \
             FROM worktrees WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|source| DbError::Query {
            context: "load worktree",
            source,
        })?
        .ok_or_else(|| ProjectError::WorktreeMissing { id: id.to_string() }.into())
    }

    /// The (project, ticket) worktree row if one exists (live or marker).
    async fn row_for(
        &self,
        project_id: Uuid,
        ticket_id: Uuid,
    ) -> Result<Option<Worktree>, DbError> {
        sqlx::query_as::<_, Worktree>(
            "SELECT id, project_id, ticket_id, name, branch, removed_at, created_at \
             FROM worktrees WHERE project_id = $1 AND ticket_id = $2",
        )
        .bind(project_id)
        .bind(ticket_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|source| DbError::Query {
            context: "load worktree for project+ticket",
            source,
        })
    }

    /// A project's repository path on disk.
    async fn project_path(&self, project_id: Uuid) -> Result<String, AppError> {
        sqlx::query_scalar::<_, String>("SELECT path FROM projects WHERE id = $1")
            .bind(project_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|source| DbError::Query {
                context: "load project path",
                source,
            })?
            .ok_or_else(|| {
                DbError::NotFound {
                    entity: "project",
                    id: project_id.to_string(),
                }
                .into()
            })
    }

    /// A project's repository path AND its setup script (both needed to provision a worktree),
    /// fetched together so provision does one round-trip instead of two.
    async fn project_path_and_script(
        &self,
        project_id: Uuid,
    ) -> Result<(String, String), AppError> {
        sqlx::query_as::<_, (String, String)>(
            "SELECT path, setup_script FROM projects WHERE id = $1",
        )
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|source| DbError::Query {
            context: "load project path and setup script",
            source,
        })?
        .ok_or_else(|| {
            DbError::NotFound {
                entity: "project",
                id: project_id.to_string(),
            }
            .into()
        })
    }

    /// A project's display name (for error messages); falls back to the id if it's gone.
    async fn project_name(&self, project_id: Uuid) -> Result<String, AppError> {
        Ok(
            sqlx::query_scalar::<_, String>("SELECT name FROM projects WHERE id = $1")
                .bind(project_id)
                .fetch_optional(&self.pool)
                .await
                .map_err(|source| DbError::Query {
                    context: "load project name",
                    source,
                })?
                .unwrap_or_else(|| project_id.to_string()),
        )
    }
}
