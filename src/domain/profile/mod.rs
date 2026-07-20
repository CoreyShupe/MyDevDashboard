//! `profile` feature — domain types.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// The owner's profile. This is a single-user tool; the first profile is "the" profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, FromRow)]
pub struct Profile {
    pub id: Uuid,
    pub display_name: String,
    pub created_at: DateTime<Utc>,
}
