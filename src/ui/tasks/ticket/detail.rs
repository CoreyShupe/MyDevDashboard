//! `tasks` UI: the ticket detail view, in two interchangeable presentations —
//!   - a **modal** overlay (default), and
//!   - a **full page** in the workspace (via the modal's expand button; Back returns).
//!
//! Both share one editing state ([`TicketModal`]) and the same body renderers
//! ([`body_main`] plus [`notes_section`]); they differ only in the surrounding chrome and
//! how they lay those out — the modal stacks a capped notes list under the body, the full
//! page puts the full notes list in a wide column beside it. Intents are collected and
//! dispatched after the state's mutable borrow ends.

use uuid::Uuid;

use crate::app::Bridge;
use crate::app::projects::View as ProjectsView;
use crate::app::tasks::{Event, View as TasksView};
use crate::domain::tasks::{Note, Ticket};
use crate::ui::components::{button, input};
use crate::ui::theme;

use crate::ui::tasks::BoardState;

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
    /// Switch the open detail to this ticket (parent/child quick-link).
    navigate: Option<Uuid>,
    /// Close the detail entirely (delete / close button / backdrop).
    close: bool,
    /// The detail asked to create a worktree for this ticket (the shell opens the picker).
    create_worktree: bool,
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
                    ui.heading("Ticket");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if button::icon(ui, theme::icon::CLOSE, "Close").clicked() {
                            out.close = true;
                        }
                        if button::icon(ui, theme::icon::EXPAND, "Expand to full page").clicked() {
                            modal.expanded = true;
                        }
                    });
                });
                ui.add_space(10.0);
                body_main(ui, modal, view, projects, bridge, &mut out);
                ui.add_space(14.0);
                ui.separator();
                ui.add_space(6.0);
                // Modal stays compact: show only the last 2 notes with a "N more" line.
                notes_section(ui, modal, &mut out, Some(2));
            });

        let dismissed = response.should_close();
        if out.create_worktree {
            self.pending_worktree = Some(ticket_id);
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
            if button::link(ui, &format!("{} Back", theme::icon::BACK)).clicked() {
                modal.expanded = false; // return to the modal presentation
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
                    body_main(&mut cols[0], modal, view, projects, bridge, &mut out);
                    cols[1].spacing_mut().item_spacing.x = 8.0;
                    notes_section(&mut cols[1], modal, &mut out, None);
                });
            });

        if out.create_worktree {
            self.pending_worktree = Some(ticket_id);
        }
        self.settle_detail_page(bridge, view, out);
    }

    /// Dispatch a modal's collected outcome. `dismissed` = backdrop/escape closed it.
    fn settle_detail(&mut self, bridge: &Bridge, view: &TasksView, out: Outcome, dismissed: bool) {
        for event in out.events {
            bridge.send(event);
        }
        if let Some(target) = out.navigate {
            if let Some(ticket) = view.ticket(target) {
                self.open_ticket_modal(bridge, ticket);
            }
        } else if out.close || dismissed {
            self.modal = None;
        }
    }

    /// Dispatch a full-page outcome; parent/child navigation stays on the full page.
    fn settle_detail_page(&mut self, bridge: &Bridge, view: &TasksView, out: Outcome) {
        for event in out.events {
            bridge.send(event);
        }
        if let Some(target) = out.navigate {
            if let Some(ticket) = view.ticket(target) {
                self.open_ticket_modal(bridge, ticket);
                if let Some(modal) = self.modal.as_mut() {
                    modal.expanded = true; // keep the full-page presentation
                }
            }
        } else if out.close {
            self.modal = None;
        }
    }
}

/// The main detail body: title, description, actions, relationships. The notes section is
/// rendered separately so each presentation can place it differently — the modal appends a
/// capped list below this, the full page gives it its own wide column beside this.
fn body_main(
    ui: &mut egui::Ui,
    modal: &mut TicketModal,
    view: &TasksView,
    projects: &ProjectsView,
    bridge: &Bridge,
    out: &mut Outcome,
) {
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
                out.events.push(Event::delete_ticket(ticket_id));
                out.close = true;
            }
        });
    });

    ui.add_space(14.0);
    ui.separator();
    ui.add_space(6.0);
    ui.label(egui::RichText::new("Relationships").strong().size(15.0));
    ui.add_space(6.0);
    super::link::render(ui, modal, view, &mut out.events, &mut out.navigate);

    ui.add_space(14.0);
    ui.separator();
    ui.add_space(6.0);
    ui.label(egui::RichText::new("Worktrees").strong().size(15.0));
    ui.add_space(6.0);
    // Worktrees are a cross-feature (projects) concern; the projects UI renders this ticket's
    // worktrees and reports whether the owner asked to create one (AGENTS.md §2). Open/remove/
    // recreate emit projects events directly through the bridge.
    if crate::ui::projects::render_ticket_worktrees(ui, bridge, ticket_id, projects) {
        out.create_worktree = true;
    }
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
