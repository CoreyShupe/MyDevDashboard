//! Solid, opaque, rounded surfaces that sit over the grid background.
//!
//! Because the workspace has a grid texture, anything floating over it MUST use a solid
//! surface so the grid doesn't bleed through (AGENTS.md §7). `card` is that surface.

use egui::{InnerResponse, Ui};

use crate::ui::theme;

/// A rounded, solid card with a soft shadow (elevation 1). Use for columns, modals, and
/// any standalone surface sitting directly over the grid.
///
/// Content stacks vertically regardless of the surrounding layout (cards placed inside a
/// horizontal row must still lay their own contents top-to-bottom).
pub fn card<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R> {
    theme::surface_frame().show(ui, |ui| ui.vertical(add).inner)
}

/// A rounded, solid inset card (elevation 2, one shade lighter). Use for cards nested
/// inside a `card` — ticket cards inside a column, note rows inside the modal.
pub fn inset<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R> {
    theme::inset_frame().show(ui, |ui| ui.vertical(add).inner)
}
