//! All typed errors for the dashboard. See AGENTS.md §3.
//!
//! Rules embodied here:
//! - Every *known* failure has a distinct, typed variant carrying context, so it is
//!   obvious WHERE and WHAT went wrong.
//! - Sub-errors (`ConfigError`, `DbError`, `TaskError`) roll up into `AppError` via `#[from]`.
//! - `#[error("…")]` messages say what failed and, where actionable, how to fix it.
//! - `UserFacingError` is the shape the UI renders in a modal: title + detail + remediation.

use thiserror::Error;

/// Top-level error. Everything fallible in the app ultimately becomes one of these.
#[derive(Debug, Error)]
pub enum AppError {
    #[error(transparent)]
    Config(#[from] ConfigError),

    #[error(transparent)]
    Db(#[from] DbError),

    #[error(transparent)]
    Task(#[from] TaskError),
}

/// Configuration / environment problems (loading `.env`, reading `DATABASE_URL`, …).
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error(
        "required environment variable `{key}` is not set. Add it to your `.env` (see .env.example)"
    )]
    MissingEnv { key: &'static str },

    #[error("environment variable `{key}` is invalid: {reason}")]
    InvalidEnv { key: &'static str, reason: String },
}

/// Database problems. Each distinct cause is its own variant with context.
#[derive(Debug, Error)]
pub enum DbError {
    #[error(
        "cannot reach PostgreSQL at `{target}`. Is the database running? Start it with `./scripts/db-up.sh`"
    )]
    Connect {
        target: String,
        #[source]
        source: sqlx::Error,
    },

    #[error(
        "database migration failed: {source}. Try `./scripts/db-reset.sh` if the schema is corrupt"
    )]
    Migrate {
        #[source]
        source: sqlx::migrate::MigrateError,
    },

    #[error("{entity} `{id}` was not found")]
    NotFound { entity: &'static str, id: String },

    #[error("database query `{context}` failed: {source}")]
    Query {
        context: &'static str,
        #[source]
        source: sqlx::Error,
    },
}

/// Task-domain (stages/tickets/notes) validation & rule violations.
#[derive(Debug, Error)]
pub enum TaskError {
    #[error("a {field} is required and cannot be empty")]
    Empty { field: &'static str },

    #[error(
        "cannot delete stage `{stage}`: it still contains {count} ticket(s). Move or delete them first"
    )]
    StageNotEmpty { stage: String, count: i64 },
}

/// The user-aware view of an error, rendered by the UI modal.
///
/// Carries a short `title`, a `detail` describing what happened, a concrete `remediation`
/// telling the owner how to fix it, and whether re-attempting is likely to help (so the
/// modal can offer a **Retry** button — used for database outages).
#[derive(Debug, Clone)]
pub struct UserFacingError {
    pub title: String,
    pub detail: String,
    pub remediation: String,
    /// True when retrying (reconnecting + reloading) may resolve it — i.e. DB problems.
    pub retryable: bool,
}

impl UserFacingError {
    /// Translate any [`AppError`] into an actionable, owner-facing message.
    pub fn from_app_error(err: &AppError) -> Self {
        match err {
            AppError::Config(e) => Self {
                title: "Configuration problem".to_owned(),
                detail: e.to_string(),
                remediation: "Check your `.env` against `.env.example`, then restart the app."
                    .to_owned(),
                retryable: false,
            },
            AppError::Db(DbError::Connect { .. }) => Self {
                title: "Database unavailable".to_owned(),
                detail: err.to_string(),
                remediation: "Start PostgreSQL with `./scripts/db-up.sh`, then press Retry."
                    .to_owned(),
                retryable: true,
            },
            AppError::Db(DbError::Migrate { .. }) => Self {
                title: "Database migration failed".to_owned(),
                detail: err.to_string(),
                remediation:
                    "Inspect the DB, or run `./scripts/db-reset.sh` for a clean schema, then Retry."
                        .to_owned(),
                retryable: true,
            },
            // Any other DB failure (query/not-found) is worth retrying — the connection
            // may have blipped; retrying reconnects and reloads without losing state.
            AppError::Db(_) => Self {
                title: "Database error".to_owned(),
                detail: err.to_string(),
                remediation: "Press Retry. If it persists, check the console logs.".to_owned(),
                retryable: true,
            },
            AppError::Task(_) => Self {
                title: "Couldn't complete that".to_owned(),
                detail: err.to_string(),
                remediation: "Adjust the input and try again.".to_owned(),
                retryable: false,
            },
        }
    }
}
