//! `projects::project` part sub-root: its `Command` + `handle()`.

use uuid::Uuid;

use crate::app::bridge::Emitter;
use crate::system::Backend;

/// Project actions.
#[derive(Debug, Clone)]
pub enum Command {
    /// Register an existing local repository as a project.
    Create { name: String, path: String },
    /// Forget a project (its worktree rows cascade; the repo on disk is untouched).
    Delete { id: Uuid },
    /// Set (or clear) a project's setup script — the bash run inside each new worktree (§10).
    SetSetupScript { id: Uuid, script: String },
    /// Set (or clear) a project's teardown script — the bash run inside each worktree right
    /// before it's removed (§10).
    SetTeardownScript { id: Uuid, script: String },
    /// Refetch live git status for the active profile's projects (AGENTS.md §10).
    RefreshStatus,
    /// `git pull --rebase origin <branch>` for one project on a shared branch, then refetch just
    /// that project's status (AGENTS.md §10).
    Pull { id: Uuid },
}

impl Command {
    pub fn create(name: String, path: String) -> Self {
        Self::Create { name, path }
    }
    pub fn delete(id: Uuid) -> Self {
        Self::Delete { id }
    }
    pub fn set_setup_script(id: Uuid, script: String) -> Self {
        Self::SetSetupScript { id, script }
    }
    pub fn set_teardown_script(id: Uuid, script: String) -> Self {
        Self::SetTeardownScript { id, script }
    }
    pub fn refresh_status() -> Self {
        Self::RefreshStatus
    }
    pub fn pull(id: Uuid) -> Self {
        Self::Pull { id }
    }
}

/// Perform a project command, then refresh or surface the error.
pub async fn handle(backend: &Backend, emitter: &Emitter, cmd: Command) {
    match cmd {
        // A new project lands in the active profile (AGENTS.md §9).
        Command::Create { name, path } => {
            let result = match crate::app::profile::active_id(backend).await {
                Ok(profile_id) => backend
                    .projects
                    .project
                    .create(profile_id, &name, &path)
                    .await
                    .map(|_| ()),
                Err(e) => Err(e),
            };
            match result {
                // Show the new card immediately, then load its git status in the background so
                // its "checking" spinner resolves without blocking anything (AGENTS.md §10).
                Ok(()) => {
                    crate::app::projects::spawn_git_refresh(backend, emitter);
                    emitter.snapshot(backend).await;
                }
                Err(e) => emitter.error(&e),
            }
        }
        Command::Delete { id } => {
            emitter
                .settle(backend, backend.projects.project.delete(id).await)
                .await;
        }
        // Persist the script; it only runs later, on worktree creation (§10). A plain settle:
        // reload on success, surface a typed error on failure.
        Command::SetSetupScript { id, script } => {
            emitter
                .settle(
                    backend,
                    backend.projects.project.set_setup_script(id, &script).await,
                )
                .await;
        }
        // Same as the setup script: persist verbatim; it only runs later, on worktree removal (§10).
        Command::SetTeardownScript { id, script } => {
            emitter
                .settle(
                    backend,
                    backend
                        .projects
                        .project
                        .set_teardown_script(id, &script)
                        .await,
                )
                .await;
        }
        // Kick a background git refresh (never blocks the loop) and snapshot now so the tab shows
        // its loading state; the spawned fetch emits the settled snapshot when it lands.
        Command::RefreshStatus => {
            crate::app::projects::spawn_git_refresh(backend, emitter);
            emitter.snapshot(backend).await;
        }
        // Pull is network-bound, so spawn it off the loop (like a git refresh) with a per-project
        // guard against concurrent/duplicate pulls, then snapshot now so the card shows its pulling
        // spinner immediately; the spawned task settles (synced card, or the error) when it lands.
        Command::Pull { id } => {
            crate::app::projects::spawn_pull(backend, emitter, id);
            emitter.snapshot(backend).await;
        }
    }
}
