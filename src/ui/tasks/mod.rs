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
use crate::app::tasks::{Message, View as TasksView};
use crate::ui::theme;

use stage::StageModal;
use ticket::{NewTicketModal, TicketModal};

/// How a ticket link was activated → which presentation to open. Left-click opens the quick
/// **modal**; right-click (secondary) opens the **full page**. Every ticket link in the app — board
/// cards, parent/child quick-links, the Home overview, a project's "Open ticket" — funnels through
/// this so the gesture is consistent everywhere (tickets are the most-featured model, so opening
/// one behaves the same wherever it's referenced).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TicketOpen {
    /// The quick overlay modal (left-click).
    Modal,
    /// The full-page detail (right-click), rendered over whatever tab you're on.
    Page,
}

impl TicketOpen {
    /// Whether this presentation is the expanded full page.
    fn expanded(self) -> bool {
        matches!(self, Self::Page)
    }
}

/// Map a clickable ticket-link `Response` to how it was activated: left-click → `Modal`,
/// right-click → `Page`, no activation → `None`. The shared gesture for every ticket link.
pub fn ticket_open_from(response: &egui::Response) -> Option<TicketOpen> {
    if response.clicked() {
        Some(TicketOpen::Modal)
    } else if response.secondary_clicked() {
        Some(TicketOpen::Page)
    } else {
        None
    }
}

/// One step of ticket back-history: which ticket, and which presentation it was shown in, so
/// "Back" restores it exactly (AGENTS.md §2 "Ticket navigation").
#[derive(Debug, Clone, Copy)]
struct BackEntry {
    ticket_id: Uuid,
    expanded: bool,
}

/// All transient UI state for the board. Lives in the UI only.
#[derive(Default)]
pub struct BoardState {
    new_stage_name: String,
    /// Live board search query. When non-empty, only tickets whose title or description contain it
    /// (case-insensitive) are shown, across every column — and terminal stages reveal their matches
    /// instead of collapsing to a count, so nothing is hidden behind the count.
    search: String,
    /// The open "edit stage" modal, if any (name + terminal toggle + delete).
    editing_stage: Option<StageModal>,
    /// Terminal stages whose tickets are currently revealed (via "View tickets").
    viewing_terminal: HashSet<Uuid>,
    /// The open "new ticket" modal, if any (board-wide; one at a time).
    new_ticket: Option<NewTicketModal>,
    /// The open ticket detail, if any (modal or full page — `TicketModal::expanded` picks which).
    modal: Option<TicketModal>,
    /// Back-history of tickets viewed BELOW the current one (`modal`). Following a ticket link from
    /// within a detail pushes the current entry; "Back" pops it (or, when empty, closes the detail
    /// and returns to the tab underneath). A fresh open from outside a detail clears it (AGENTS.md
    /// tasks navigation).
    back_stack: Vec<BackEntry>,
    /// Set when the open ticket detail asks to create a worktree. The app shell drains this and
    /// opens the projects-owned create-worktree picker (cross-feature, AGENTS.md §2).
    pending_worktree: Option<Uuid>,
    /// Set when the ticket detail asks to remove a worktree — the shell hands it to the projects
    /// UI, which owns the remove confirmation (cross-feature, §2 + §13).
    pending_remove_worktree: Option<Uuid>,
    /// Set when the ticket detail asks to recreate a marker on a new branch — the shell hands it to
    /// the projects UI, which owns the branch picker (cross-feature, §2 + §10).
    pending_recreate_worktree_as: Option<Uuid>,
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

    /// Dev-only: preset the board search query so the filtered board can be captured (see `ui::dev`).
    pub fn dev_set_search(&mut self, query: &str) {
        self.search = query.to_owned();
    }

    /// The active search query, trimmed and lowercased for case-insensitive matching. Empty when
    /// no search is in effect (the board shows everything).
    fn search_query(&self) -> String {
        self.search.trim().to_lowercase()
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

    /// Dev-only: open a ticket detail with a back-history entry beneath it, so the "Back" affordance
    /// (which only shows when there's somewhere to go back to) is reviewable. See `ui::dev`.
    pub fn dev_open_with_back(
        &mut self,
        ticket: &crate::domain::tasks::Ticket,
        back_ticket_id: Uuid,
        expanded: bool,
    ) {
        self.back_stack = vec![BackEntry {
            ticket_id: back_ticket_id,
            expanded: false,
        }];
        self.dev_open(ticket, expanded);
    }

    /// Close a stale modal / confirmation if its ticket or stage disappeared in the latest
    /// snapshot (e.g. deleted elsewhere), so nothing dangles pointing at a gone entity.
    pub fn reconcile(&mut self, view: &TasksView) {
        if let Some(modal) = &self.modal
            && view.ticket(modal.ticket_id).is_none()
        {
            self.modal = None;
        }
        // Drop back-history entries for tickets deleted elsewhere, so "Back" never targets a ghost.
        self.back_stack
            .retain(|e| view.ticket(e.ticket_id).is_some());
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

    /// Take a pending "recreate marker on a new branch" request raised by the ticket detail; the
    /// shell hands it to the projects UI, which owns the branch picker (§2 + §10).
    pub fn take_pending_recreate_worktree_as(&mut self) -> Option<Uuid> {
        self.pending_recreate_worktree_as.take()
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
    /// The full-page ticket view is NOT rendered here — when a ticket is expanded the shell renders
    /// it as a workspace takeover over WHATEVER tab is active (so a ticket opened from Home stays on
    /// Home underneath), see `has_expanded_ticket`. The modal overlay likewise floats over any tab.
    pub fn render_workspace(&mut self, ui: &mut egui::Ui, bridge: &Bridge, view: &TasksView) {
        ui.horizontal(|ui| {
            ui.heading("Tasks");
            ui.add_space(16.0);
            self.render_add_stage(ui, bridge);
            // Search filters tickets across every column. Only offered once there are stages —
            // an empty board has nothing to search, so the field would just be noise.
            if !view.stages.is_empty() {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if !self.search.is_empty()
                        && crate::ui::components::button::icon(
                            ui,
                            theme::icon::CLOSE,
                            "Clear search",
                        )
                        .clicked()
                    {
                        self.search.clear();
                    }
                    crate::ui::components::input::text_field_sized(
                        ui,
                        &mut self.search,
                        "Search tickets…",
                        220.0,
                    );
                });
            }
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
