//! Database connection pool + migrations. The only place a pool is created.

use std::time::Duration;

use sqlx::postgres::{PgPool, PgPoolOptions};

use crate::config::Config;
use crate::error::DbError;

/// Embedded migrations from `migrations/`, applied at startup.
static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

/// Connect to PostgreSQL and run pending migrations.
///
/// Returns a distinct [`DbError::Connect`] (with a fix hint) if the DB is unreachable,
/// or [`DbError::Migrate`] if a migration fails — never panics.
pub async fn connect_and_migrate(config: &Config) -> Result<PgPool, DbError> {
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&config.database_url)
        .await
        .map_err(|source| DbError::Connect {
            target: config.redacted_target(),
            source,
        })?;

    MIGRATOR
        .run(&pool)
        .await
        .map_err(|source| DbError::Migrate { source })?;

    Ok(pool)
}
