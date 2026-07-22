//! `tasks` UI: the ticket detail view, in two interchangeable presentations —
//!   - a **modal** overlay (default; left-click a ticket link), and
//!   - a **full page** over the active tab (right-click a link, or the modal's Expand button).
//!
//! Navigation is browser-like: following a ticket link pushes onto a back-stack, and "Back" pops
//! to the previous ticket (or closes the detail and returns to the tab underneath). See
//! `BoardState::{open_ticket, navigate_to, go_back, expand_current}` (tasks navigation).
//!
//! Both share one editing state ([`TicketModal`]). They deliberately show DIFFERENT amounts:
//!   - the **modal** is a quick editor — [`body_core`] (title, description, save / move /
//!     delete) + [`body_relationships`] (parent/child are quick navigation), then a one-line
//!     summary and an Expand link. It stays short.
//!   - the **full page** adds [`body_worktrees`] beside a full [`notes_section`] — everything,
//!     with room to breathe.
//!
//! Intents are collected into an [`Outcome`] and dispatched after the state's mutable borrow ends.

use uuid::Uuid;

use crate::app::Bridge;
use crate::app::projects::View as ProjectsView;
use crate::app::tasks::{Event, View as TasksView};
use crate::domain::tasks::{Note, Ticket};
use crate::ui::components::{button, input};
use crate::ui::theme;

use crate::ui::tasks::{BoardState, TicketOpen};

use super::note;

/// State backing the open ticket detail (whichever presentation is active).
///
/// Fields are `pub(crate)` because the board (`tasks::mod`) reads a few of them to
/// reconcile/route messages, while the `ticket` sub-parts read/write the rest.
pub(crate) struct TicketModal {
    pub(crate) ticket_id: Uuid,
    pub(crate) title: String,
    pub(crate) description: String,
    pub(crate) notes: Vec<Note>,
    pub(crate) notes_loaded: bool,
    pub(crate) new_note: String,
    // Add-child draft (relationships section).
    pub(crate) new_child_title: String,
    pub(crate) new_child_desc: String,
    pub(crate) adding_child: bool,
    /// false = modal overlay, true = full-page view in the workspace.
    pub(crate) expanded: bool,
}

impl TicketModal {
    pub(crate) fn new(ticket: &Ticket) -> Self {
        Self {
            ticket_id: ticket.id,
            title: ticket.title.clone(),
            description: ticket.description.clone(),
            notes: Vec::new(),
            notes_loaded: false,
            new_note: String::new(),
            new_child_title: String::new(),
            new_child_desc: String::new(),
            adding_child: false,
            expanded: false,
        }
    }
}

/// Collected outcomes of rendering the detail body/chrome.
#[derive(Default)]
struct Outcome {
    events: Vec<Event>,
    /// Follow a ticket link (parent/child quick-link) to another ticket, in the given presentation
    /// (left-click → continue current; right-click → full page). Pushes back-history.
    navigate: Option<(Uuid, TicketOpen)>,
    /// "Back" was pressed — pop the back-stack (or close the detail if empty).
    back: bool,
    /// Expand the modal to the full page (pushes back-history so Back returns to the modal).
    expand: bool,
    /// Close the detail entirely (delete / close button / backdrop).
    close: bool,
    /// The detail asked to create a worktree for this ticket (the shell opens the picker).
    create_worktree: bool,
    /// The owner clicked Delete — the board opens a confirmation before it actually deletes
    /// (destructive, AGENTS.md §13).
    request_delete: bool,
    /// The owner asked to remove this worktree — routed to the projects UI to confirm (§13).
    remove_worktree: Option<Uuid>,
    /// The owner asked to recreate this marker on a NEW branch — routed to the projects UI, which
    /// owns the branch picker (cross-feature, §2 + §10).
    recreate_worktree_as: Option<Uuid>,
}

