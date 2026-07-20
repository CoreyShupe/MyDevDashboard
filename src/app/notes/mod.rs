//! `notes` feature sub-root: its `Event`, `View`, and `handle()`.
//!
//! "Uncategorized notes" are quick captures. Beyond add/delete, a note can be *filed*:
//! turned into a brand-new ticket (handled UI-side by opening the ticket create modal, then
//! deleting the source note once the ticket is made) or added onto an existing ticket.
//!
//! `FileIntoTicket` is a deliberate CROSS-FEATURE reach (AGENTS.md §2): the handler writes
//! into `tasks` (the ticket's notes) and then removes the note from this feature. It's the
//! only place `notes` touches another feature, and it does so through the shared `Backend`.

use uuid::Uuid;

use crate::domain::notes::Note;
use crate::error::{AppError, DbError};
use crate::system::Backend;
use crate::system::notes::NotesService;

use super::bridge::Emitter;
use super::event::UiEvent;

/// Intent for the notes feature.
#[derive(Debug, Clone)]
pub enum Event {
    /// Capture a new uncategorized note.
    Add { body: String },
    /// Remove a note from the uncategorized list.
    Delete { id: Uuid },
    /// File a note onto an EXISTING ticket: add it as a ticket note, then drop the
    /// uncategorized note. Cross-feature (touches `tasks`). The body travels with the event
    /// so the handler needn't re-read it — the note is immutable today.
    FileIntoTicket {
        id: Uuid,
        ticket_id: Uuid,
        body: String,
    },
    /// Turn a note into a TODO: add it to the todos list, then drop the uncategorized note.
    /// Cross-feature (touches `todos`), same shape as `FileIntoTicket`.
    FileIntoTodo { id: Uuid, body: String },
}

impl Event {
    pub fn add(body: String) -> Self {
        Self::Add { body }
    }
    pub fn delete(id: Uuid) -> Self {
        Self::Delete { id }
    }
    pub fn file_into_ticket(id: Uuid, ticket_id: Uuid, body: String) -> Self {
        Self::FileIntoTicket {
            id,
            ticket_id,
            body,
        }
    }
    pub fn file_into_todo(id: Uuid, body: String) -> Self {
        Self::FileIntoTodo { id, body }
    }
}

// Lets UI call `bridge.send(notes::Event::add(body))` without naming the root enum.
impl From<Event> for UiEvent {
    fn from(event: Event) -> Self {
        UiEvent::Notes(event)
    }
}

/// The notes feature's slice of the rendered snapshot.
#[derive(Debug, Clone, Default)]
pub struct View {
    pub notes: Vec<Note>,
}

impl View {
    /// Load the uncategorized notes for one profile (AGENTS.md §9).
    pub async fn load(service: &NotesService, profile_id: Uuid) -> Result<Self, DbError> {
        Ok(Self {
            notes: service.list(profile_id).await?,
        })
    }
}

/// Sub-root dispatch for the notes feature. Every arm settles to a fresh snapshot so the
/// list (and, for filing, the tasks board) reflects the change.
pub async fn handle(backend: &Backend, emitter: &Emitter, event: Event) {
    let result = match event {
        // A captured note lands in the active profile (AGENTS.md §9).
        Event::Add { body } => match crate::app::profile::active_id(backend).await {
            Ok(profile_id) => backend.notes.add(profile_id, &body).await.map(|_| ()),
            Err(e) => Err(e),
        },
        Event::Delete { id } => backend.notes.delete(id).await,
        Event::FileIntoTicket {
            id,
            ticket_id,
            body,
        } => file_into_ticket(backend, id, ticket_id, &body).await,
        Event::FileIntoTodo { id, body } => file_into_todo(backend, id, &body).await,
    };
    emitter.settle(backend, result).await;
}

/// Add the note's body onto an existing ticket, then remove it from the uncategorized list.
/// The ticket note is written first so a failure there leaves the source note in place
/// (nothing is silently lost).
async fn file_into_ticket(
    backend: &Backend,
    id: Uuid,
    ticket_id: Uuid,
    body: &str,
) -> Result<(), AppError> {
    backend.tasks.note.add(ticket_id, body).await?;
    backend.notes.delete(id).await
}

/// Turn a note into a todo in the active profile, then remove the source note. The todo is
/// created first so a failure there leaves the note in place (nothing is silently lost).
async fn file_into_todo(backend: &Backend, id: Uuid, body: &str) -> Result<(), AppError> {
    let profile_id = crate::app::profile::active_id(backend).await?;
    backend.todos.add(profile_id, body).await?;
    backend.notes.delete(id).await
}
