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
    /// The workspace page this profile was last viewing — persisted so switching profiles or
    /// relaunching restores where the owner left off (AGENTS.md §9). Defaults to the main
    /// dashboard (`Tasks`), which is where a freshly-created profile lands.
    pub last_view: ProfileView,
}

/// Which workspace page a profile is on — the persisted "last viewed page". Mirrors the UI's
/// nav tabs; kept in `domain` because it is stored per profile (the `profiles.last_view` TEXT
/// column). The UI `Tab` converts to/from this.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ProfileView {
    /// The main dashboard (Tasks board) — the default landing page.
    #[default]
    Tasks,
    Notes,
    Todos,
    Projects,
}

impl ProfileView {
    /// The stored string form (the `profiles.last_view` value).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Tasks => "tasks",
            Self::Notes => "notes",
            Self::Todos => "todos",
            Self::Projects => "projects",
        }
    }

    /// Parse the stored form, falling back to the default (main dashboard) for anything
    /// unrecognized so a stray or legacy value never breaks navigation.
    pub fn from_db(s: &str) -> Self {
        match s {
            "notes" => Self::Notes,
            "todos" => Self::Todos,
            "projects" => Self::Projects,
            _ => Self::Tasks,
        }
    }
}

// Persisted as TEXT (see migration 0009). We map it transparently over `String` so `Profile`
// keeps deriving `FromRow`; `from_db` guards against unknown values on decode. We bind it as a
// `&str` (`as_str`) when writing, so no `Encode` impl is needed here.
impl sqlx::Type<sqlx::Postgres> for ProfileView {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        <String as sqlx::Type<sqlx::Postgres>>::type_info()
    }

    fn compatible(ty: &sqlx::postgres::PgTypeInfo) -> bool {
        <String as sqlx::Type<sqlx::Postgres>>::compatible(ty)
    }
}

impl<'r> sqlx::Decode<'r, sqlx::Postgres> for ProfileView {
    fn decode(value: sqlx::postgres::PgValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
        let s = <String as sqlx::Decode<sqlx::Postgres>>::decode(value)?;
        Ok(ProfileView::from_db(&s))
    }
}
