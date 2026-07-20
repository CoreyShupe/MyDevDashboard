//! `tasks::ticket` part sub-root: its `Command` + `handle()`.

use uuid::Uuid;

use crate::app::bridge::Emitter;
use crate::system::Backend;

/// Ticket actions.
#[derive(Debug, Clone)]
pub enum Command {
    Create {
        stage_id: Uuid,
        title: String,
        description: String,
        /// Optional first note recorded against the new ticket (from the create modal).
        note: Option<String>,
    },
    Update {
        id: Uuid,
        title: String,
        description: String,
    },
    Move {
        id: Uuid,
        new_stage_id: Uuid,
    },
    Delete {
        id: Uuid,
    },
    /// Create a child ticket under `parent_id` (placed in the parent's stage).
    CreateChild {
        parent_id: Uuid,
        title: String,
        description: String,
    },
    /// Detach a ticket from its parent (make it top-level).
    Unlink {
        id: Uuid,
    },
}

impl Command {
    pub fn create(
        stage_id: Uuid,
        title: String,
        description: String,
        note: Option<String>,
    ) -> Self {
        Self::Create {
            stage_id,
            title,
            description,
            note,
        }
    }
    pub fn update(id: Uuid, title: String, description: String) -> Self {
        Self::Update {
            id,
            title,
            description,
        }
    }
    pub fn move_to(id: Uuid, new_stage_id: Uuid) -> Self {
        Self::Move { id, new_stage_id }
    }
    pub fn delete(id: Uuid) -> Self {
        Self::Delete { id }
    }
    pub fn create_child(parent_id: Uuid, title: String, description: String) -> Self {
        Self::CreateChild {
            parent_id,
            title,
            description,
        }
    }
    pub fn unlink(id: Uuid) -> Self {
        Self::Unlink { id }
    }
}

/// Perform a ticket command, then refresh or surface the error.
pub async fn handle(backend: &Backend, emitter: &Emitter, cmd: Command) {
    let result = match cmd {
        Command::Create {
            stage_id,
            title,
            description,
            note,
        } => {
            // Create the ticket, then (if given) record its first note against the new id.
            match backend
                .tasks
                .ticket
                .create(stage_id, &title, &description, None)
                .await
            {
                Ok(ticket) => match note.as_deref().map(str::trim) {
                    Some(body) if !body.is_empty() => {
                        backend.tasks.note.add(ticket.id, body).await.map(|_| ())
                    }
                    _ => Ok(()),
                },
                Err(e) => Err(e),
            }
        }
        Command::Update {
            id,
            title,
            description,
        } => backend
            .tasks
            .ticket
            .update(id, &title, &description)
            .await
            .map(|_| ()),
        Command::Move { id, new_stage_id } => {
            backend.tasks.ticket.move_to_stage(id, new_stage_id).await
        }
        Command::Delete { id } => backend.tasks.ticket.delete(id).await,
        Command::CreateChild {
            parent_id,
            title,
            description,
        } => backend
            .tasks
            .ticket
            .create_child(parent_id, &title, &description)
            .await
            .map(|_| ()),
        Command::Unlink { id } => backend.tasks.ticket.set_parent(id, None).await,
    };
    emitter.settle(backend, result).await;
}
