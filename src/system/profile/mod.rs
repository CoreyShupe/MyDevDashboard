//! `profile` feature — business logic. Owns all DB access for the `profiles` table.
//!
//! Profiles are the top-level CONTAINERS (AGENTS.md §9): every stage/ticket/note belongs to
//! one. Multiple profiles can exist; exactly one is "active" (its workspace is shown). This
//! service creates profiles, lists them for the switcher, and tracks/flips the active one.

use sqlx::postgres::PgPool;
use uuid::Uuid;

use crate::domain::profile::{Profile, ProfileView};
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
            "SELECT id, display_name, created_at, last_view FROM profiles ORDER BY created_at ASC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|source| DbError::Query {
            context: "list profiles",
            source,
        })
    }

    /// The explicitly-active profile (the one whose workspace is shown), or `None`.
    ///
    /// `None` means "no profile is active" — either first run (no profiles at all) or the active
    /// profile was just deleted. The UI turns that into onboarding (no profiles) or a profile
    /// picker (others remain). There is deliberately NO fallback to the oldest profile: after a
    /// delete the owner chooses which workspace to open rather than being dropped into one.
    pub async fn active(&self) -> Result<Option<Profile>, DbError> {
        sqlx::query_as::<_, Profile>(
            "SELECT id, display_name, created_at, last_view FROM profiles WHERE is_active LIMIT 1",
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

        // `last_view` is omitted so it takes its DEFAULT ('tasks') — a new profile lands on the
        // main dashboard (§9). RETURNING reads it back for the caller.
        let profile = sqlx::query_as::<_, Profile>(
            "INSERT INTO profiles (id, display_name) VALUES ($1, $2) \
             RETURNING id, display_name, created_at, last_view",
        )
        .bind(Uuid::new_v4())
        .bind(name)
        .fetch_one(&self.pool)
        .await
        .map_err(|source| DbError::Query {
            context: "create profile",
            source,
        })?;

        tracing::info!(
            "created profile '{}' ({})",
            profile.display_name,
            profile.id
        );
        // A newly-created profile becomes the one you're working in.
        self.set_active(profile.id).await?;
        Ok(profile)
    }

    /// Make `id` the active profile (all others become inactive). Done in a transaction that
    /// CLEARS every active row first, then sets the target — a single `SET is_active = (id = $1)`
    /// is checked per-row by the (non-deferrable) `idx_profiles_one_active` partial index, so if
    /// the new row is visited before the old active one is cleared, two rows are transiently
    /// active and the update fails with a duplicate-key violation. Clearing first can't collide.
    pub async fn set_active(&self, id: Uuid) -> Result<(), AppError> {
        tracing::info!("switching active profile to {id}");
        let mut tx = self.pool.begin().await.map_err(|source| DbError::Query {
            context: "set active profile (begin)",
            source,
        })?;
        sqlx::query("UPDATE profiles SET is_active = FALSE WHERE is_active AND id <> $1")
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(|source| DbError::Query {
                context: "set active profile (clear)",
                source,
            })?;
        sqlx::query("UPDATE profiles SET is_active = TRUE WHERE id = $1")
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(|source| DbError::Query {
                context: "set active profile (set)",
                source,
            })?;
        tx.commit().await.map_err(|source| DbError::Query {
            context: "set active profile (commit)",
            source,
        })?;
        Ok(())
    }

    /// Persist which workspace page `id` was last viewing (AGENTS.md §9). Bound as its stored
    /// string form; a no-op-shaped `UPDATE` that just records where the owner was.
    pub async fn set_last_view(&self, id: Uuid, view: ProfileView) -> Result<(), AppError> {
        sqlx::query("UPDATE profiles SET last_view = $1 WHERE id = $2")
            .bind(view.as_str())
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|source| DbError::Query {
                context: "set profile last view",
                source,
            })?;
        Ok(())
    }

    /// Delete a profile and, via `ON DELETE CASCADE`, its ENTIRE workspace — stages, tickets,
    /// notes, projects, worktree rows, and todos (AGENTS.md §9). Deliberately does NOT activate
    /// another profile: if this was the active one, the app lands on the picker/onboarding so the
    /// owner chooses next (see [`active`](Self::active)). On-disk worktree folders are left as-is
    /// (same as project delete) — the create guard adopts them if a worktree is ever remade.
    pub async fn delete(&self, id: Uuid) -> Result<(), AppError> {
        let affected = sqlx::query("DELETE FROM profiles WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|source| DbError::Query {
                context: "delete profile",
                source,
            })?
            .rows_affected();

        if affected == 0 {
            return Err(DbError::NotFound {
                entity: "profile",
                id: id.to_string(),
            }
            .into());
        }
        tracing::warn!("deleted profile {id} and its entire workspace (cascade)");
        Ok(())
    }
}
