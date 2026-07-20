//! Dev-only view overrides for fast visual review (AGENTS.md §8).
//!
//! Gated entirely behind the `DEV_VIEW` env var; a normal run never touches this. It
//! injects MOCK in-memory state only — no DB, no seeding — so any screen can be captured
//! without a database or hand-entered data. When active, the app ignores worker snapshots
//! so the mock state stays put.

use chrono::Utc;
use uuid::Uuid;

use crate::app::{ViewData, profile, tasks};
use crate::domain::profile::Profile;
use crate::domain::tasks::{Stage, Ticket};
use crate::error::UserFacingError;

/// Which screen to force. Selected via `DEV_VIEW={onboarding|board|ticket|page|error}`.
#[derive(Debug, Clone, Copy)]
pub enum DevView {
    Onboarding,
    Board,
    /// The ticket detail as a modal overlay.
    Ticket,
    /// The ticket detail as the full-page (expanded) view.
    Page,
    Error,
}

impl DevView {
    /// Read the `DEV_VIEW` env var. Returns `None` for a normal run.
    pub fn from_env() -> Option<Self> {
        match std::env::var("DEV_VIEW").ok()?.trim() {
            "onboarding" => Some(Self::Onboarding),
            "board" => Some(Self::Board),
            "ticket" => Some(Self::Ticket),
            "page" => Some(Self::Page),
            "error" => Some(Self::Error),
            _ => None,
        }
    }
}

/// A mock populated board (profile + three stages + a few tickets).
pub fn mock_board() -> ViewData {
    let now = Utc::now();
    let profile = profile::View {
        profile: Some(Profile {
            id: Uuid::new_v4(),
            display_name: "Corey".to_owned(),
            created_at: now,
        }),
    };
    let stage = |name: &str, position: i32| Stage {
        id: Uuid::new_v4(),
        name: name.to_owned(),
        position,
        created_at: now,
    };
    let stages = vec![
        stage("Pending", 0),
        stage("In Progress", 1),
        stage("Complete", 2),
    ];

    let ticket = |stage_id: Uuid, title: &str, description: &str, position: i32| Ticket {
        id: Uuid::new_v4(),
        stage_id,
        title: title.to_owned(),
        description: description.to_owned(),
        position,
        parent_id: None,
        created_at: now,
        updated_at: now,
    };
    let mut tickets = vec![
        ticket(
            stages[0].id,
            "Wire up GitHub Actions",
            "Run cargo build + clippy on every push",
            0,
        ),
        ticket(
            stages[0].id,
            "Ticket drag-and-drop",
            "Reorder within and across columns",
            1,
        ),
        ticket(
            stages[1].id,
            "Design system pass",
            "Soft-dark, teal, Nunito, bubbly, real icons",
            0,
        ),
        ticket(
            stages[2].id,
            "Scaffold project",
            "Feature-sliced architecture + docker postgres",
            0,
        ),
    ];
    // Make "drag-and-drop" a child of "Wire up GitHub Actions" so DEV_VIEW shows relationships.
    let parent = tickets[0].id;
    tickets[1].parent_id = Some(parent);

    ViewData {
        profile,
        tasks: tasks::View { stages, tickets },
    }
}

/// A sample error for the error-modal screen.
pub fn mock_error() -> UserFacingError {
    UserFacingError {
        title: "Database unavailable".to_owned(),
        detail: "cannot reach PostgreSQL at `localhost:5433/devdash`. Is the database running?"
            .to_owned(),
        remediation: "Start PostgreSQL with `./scripts/db-up.sh`, then press Retry.".to_owned(),
        retryable: true,
    }
}
