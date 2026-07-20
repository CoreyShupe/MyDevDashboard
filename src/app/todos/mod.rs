//! `todos` feature sub-root: its `Event`, `View`, and `handle()`.
//!
//! Quick tasks to remember, scoped to the active profile (AGENTS.md §9). Mirrors `notes`: a
//! flat list with add/delete, plus a `done` toggle. A note can be turned INTO a todo — but that
//! cross-feature reach lives in `app/notes` (the side that owns the note being consumed), just
//! like `notes::FileIntoTicket` reaches into `tasks`.

use uuid::Uuid;

use crate::domain::todos::Todo;
use crate::error::DbError;
use crate::system::Backend;
use crate::system::todos::TodosService;

use super::bridge::Emitter;
use super::event::UiEvent;

/// Intent for the todos feature.
#[derive(Debug, Clone)]
pub enum Event {
    /// Capture a new todo.
    Add { body: String },
    /// Check a todo off (or back on).
    SetDone { id: Uuid, done: bool },
    /// Remove a todo.
    Delete { id: Uuid },
}

impl Event {
    pub fn add(body: String) -> Self {
        Self::Add { body }
    }
    pub fn set_done(id: Uuid, done: bool) -> Self {
        Self::SetDone { id, done }
    }
    pub fn delete(id: Uuid) -> Self {
        Self::Delete { id }
    }
}

// Lets UI call `bridge.send(todos::Event::add(body))` without naming the root enum.
impl From<Event> for UiEvent {
    fn from(event: Event) -> Self {
        UiEvent::Todos(event)
    }
}

/// The todos feature's slice of the rendered snapshot.
#[derive(Debug, Clone, Default)]
pub struct View {
    pub todos: Vec<Todo>,
}

impl View {
    /// Load the todos for one profile (AGENTS.md §9).
    pub async fn load(service: &TodosService, profile_id: Uuid) -> Result<Self, DbError> {
        Ok(Self {
            todos: service.list(profile_id).await?,
        })
    }
}

/// Sub-root dispatch for the todos feature. Every arm settles to a fresh snapshot.
pub async fn handle(backend: &Backend, emitter: &Emitter, event: Event) {
    let result = match event {
        // A captured todo lands in the active profile (AGENTS.md §9).
        Event::Add { body } => match crate::app::profile::active_id(backend).await {
            Ok(profile_id) => backend.todos.add(profile_id, &body).await.map(|_| ()),
            Err(e) => Err(e),
        },
        Event::SetDone { id, done } => backend.todos.set_done(id, done).await,
        Event::Delete { id } => backend.todos.delete(id).await,
    };
    emitter.settle(backend, result).await;
}
