//! `projects` feature sub-root. Composed of parts: `project`, `worktree`.
//!
//! Stays thin: wraps each part's `Command`, exposes the feature `View` (projects with their
//! live git status, plus all worktrees), and routes `handle()` to the owning part. Per-action
//! logic lives in the part files.

pub mod project;
pub mod worktree;

use uuid::Uuid;

use crate::domain::projects::{GitStatus, Project, Worktree};
use crate::error::AppError;
use crate::system::Backend;
use crate::system::projects::ProjectsService;

use super::bridge::Emitter;
use super::event::UiEvent;

/// Intent for the projects feature — one variant per part.
#[derive(Debug, Clone)]
pub enum Event {
    Project(project::Command),
    Worktree(worktree::Command),
}

// Lets UI call `bridge.send(projects::Event::create_project(..))` without naming the root enum.
impl From<Event> for UiEvent {
    fn from(event: Event) -> Self {
        UiEvent::Projects(event)
    }
}

// Convenience constructors delegating to each part, so UI call sites stay flat & readable.
impl Event {
    pub fn create_project(name: String, path: String) -> Self {
        Self::Project(project::Command::create(name, path))
    }
    pub fn delete_project(id: Uuid) -> Self {
        Self::Project(project::Command::delete(id))
    }
    pub fn create_worktree(project_id: Uuid, ticket_id: Uuid, branch: String) -> Self {
        Self::Worktree(worktree::Command::create(project_id, ticket_id, branch))
    }
    pub fn recreate_worktree(id: Uuid) -> Self {
        Self::Worktree(worktree::Command::recreate(id))
    }
    pub fn remove_worktree(id: Uuid) -> Self {
        Self::Worktree(worktree::Command::remove(id))
    }
    pub fn open_worktree(id: Uuid) -> Self {
        Self::Worktree(worktree::Command::open(id))
    }
    pub fn refresh_status() -> Self {
        Self::Project(project::Command::refresh_status())
    }
}

/// One project as the grid renders it: the stored project plus its live git status.
#[derive(Debug, Clone)]
pub struct ProjectCard {
    pub project: Project,
    pub git: GitStatus,
}

/// The projects feature's slice of the rendered snapshot.
#[derive(Debug, Clone, Default)]
pub struct View {
    /// Projects with their (cached) git status, newest first.
    pub projects: Vec<ProjectCard>,
    /// Every worktree across those projects — live ones and historical markers.
    pub worktrees: Vec<Worktree>,
    /// Whether a git-status refresh is in flight — the tab shows a loading state while true.
    pub refreshing: bool,
}

impl View {
    /// Load the projects workspace for one profile (AGENTS.md §9): reconcile worktrees against
    /// disk first (so counts are honest), then list projects + compute their git status
    /// concurrently, and list every worktree.
    pub async fn load(service: &ProjectsService, profile_id: Uuid) -> Result<Self, AppError> {
        service.worktree.reconcile(profile_id).await?;

        let projects = service.project.list(profile_id).await?;
        let paths: Vec<String> = projects.iter().map(|p| p.path.clone()).collect();
        // Read the cached git status (refreshed on open + on demand) — never a fetch, so building
        // a snapshot after any mutation stays instant (AGENTS.md §10).
        let statuses = service.project.cached_statuses(&paths);
        let projects = projects
            .into_iter()
            .zip(statuses)
            .map(|(project, git)| ProjectCard { project, git })
            .collect();

        let worktrees = service.worktree.list_for_profile(profile_id).await?;
        Ok(Self {
            projects,
            worktrees,
            refreshing: service.project.is_refreshing(),
        })
    }

    /// Look up a project card by id.
    pub fn project(&self, id: Uuid) -> Option<&ProjectCard> {
        self.projects.iter().find(|c| c.project.id == id)
    }

    /// All worktrees (live + markers) for a project, in creation order.
    pub fn worktrees_for_project(&self, project_id: Uuid) -> impl Iterator<Item = &Worktree> {
        self.worktrees
            .iter()
            .filter(move |w| w.project_id == project_id)
    }

    /// All worktrees (live + markers) tied to a ticket, across projects.
    pub fn worktrees_for_ticket(&self, ticket_id: Uuid) -> impl Iterator<Item = &Worktree> {
        self.worktrees
            .iter()
            .filter(move |w| w.ticket_id == ticket_id)
    }

    /// The number of LIVE worktrees in a project (what a card's count shows).
    pub fn live_count_for_project(&self, project_id: Uuid) -> usize {
        self.worktrees_for_project(project_id)
            .filter(|w| w.is_live())
            .count()
    }
}

/// Kick off a non-blocking git refresh (AGENTS.md §10). If no refresh is already running, claim
/// the "refreshing" flag and **spawn** the (possibly network-bound) fetch off the worker's event
/// loop, so it never delays the Postgres snapshot, other mutations, or the UI thread. When the
/// fetch lands it clears the flag and emits a fresh snapshot. The CALLER should emit a snapshot
/// right after calling this so the UI shows the loading state immediately. No-op if a refresh is
/// already in flight.
pub fn spawn_git_refresh(backend: &Backend, emitter: &Emitter) {
    if !backend.projects.project.begin_refresh() {
        return; // one is already running; its snapshot will carry the results
    }
    let backend = backend.clone();
    let emitter = emitter.clone();
    tokio::spawn(async move {
        if let Err(e) = refresh_git(&backend).await {
            tracing::warn!(error = %e, "background git refresh failed; leaving status unchanged");
        }
        backend.projects.project.end_refresh();
        emitter.snapshot(&backend).await;
    });
}

/// Refetch live git status for the active profile's projects into the service cache (AGENTS.md
/// §10). Best-effort + profile-scoped (§9); a no-op during onboarding (no active profile). The
/// caller snapshots afterwards so the refreshed status reaches the UI.
pub async fn refresh_git(backend: &Backend) -> Result<(), AppError> {
    if let Some(profile) = backend.profile.active().await? {
        let paths = backend
            .projects
            .project
            .list(profile.id)
            .await?
            .into_iter()
            .map(|p| p.path)
            .collect();
        backend.projects.project.refresh_statuses(paths).await;
    }
    Ok(())
}

/// Feature dispatch: route to the owning part. Kept tiny on purpose (AGENTS.md §4).
pub async fn handle(backend: &Backend, emitter: &Emitter, event: Event) {
    match event {
        Event::Project(cmd) => project::handle(backend, emitter, cmd).await,
        Event::Worktree(cmd) => worktree::handle(backend, emitter, cmd).await,
    }
}
