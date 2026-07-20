//! `tasks::ticket` part — business logic. Owns DB access for the `tickets` table.

use sqlx::postgres::PgPool;
use uuid::Uuid;

use crate::domain::tasks::Ticket;
use crate::error::{AppError, DbError, TaskError};

#[derive(Clone)]
pub struct TicketService {
    pool: PgPool,
}

impl TicketService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// All tickets across every stage, ordered by in-column position.
    pub async fn list(&self) -> Result<Vec<Ticket>, DbError> {
        sqlx::query_as::<_, Ticket>(
            "SELECT id, stage_id, title, description, position, parent_id, created_at, updated_at \
             FROM tickets ORDER BY position ASC, created_at ASC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|source| DbError::Query {
            context: "list tickets",
            source,
        })
    }

    /// Create a ticket in a stage, optionally linked to a `parent_id`. Title is required.
    pub async fn create(
        &self,
        stage_id: Uuid,
        title: &str,
        description: &str,
        parent_id: Option<Uuid>,
    ) -> Result<Ticket, AppError> {
        let title = title.trim();
        if title.is_empty() {
            return Err(TaskError::Empty {
                field: "ticket title",
            }
            .into());
        }

        let next_pos: i32 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(position), -1) + 1 FROM tickets WHERE stage_id = $1",
        )
        .bind(stage_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|source| DbError::Query {
            context: "compute next ticket position",
            source,
        })?;

        let ticket = sqlx::query_as::<_, Ticket>(
            "INSERT INTO tickets (id, stage_id, title, description, position, parent_id) \
             VALUES ($1, $2, $3, $4, $5, $6) \
             RETURNING id, stage_id, title, description, position, parent_id, created_at, updated_at",
        )
        .bind(Uuid::new_v4())
        .bind(stage_id)
        .bind(title)
        .bind(description.trim())
        .bind(next_pos)
        .bind(parent_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|source| DbError::Query {
            context: "create ticket",
            source,
        })?;

        Ok(ticket)
    }

    /// Create a child ticket under `parent_id`, placed in the parent's stage.
    pub async fn create_child(
        &self,
        parent_id: Uuid,
        title: &str,
        description: &str,
    ) -> Result<Ticket, AppError> {
        let parent_stage: Option<Uuid> =
            sqlx::query_scalar("SELECT stage_id FROM tickets WHERE id = $1")
                .bind(parent_id)
                .fetch_optional(&self.pool)
                .await
                .map_err(|source| DbError::Query {
                    context: "load parent ticket stage",
                    source,
                })?;

        let stage_id = parent_stage.ok_or_else(|| DbError::NotFound {
            entity: "parent ticket",
            id: parent_id.to_string(),
        })?;

        self.create(stage_id, title, description, Some(parent_id))
            .await
    }

    /// Set (or clear, with `None`) a ticket's parent.
    pub async fn set_parent(&self, id: Uuid, parent_id: Option<Uuid>) -> Result<(), AppError> {
        let affected =
            sqlx::query("UPDATE tickets SET parent_id = $1, updated_at = now() WHERE id = $2")
                .bind(parent_id)
                .bind(id)
                .execute(&self.pool)
                .await
                .map_err(|source| DbError::Query {
                    context: "set ticket parent",
                    source,
                })?
                .rows_affected();

        if affected == 0 {
            return Err(DbError::NotFound {
                entity: "ticket",
                id: id.to_string(),
            }
            .into());
        }
        Ok(())
    }

    /// Update a ticket's title and description.
    pub async fn update(
        &self,
        id: Uuid,
        title: &str,
        description: &str,
    ) -> Result<Ticket, AppError> {
        let title = title.trim();
        if title.is_empty() {
            return Err(TaskError::Empty {
                field: "ticket title",
            }
            .into());
        }

        let ticket = sqlx::query_as::<_, Ticket>(
            "UPDATE tickets SET title = $1, description = $2, updated_at = now() \
             WHERE id = $3 \
             RETURNING id, stage_id, title, description, position, parent_id, created_at, updated_at",
        )
        .bind(title)
        .bind(description.trim())
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|source| DbError::Query {
            context: "update ticket",
            source,
        })?;

        ticket.ok_or_else(|| {
            DbError::NotFound {
                entity: "ticket",
                id: id.to_string(),
            }
            .into()
        })
    }

    /// Move a ticket to a different stage, appending it to that column.
    pub async fn move_to_stage(&self, id: Uuid, new_stage_id: Uuid) -> Result<(), AppError> {
        let next_pos: i32 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(position), -1) + 1 FROM tickets WHERE stage_id = $1",
        )
        .bind(new_stage_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|source| DbError::Query {
            context: "compute move target position",
            source,
        })?;

        let affected = sqlx::query(
            "UPDATE tickets SET stage_id = $1, position = $2, updated_at = now() WHERE id = $3",
        )
        .bind(new_stage_id)
        .bind(next_pos)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(|source| DbError::Query {
            context: "move ticket",
            source,
        })?
        .rows_affected();

        if affected == 0 {
            return Err(DbError::NotFound {
                entity: "ticket",
                id: id.to_string(),
            }
            .into());
        }
        Ok(())
    }

    /// Delete a ticket (and its notes, via ON DELETE CASCADE).
    pub async fn delete(&self, id: Uuid) -> Result<(), AppError> {
        let affected = sqlx::query("DELETE FROM tickets WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|source| DbError::Query {
                context: "delete ticket",
                source,
            })?
            .rows_affected();

        if affected == 0 {
            return Err(DbError::NotFound {
                entity: "ticket",
                id: id.to_string(),
            }
            .into());
        }
        Ok(())
    }
}
