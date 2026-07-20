//! `todos` feature — business logic. Owns DB access for the `todos` table.
//!
//! No egui here (AGENTS.md §2). Mirrors `notes` closely: a flat list scoped to a profile, with
//! add/delete — plus a `done` toggle, the one thing a task has that a note doesn't.

use sqlx::postgres::PgPool;
use uuid::Uuid;

use crate::domain::todos::Todo;
use crate::error::{AppError, DbError, TaskError};

#[derive(Clone)]
pub struct TodosService {
    pool: PgPool,
}

impl TodosService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// All todos in a profile: open ones first, newest capture on top within each group.
    pub async fn list(&self, profile_id: Uuid) -> Result<Vec<Todo>, DbError> {
        sqlx::query_as::<_, Todo>(
            "SELECT id, body, done, created_at FROM todos \
             WHERE profile_id = $1 ORDER BY done ASC, created_at DESC",
        )
        .bind(profile_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|source| DbError::Query {
            context: "list todos",
            source,
        })
    }

    /// Capture a new todo in a profile. Body is required. Starts open (`done = false`).
    pub async fn add(&self, profile_id: Uuid, body: &str) -> Result<Todo, AppError> {
        let body = body.trim();
        if body.is_empty() {
            return Err(TaskError::Empty { field: "todo" }.into());
        }

        let todo = sqlx::query_as::<_, Todo>(
            "INSERT INTO todos (id, profile_id, body) VALUES ($1, $2, $3) \
             RETURNING id, body, done, created_at",
        )
        .bind(Uuid::new_v4())
        .bind(profile_id)
        .bind(body)
        .fetch_one(&self.pool)
        .await
        .map_err(|source| DbError::Query {
            context: "add todo",
            source,
        })?;

        Ok(todo)
    }

    /// Check a todo off (or back on).
    pub async fn set_done(&self, id: Uuid, done: bool) -> Result<(), AppError> {
        let affected = sqlx::query("UPDATE todos SET done = $1 WHERE id = $2")
            .bind(done)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|source| DbError::Query {
                context: "set todo done",
                source,
            })?
            .rows_affected();

        if affected == 0 {
            return Err(DbError::NotFound {
                entity: "todo",
                id: id.to_string(),
            }
            .into());
        }
        Ok(())
    }

    /// Remove a todo.
    pub async fn delete(&self, id: Uuid) -> Result<(), AppError> {
        let affected = sqlx::query("DELETE FROM todos WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|source| DbError::Query {
                context: "delete todo",
                source,
            })?
            .rows_affected();

        if affected == 0 {
            return Err(DbError::NotFound {
                entity: "todo",
                id: id.to_string(),
            }
            .into());
        }
        Ok(())
    }
}
