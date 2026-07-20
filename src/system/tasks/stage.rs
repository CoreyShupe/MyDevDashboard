//! `tasks::stage` part — business logic. Owns DB access for the `stages` table.

use sqlx::postgres::PgPool;
use uuid::Uuid;

use crate::domain::tasks::Stage;
use crate::error::{AppError, DbError, TaskError};

#[derive(Clone)]
pub struct StageService {
    pool: PgPool,
}

impl StageService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// All stages in a profile, ordered left-to-right.
    pub async fn list(&self, profile_id: Uuid) -> Result<Vec<Stage>, DbError> {
        sqlx::query_as::<_, Stage>(
            "SELECT id, name, position, terminal, created_at FROM stages \
             WHERE profile_id = $1 ORDER BY position ASC, created_at ASC",
        )
        .bind(profile_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|source| DbError::Query {
            context: "list stages",
            source,
        })
    }

    /// Create a new stage in a profile, appended to the right of its board.
    pub async fn create(&self, profile_id: Uuid, name: &str) -> Result<Stage, AppError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(TaskError::Empty {
                field: "stage name",
            }
            .into());
        }

        // Position is per-profile so each board numbers its columns from zero.
        let next_pos: i32 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(position), -1) + 1 FROM stages WHERE profile_id = $1",
        )
        .bind(profile_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|source| DbError::Query {
            context: "compute next stage position",
            source,
        })?;

        let stage = sqlx::query_as::<_, Stage>(
            "INSERT INTO stages (id, profile_id, name, position) VALUES ($1, $2, $3, $4) \
             RETURNING id, name, position, terminal, created_at",
        )
        .bind(Uuid::new_v4())
        .bind(profile_id)
        .bind(name)
        .bind(next_pos)
        .fetch_one(&self.pool)
        .await
        .map_err(|source| DbError::Query {
            context: "create stage",
            source,
        })?;

        Ok(stage)
    }

    /// Rename an existing stage.
    pub async fn rename(&self, id: Uuid, name: &str) -> Result<(), AppError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(TaskError::Empty {
                field: "stage name",
            }
            .into());
        }

        let affected = sqlx::query("UPDATE stages SET name = $1 WHERE id = $2")
            .bind(name)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|source| DbError::Query {
                context: "rename stage",
                source,
            })?
            .rows_affected();

        if affected == 0 {
            return Err(DbError::NotFound {
                entity: "stage",
                id: id.to_string(),
            }
            .into());
        }
        Ok(())
    }

    /// Persist a new left-to-right stage order: each id's `position` becomes its index in
    /// `ids`. Ids come from the active profile's board, so we update by id.
    pub async fn reorder(&self, ids: &[Uuid]) -> Result<(), AppError> {
        for (index, id) in ids.iter().enumerate() {
            sqlx::query("UPDATE stages SET position = $1 WHERE id = $2")
                .bind(index as i32)
                .bind(id)
                .execute(&self.pool)
                .await
                .map_err(|source| DbError::Query {
                    context: "reorder stages",
                    source,
                })?;
        }
        Ok(())
    }

    /// Mark a stage as terminal (an end state) or not.
    pub async fn set_terminal(&self, id: Uuid, terminal: bool) -> Result<(), AppError> {
        let affected = sqlx::query("UPDATE stages SET terminal = $1 WHERE id = $2")
            .bind(terminal)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|source| DbError::Query {
                context: "set stage terminal",
                source,
            })?
            .rows_affected();

        if affected == 0 {
            return Err(DbError::NotFound {
                entity: "stage",
                id: id.to_string(),
            }
            .into());
        }
        Ok(())
    }

    /// Delete a stage, but only if it holds no tickets.
    ///
    /// Reads the `tickets` table to enforce the rule — a deliberate, contained cross-part
    /// query kept here because "a stage cannot be deleted while non-empty" is a stage rule.
    pub async fn delete(&self, id: Uuid) -> Result<(), AppError> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tickets WHERE stage_id = $1")
            .bind(id)
            .fetch_one(&self.pool)
            .await
            .map_err(|source| DbError::Query {
                context: "count tickets in stage",
                source,
            })?;

        if count > 0 {
            // Fetch the name for a clearer message; fall back to the id.
            let name: Option<String> = sqlx::query_scalar("SELECT name FROM stages WHERE id = $1")
                .bind(id)
                .fetch_optional(&self.pool)
                .await
                .map_err(|source| DbError::Query {
                    context: "load stage name",
                    source,
                })?;
            return Err(TaskError::StageNotEmpty {
                stage: name.unwrap_or_else(|| id.to_string()),
                count,
            }
            .into());
        }

        let affected = sqlx::query("DELETE FROM stages WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|source| DbError::Query {
                context: "delete stage",
                source,
            })?
            .rows_affected();

        if affected == 0 {
            return Err(DbError::NotFound {
                entity: "stage",
                id: id.to_string(),
            }
            .into());
        }
        Ok(())
    }
}
