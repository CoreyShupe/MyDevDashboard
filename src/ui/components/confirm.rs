//! A shared confirmation modal for destructive actions. EVERY delete/remove funnels through
//! this (AGENTS.md §7, §13) so confirmations look and behave identically: a red warning title,
//! an explanatory body, and a Delete / Cancel row. Backdrop or Escape counts as Cancel.
//!
//! It is stateless — the caller owns the "is this confirm open" flag (typically an
//! `Option<Id>` on the feature's UI state), shows the modal each frame while it's set, and acts
//! on the returned [`Choice`].

use std::fmt::Debug;
use std::hash::Hash;

use crate::ui::components::button;
use crate::ui::theme;

/// The result of showing a confirmation this frame.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Choice {
    /// Neither button pressed yet — keep the confirm open.
    Pending,
    /// The owner confirmed the destructive action — perform it and clear the flag.
    Confirmed,
    /// The owner cancelled (Cancel button, backdrop, or Escape) — just clear the flag.
    Cancelled,
}

/// Show a destructive-confirmation modal. `id_salt` disambiguates it from any other modal on
/// screen (pass something stable + unique, e.g. `("delete_ticket", id)`); `title`/`body` are the
/// copy; `confirm_label` names the destructive button (e.g. "Delete", "Remove").
pub fn destructive(
    ctx: &egui::Context,
    id_salt: impl Hash + Debug,
    title: &str,
    body: &str,
    confirm_label: &str,
) -> Choice {
    let p = theme::palette();
    let mut confirm = false;
    let mut cancel = false;

    let response = egui::Modal::new(egui::Id::new(("confirm_destructive", id_salt)))
        .frame(theme::surface_frame())
        .show(ctx, |ui| {
            ui.set_max_width(440.0);
            ui.label(
                egui::RichText::new(format!("{} {title}", theme::icon::WARNING))
                    .heading()
                    .color(p.danger),
            );
            ui.add_space(8.0);
            ui.label(body);
            ui.add_space(14.0);
            ui.horizontal(|ui| {
                confirm = button::danger(ui, &format!("{} {confirm_label}", theme::icon::DELETE))
                    .clicked();
                cancel = button::secondary(ui, "Cancel").clicked();
            });
        });

    // Backdrop / Escape closes without acting.
    if response.should_close() {
        cancel = true;
    }

    if confirm {
        Choice::Confirmed
    } else if cancel {
        Choice::Cancelled
    } else {
        Choice::Pending
    }
}
