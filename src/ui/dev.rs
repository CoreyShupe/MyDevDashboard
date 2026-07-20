//! Dev-only view overrides for fast visual review (AGENTS.md §8).
//!
//! Gated entirely behind the `DEV_VIEW` env var; a normal run never touches this. It
//! injects MOCK in-memory state only — no DB, no seeding — so any screen can be captured
//! without a database or hand-entered data. When active, the app ignores worker snapshots
//! so the mock state stays put.

use chrono::Utc;
use uuid::Uuid;

use crate::app::projects::ProjectCard;
use crate::app::{ViewData, notes, profile, projects, tasks, todos};
use crate::domain::notes::Note as UncategorizedNote;
use crate::domain::profile::Profile;
use crate::domain::projects::{GitStatus, Project, Worktree};
use crate::domain::tasks::{Note, Stage, Ticket};
use crate::domain::todos::Todo;
use crate::error::UserFacingError;

/// Which screen to force. Selected via `DEV_VIEW={onboarding|board|ticket|page|error}`.
#[derive(Debug, Clone, Copy)]
pub enum DevView {
    Onboarding,
    /// The "new profile" create screen (opened from the switcher), over existing profiles.
    NewProfile,
    Board,
    /// The ticket detail as a modal overlay.
    Ticket,
    /// The ticket detail as the full-page (expanded) view.
    Page,
    /// The "new ticket" create modal, open over the board.
    Create,
    /// The "edit stage" modal, open over the board.
    StageEdit,
    /// The Notes tab, populated with uncategorized notes.
    Notes,
    /// The Notes tab with the "Add to ticket" search picker open.
    NotesFile,
    /// The Projects tab: a grid of repository cards.
    Projects,
    /// A single project's full-page detail (metadata + worktrees).
    Project,
    /// The Todos tab: quick tasks, one already checked off.
    Todos,
    /// Empty states (profile exists, but the feature has no data yet).
    BoardEmpty,
    NotesEmpty,
    TodosEmpty,
    ProjectsEmpty,
    Error,
}

impl DevView {
    /// Read the `DEV_VIEW` env var. Returns `None` for a normal run.
    pub fn from_env() -> Option<Self> {
        match std::env::var("DEV_VIEW").ok()?.trim() {
            "onboarding" => Some(Self::Onboarding),
            "new-profile" => Some(Self::NewProfile),
            "board" => Some(Self::Board),
            "ticket" => Some(Self::Ticket),
            "page" => Some(Self::Page),
            "create" => Some(Self::Create),
            "stage-edit" => Some(Self::StageEdit),
            "notes" => Some(Self::Notes),
            "notes-file" => Some(Self::NotesFile),
            "projects" => Some(Self::Projects),
            "project" => Some(Self::Project),
            "todos" => Some(Self::Todos),
            "board-empty" => Some(Self::BoardEmpty),
            "notes-empty" => Some(Self::NotesEmpty),
            "todos-empty" => Some(Self::TodosEmpty),
            "projects-empty" => Some(Self::ProjectsEmpty),
            "error" => Some(Self::Error),
            _ => None,
        }
    }
}

/// A mock with an active profile but NO feature data — for capturing empty states (empty
/// board / notes / todos / projects). The profile switcher still has two profiles.
pub fn mock_empty() -> ViewData {
    let now = Utc::now();
    let work = Profile {
        id: Uuid::new_v4(),
        display_name: "Work".to_owned(),
        created_at: now,
    };
    let personal = Profile {
        id: Uuid::new_v4(),
        display_name: "Personal".to_owned(),
        created_at: now,
    };
    ViewData {
        profile: profile::View {
            profiles: vec![work.clone(), personal],
            active: Some(work),
        },
        ..ViewData::default()
    }
}

