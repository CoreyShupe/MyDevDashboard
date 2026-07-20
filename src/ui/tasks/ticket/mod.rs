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
use crate::ui::components::{card, dnd};
use crate::ui::theme;

use super::BoardState;

// The open ticket detail's state lives with the `detail` sub-part; the create modal's state
// lives with `create`. Re-export both so the board (`tasks::mod`) can hold them in `BoardState`.
pub(super) use create::NewTicketModal;
pub(super) use detail::TicketModal;

impl BoardState {
    /// A single ticket card (inset surface). The WHOLE card is both draggable (to move the
    /// ticket between stages) and clickable (opens the detail modal) via one `click_and_drag`
    /// sense — a press-and-release is a click, a press-and-drag lifts the card. The 6-dot grip
    /// (top-right) is kept purely as a visual affordance advertising that the card drags.
    pub(super) fn render_ticket_card(
        &mut self,
        ui: &mut egui::Ui,
        bridge: &Bridge,
        ticket: &Ticket,
    ) {
        let muted = theme::palette().muted;
        let drag_id = egui::Id::new(("ticket_drag", ticket.id));

        // Renders the card surface once; returns the rect of the 6-dot grip so the caller can
        // show a grab cursor over it. Used both for the normal in-column layout and for the
        // floating copy painted while dragging.
        let render_body = |ui: &mut egui::Ui| -> egui::Rect {
            let mut handle_rect = egui::Rect::NOTHING;
            card::inset(ui, |ui| {
                ui.set_width(ui.available_width());
                // The 6-dot grip lives in its OWN right-hand gutter that the title can never
                // enter: reserve a fixed width for the handle, then wrap the (char-capped)
                // title into the remaining space so long titles grow downward, not sideways.
                const HANDLE_GUTTER: f32 = 22.0;
                ui.horizontal_top(|ui| {
                    let title_width = (ui.available_width() - HANDLE_GUTTER).max(0.0);
                    ui.allocate_ui_with_layout(
                        egui::vec2(title_width, 0.0),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            ui.set_width(title_width);
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(truncate_title(&ticket.title)).strong(),
                                )
                                .wrap(),
                            );
                        },
                    );
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

        // While dragging, lift the WHOLE card onto a floating layer that follows the pointer
        // from the grab point (shared `dnd::drag_ghost`). Payload = the ticket id, re-set each
        // frame like `dnd_drag_source` does.
        if ui.ctx().is_being_dragged(drag_id) {
            egui::DragAndDrop::set_payload(ui.ctx(), ticket.id);
            dnd::drag_ghost(ui, drag_id, |ui| {
                render_body(ui);
            });
            return;
        }

        // Normal layout: lay out the card, then wire the interaction on top of it.
        let egui::InnerResponse {
            inner: handle_rect,
            response,
        } = ui.scope(render_body);

        // The WHOLE card is the drag source AND the click target: a press-and-release opens the
        // detail modal, a press-and-drag lifts the card to reorder it (`click_and_drag`).
        let card = ui.interact(response.rect, drag_id, egui::Sense::click_and_drag());

        // Cursor: Grab over the 6-dot grip (advertising the drag), PointingHand over the rest of
        // the card (advertising the click-to-open).
        if card.hovered()
            && let Some(pos) = ui.ctx().pointer_hover_pos()
        {
            ui.ctx().set_cursor_icon(if handle_rect.contains(pos) {
                egui::CursorIcon::Grab
            } else {
                egui::CursorIcon::PointingHand
            });
        }

        if card.clicked() {
            self.open_ticket_modal(bridge, ticket);
        }
    }

    /// Open the detail modal for a ticket and request its notes from the worker.
    pub(super) fn open_ticket_modal(&mut self, bridge: &Bridge, ticket: &Ticket) {
        bridge.send(Event::load_notes(ticket.id));
        self.modal = Some(detail::TicketModal::new(ticket));
    }
}

/// Cap a ticket title at 23 chars for the card, but break at a word boundary so a word is
/// never sliced mid-way — back up to the last space when the cap lands inside a word (unless
/// the very first word already exceeds the cap, where a hard cut is the only option). The
/// card renders this wrapped, so the kept text can still flow onto a second line by its spaces.
fn truncate_title(s: &str) -> String {
    const MAX: usize = 23;
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= MAX {
        return s.to_owned();
    }
    let capped: String = chars[..MAX].iter().collect();
    // If the first dropped char is whitespace the cap already sits on a word boundary; else
    // retreat to the last space so we don't cut a word in half.
    let cut = if chars[MAX].is_whitespace() {
        capped.trim_end()
    } else {
        match capped.rfind(char::is_whitespace) {
            Some(idx) => capped[..idx].trim_end(),
            None => capped.trim_end(), // single over-long word — hard cap
        }
    };
    format!("{cut}…")
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
