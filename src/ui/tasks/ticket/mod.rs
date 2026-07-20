//! `tasks::ticket` part UI, composed of its own sub-parts:
//!   - this file — ticket cards on the board + the new-ticket draft,
//!   - `detail`  — the ticket detail view (modal overlay + full-page presentations),
//!   - `link`    — the parent/child relationships section,
//!   - `note`    — the notes list/entry.

mod detail;
mod link;
mod note;

use uuid::Uuid;

use crate::app::Bridge;
use crate::app::tasks::Event;
use crate::domain::tasks::Ticket;
use crate::ui::components::{button, card, input};
use crate::ui::theme;

use super::BoardState;

// The open ticket detail's state lives with the `detail` sub-part; re-export so the board
// (`tasks::mod`) can hold it in `BoardState`.
pub(super) use detail::TicketModal;

/// Draft state for creating a ticket within a specific stage column.
#[derive(Default)]
pub(super) struct NewTicketDraft {
    title: String,
    description: String,
    open: bool,
}

impl BoardState {
    /// A single ticket card (inset surface). The 6-dot handle (top-right) is a drag source
    /// for moving the ticket between stages; clicking the body opens the detail modal.
    pub(super) fn render_ticket_card(
        &mut self,
        ui: &mut egui::Ui,
        bridge: &Bridge,
        ticket: &Ticket,
    ) {
        let muted = theme::palette().muted;
        let drag_id = egui::Id::new(("ticket_drag", ticket.id));

        // The WHOLE card is the drag source, so the whole card floats while dragging (the
        // 6-dot handle is just the visual affordance). Payload = the ticket id.
        let card = ui
            .dnd_drag_source(drag_id, ticket.id, |ui| {
                card::inset(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(&ticket.title).strong());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(theme::icon::DRAG.to_string())
                                        .color(muted)
                                        .size(16.0),
                                )
                                .selectable(false),
                            );
                        });
                    });
                    if !ticket.description.is_empty() {
                        ui.add_space(3.0);
                        let preview = truncate(&ticket.description, 90);
                        ui.label(egui::RichText::new(preview).color(muted).size(12.5));
                    }
                });
            })
            .response;

        // The drag source only senses drags, so add a separate click sense (same rect) to
        // open the modal. A plain click opens; a drag moves — egui separates the two.
        // Force the pointing-hand cursor (the card is clickable) over the drag source's grab.
        let click = ui
            .interact(card.rect, drag_id.with("open"), egui::Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        if click.clicked() {
            self.open_ticket_modal(bridge, ticket);
        }
    }

    /// The collapsed "+ New ticket" affordance that expands into a title/description form.
    pub(super) fn render_new_ticket(&mut self, ui: &mut egui::Ui, bridge: &Bridge, stage_id: Uuid) {
        let draft = self.new_ticket.entry(stage_id).or_default();

        if !draft.open {
            if button::ghost(ui, &format!("{} New ticket", theme::icon::ADD)).clicked() {
                draft.open = true;
            }
            return;
        }

        input::text_field(ui, &mut draft.title, "Title");
        ui.add_space(4.0);
        input::text_area(ui, &mut draft.description, "Description (optional)", 2);
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            let can_add = !draft.title.trim().is_empty();
            let add = button::primary_enabled(ui, "Add", can_add).clicked();
            let cancel = button::secondary(ui, "Cancel").clicked();

            if add && can_add {
                bridge.send(Event::create_ticket(
                    stage_id,
                    draft.title.trim().to_owned(),
                    draft.description.trim().to_owned(),
                ));
                *draft = NewTicketDraft::default();
            } else if cancel {
                *draft = NewTicketDraft::default();
            }
        });
    }

    /// Open the detail modal for a ticket and request its notes from the worker.
    pub(super) fn open_ticket_modal(&mut self, bridge: &Bridge, ticket: &Ticket) {
        bridge.send(Event::load_notes(ticket.id));
        self.modal = Some(detail::TicketModal::new(ticket));
    }
}

/// Truncate a string to at most `max` chars, appending an ellipsis if cut.
fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_owned();
    }
    let mut out: String = s.chars().take(max).collect();
    out.push('…');
    out
}
