//! `profile` feature — business logic. Owns all DB access for the `profiles` table.

use sqlx::postgres::PgPool;
use uuid::Uuid;

use crate::domain::profile::Profile;
use crate::error::{AppError, DbError, TaskError};

/// Service for the owner's profile (onboarding / "setup profile").
#[derive(Clone)]
pub struct ProfileService {
    pool: PgPool,
}

impl ProfileService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Load the existing profile, if onboarding has been completed.
    pub async fn current(&self) -> Result<Option<Profile>, DbError> {
        sqlx::query_as::<_, Profile>(
            "SELECT id, display_name, created_at FROM profiles ORDER BY created_at ASC LIMIT 1",
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|source| DbError::Query {
            context: "load current profile",
            source,
        })
    }

    /// Create the owner's profile with the given display name.
    ///
    /// Rejects an empty name with a distinct [`TaskError::Empty`].
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

        Ok(profile)
    }
}