impl BoardState {
    /// Modal presentation (overlay). Skipped while the detail is expanded to a full page.
    pub fn render_overlays(
        &mut self,
        ctx: &egui::Context,
        bridge: &Bridge,
        view: &TasksView,
        projects: &ProjectsView,
    ) {
        let can_back = self.can_go_back();
        let Some(modal) = self.modal.as_mut() else {
            return;
        };
        if modal.expanded {
            return; // the full-page view (in the workspace) is handling it
        }
        let ticket_id = modal.ticket_id;

        let mut out = Outcome::default();
        let response = egui::Modal::new(egui::Id::new(("ticket_modal", modal.ticket_id)))
            .frame(theme::surface_frame())
            .show(ctx, |ui| {
                ui.set_min_width(460.0);
                ui.horizontal(|ui| {
                    // "Back" only appears when there's history to return to (otherwise the X is the
                    // way out) — a real back step, mirroring the full page (tasks navigation).
                    if can_back
                        && button::link(ui, &format!("{} Back", theme::icon::BACK)).clicked()
                    {
                        out.back = true;
                    }
                    ui.heading("Ticket");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if button::icon(ui, theme::icon::CLOSE, "Close").clicked() {
                            out.close = true;
                        }
                        if button::icon(ui, theme::icon::EXPAND, "Expand to full page").clicked() {
                            out.expand = true;
                        }
                    });
                });
                ui.add_space(10.0);
                // The modal is the QUICK editor: core fields + actions + relationships (parent/
                // child are quick navigation), then a compact summary. Worktrees and the notes
                // list live on the full page (Expand) so the modal stays short (AGENTS.md §8).
                body_core(ui, modal, view, &mut out);
                body_relationships(ui, modal, view, &mut out);
                ui.add_space(14.0);
                ui.separator();
                ui.add_space(8.0);
                modal_summary(ui, modal, projects);
                ui.add_space(6.0);
                if button::link(
                    ui,
                    &format!("{} Expand for worktrees & notes", theme::icon::EXPAND),
                )
                .clicked()
                {
                    out.expand = true;
                }
            });

        let dismissed = response.should_close();
        if out.create_worktree {
            self.pending_worktree = Some(ticket_id);
        }
        if let Some(id) = out.remove_worktree {
            self.pending_remove_worktree = Some(id);
        }
        if let Some(id) = out.recreate_worktree_as {
            self.pending_recreate_worktree_as = Some(id);
        }
        self.settle_detail(bridge, view, out, dismissed);
    }

    /// Full-page presentation, rendered in the workspace when the detail is expanded.
    pub(crate) fn render_ticket_page(
        &mut self,
        ui: &mut egui::Ui,
        bridge: &Bridge,
        view: &TasksView,
        projects: &ProjectsView,
    ) {
        let Some(modal) = self.modal.as_mut() else {
            return;
        };
        let ticket_id = modal.ticket_id;

        let mut out = Outcome::default();
        ui.horizontal(|ui| {
            // "Back" is a real back step: the previous ticket if there is one, else it closes the
            // detail and returns to the tab underneath (tasks navigation).
            if button::link(ui, &format!("{} Back", theme::icon::BACK)).clicked() {
                out.back = true;
            }
        });
        ui.add_space(10.0);

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.heading("Ticket");
                ui.add_space(12.0);
                // Full-width two columns: ticket data pinned to the left edge, the full notes
                // list pinned to the right edge, with a clear gap between them (the outer
                // margins come from the workspace's own 18px `CentralPanel` inset). Widen the
                // inter-column gap, then restore normal spacing inside each column so button
                // rows etc. aren't stretched.
                ui.spacing_mut().item_spacing.x = 40.0;
                ui.columns(2, |cols| {
                    cols[0].spacing_mut().item_spacing.x = 8.0;
                    body_core(&mut cols[0], modal, view, &mut out);
                    body_worktrees(&mut cols[0], modal, projects, bridge, &mut out);
                    body_relationships(&mut cols[0], modal, view, &mut out);
                    cols[1].spacing_mut().item_spacing.x = 8.0;
                    notes_section(&mut cols[1], modal, &mut out, None);
                });
            });

        if out.create_worktree {
            self.pending_worktree = Some(ticket_id);
        }
        if let Some(id) = out.remove_worktree {
            self.pending_remove_worktree = Some(id);
        }
        if let Some(id) = out.recreate_worktree_as {
            self.pending_recreate_worktree_as = Some(id);
        }
        self.settle_detail_page(bridge, view, out);
    }

    /// Dispatch a modal's collected outcome. `dismissed` = backdrop/escape closed it. Both
    /// presentations settle the same way (via [`settle`](Self::settle)); the modal adds
    /// backdrop-dismiss as a full close.
    fn settle_detail(&mut self, bridge: &Bridge, view: &TasksView, out: Outcome, dismissed: bool) {
        let close = out.close || dismissed;
        self.settle(bridge, view, out, close);
    }

    /// Dispatch a full-page outcome.
    fn settle_detail_page(&mut self, bridge: &Bridge, view: &TasksView, out: Outcome) {
        let close = out.close;
        self.settle(bridge, view, out, close);
    }

    /// Shared outcome dispatch for both presentations. Priority: send events, then handle (in
    /// order) delete-confirm, back, expand, navigate, close — each mutually exclusive per frame.
    fn settle(&mut self, bridge: &Bridge, view: &TasksView, out: Outcome, close: bool) {
        for event in out.events {
            bridge.send(event);
        }
        if out.request_delete {
            // Open the confirmation but leave the detail up behind it (§13) — cancelling returns
            // the owner to the ticket they were viewing.
            self.confirm_delete_ticket = self.modal.as_ref().map(|m| m.ticket_id);
        } else if out.back {
            self.go_back(bridge, view); // previous ticket, or close to the tab if none
        } else if out.expand {
            self.expand_current(); // modal → full page, pushing back-history
        } else if let Some((target, open)) = out.navigate {
            if let Some(ticket) = view.ticket(target) {
                self.navigate_to(bridge, ticket, open);
            }
        } else if close {
            self.modal = None;
            self.back_stack.clear();
        }
    }
}