/// A mock populated board (profile + three stages + a few tickets).
pub fn mock_board() -> ViewData {
    let now = Utc::now();
    // Two profiles so the switcher has something to switch to; "Work" is active.
    let work = Profile {
        id: Uuid::new_v4(),
        display_name: "Work".to_owned(),
        created_at: now,
    };
    let work_id = work.id;
    let personal = Profile {
        id: Uuid::new_v4(),
        display_name: "Personal".to_owned(),
        created_at: now,
    };
    let profile = profile::View {
        profiles: vec![work.clone(), personal],
        active: Some(work),
    };
    let stage = |name: &str, position: i32, terminal: bool| Stage {
        id: Uuid::new_v4(),
        name: name.to_owned(),
        position,
        terminal,
        created_at: now,
    };
    let stages = vec![
        stage("Pending", 0, false),
        stage("In Progress", 1, false),
        // Terminal end state: collapses to a count on the board (exercise the mechanism).
        stage("Complete", 2, true),
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
        // A second ticket in the terminal "Complete" stage so its count reads as "2 tickets".
        ticket(
            stages[2].id,
            "Pick the color palette",
            "Soft-dark + teal",
            1,
        ),
    ];
    // Make "drag-and-drop" a child of "Wire up GitHub Actions" so DEV_VIEW shows relationships.
    let parent = tickets[0].id;
    tickets[1].parent_id = Some(parent);

    // Projects + worktrees, exercising the grid states (up-to-date, out-of-sync, no-origin) and
    // the worktree section on the ticket detail (a ticket with two shared-branch worktrees plus
    // a removed marker). Worktrees link to real mock tickets so titles resolve.
    let project = |name: &str, path: &str| Project {
        id: Uuid::new_v4(),
        profile_id: work_id,
        name: name.to_owned(),
        path: path.to_owned(),
        created_at: now,
        updated_at: now,
    };
    let dashboard = project("my-dev-dashboard", "/Users/you/Programming/MyDevDashboard");
    let api = project("acme-api", "/Users/you/Programming/acme-api");
    let scratch = project("scratchpad", "/Users/you/Programming/scratchpad");
    let projects_cards = vec![
        ProjectCard {
            project: dashboard.clone(),
            git: GitStatus {
                is_repo: true,
                origin_url: Some("git@github.com:you/my-dev-dashboard.git".to_owned()),
                branch: Some("main".to_owned()),
                clean: true,
                has_upstream: true,
                ahead: 0,
                behind: 0,
                fetched: true,
            },
        },
        ProjectCard {
            project: api.clone(),
            git: GitStatus {
                is_repo: true,
                origin_url: Some("git@github.com:acme/acme-api.git".to_owned()),
                branch: Some("feature/worktrees".to_owned()),
                clean: false,
                has_upstream: true,
                ahead: 2,
                behind: 1,
                fetched: true,
            },
        },
        ProjectCard {
            project: scratch.clone(),
            git: GitStatus {
                is_repo: true,
                origin_url: None,
                branch: Some("main".to_owned()),
                clean: true,
                has_upstream: false,
                ahead: 0,
                behind: 0,
                fetched: false,
            },
        },
    ];
    let worktree = |project_id, ticket_id, name: &str, branch: &str, removed: bool| Worktree {
        id: Uuid::new_v4(),
        project_id,
        ticket_id,
        name: name.to_owned(),
        branch: branch.to_owned(),
        removed_at: removed.then(|| now - chrono::Duration::days(2)),
        created_at: now,
    };
    // Attach the shared-branch worktrees to the ticket the ticket/page DEV_VIEWs open (the child
    // "Ticket drag-and-drop", tickets[1]) so its Worktrees section renders populated (§8).
    let worktrees = vec![
        // One ticket, one shared branch, live worktrees in two projects (branch sync in action).
        worktree(
            dashboard.id,
            tickets[1].id,
            "projects-tab",
            "feature/projects-tab",
            false,
        ),
        worktree(
            api.id,
            tickets[1].id,
            "projects-tab",
            "feature/projects-tab",
            false,
        ),
        // A removed worktree (historical marker) on another ticket — recreatable.
        worktree(
            dashboard.id,
            tickets[2].id,
            "design-pass",
            "feature/design-pass",
            true,
        ),
    ];

    // A handful of todos, one already checked off, so the Todos tab renders both states.
    let todo = |body: &str, done: bool| Todo {
        id: Uuid::new_v4(),
        body: body.to_owned(),
        done,
        created_at: now,
    };
    let todos = vec![
        todo("Reply to the design-review thread", false),
        todo("Bump the staging deploy after lunch", false),
        todo("Renew the TLS cert before it expires", true),
    ];

    ViewData {
        profile,
        tasks: tasks::View { stages, tickets },
        notes: notes::View::default(),
        projects: projects::View {
            projects: projects_cards,
            worktrees,
        },
        todos: todos::View { todos },
    }
}

