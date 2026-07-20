//! `tasks` feature UI: the kanban board, composed of part renderers (AGENTS.md §2).
//!
//! `BoardState` holds all transient board state centrally; the rendering for each part
//! lives in its own file as `impl BoardState`:
//!   - `stage` — the add-stage control, columns, and stage headers.
//!   - `ticket/` — its own folder of sub-parts: cards, detail (modal + full page),
//!     relationships, and notes.

mod stage;
mod ticket;

use std::collections::HashSet;

use uuid::Uuid;

use crate::app::Bridge;
use crate::app::projects::View as ProjectsView;
use crate::app::tasks::{Message, View as TasksView};
use crate::ui::theme;

use stage::StageModal;
use ticket::{NewTicketModal, TicketModal};

/// All transient UI state for the board. Lives in the UI only.
#[derive(Default)]
pub struct BoardState {
    new_stage_name: String,
    /// The open "edit stage" modal, if any (name + terminal toggle + delete).
    editing_stage: Option<StageModal>,
    /// Terminal stages whose tickets are currently revealed (via "View tickets").
    viewing_terminal: HashSet<Uuid>,
    /// The open "new ticket" modal, if any (board-wide; one at a time).
    new_ticket: Option<NewTicketModal>,
    /// The open ticket detail modal, if any.
    modal: Option<TicketModal>,
    /// Set when the open ticket detail asks to create a worktree. The app shell drains this and
    /// opens the projects-owned create-worktree picker (cross-feature, AGENTS.md §2).
    pending_worktree: Option<Uuid>,
    /// Set when the ticket detail asks to remove a worktree — the shell hands it to the projects
    /// UI, which owns the remove confirmation (cross-feature, §2 + §13).
    pending_remove_worktree: Option<Uuid>,
    /// A stage pending a delete confirmation, if any (destructive, §13).
    confirm_delete_stage: Option<Uuid>,
    /// A ticket pending a delete confirmation, if any (destructive, §13).
    confirm_delete_ticket: Option<Uuid>,
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

    /// Dev-only: open the delete-ticket confirmation directly for review (see `ui::dev`).
    pub fn dev_open_delete_ticket_confirm(&mut self, ticket_id: Uuid) {
        self.confirm_delete_ticket = Some(ticket_id);
    }

    /// Dev-only: open the ticket detail directly (no worker round-trip). `expanded` picks
    /// the full-page presentation over the modal. See `ui::dev`.
    pub fn dev_open(&mut self, ticket: &crate::domain::tasks::Ticket, expanded: bool) {
        let mut modal = TicketModal::new(ticket);
        modal.notes = crate::ui::dev::mock_notes(ticket.id); // exercise the notes cap / wide list
        modal.notes_loaded = true;
        modal.expanded = expanded;
        // Opens clean (buffers == saved), so Save starts disabled and no field is outlined —
        // edit a field live to see the "changed" outline + enabled Save.
        self.modal = Some(modal);
    }

    /// Close a stale modal / confirmation if its ticket or stage disappeared in the latest
    /// snapshot (e.g. deleted elsewhere), so nothing dangles pointing at a gone entity.
    pub fn reconcile(&mut self, view: &TasksView) {
        if let Some(modal) = &self.modal
            && view.ticket(modal.ticket_id).is_none()
        {
            self.modal = None;
        }
        if self
            .confirm_delete_ticket
            .is_some_and(|id| view.ticket(id).is_none())
        {
            self.confirm_delete_ticket = None;
        }
        if self
            .confirm_delete_stage
            .is_some_and(|id| view.stage(id).is_none())
        {
            self.confirm_delete_stage = None;
        }
    }

    /// Take a pending "create worktree for this ticket" request, if the open detail raised one.
    /// The app shell calls this after rendering to hand it to the projects UI.
    pub fn take_pending_worktree(&mut self) -> Option<Uuid> {
        self.pending_worktree.take()
    }

    /// Take a pending "remove worktree" request raised by the ticket detail; the shell hands it
    /// to the projects UI, which owns the remove confirmation (§13).
    pub fn take_pending_remove_worktree(&mut self) -> Option<Uuid> {
        self.pending_remove_worktree.take()
    }

    /// The board's destructive-confirmation overlays (delete ticket / delete stage). Rendered
    /// from the app shell alongside the other board modals. Each only shows while its
    /// `confirm_delete_*` slot is set; confirm fires the real event, cancel just clears it (§13).
    pub fn render_confirmations(&mut self, ctx: &egui::Context, bridge: &Bridge, view: &TasksView) {
        use crate::app::tasks::Event;
        use crate::ui::components::confirm::{self, Choice};

        if let Some(id) = self.confirm_delete_ticket {
            let title = view.ticket(id).map(|t| t.title.clone()).unwrap_or_default();
            let body = format!(
                "Delete this ticket? This can't be undone — its notes and any worktree records \
                 go with it, and its child tickets become top-level.\n\n“{title}”"
            );
            match confirm::destructive(ctx, ("delete_ticket", id), "Delete ticket", &body, "Delete")
            {
                Choice::Confirmed => {
                    bridge.send(Event::delete_ticket(id));
                    self.confirm_delete_ticket = None;
                    self.modal = None; // the ticket is gone; leave its detail
                }
                Choice::Cancelled => self.confirm_delete_ticket = None,
                Choice::Pending => {}
            }
        }

        if let Some(id) = self.confirm_delete_stage {
            let name = view.stage(id).map(|s| s.name.clone()).unwrap_or_default();
            let body = format!(
                "Delete the “{name}” stage? A stage can only be deleted once it's empty — move or \
                 delete its tickets first."
            );
            match confirm::destructive(ctx, ("delete_stage", id), "Delete stage", &body, "Delete") {
                Choice::Confirmed => {
                    bridge.send(Event::delete_stage(id));
                    self.confirm_delete_stage = None;
                }
                Choice::Cancelled => self.confirm_delete_stage = None,
                Choice::Pending => {}
            }
        }
    }

    /// The board workspace: header (title + add-stage) and the horizontal stage columns.
    ///
    /// When a ticket detail is expanded, the full-page ticket view takes over the workspace
    /// instead of the board (the modal overlay is suppressed while expanded).
    pub fn render_workspace(
        &mut self,
        ui: &mut egui::Ui,
        bridge: &Bridge,
        view: &TasksView,
        projects: &ProjectsView,
    ) {
        if self.modal.as_ref().is_some_and(|m| m.expanded) {
            self.render_ticket_page(ui, bridge, view, projects);
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
