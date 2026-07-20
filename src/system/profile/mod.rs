//! `profile` feature — business logic. Owns all DB access for the `profiles` table.
//!
//! Profiles are the top-level CONTAINERS (AGENTS.md §9): every stage/ticket/note belongs to
//! one. Multiple profiles can exist; exactly one is "active" (its workspace is shown). This
//! service creates profiles, lists them for the switcher, and tracks/flips the active one.

use sqlx::postgres::PgPool;
use uuid::Uuid;

use crate::domain::profile::Profile;
use crate::error::{AppError, DbError, TaskError};

/// Service for the owner's profiles (onboarding, switching, listing).
#[derive(Clone)]
pub struct ProfileService {
    pool: PgPool,
}

impl ProfileService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Every profile, oldest first — the switcher's menu.
    pub async fn list(&self) -> Result<Vec<Profile>, DbError> {
        sqlx::query_as::<_, Profile>(
            "SELECT id, display_name, created_at FROM profiles ORDER BY created_at ASC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|source| DbError::Query {
            context: "list profiles",
            source,
        })
    }

    /// The active profile (the one whose workspace is shown), or `None` on first run.
    ///
    /// Prefers the explicitly-active row; falls back to the oldest profile so a DB migrated
    /// from the single-profile era (no active flag set yet) still resolves to a profile.
    pub async fn active(&self) -> Result<Option<Profile>, DbError> {
        sqlx::query_as::<_, Profile>(
            "SELECT id, display_name, created_at FROM profiles \
             ORDER BY is_active DESC, created_at ASC LIMIT 1",
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|source| DbError::Query {
            context: "load active profile",
            source,
        })
    }

    /// Create a profile and make it the active one. Rejects an empty name.
    pub async fn create(&self, display_name: &str) -> Result<Profile, AppError> {
        let name = display_name.trim();
        if name.is_empty() {
            return Err(TaskError::Empty { field: "name" }.into());
        }

        let profile = sqlx::query_as::<_, Profile>(
            "INSERT INTO profiles (id, display_name) VALUES ($1, $2) \
             RETURNING id, display_name, created_at",
        )
        .bind(Uuid::new_v4())
        .bind(name)
        .fetch_one(&self.pool)
        .await
        .map_err(|source| DbError::Query {
            context: "create profile",
            source,
        })?;

        // A newly-created profile becomes the one you're working in.
        self.set_active(profile.id).await?;
        Ok(profile)
    }

    /// Make `id` the active profile (all others become inactive). Single-statement flip keeps
    /// the "at most one active" index satisfied.
    pub async fn set_active(&self, id: Uuid) -> Result<(), AppError> {
        sqlx::query("UPDATE profiles SET is_active = (id = $1)")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|source| DbError::Query {
                context: "set active profile",
                source,
            })?;
        Ok(())
    }
}
