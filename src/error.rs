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

    #[error(transparent)]
    Profile(#[from] ProfileError),

    #[error(transparent)]
    Project(#[from] ProjectError),

    #[error(transparent)]
    Process(#[from] ProcessError),
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
        "cannot reach PostgreSQL at `{target}`. Is the database running? Start it with `dev-dash db up`"
    )]
    Connect {
        target: String,
        #[source]
        source: sqlx::Error,
    },

    #[error(
        "database migration failed: {source}. Try `dev-dash db reset` if the schema is corrupt"
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

/// Profile-scoping violations. Every stage/ticket/note belongs to a profile (AGENTS.md §9),
/// so an action that needs an active profile has none is a distinct, typed failure.
#[derive(Debug, Error)]
pub enum ProfileError {
    #[error("no active profile — create or select a profile first")]
    NoActive,
}

/// Projects/worktrees domain validation & rule violations (AGENTS.md §10). Distinct from
/// [`ProcessError`] (a git/editor command actually failing) — these are refusals *before* we
/// ever shell out, because an input or a rule doesn't hold.
#[derive(Debug, Error)]
pub enum ProjectError {
    #[error("a {field} is required and cannot be empty")]
    Empty { field: &'static str },

    #[error(
        "the path `{path}` does not exist on disk. Enter the path to a repository you already have"
    )]
    PathMissing { path: String },

    #[error(
        "`{path}` is not a git repository. This tool points at existing local repos (it never clones)"
    )]
    NotARepo { path: String },

    #[error(
        "this ticket already has a worktree in project `{project}`. A ticket may have only one worktree per project"
    )]
    WorktreeExists { project: String },

    #[error("worktree `{id}` was not found")]
    WorktreeMissing { id: String },

    #[error(
        "`{branch}` is not a valid branch name — it resolves outside its worktree directory (e.g. a `..` or an absolute path). Use a plain branch name"
    )]
    InvalidBranch { branch: String },

    #[error(
        "`{path}` can't be pulled from here — the one-click pull only runs on a shared `main`/`develop` branch. Pull it yourself for anything else"
    )]
    NotPullable { path: String },
}

/// An external command we shelled out to (git, or the editor launcher) failed. Kept separate
/// from [`ProjectError`] so "git refused / isn't installed" reads differently from "your input
/// was invalid". Git *status* reads never produce this (they degrade to empty fields); only
/// explicit actions (worktree add/remove, open in editor) surface it.
#[derive(Debug, Error)]
pub enum ProcessError {
    #[error("could not run `{program}`: {source}. Is it installed and on your PATH?")]
    Spawn {
        program: String,
        #[source]
        source: std::io::Error,
    },

    #[error("`{program}` failed while {context}: {stderr}")]
    Exited {
        program: String,
        context: &'static str,
        stderr: String,
    },
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
                remediation: "Start PostgreSQL with `dev-dash db up`, then press Retry.".to_owned(),
                retryable: true,
            },
            AppError::Db(DbError::Migrate { .. }) => Self {
                title: "Database migration failed".to_owned(),
                detail: err.to_string(),
                remediation:
                    "Inspect the DB, or run `dev-dash db reset` for a clean schema, then Retry."
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
            AppError::Profile(_) => Self {
                title: "No active profile".to_owned(),
                detail: err.to_string(),
                remediation: "Create or select a profile, then try again.".to_owned(),
                retryable: false,
            },
            AppError::Project(_) => Self {
                title: "Couldn't complete that".to_owned(),
                detail: err.to_string(),
                remediation: "Adjust the input and try again.".to_owned(),
                retryable: false,
            },
            AppError::Process(_) => Self {
                title: "A command failed".to_owned(),
                detail: err.to_string(),
                remediation:
                    "Check the repository state and that git (and VS Code, for opening) are \
                     installed, then try again."
                        .to_owned(),
                retryable: false,
            },
        }
    }
}
