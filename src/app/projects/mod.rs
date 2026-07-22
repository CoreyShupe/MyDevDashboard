//! `projects` feature sub-root. Composed of parts: `project`, `worktree`.
//!
//! Stays thin: wraps each part's `Command`, exposes the feature `View` (projects with their
//! live git status, plus all worktrees), and routes `handle()` to the owning part. Per-action
//! logic lives in the part files.

pub mod project;
pub mod worktree;

use uuid::Uuid;

use crate::domain::projects::{GitStatus, Project, Worktree, WorktreeBusy};
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
    pub fn set_setup_script(id: Uuid, script: String) -> Self {
        Self::Project(project::Command::set_setup_script(id, script))
    }
    pub fn set_teardown_script(id: Uuid, script: String) -> Self {
        Self::Project(project::Command::set_teardown_script(id, script))
    }
    pub fn create_worktree(project_id: Uuid, ticket_id: Uuid, branch: String) -> Self {
        Self::Worktree(worktree::Command::create(project_id, ticket_id, branch))
    }
    pub fn recreate_worktree(id: Uuid) -> Self {
        Self::Worktree(worktree::Command::recreate(id))
    }
    pub fn recreate_worktree_as(id: Uuid, branch: String) -> Self {
        Self::Worktree(worktree::Command::recreate_as(id, branch))
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
    pub fn pull_project(id: Uuid) -> Self {
        Self::Project(project::Command::pull(id))
    }
}

/// One project as the grid renders it: the stored project plus its live git status.
#[derive(Debug, Clone)]
pub struct ProjectCard {
    pub project: Project,
    pub git: GitStatus,
    /// Whether a `git pull --rebase` is currently in flight for this project — the card shows a
    /// pulling spinner in place of its Pull button while true (AGENTS.md §10).
    pub pulling: bool,
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
    /// `(project_id, ticket_id)` pairs whose worktree is being provisioned right now (git add +
    /// setup script). The ticket/project detail shows a loading row for each until it lands, so a
    /// worktree isn't presented as ready until its setup script has finished (AGENTS.md §10).
    pub creating: Vec<(Uuid, Uuid)>,
    /// Existing worktrees with a slow action (remove / open) in flight, keyed by worktree id. Their
    /// row swaps its action buttons for a "waiting" spinner until it lands (AGENTS.md §10).
    pub busy: Vec<(Uuid, WorktreeBusy)>,
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
        let pulling = service.project.pulling_ids();
        let projects = projects
            .into_iter()
            .zip(statuses)
            .map(|(project, git)| {
                let pulling = pulling.contains(&project.id);
                ProjectCard {
                    project,
                    git,
                    pulling,
                }
            })
            .collect();

        let worktrees = service.worktree.list_for_profile(profile_id).await?;
        let creating = service.worktree.creating_ids().into_iter().collect();
        let busy = service.worktree.busy_ids().into_iter().collect();
        Ok(Self {
            projects,
            worktrees,
            refreshing: service.project.is_refreshing(),
            creating,
            busy,
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

    /// Projects with a worktree being provisioned for `ticket_id` right now (drives the ticket
    /// detail's loading rows).
    pub fn creating_for_ticket(&self, ticket_id: Uuid) -> impl Iterator<Item = Uuid> + '_ {
        self.creating
            .iter()
            .filter(move |(_, t)| *t == ticket_id)
            .map(|(p, _)| *p)
    }

    /// Tickets with a worktree being provisioned in `project_id` right now (drives the project
    /// detail's loading rows).
    pub fn creating_for_project(&self, project_id: Uuid) -> impl Iterator<Item = Uuid> + '_ {
        self.creating
            .iter()
            .filter(move |(p, _)| *p == project_id)
            .map(|(_, t)| *t)
    }

    /// Whether a worktree for this exact `(project, ticket)` is being provisioned right now.
    pub fn is_creating(&self, project_id: Uuid, ticket_id: Uuid) -> bool {
        self.creating.contains(&(project_id, ticket_id))
    }