/// The core detail body shown in BOTH presentations: title, description, and the action row
/// (save / move stage / delete). Kept deliberately small so the modal stays short.
fn body_core(ui: &mut egui::Ui, modal: &mut TicketModal, view: &TasksView, out: &mut Outcome) {
    let muted = theme::palette().muted;
    let ticket_id = modal.ticket_id;
    let saved = view.ticket(ticket_id);
    let current_stage = saved.map(|t| t.stage_id);

    // Which fields differ from what's persisted? (Compared trimmed, since that's what a save
    // writes.) Drives both the per-field "edited" outline and the Save button's enabled state.
    let title_changed = saved.map(|t| t.title.as_str()) != Some(modal.title.trim());
    let desc_changed = saved.map(|t| t.description.as_str()) != Some(modal.description.trim());
    let dirty = title_changed || desc_changed;

    ui.label(egui::RichText::new("Title").strong().color(muted));
    ui.add_space(4.0);
    input::text_field_marked(ui, &mut modal.title, "Ticket title", title_changed);

    ui.add_space(10.0);
    ui.label(egui::RichText::new("Description").strong().color(muted));
    ui.add_space(4.0);
    input::text_area_marked(
        ui,
        &mut modal.description,
        "What needs doing?",
        4,
        desc_changed,
    );

    ui.add_space(12.0);
    ui.horizontal(|ui| {
        // Buttons and the combo box each clamp their height UP to `interact_size.y`. The
        // buttons' text+padding naturally exceeds the default (30), but the combo sits at the
        // floor — so they'd render at different heights. Raise the floor above the buttons'
        // natural height and everything in this row lands at the same height.
        ui.spacing_mut().interact_size.y = 34.0;

        // Only savable when there's an actual (non-empty-title) change to persist.
        let can_save = dirty && !modal.title.trim().is_empty();
        let save_label = format!("{} Save changes", theme::icon::SAVE);
        if button::primary_enabled(ui, &save_label, can_save).clicked() {
            out.events.push(Event::update_ticket(
                ticket_id,
                modal.title.trim().to_owned(),
                modal.description.trim().to_owned(),
            ));
        }

        if let Some(current) = current_stage {
            move_stage_combo(ui, view, current, ticket_id, &mut out.events);
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if button::danger(ui, &format!("{} Delete", theme::icon::DELETE)).clicked() {
                // Ask, don't act: the board opens a confirmation and only then deletes (§13).
                out.request_delete = true;
            }
        });
    });
}

