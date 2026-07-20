//! `projects::worktree` part sub-root: its `Command` + `handle()`.

use uuid::Uuid;

use crate::app::bridge::Emitter;
use crate::system::Backend;

/// Worktree actions. Creation is always ticket-driven (the 1:1 rule, AGENTS.md §10) — the
/// `ticket_id` rides along on `Create`.
#[derive(Debug, Clone)]
pub enum Command {
    /// Create a worktree for a ticket in a project. `branch` is used only if the ticket has
    /// no worktree yet; otherwise its existing shared branch wins.
    Create {
        project_id: Uuid,
        ticket_id: Uuid,
        branch: String,
    },
    /// Recreate a removed worktree from its historical marker.
    Recreate { id: Uuid },
    /// Remove a worktree's folder, leaving a marker on the ticket.
    Remove { id: Uuid },
    /// Open a worktree's folder in VS Code (a pure side effect — no state change).
    Open { id: Uuid },
}

impl Command {
    pub fn create(project_id: Uuid, ticket_id: Uuid, branch: String) -> Self {
        Self::Create {
            project_id,
            ticket_id,
            branch,
        }
    }
    pub fn recreate(id: Uuid) -> Self {
        Self::Recreate { id }
    }
    pub fn remove(id: Uuid) -> Self {
        Self::Remove { id }
    }
    pub fn open(id: Uuid) -> Self {
        Self::Open { id }
    }
}

/// Perform a worktree command. Mutating commands settle to a fresh snapshot; `Open` changes no
/// state, so it only surfaces an error (a snapshot would needlessly re-fetch every project).
pub async fn handle(backend: &Backend, emitter: &Emitter, cmd: Command) {
    let result = match cmd {
        Command::Create {
            project_id,
            ticket_id,
            branch,
        } => backend
            .projects
            .worktree
            .create(project_id, ticket_id, &branch)
            .await
            .map(|_| ()),
        Command::Recreate { id } => backend.projects.worktree.recreate(id).await.map(|_| ()),
        Command::Remove { id } => backend.projects.worktree.remove(id).await,
        Command::Open { id } => {
            if let Err(e) = backend.projects.worktree.open_in_editor(id).await {
                emitter.error(&e);
            }
            return; // no state change → no snapshot
        }
    };
    emitter.settle(backend, result).await;
}
