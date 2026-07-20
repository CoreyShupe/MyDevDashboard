//! `tasks` feature sub-root. Composed of parts: `stage`, `ticket`, `note`.
//!
//! This file stays thin: it wraps each part's `Command`, exposes the feature `View`, and
//! routes `handle()` to the owning part. Per-action logic lives in the part files.

pub mod note;
pub mod stage;
pub mod ticket;

use uuid::Uuid;

use crate::domain::tasks::{Note, Stage, Ticket};
use crate::error::DbError;
use crate::system::Backend;
use crate::system::tasks::TasksService;

use super::bridge::Emitter;
use super::event::{AppMessage, UiEvent};

/// Intent for the tasks board — one variant per part.
#[derive(Debug, Clone)]
pub enum Event {
    Stage(stage::Command),
    Ticket(ticket::Command),
    Note(note::Command),
}

// Lets UI call `bridge.send(tasks::Event::create_stage(name))` without naming the root enum.
impl From<Event> for UiEvent {
    fn from(event: Event) -> Self {
        UiEvent::Tasks(event)
    }
}

// Convenience constructors delegating to each part, so UI call sites stay flat & readable.
impl Event {
    pub fn create_stage(name: String) -> Self {
        Self::Stage(stage::Command::create(name))
    }
    pub fn rename_stage(id: Uuid, name: String) -> Self {
        Self::Stage(stage::Command::rename(id, name))
    }
    pub fn delete_stage(id: Uuid) -> Self {
        Self::Stage(stage::Command::delete(id))
    }
    pub fn create_ticket(stage_id: Uuid, title: String, description: String) -> Self {
        Self::Ticket(ticket::Command::create(stage_id, title, description))
    }
    pub fn update_ticket(id: Uuid, title: String, description: String) -> Self {
        Self::Ticket(ticket::Command::update(id, title, description))
    }
    pub fn move_ticket(id: Uuid, new_stage_id: Uuid) -> Self {
        Self::Ticket(ticket::Command::move_to(id, new_stage_id))
    }
    pub fn delete_ticket(id: Uuid) -> Self {
        Self::Ticket(ticket::Command::delete(id))
    }
    pub fn create_child(parent_id: Uuid, title: String, description: String) -> Self {
        Self::Ticket(ticket::Command::create_child(parent_id, title, description))
    }
    pub fn unlink_ticket(id: Uuid) -> Self {
        Self::Ticket(ticket::Command::unlink(id))
    }
    pub fn load_notes(ticket_id: Uuid) -> Self {
        Self::Note(note::Command::load(ticket_id))
    }
    pub fn add_note(ticket_id: Uuid, body: String) -> Self {
        Self::Note(note::Command::add(ticket_id, body))
    }
}

/// Feature-specific results (things that aren't a full snapshot). Produced by the `note` part.
#[derive(Debug, Clone)]
pub enum Message {
    Notes { ticket_id: Uuid, notes: Vec<Note> },
}

impl From<Message> for AppMessage {
    fn from(message: Message) -> Self {
        AppMessage::Tasks(message)
    }
}

/// The tasks feature's slice of the rendered snapshot.
#[derive(Debug, Clone, Default)]
pub struct View {
    pub stages: Vec<Stage>,
    pub tickets: Vec<Ticket>,
}

impl View {
    pub async fn load(service: &TasksService) -> Result<Self, DbError> {
        Ok(Self {
            stages: service.stage.list().await?,
            tickets: service.ticket.list().await?,
        })
    }

    /// Tickets belonging to a given stage, in display order.
    pub fn tickets_for(&self, stage_id: Uuid) -> impl Iterator<Item = &Ticket> {
        self.tickets.iter().filter(move |t| t.stage_id == stage_id)
    }

    /// Look up a ticket by id.
    pub fn ticket(&self, id: Uuid) -> Option<&Ticket> {
        self.tickets.iter().find(|t| t.id == id)
    }

    /// The parent of a ticket, if it has one.
    pub fn parent_of(&self, ticket: &Ticket) -> Option<&Ticket> {
        ticket.parent_id.and_then(|pid| self.ticket(pid))
    }

    /// The direct children of a ticket, in display order.
    pub fn children_of(&self, ticket_id: Uuid) -> impl Iterator<Item = &Ticket> {
        self.tickets
            .iter()
            .filter(move |t| t.parent_id == Some(ticket_id))
    }
}

/// Feature dispatch: route to the owning part. Kept tiny on purpose (AGENTS.md §4).
pub async fn handle(backend: &Backend, emitter: &Emitter, event: Event) {
    match event {
        Event::Stage(cmd) => stage::handle(backend, emitter, cmd).await,
        Event::Ticket(cmd) => ticket::handle(backend, emitter, cmd).await,
        Event::Note(cmd) => note::handle(backend, emitter, cmd).await,
    }
}
