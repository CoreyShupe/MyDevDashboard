//! `todos` feature — domain type.
//!
//! A "todo": a quick, easy task the owner just needs to remember, without the ceremony of a
//! ticket. It's the raw material behind the Todos tab, and works like an uncategorized note —
//! but a todo is something to *do*, so it carries a `done` flag the note doesn't.
//!
//! Kept intentionally minimal (see AGENTS.md §2 — this feature is a single concept today,
//! like `notes`/`profile`, so it has one `mod.rs` per layer rather than nested parts).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// A quick task to remember. `profile_id` scopes it (AGENTS.md §9) but isn't surfaced on the
/// type — the query already scopes to the active profile, mirroring `notes::Note`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, FromRow)]
pub struct Todo {
    pub id: Uuid,
    pub body: String,
    /// Whether the task has been checked off. Open todos sort above done ones.
    pub done: bool,
    pub created_at: DateTime<Utc>,
}
