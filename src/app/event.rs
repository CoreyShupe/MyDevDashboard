//! ROOT vocabulary crossing the UI <-> System boundary.
//!
//! These enums are deliberately thin: one variant per feature, wrapping that feature's
//! own `Event` / `Message` (defined under `app/<feature>/`). This is the top of the
//! `root { sub-root -> feature }` dispatch (AGENTS.md §2). Per-action detail lives in the
//! feature modules, never here.

use crate::error::UserFacingError;

use super::state::ViewData;
use super::{notes, profile, projects, tasks};

/// Intent flowing UI -> worker. `ReloadAll` is global; the rest route to a feature.
///
/// UI code builds a feature `Event` and relies on `Bridge::send(impl Into<UiEvent>)` to
/// wrap it, so call sites read `bridge.send(tasks::Event::create_stage(name))`.
#[derive(Debug, Clone)]
pub enum UiEvent {
    ReloadAll,
    Profile(profile::Event),
    Tasks(tasks::Event),
    Notes(notes::Event),
    Projects(projects::Event),
}

/// Results flowing worker -> UI.
#[derive(Debug, Clone)]
pub enum AppMessage {
    /// A fresh, complete snapshot of app state for the UI to render.
    Snapshot(ViewData),
    /// A feature-specific message (e.g. `tasks` notes loaded for a ticket).
    Tasks(tasks::Message),
    /// An error to surface to the owner (modal + already logged to console).
    Error(UserFacingError),
}
