//! The `app` layer: the bridge between UI and System, and the orchestration ROOT.
//!
//! Shared root pieces live directly here (`bridge`, `event`, `state`, `worker`); each
//! feature has a sub-root folder (`profile/`, `tasks/`) mirroring the other layers and
//! owning that feature's `Event`/`View`/`handle()`. See AGENTS.md §2.

pub mod bridge;
pub mod event;
pub mod notes;
pub mod profile;
pub mod projects;
pub mod state;
pub mod tasks;
pub mod todos;
pub mod worker;

pub use bridge::{Bridge, repainter_from_ctx};
pub use event::{AppMessage, UiEvent};
pub use state::ViewData;
