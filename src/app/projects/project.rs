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
}

impl Command {
    pub fn create(name: String, path: String) -> Self {
        Self::Create { name, path }
    }
    pub fn delete(id: Uuid) -> Self {
        Self::Delete { id }
    }
}

/// Perform a project command, then refresh or surface the error.
pub async fn handle(backend: &Backend, emitter: &Emitter, cmd: Command) {
    let result = match cmd {
        // A new project lands in the active profile (AGENTS.md §9).
        Command::Create { name, path } => match crate::app::profile::active_id(backend).await {
            Ok(profile_id) => backend
                .projects
                .project
                .create(profile_id, &name, &path)
                .await
                .map(|_| ()),
            Err(e) => Err(e),
        },
        Command::Delete { id } => backend.projects.project.delete(id).await,
    };
    emitter.settle(backend, result).await;
}