    /// The slow action in flight on a worktree right now (remove / open), if any — the row shows a
    /// "waiting" spinner in its place.
    pub fn is_busy(&self, worktree_id: Uuid) -> Option<WorktreeBusy> {
        self.busy
            .iter()
            .find(|(id, _)| *id == worktree_id)
            .map(|(_, action)| *action)
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

/// Kick off a non-blocking one-click **Pull** for a project (AGENTS.md §10). Claims the
/// per-project pulling guard first: a second click (or a queued duplicate) while one is in flight
/// is a no-op, so there are never two concurrent `git pull --rebase` on the same repo. Spawns the
/// (network-bound) pull off the worker's event loop — like [`spawn_git_refresh`] — so it never
/// blocks the loop or the UI; `ProjectService::pull` also refetches just that project's status when
/// it lands. When done it clears the guard and settles (fresh snapshot, or the error in a modal).
/// The CALLER should snapshot right after calling this so the card shows its pulling state at once.
pub fn spawn_pull(backend: &Backend, emitter: &Emitter, id: Uuid) {
    if !backend.projects.project.begin_pull(id) {
        return; // already pulling this project; its settle will carry the result
    }
    let backend = backend.clone();
    let emitter = emitter.clone();
    tokio::spawn(async move {
        let result = backend.projects.project.pull(id).await;
        backend.projects.project.end_pull(id);
        emitter.settle(&backend, result).await;
    });
}

/// Kick off a non-blocking worktree **removal** (AGENTS.md §10). Runs the project's teardown script
/// (if any) FIRST — while the folder still exists — then `git worktree remove`; both shell out, so
/// — like create — this is spawned off the worker loop with a "waiting" state rather than awaited
/// inline. Claims the per-worktree busy guard first (a double-click is a no-op), emits the loading
/// snapshot, THEN spawns; snapshotting before the spawn guarantees the loading state reaches the UI
/// before the (possibly quick) clearing snapshot, so the spinner can't get stuck on. `settle_reload`
/// always re-snapshots (clearing the busy state) and surfaces any error — a `git` refusal (dirty
/// tree) leaves the worktree live and shows the error.
///
/// A failing teardown is NON-fatal, mirroring setup: the removal still proceeds (so a broken
/// teardown can't strand a worktree the owner asked to remove), and the teardown error is surfaced
/// only when the removal itself succeeded — a git refusal takes precedence, since then the worktree
/// is still live and that's the actionable failure.
pub async fn spawn_worktree_remove(backend: &Backend, emitter: &Emitter, id: Uuid) {
    if !backend
        .projects
        .worktree
        .begin_busy(id, WorktreeBusy::Removing)
    {
        return; // already removing this worktree; its settle will carry the result
    }
    emitter.snapshot(backend).await; // show the "Removing…" spinner before the work starts
    let backend = backend.clone();
    let emitter = emitter.clone();
    tokio::spawn(async move {
        // Teardown runs before the folder is deleted; keep its result aside and remove regardless.
        let teardown = backend.projects.worktree.run_teardown(id).await;
        let removed = backend.projects.worktree.remove(id).await;
        backend.projects.worktree.end_busy(id);
        // Removal failure is fatal (the worktree stays live) and wins; otherwise surface any
        // teardown failure, which didn't stop the removal but the owner should still see.
        emitter.settle_reload(&backend, removed.and(teardown)).await;
    });
}

/// Kick off a non-blocking **Open in VS Code** for a worktree (AGENTS.md §10). Launching the editor
/// shells out and changes no state, but a click still deserves immediate feedback — so, like
/// [`spawn_worktree_remove`], it claims the per-worktree busy guard, shows an "Opening…" spinner,
/// then spawns the launch. `settle_reload` re-snapshots to clear the spinner (and surfaces a launch
/// error). Snapshotting before the spawn keeps the quick launch from clearing the spinner before it
/// ever shows.
pub async fn spawn_worktree_open(backend: &Backend, emitter: &Emitter, id: Uuid) {
    if !backend
        .projects
        .worktree
        .begin_busy(id, WorktreeBusy::Opening)
    {
        return; // already opening this worktree
    }
    emitter.snapshot(backend).await; // show the "Opening…" spinner before the launch
    let backend = backend.clone();
    let emitter = emitter.clone();
    tokio::spawn(async move {
        let result = backend.projects.worktree.open_in_editor(id).await;
        backend.projects.worktree.end_busy(id);
        emitter.settle_reload(&backend, result).await;
    });
}

/// Kick off a non-blocking worktree **creation** for a ticket in a project (AGENTS.md §10).
/// Provisioning shells out to git and then runs the project's setup script (e.g. `bun install`),
/// which can be slow — so it must never block the worker's event loop or the UI. Claims the
/// per-`(project, ticket)` "creating" guard first (a double-click is a no-op), then spawns the
/// provision; the CALLER snapshots right after so the ticket detail shows the loading row at once.
/// When it lands the guard is cleared and it settles.
///
/// A failing setup script is surfaced (error modal) but does NOT fail the worktree: it's created
/// and tracked regardless, so the owner can fix the script and re-run in place (per owner intent).
/// A git/DB provisioning failure, by contrast, means no worktree — that error is fatal.
pub fn spawn_worktree_create(
    backend: &Backend,
    emitter: &Emitter,
    project_id: Uuid,
    ticket_id: Uuid,
    branch: String,
) {
    if !backend
        .projects
        .worktree
        .begin_create(project_id, ticket_id)
    {
        return; // already provisioning this worktree; its settle will carry the result
    }
    let backend = backend.clone();
    let emitter = emitter.clone();
    tokio::spawn(async move {
        let result = provision_and_setup(
            backend
                .projects
                .worktree
                .create(project_id, ticket_id, &branch)
                .await,
            &backend,
        )
        .await;
        backend.projects.worktree.end_create(project_id, ticket_id);
        emitter.settle_reload(&backend, result).await;
    });
}

/// Kick off a non-blocking worktree **recreation** from a removed marker (AGENTS.md §10). Since
/// removal deleted the folder, recreating re-runs the project's setup script, so — like
/// [`spawn_worktree_create`] — it's spawned off the loop with the same loading state (and the same
/// "setup failure is non-fatal" behaviour). Its `(project, ticket)` is looked up from the row
/// first (recreate starts from only a worktree id), then the loading snapshot is emitted from
/// within the task. No-op if one is already in flight.
///
/// `new_branch`: `None` recreates on the marker's original branch; `Some(branch)` is the
/// non-destructive branch switch (`recreate_as`) — the same fresh provision, just onto a different
/// branch, leaving the ticket's other worktrees untouched.
pub fn spawn_worktree_recreate(
    backend: &Backend,
    emitter: &Emitter,
    id: Uuid,
    new_branch: Option<String>,
) {
    let backend = backend.clone();
    let emitter = emitter.clone();
    tokio::spawn(async move {
        let (project_id, ticket_id) = match backend.projects.worktree.ids_of(id).await {
            Ok(ids) => ids,
            Err(e) => {
                emitter.error(&e);
                return;
            }
        };
        if !backend
            .projects
            .worktree
            .begin_create(project_id, ticket_id)
        {
            return; // already provisioning this worktree
        }
        // Show the loading row now (the lookup was async, so we couldn't snapshot before spawning).
        emitter.snapshot(&backend).await;
        let provisioned = match &new_branch {
            Some(branch) => backend.projects.worktree.recreate_as(id, branch).await,
            None => backend.projects.worktree.recreate(id).await,
        };
        let result = provision_and_setup(provisioned, &backend).await;
        backend.projects.worktree.end_create(project_id, ticket_id);
        emitter.settle_reload(&backend, result).await;
    });
}

/// Given a provision result `(worktree, fresh)`, run the project's setup script when the worktree
/// was freshly created, and fold the outcome into a single `Result` for [`Emitter::settle_reload`]:
/// - provisioning failed → its (fatal) error propagates (no worktree exists);
/// - provisioned + setup failed → the SETUP error is returned, but the worktree is already created
///   and tracked, so `settle_reload` shows the error yet the snapshot still includes the worktree;
/// - provisioned + setup ok (or an adopted folder → no setup) → `Ok(())`.
async fn provision_and_setup(
    provision: Result<(Worktree, bool), AppError>,
    backend: &Backend,
) -> Result<(), AppError> {
    match provision {
        Ok((worktree, true)) => backend.projects.worktree.run_setup(&worktree).await,
        Ok((_, false)) => Ok(()),
        Err(e) => Err(e),
    }
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
