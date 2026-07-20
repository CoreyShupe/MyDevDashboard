//! `tasks::stage` part sub-root: its `Command` + `handle()`.

use uuid::Uuid;

use crate::app::bridge::Emitter;
use crate::system::Backend;

/// Stage actions.
#[derive(Debug, Clone)]
pub enum Command {
    Create { name: String },
    Rename { id: Uuid, name: String },
    Delete { id: Uuid },
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
}

/// Perform a stage command, then refresh or surface the error.
pub async fn handle(backend: &Backend, emitter: &Emitter, cmd: Command) {
    let result = match cmd {
        Command::Create { name } => backend.tasks.stage.create(&name).await.map(|_| ()),
        Command::Rename { id, name } => backend.tasks.stage.rename(id, &name).await,
        Command::Delete { id } => backend.tasks.stage.delete(id).await,
    };
    emitter.settle(backend, result).await;
}
