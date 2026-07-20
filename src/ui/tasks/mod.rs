//! `tasks` feature UI: the kanban board, composed of part renderers (AGENTS.md §2).
//!
//! `BoardState` holds all transient board state centrally; the rendering for each part
//! lives in its own file as `impl BoardState`:
//!   - `stage` — the add-stage control, columns, and stage headers.
//!   - `ticket/` — its own folder of sub-parts: cards, detail (modal + full page),
//!     relationships, and notes.

mod stage;
mod ticket;

use std::collections::HashMap;

use uuid::Uuid;

use crate::app::Bridge;
use crate::app::tasks::{Message, View as TasksView};
use crate::ui::theme;

use ticket::{NewTicketDraft, TicketModal};

/// All transient UI state for the board. Lives in the UI only.
#[derive(Default)]
pub struct BoardState {
    new_stage_name: String,
    editing_stage: Option<(Uuid, String)>,
    /// Per-stage new-ticket drafts, keyed by stage id.
    new_ticket: HashMap<Uuid, NewTicketDraft>,
    /// The open ticket modal, if any.
    modal: Option<TicketModal>,
}

impl BoardState {
    /// Handle a feature message routed from the worker (e.g. notes finished loading).
    pub fn apply_message(&mut self, message: &Message) {
        match message {
            Message::Notes { ticket_id, notes } => {
                if let Some(modal) = &mut self.modal
                    && modal.ticket_id == *ticket_id
                {
                    modal.notes = notes.clone();
                    modal.notes_loaded = true;
                    modal.new_note.clear();
                }
            }
        }
    }

    /// Dev-only: open the ticket detail directly (no worker round-trip). `expanded` picks
    /// the full-page presentation over the modal. See `ui::dev`.
    pub fn dev_open(&mut self, ticket: &crate::domain::tasks::Ticket, expanded: bool) {
        let mut modal = TicketModal::new(ticket);
        modal.notes_loaded = true; // show the empty-notes state, not "Loading…"
        modal.expanded = expanded;
        self.modal = Some(modal);
    }

    /// Close a stale modal if its ticket disappeared in the latest snapshot.
    pub fn reconcile(&mut self, view: &TasksView) {
        if let Some(modal) = &self.modal
            && view.ticket(modal.ticket_id).is_none()
        {
            self.modal = None;
        }
    }

    /// The board workspace: header (title + add-stage) and the horizontal stage columns.
    ///
    /// When a ticket detail is expanded, the full-page ticket view takes over the workspace
    /// instead of the board (the modal overlay is suppressed while expanded).
    pub fn render_workspace(&mut self, ui: &mut egui::Ui, bridge: &Bridge, view: &TasksView) {
        if self.modal.as_ref().is_some_and(|m| m.expanded) {
            self.render_ticket_page(ui, bridge, view);
            return;
        }

        ui.horizontal(|ui| {
            ui.heading("Tasks");
            ui.add_space(16.0);
            self.render_add_stage(ui, bridge);
        });
        ui.add_space(10.0);

        if view.stages.is_empty() {
            // The "add stage" control in the header above is the single creation entry point
            // (AGENTS.md §5) — just point the owner at it; no duplicate control here.
            let muted = theme::palette().muted;
            ui.vertical_centered(|ui| {
                ui.add_space(48.0);
                ui.heading("Your board is empty");
                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new(
                        "Add your first stage above — e.g. \"Pending\", \"In Progress\", \"Complete\".",
                    )
                    .color(muted),
                );
            });
            return;
        }

        egui::ScrollArea::horizontal().show(ui, |ui| {
            ui.horizontal_top(|ui| {
                for stage in &view.stages {
                    self.render_stage_column(ui, bridge, stage, view);
                    ui.add_space(8.0);
                }
            });
        });
    }
}
