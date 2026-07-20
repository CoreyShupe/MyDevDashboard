//! `tasks::note` part — business logic. Owns DB access for the `notes` table.

use sqlx::postgres::PgPool;
use uuid::Uuid;

use crate::domain::tasks::Note;
use crate::error::{AppError, DbError, TaskError};

#[derive(Clone)]
pub struct NoteService {
    pool: PgPool,
}

impl NoteService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// All notes for a ticket, oldest first.
    pub async fn list_for_ticket(&self, ticket_id: Uuid) -> Result<Vec<Note>, DbError> {
        sqlx::query_as::<_, Note>(
            "SELECT id, ticket_id, body, created_at FROM notes \
             WHERE ticket_id = $1 ORDER BY created_at ASC",
        )
        .bind(ticket_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|source| DbError::Query {
            context: "list notes",
            source,
        })
    }

    /// Add a note to a ticket. Body is required.
    pub async fn add(&self, ticket_id: Uuid, body: &str) -> Result<Note, AppError> {
        let body = body.trim();
        if body.is_empty() {
            return Err(TaskError::Empty { field: "note" }.into());
        }

        let note = sqlx::query_as::<_, Note>(
            "INSERT INTO notes (id, ticket_id, body) VALUES ($1, $2, $3) \
             RETURNING id, ticket_id, body, created_at",
        )
        .bind(Uuid::new_v4())
        .bind(ticket_id)
        .bind(body)
        .fetch_one(&self.pool)
        .await
        .map_err(|source| DbError::Query {
            context: "add note",
            source,
        })?;

        Ok(note)
    }
}
