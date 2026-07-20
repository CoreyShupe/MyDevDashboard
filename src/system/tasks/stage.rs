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

    /// All stages, ordered left-to-right.
    pub async fn list(&self) -> Result<Vec<Stage>, DbError> {
        sqlx::query_as::<_, Stage>(
            "SELECT id, name, position, created_at FROM stages ORDER BY position ASC, created_at ASC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|source| DbError::Query {
            context: "list stages",
            source,
        })
    }

    /// Create a new stage appended to the right of the board.
    pub async fn create(&self, name: &str) -> Result<Stage, AppError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(TaskError::Empty {
                field: "stage name",
            }
            .into());
        }

        let next_pos: i32 =
            sqlx::query_scalar("SELECT COALESCE(MAX(position), -1) + 1 FROM stages")
                .fetch_one(&self.pool)
                .await
                .map_err(|source| DbError::Query {
                    context: "compute next stage position",
                    source,
                })?;

        let stage = sqlx::query_as::<_, Stage>(
            "INSERT INTO stages (id, name, position) VALUES ($1, $2, $3) \
             RETURNING id, name, position, created_at",
        )
        .bind(Uuid::new_v4())
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
