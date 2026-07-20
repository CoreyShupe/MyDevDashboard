//! `tasks::ticket` create flow: the "new ticket" **modal** (title + description + optional
//! first note).
//!
//! This is deliberately a modal rather than the old inline column form — the same creation
//! flow is meant to be reused elsewhere in the app, and a modal composes anywhere a stage id
//! is in hand. The "+ New ticket" affordance in each stage column just opens this; the modal
//! itself renders as an overlay (like the ticket detail modal) so it floats over the board.

use uuid::Uuid;

use crate::app::Bridge;
use crate::app::tasks::Event;
use crate::ui::components::{button, input};
use crate::ui::theme;

use crate::ui::tasks::BoardState;

/// Draft state for the open "new ticket" modal. Only one is open at a time (board-wide).
pub(crate) struct NewTicketModal {
    /// Stage the ticket will be created in.
    stage_id: Uuid,
    title: String,
    description: String,
    /// Optional first note captured alongside the ticket.
    note: String,
}

impl NewTicketModal {
    fn new(stage_id: Uuid) -> Self {
        Self {
            stage_id,
            title: String::new(),
            description: String::new(),
            note: String::new(),
        }
    }
}

impl BoardState {
    /// The "+ New ticket" affordance in a stage column. Opens the create modal for `stage_id`.
    pub(in crate::ui::tasks) fn render_new_ticket(&mut self, ui: &mut egui::Ui, stage_id: Uuid) {
        if button::ghost(ui, &format!("{} New ticket", theme::icon::ADD)).clicked() {
            self.new_ticket = Some(NewTicketModal::new(stage_id));
        }
    }

    /// Dev-only: open the create modal directly for review (see `ui::dev`).
    pub(crate) fn dev_open_new_ticket(&mut self, stage_id: Uuid) {
        self.new_ticket = Some(NewTicketModal::new(stage_id));
    }

    /// Render the create-ticket modal overlay, if open. Called alongside the other overlays.
    pub(crate) fn render_create_modal(&mut self, ctx: &egui::Context, bridge: &Bridge) {
        let Some(draft) = self.new_ticket.as_mut() else {
            return;
        };
        let muted = theme::palette().muted;
        let mut submit = false;
        let mut cancel = false;

        let response = egui::Modal::new(egui::Id::new("new_ticket_modal"))
            .frame(theme::surface_frame())
            .show(ctx, |ui| {
                ui.set_min_width(460.0);
                ui.heading("New ticket");
                ui.add_space(10.0);

                ui.label(egui::RichText::new("Title").strong().color(muted));
                ui.add_space(4.0);
                input::text_field(ui, &mut draft.title, "Ticket title");

                ui.add_space(10.0);
                ui.label(egui::RichText::new("Description").strong().color(muted));
                ui.add_space(4.0);
                input::text_area(
                    ui,
                    &mut draft.description,
                    "What needs doing? (optional)",
                    3,
                );

                ui.add_space(10.0);
                ui.label(egui::RichText::new("First note").strong().color(muted));
                ui.add_space(4.0);
                input::text_area(ui, &mut draft.note, "Add an initial note (optional)", 2);

                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    let can_add = !draft.title.trim().is_empty();
                    submit = button::primary_enabled(ui, "Create ticket", can_add).clicked();
                    cancel = button::secondary(ui, "Cancel").clicked();
                });
            });

        // Backdrop / escape also cancels.
        if response.should_close() {
            cancel = true;
        }

        if submit && !draft.title.trim().is_empty() {
            let note = Some(draft.note.trim().to_owned()).filter(|n| !n.is_empty());
            bridge.send(Event::create_ticket(
                draft.stage_id,
                draft.title.trim().to_owned(),
                draft.description.trim().to_owned(),
                note,
            ));
            self.new_ticket = None;
        } else if cancel {
            self.new_ticket = None;
        }
    }
}