/// The **Relationships** section (parent quick-link, children, add-child). Shown in BOTH
/// presentations — parent/child are quick navigation, which belongs in the quick modal too.
fn body_relationships(
    ui: &mut egui::Ui,
    modal: &mut TicketModal,
    view: &TasksView,
    out: &mut Outcome,
) {
    ui.add_space(14.0);
    ui.separator();
    ui.add_space(6.0);
    ui.label(egui::RichText::new("Relationships").strong().size(15.0));
    ui.add_space(6.0);
    super::link::render(ui, modal, view, &mut out.events, &mut out.navigate);
}

/// The **Worktrees** section — full page only (the modal summarises it, to stay short).
fn body_worktrees(
    ui: &mut egui::Ui,
    modal: &TicketModal,
    projects: &ProjectsView,
    bridge: &Bridge,
    out: &mut Outcome,
) {
    ui.add_space(14.0);
    ui.separator();
    ui.add_space(6.0);
    ui.label(egui::RichText::new("Worktrees").strong().size(15.0));
    ui.add_space(6.0);
    // Worktrees are a cross-feature (projects) concern; the projects UI renders this ticket's
    // worktrees and reports whether the owner asked to create one (AGENTS.md §2). Open/recreate
    // emit projects events directly; create + remove (destructive, §13) are reported back so the
    // shell hands them to the projects UI (which opens the picker / confirmation).
    if crate::ui::projects::render_ticket_worktrees(
        ui,
        bridge,
        modal.ticket_id,
        projects,
        &mut out.remove_worktree,
        &mut out.recreate_worktree_as,
    ) {
        out.create_worktree = true;
    }
}

/// The modal's one-line hint at what's behind Expand — note + worktree counts (children are
/// already shown inline via Relationships, so they're not repeated here).
fn modal_summary(ui: &mut egui::Ui, modal: &TicketModal, projects: &ProjectsView) {
    let muted = theme::palette().muted;
    let count = |n: usize, word: &str| format!("{n} {word}{}", if n == 1 { "" } else { "s" });

    let live_worktrees = projects
        .worktrees_for_ticket(modal.ticket_id)
        .filter(|w| w.is_live())
        .count();

    let mut parts = Vec::new();
    if modal.notes_loaded {
        parts.push(count(modal.notes.len(), "note"));
    }
    parts.push(count(live_worktrees, "worktree"));

    ui.label(
        egui::RichText::new(parts.join("  ·  "))
            .color(muted)
            .size(12.5),
    );
}

/// The notes section (header + list + entry). `limit` caps how many recent notes show —
/// the modal passes `Some(2)`, the full page passes `None` for the complete list.
fn notes_section(
    ui: &mut egui::Ui,
    modal: &mut TicketModal,
    out: &mut Outcome,
    limit: Option<usize>,
) {
    let ticket_id = modal.ticket_id;
    ui.label(egui::RichText::new("Notes").strong().size(15.0));
    ui.add_space(6.0);
    note::render_section(ui, modal, ticket_id, &mut out.events, limit);
}

/// Combo box to move a ticket to a different stage.
fn move_stage_combo(
    ui: &mut egui::Ui,
    view: &TasksView,
    current: Uuid,
    ticket_id: Uuid,
    events: &mut Vec<Event>,
) {
    let current_name = view
        .stages
        .iter()
        .find(|s| s.id == current)
        .map(|s| s.name.as_str())
        .unwrap_or("—");

    egui::ComboBox::from_id_salt(("move_stage", ticket_id))
        .selected_text(format!("Move to: {current_name}"))
        .show_ui(ui, |ui| {
            for stage in &view.stages {
                if stage.id == current {
                    continue;
                }
                if ui.selectable_label(false, &stage.name).clicked() {
                    events.push(Event::move_ticket(ticket_id, stage.id));
                }
            }
        });
}
