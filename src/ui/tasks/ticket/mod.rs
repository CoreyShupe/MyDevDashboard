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
use crate::app::tasks::{Event, View as TasksView};
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
                                    egui::RichText::new(truncate(&ticket.title, TITLE_MAX))
                                        .strong(),
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
                    let preview = truncate(&ticket.description, DESC_PREVIEW_MAX);
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

        // Left-click opens the quick modal; right-click opens the full page (the shared ticket-link
        // gesture, §2 tasks navigation). A drag is neither, so reordering never opens the detail.
        if let Some(open) = super::ticket_open_from(&card) {
            self.open_ticket(bridge, ticket, open);
        }
    }

    /// Open a ticket detail FRESH — from OUTSIDE any detail (a board card, the Home overview, a
    /// project's worktree row). Clears the back-history, so "Back" from here closes the detail and
    /// returns to the tab underneath. `pub(crate)` so the app shell can open a ticket from another
    /// feature. `open` picks the presentation (modal vs full page).
    pub(crate) fn open_ticket(
        &mut self,
        bridge: &Bridge,
        ticket: &Ticket,
        open: super::TicketOpen,
    ) {
        self.back_stack.clear();
        self.set_current(bridge, ticket, open.expanded());
    }

    /// Navigate to another ticket from WITHIN a detail (a parent/child or other ticket link). Pushes
    /// the current ticket onto the back-stack (so "Back" returns to it), then shows the new one. A
    /// right-click (`Page`) always forces the full page; a left-click (`Modal`) continues in the
    /// CURRENT presentation, so following links inside a full page stays full-page (and inside a
    /// modal stays a modal) rather than jarringly switching.
    pub(crate) fn navigate_to(
        &mut self,
        bridge: &Bridge,
        ticket: &Ticket,
        open: super::TicketOpen,
    ) {
        let current_expanded = self.modal.as_ref().is_some_and(|m| m.expanded);
        let expanded = open.expanded() || current_expanded;
        if let Some(current) = self.modal.as_ref() {
            self.back_stack.push(super::BackEntry {
                ticket_id: current.ticket_id,
                expanded: current.expanded,
            });
        }
        self.set_current(bridge, ticket, expanded);
    }

    /// Expand the current modal to the full page, pushing the modal presentation onto the back-stack
    /// so "Back" returns to it (the "Expand" affordance is thus a real forward navigation step).
    pub(crate) fn expand_current(&mut self) {
        if let Some(current) = self.modal.as_ref() {
            self.back_stack.push(super::BackEntry {
                ticket_id: current.ticket_id,
                expanded: false,
            });
        }
        if let Some(modal) = self.modal.as_mut() {
            modal.expanded = true;
        }
    }

    /// "Back": pop the previous ticket (restoring its presentation) if there is one; otherwise close
    /// the detail entirely and fall back to the tab underneath. Skips any popped ticket that has
    /// since vanished from the snapshot so Back never strands on a dead entry.
    pub(crate) fn go_back(&mut self, bridge: &Bridge, view: &TasksView) {
        while let Some(entry) = self.back_stack.pop() {
            if let Some(ticket) = view.ticket(entry.ticket_id) {
                self.set_current(bridge, ticket, entry.expanded);
                return;
            }
        }
        self.modal = None; // nothing left to go back to → leave the detail
    }

    /// Whether a ticket detail is currently expanded to the full page — the shell renders the page
    /// as a workspace takeover over the active tab while this holds.
    pub(crate) fn has_expanded_ticket(&self) -> bool {
        self.modal.as_ref().is_some_and(|m| m.expanded)
    }

    /// Close any open ticket detail (modal or full page) and clear back-history. Called when the
    /// owner clicks a nav tab: a ticket detail renders OVER the active tab (§2 ticket navigation),
    /// so switching tabs must drop it — otherwise the detail stays on top and the tab click looks
    /// dead. Returns you to the clicked tab's dashboard.
    pub(crate) fn close_detail(&mut self) {
        self.modal = None;
        self.back_stack.clear();
    }

    /// Whether "Back" would return to a previous ticket (vs. close the detail) — drives whether the
    /// detail shows a "Back" affordance.
    pub(crate) fn can_go_back(&self) -> bool {
        !self.back_stack.is_empty()
    }

    /// Set the currently-shown ticket in the given presentation and request its notes.
    fn set_current(&mut self, bridge: &Bridge, ticket: &Ticket, expanded: bool) {
        bridge.send(Event::load_notes(ticket.id));
        let mut modal = detail::TicketModal::new(ticket);
        modal.expanded = expanded;
        self.modal = Some(modal);
    }
}

/// Char cap for the description preview on a card.
const DESC_PREVIEW_MAX: usize = 90;
/// Char cap for the title on a card. The title is rendered WRAPPED (like the description), so
/// a long title flows onto extra lines and stays readable rather than being ellipsised into an
/// unreadable single-line preview — it just gets a touch more room than the description (5 chars).
const TITLE_MAX: usize = DESC_PREVIEW_MAX + 5;

/// Truncate a string to at most `max` chars, appending an ellipsis if cut.
fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_owned();
    }
    let mut out: String = s.chars().take(max).collect();
    out.push('…');
    out
}
