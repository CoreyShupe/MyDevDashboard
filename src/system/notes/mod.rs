//! `notes` feature — business logic. Owns DB access for the `uncategorized_notes` table.
//!
//! No egui here (AGENTS.md §2). Filing a note *onto a ticket* is NOT done here — that's a
//! cross-feature action orchestrated in `app/notes` (it reaches into `tasks` too); this
//! service only knows about the uncategorized-notes table itself.

use sqlx::postgres::PgPool;
use uuid::Uuid;

use crate::domain::notes::Note;
use crate::error::{AppError, DbError, TaskError};

#[derive(Clone)]
pub struct NotesService {
    pool: PgPool,
}

impl NotesService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// All uncategorized notes in a profile, newest first (most-recent capture on top).
    pub async fn list(&self, profile_id: Uuid) -> Result<Vec<Note>, DbError> {
        sqlx::query_as::<_, Note>(
            "SELECT id, body, created_at FROM uncategorized_notes \
             WHERE profile_id = $1 ORDER BY created_at DESC",
        )
        .bind(profile_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|source| DbError::Query {
            context: "list uncategorized notes",
            source,
        })
    }

    /// Capture a new note in a profile. Body is required.
    pub async fn add(&self, profile_id: Uuid, body: &str) -> Result<Note, AppError> {
        let body = body.trim();
        if body.is_empty() {
            return Err(TaskError::Empty { field: "note" }.into());
        }

        let note = sqlx::query_as::<_, Note>(
            "INSERT INTO uncategorized_notes (id, profile_id, body) VALUES ($1, $2, $3) \
             RETURNING id, body, created_at",
        )
        .bind(Uuid::new_v4())
        .bind(profile_id)
        .bind(body)
        .fetch_one(&self.pool)
        .await
        .map_err(|source| DbError::Query {
            context: "add uncategorized note",
            source,
        })?;

        Ok(note)
    }

    /// Remove a note from the uncategorized list (e.g. once it's been filed into a ticket).
    pub async fn delete(&self, id: Uuid) -> Result<(), AppError> {
        let affected = sqlx::query("DELETE FROM uncategorized_notes WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|source| DbError::Query {
                context: "delete uncategorized note",
                source,
            })?
            .rows_affected();

        if affected == 0 {
            return Err(DbError::NotFound {
                entity: "note",
                id: id.to_string(),
            }
            .into());
        }
        Ok(())
    }
}
