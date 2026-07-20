//! `tasks::note` part sub-root: its `Command` + `handle()`.
//!
//! Notes aren't part of the board snapshot (they load on demand), so this part produces a
//! feature `Message` (`tasks::Message::Notes`) rather than a snapshot refresh.

use uuid::Uuid;

use crate::app::bridge::Emitter;
use crate::system::Backend;

use super::Message;

/// Note actions on a ticket.
#[derive(Debug, Clone)]
pub enum Command {
    Load { ticket_id: Uuid },
    Add { ticket_id: Uuid, body: String },
}

impl Command {
    pub fn load(ticket_id: Uuid) -> Self {
        Self::Load { ticket_id }
    }
    pub fn add(ticket_id: Uuid, body: String) -> Self {
        Self::Add { ticket_id, body }
    }
}

/// Perform a note command; both actions end by sending the ticket's current notes.
pub async fn handle(backend: &Backend, emitter: &Emitter, cmd: Command) {
    match cmd {
        Command::Load { ticket_id } => emit_notes(backend, emitter, ticket_id).await,
        Command::Add { ticket_id, body } => match backend.tasks.note.add(ticket_id, &body).await {
            Ok(_) => emit_notes(backend, emitter, ticket_id).await,
            Err(e) => emitter.error(&e),
        },
    }
}

async fn emit_notes(backend: &Backend, emitter: &Emitter, ticket_id: Uuid) {
    match backend.tasks.note.list_for_ticket(ticket_id).await {
        Ok(notes) => emitter.send(Message::Notes { ticket_id, notes }.into()),
        Err(e) => emitter.error(&e.into()),
    }
}