/// The Notes tab, populated. Reuses the mock board (so the "Create Ticket" stage picker and
/// "Add To Ticket" search have real stages/tickets to work with) and layers uncategorized
/// notes on top — including one long note so the row wrapping / extra height is exercised
/// (AGENTS.md §8).
pub fn mock_notes_view() -> ViewData {
    let mut data = mock_board();
    data.notes = notes::View {
        notes: mock_uncategorized_notes(),
    };
    data
}

/// A handful of uncategorized notes, newest first (matching the real listing order). One is
/// deliberately long to show the wrapped, taller row.
pub fn mock_uncategorized_notes() -> Vec<UncategorizedNote> {
    let now = Utc::now();
    let bodies = [
        "Look into the flaky drag-and-drop test on CI — seems timing dependent.",
        "Ask design whether the teal accent should get a lighter tint for hover states.",
        "Longer thought: the notes tab could eventually grow tags and a pinned section, and \
         these two row actions (Create Ticket / Add To Ticket) will likely become a small \
         menu once there are more ways to file a note. Keep the row layout flexible for now.",
        "Remember to document the DEV_VIEW=notes screen in AGENTS.md §8.",
    ];
    bodies
        .iter()
        .enumerate()
        .map(|(i, body)| UncategorizedNote {
            id: Uuid::new_v4(),
            body: (*body).to_owned(),
            // Space them apart so the newest sits on top with plausible timestamps.
            created_at: now - chrono::Duration::minutes((i as i64) * 37),
        })
        .collect()
}

/// Mock notes for a ticket detail screen — enough (5) that the modal's 2-note cap kicks in
/// (showing "3 earlier notes not shown") and the full page's wide notes column has body.
/// Oldest-first, matching how real notes arrive.
pub fn mock_notes(ticket_id: Uuid) -> Vec<Note> {
    let now = Utc::now();
    let bodies = [
        "Kicked this off — scoped the work and pulled the relevant modules.",
        "Blocked on the migration; pinged the DB owner for a review.",
        "Unblocked. Rewrote the query to avoid the N+1 and it's ~4x faster now.",
        "Draft up for review. Left two TODOs around error handling.",
        "Addressed review feedback; ready for a final pass.",
    ];
    bodies
        .iter()
        .enumerate()
        .map(|(i, body)| Note {
            id: Uuid::new_v4(),
            ticket_id,
            body: (*body).to_owned(),
            // Space them an hour apart so the timestamps read as a plausible history.
            created_at: now - chrono::Duration::hours((bodies.len() - i) as i64),
        })
        .collect()
}

/// A sample error for the error-modal screen.
pub fn mock_error() -> UserFacingError {
    UserFacingError {
        title: "Database unavailable".to_owned(),
        detail: "cannot reach PostgreSQL at `localhost:5433/devdash`. Is the database running?"
            .to_owned(),
        remediation: "Start PostgreSQL with `dev-dash db up`, then press Retry.".to_owned(),
        retryable: true,
    }
}
