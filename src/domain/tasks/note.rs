//! `tasks::note` part — domain type.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// A free-text note attached to a ticket.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, FromRow)]
pub struct Note {
    pub id: Uuid,
    pub ticket_id: Uuid,
    pub body: String,
    pub created_at: DateTime<Utc>,
}
