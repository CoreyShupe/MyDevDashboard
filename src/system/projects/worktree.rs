//! `projects::worktree` part — business logic. Owns DB access for the `worktrees` table and
//! coordinates the on-disk git worktree with the row that tracks it.
//!
//! Invariants enforced here (AGENTS.md §10):
//! - A ticket has at most one LIVE worktree per project (`create` rejects a duplicate).
//! - A ticket's branch is chosen once and SHARED across all its worktrees (`create` reuses the
//!   ticket's existing branch, ignoring the requested one, when there already is one).
//! - Removal leaves a marker (`removed_at` set, folder cleaned); recreation revives the marker.

use sqlx::postgres::PgPool;
use uuid::Uuid;

use crate::domain::projects::Worktree;
use crate::domain::projects::worktree::{folder_name_for_branch, worktree_path};
use crate::error::{AppError, DbError, ProjectError};

use super::git;

#[derive(Clone)]
pub struct WorktreeService {
    pool: PgPool,
}

impl WorktreeService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
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
    pub async fn create(
        &self,
        project_id: Uuid,
        ticket_id: Uuid,
        requested_branch: &str,
    ) -> Result<Worktree, AppError> {
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

        let name = folder_name_for_branch(&branch);
        if name.is_empty() {
            return Err(ProjectError::Empty {
                field: "branch name",
            }
            .into());
        }
        self.provision(project_id, ticket_id, &name, &branch).await
    }

    /// Recreate a previously-removed worktree from its historical marker (same branch + folder).
    pub async fn recreate(&self, id: Uuid) -> Result<Worktree, AppError> {
        let row = self.row(id).await?;
        self.provision(row.project_id, row.ticket_id, &row.name, &row.branch)
            .await
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

    /// Create the on-disk worktree (new branch or existing) and upsert its row (reviving a
    /// marker if one exists for this project+ticket). Git runs FIRST: a git failure leaves no
    /// row, so the DB never claims a worktree that isn't there.
    async fn provision(
        &self,
        project_id: Uuid,
        ticket_id: Uuid,
        name: &str,
        branch: &str,
    ) -> Result<Worktree, AppError> {
        let repo = self.project_path(project_id).await?;
        let path = worktree_path(&repo, name);
        // If the target folder already exists, adopt it as-is: skip `git worktree add` (and branch
        // creation) entirely. This happens when a folder was left on disk after its rows were
        // cascade-deleted (deleting a profile or project drops worktree ROWS but not folders —
        // §9, §10); git would otherwise error on the existing path. We trust the structure is set
        // up for us and just (re)create the tracking row below.
        if !path.exists() {
            let new_branch = !git::branch_exists(&repo, branch).await;
            git::worktree_add(&repo, &path, branch, new_branch).await?;
        }

        sqlx::query_as::<_, Worktree>(
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
        .map_err(|source| {
            DbError::Query {
                context: "upsert worktree",
                source,
            }
            .into()
        })
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
