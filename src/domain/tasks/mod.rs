//! `tasks` feature — domain types, composed of parts (AGENTS.md §2).
//!
//! Each part (`stage`, `ticket`, `note`) owns its type here and is mirrored in
//! `system/tasks/`, `app/tasks/`, and `ui/tasks/`.

pub mod note;
pub mod stage;
pub mod ticket;

pub use note::Note;
pub use stage::Stage;
pub use ticket::Ticket;
