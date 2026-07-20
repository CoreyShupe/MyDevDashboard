//! `tasks::stage` part — domain type.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// A configurable board column (ticket stage), e.g. "Pending" / "In Progress".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, FromRow)]
pub struct Stage {
    pub id: Uuid,
    pub name: String,
    /// Left-to-right ordering on the board.
    pub position: i32,
    /// A terminal stage is an end state (e.g. "Complete", "Cancelled"): on the board it
    /// collapses to a ticket count, and its tickets are hidden from the "Add to ticket" picker.
    pub terminal: bool,
    pub created_at: DateTime<Utc>,
}
