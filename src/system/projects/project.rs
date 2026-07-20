//! `projects::project` part — business logic. Owns DB access for the `projects` table, plus
//! the live git-status reads that enrich each project (delegated to [`super::git`]).

use std::path::Path;

use sqlx::postgres::PgPool;
use uuid::Uuid;

use crate::domain::projects::{GitStatus, Project};
use crate::error::{AppError, DbError, ProjectError};

use super::git;

#[derive(Clone)]
pub struct ProjectService {
    pool: PgPool,
}

impl ProjectService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
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

    /// Live git status for a set of project paths, concurrently and order-preserving. Never
    /// errors (see [`git::status`]).
    pub async fn statuses(&self, paths: Vec<String>) -> Vec<GitStatus> {
        git::statuses(paths).await
    }
}
