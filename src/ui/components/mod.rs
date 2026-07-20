//! The shared component kit. Every feature composes its UI from these — features must NOT
//! hand-roll raw egui inputs/buttons/cards or hardcode colors (AGENTS.md §7). Add a new
//! component only when no existing one can be adapted by a small change.

pub mod button;
pub mod card;
pub mod dnd;
pub mod input;
