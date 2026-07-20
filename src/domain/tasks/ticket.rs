//! `tasks::ticket` part — domain type.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// A unit of work living in exactly one stage. May optionally have a parent ticket,
/// forming a parent → children hierarchy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, FromRow)]
pub struct Ticket {
    pub id: Uuid,
    pub stage_id: Uuid,
    pub title: String,
    pub description: String,
    /// Ordering within a stage column.
    pub position: i32,
    /// The parent ticket, if this ticket is a child. `None` for top-level tickets.
    pub parent_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
