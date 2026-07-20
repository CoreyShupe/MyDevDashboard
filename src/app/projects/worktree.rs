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

/// Perform a worktree command.
///
/// `Create`/`Recreate` provision on disk AND run the project's setup script, which can be slow, so
/// they're spawned off the worker loop with an in-flight loading state (AGENTS.md §10) rather than
/// awaited inline — the worktree isn't shown as ready until its setup finishes. `Remove` settles
/// inline (it's quick); `Open` changes no state, so it only surfaces an error.
pub async fn handle(backend: &Backend, emitter: &Emitter, cmd: Command) {
    match cmd {
        Command::Create {
            project_id,
            ticket_id,
            branch,
        } => {
            crate::app::projects::spawn_worktree_create(
                backend, emitter, project_id, ticket_id, branch,
            );
            // Snapshot now so the ticket detail shows the "setting up…" loading row immediately;
            // the spawned provision settles the ready worktree (or the error) when it lands.
            emitter.snapshot(backend).await;
        }
        Command::Recreate { id } => {
            // Recreate looks up its (project, ticket) asynchronously, so it emits the loading
            // snapshot from within the spawned task itself.
            crate::app::projects::spawn_worktree_recreate(backend, emitter, id);
        }
        Command::Remove { id } => {
            emitter
                .settle(backend, backend.projects.worktree.remove(id).await)
                .await;
        }
        Command::Open { id } => {
            if let Err(e) = backend.projects.worktree.open_in_editor(id).await {
                emitter.error(&e);
            }
        }
    }
}
