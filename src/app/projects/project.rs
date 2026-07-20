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
    /// Refetch live git status for the active profile's projects (AGENTS.md §10).
    RefreshStatus,
}

impl Command {
    pub fn create(name: String, path: String) -> Self {
        Self::Create { name, path }
    }
    pub fn delete(id: Uuid) -> Self {
        Self::Delete { id }
    }
    pub fn refresh_status() -> Self {
        Self::RefreshStatus
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
        // Kick a background git refresh (never blocks the loop) and snapshot now so the tab shows
        // its loading state; the spawned fetch emits the settled snapshot when it lands.
        Command::RefreshStatus => {
            crate::app::projects::spawn_git_refresh(backend, emitter);
            emitter.snapshot(backend).await;
        }
    }
}
