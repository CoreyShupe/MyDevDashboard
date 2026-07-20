//! `notes` feature — domain type.
//!
//! An "uncategorized note": a quick, free-text capture that hasn't been filed into a
//! ticket yet. It's the raw material behind the Notes tab. Distinct from `tasks::Note`
//! (which is always attached to a ticket) — this one belongs to nothing until the owner
//! turns it into a ticket or adds it onto one.
//!
//! Kept intentionally minimal (see AGENTS.md §2 — this feature is a single concept today,
//! like `profile`, so it has one `mod.rs` per layer rather than nested parts).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// A free-text note not yet filed into a ticket.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, FromRow)]
pub struct Note {
    pub id: Uuid,
    pub body: String,
    pub created_at: DateTime<Utc>,
}
