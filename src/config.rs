//! Loads runtime configuration from the environment (`.env` + process env).

use crate::error::ConfigError;

/// Application configuration resolved at boot.
#[derive(Debug, Clone)]
pub struct Config {
    /// PostgreSQL connection string.
    pub database_url: String,
}

impl Config {
    /// Load config from `.env` (if present) and the process environment.
    ///
    /// Missing/invalid values produce a distinct [`ConfigError`] rather than a panic.
    pub fn load() -> Result<Self, ConfigError> {
        // Best-effort: a missing .env is fine (real env vars may be set instead).
        let _ = dotenvy::dotenv();

        let database_url = match std::env::var("DATABASE_URL") {
            Ok(v) if !v.trim().is_empty() => v,
            Ok(_) => {
                return Err(ConfigError::InvalidEnv {
                    key: "DATABASE_URL",
                    reason: "value is empty".to_owned(),
                });
            }
            Err(_) => {
                return Err(ConfigError::MissingEnv {
                    key: "DATABASE_URL",
                });
            }
        };

        Ok(Self { database_url })
    }

    /// A version of the connection target safe to show/log (no password).
    pub fn redacted_target(&self) -> String {
        redact_url(&self.database_url)
    }
}

/// Strip credentials from a postgres URL for safe logging/display.
fn redact_url(url: &str) -> String {
    // postgres://user:pass@host:port/db -> host:port/db
    match url.split_once("://") {
        Some((_scheme, rest)) => match rest.split_once('@') {
            Some((_creds, host_and_db)) => host_and_db.to_owned(),
            None => rest.to_owned(),
        },
        None => "<database>".to_owned(),
    }
}
