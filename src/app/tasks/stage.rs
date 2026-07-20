//! `tasks::stage` part sub-root: its `Command` + `handle()`.

use uuid::Uuid;

use crate::app::bridge::Emitter;
use crate::system::Backend;

/// Stage actions.
#[derive(Debug, Clone)]
pub enum Command {
    Create {
        name: String,
    },
    Rename {
        id: Uuid,
        name: String,
    },
    Delete {
        id: Uuid,
    },
    SetTerminal {
        id: Uuid,
        terminal: bool,
    },
    /// New left-to-right order of stage ids (positions are set to their index).
    Reorder {
        ids: Vec<Uuid>,
    },
}

impl Command {
    pub fn create(name: String) -> Self {
        Self::Create { name }
    }
    pub fn rename(id: Uuid, name: String) -> Self {
        Self::Rename { id, name }
    }
    pub fn delete(id: Uuid) -> Self {
        Self::Delete { id }
    }
    pub fn set_terminal(id: Uuid, terminal: bool) -> Self {
        Self::SetTerminal { id, terminal }
    }
    pub fn reorder(ids: Vec<Uuid>) -> Self {
        Self::Reorder { ids }
    }
}

/// Perform a stage command, then refresh or surface the error.
pub async fn handle(backend: &Backend, emitter: &Emitter, cmd: Command) {
    let result = match cmd {
        // A new stage lands in the active profile's board (AGENTS.md §9).
        Command::Create { name } => match crate::app::profile::active_id(backend).await {
            Ok(profile_id) => backend
                .tasks
                .stage
                .create(profile_id, &name)
                .await
                .map(|_| ()),
            Err(e) => Err(e),
        },
        Command::Rename { id, name } => backend.tasks.stage.rename(id, &name).await,
        Command::Delete { id } => backend.tasks.stage.delete(id).await,
        Command::SetTerminal { id, terminal } => {
            backend.tasks.stage.set_terminal(id, terminal).await
        }
        Command::Reorder { ids } => backend.tasks.stage.reorder(&ids).await,
    };
    emitter.settle(backend, result).await;
}
