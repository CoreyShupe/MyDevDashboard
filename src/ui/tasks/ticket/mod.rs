//! `tasks::ticket` part UI, composed of its own sub-parts:
//!   - this file — ticket cards on the board,
//!   - `create`  — the "new ticket" modal (title + description + optional first note),
//!   - `detail`  — the ticket detail view (modal overlay + full-page presentations),
//!   - `link`    — the parent/child relationships section,
//!   - `note`    — the notes list/entry.

mod create;
mod detail;
mod link;
mod note;

use crate::app::Bridge;
use crate::app::tasks::Event;
use crate::domain::tasks::Ticket;
use crate::ui::components::card;
use crate::ui::theme;

use super::BoardState;

// The open ticket detail's state lives with the `detail` sub-part; the create modal's state
// lives with `create`. Re-export both so the board (`tasks::mod`) can hold them in `BoardState`.
pub(super) use create::NewTicketModal;
pub(super) use detail::TicketModal;

impl BoardState {
    /// A single ticket card (inset surface). ONLY the 6-dot handle (top-right) initiates a
    /// drag to move the ticket between stages — the body is a plain click target that opens
    /// the detail modal, so clicking into a card never risks starting a drag. While the
    /// handle is dragged the whole card floats, so the entire card visibly moves.
    pub(super) fn render_ticket_card(
        &mut self,
        ui: &mut egui::Ui,
        bridge: &Bridge,
        ticket: &Ticket,
    ) {
        let muted = theme::palette().muted;
        let drag_id = egui::Id::new(("ticket_drag", ticket.id));

        // Renders the card surface once; returns the rect of the 6-dot drag handle so the
        // caller can restrict drag-sensing to just that handle. Used both for the normal
        // in-column layout and for the floating copy painted while dragging.
        let render_body = |ui: &mut egui::Ui| -> egui::Rect {
            let mut handle_rect = egui::Rect::NOTHING;
            card::inset(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(&ticket.title).strong());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                        handle_rect = ui
                            .add(
                                egui::Label::new(
                                    egui::RichText::new(theme::icon::DRAG.to_string())
                                        .color(muted)
                                        .size(16.0),
                                )
                                .selectable(false),
                            )
                            .rect;
                    });
                });
                if !ticket.description.is_empty() {
                    ui.add_space(3.0);
                    let preview = truncate(&ticket.description, 90);
                    ui.label(egui::RichText::new(preview).color(muted).size(12.5));
                }
            });
            handle_rect
        };

        // While dragging, paint the WHOLE card onto a floating layer that follows the
        // pointer, so the entire card appears to move even though only the handle started
        // the drag. Payload = the ticket id, re-set each frame like `dnd_drag_source` does.
        if ui.ctx().is_being_dragged(drag_id) {
            egui::DragAndDrop::set_payload(ui.ctx(), ticket.id);

            let layer_id = egui::LayerId::new(egui::Order::Tooltip, drag_id);
            let response = ui
                .scope_builder(egui::UiBuilder::new().layer_id(layer_id), |ui| {
                    render_body(ui);
                })
                .response;

            if let Some(pointer_pos) = ui.ctx().pointer_interact_pos() {
                let delta = pointer_pos - response.rect.center();
                ui.ctx().transform_layer_shapes(
                    layer_id,
                    egui::emath::TSTransform::from_translation(delta),
                );
            }
            return;
        }

        // Normal layout: lay out the card, then wire interactions on top of it.
        let egui::InnerResponse {
            inner: handle_rect,
            response,
        } = ui.scope(render_body);

        // Clicking anywhere on the card opens the detail modal. Added FIRST so the handle's
        // drag widget below sits on top of it — a press on the handle then starts a drag
        // while a press anywhere else registers as a click.
        let click = ui
            .interact(response.rect, drag_id.with("open"), egui::Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand);

        // The 6-dot handle is the ONLY drag source. Grab cursor advertises it.
        ui.interact(handle_rect, drag_id, egui::Sense::drag())
            .on_hover_cursor(egui::CursorIcon::Grab);

        if click.clicked() {
            self.open_ticket_modal(bridge, ticket);
        }
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
