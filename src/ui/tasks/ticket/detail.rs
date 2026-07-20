//! `tasks` UI: the ticket detail view, in two interchangeable presentations —
//!   - a **modal** overlay (default), and
//!   - a **full page** in the workspace (via the modal's expand button; Back returns).
//!
//! Both share one editing state ([`TicketModal`]) and one body renderer ([`body`]); the
//! only difference is the surrounding chrome. Intents are collected and dispatched after
//! the state's mutable borrow ends.

use uuid::Uuid;

use crate::app::Bridge;
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
}

impl BoardState {
    /// Modal presentation (overlay). Skipped while the detail is expanded to a full page.
    pub fn render_overlays(&mut self, ctx: &egui::Context, bridge: &Bridge, view: &TasksView) {
        let Some(modal) = self.modal.as_mut() else {
            return;
        };
        if modal.expanded {
            return; // the full-page view (in the workspace) is handling it
        }

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
                body(ui, modal, view, &mut out);
            });

        let dismissed = response.should_close();
        self.settle_detail(bridge, view, out, dismissed);
    }

    /// Full-page presentation, rendered in the workspace when the detail is expanded.
    pub(crate) fn render_ticket_page(
        &mut self,
        ui: &mut egui::Ui,
        bridge: &Bridge,
        view: &TasksView,
    ) {
        let Some(modal) = self.modal.as_mut() else {
            return;
        };

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
                ui.vertical_centered(|ui| {
                    ui.scope(|ui| {
                        ui.set_max_width(760.0);
                        ui.heading("Ticket");
                        ui.add_space(12.0);
                        body(ui, modal, view, &mut out);
                    });
                });
            });

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

/// The shared detail body: title, description, actions, relationships, notes.
fn body(ui: &mut egui::Ui, modal: &mut TicketModal, view: &TasksView, out: &mut Outcome) {
    let muted = theme::palette().muted;
    let ticket_id = modal.ticket_id;
    let current_stage = view.ticket(ticket_id).map(|t| t.stage_id);

    ui.label(egui::RichText::new("Title").strong().color(muted));
    ui.add_space(4.0);
    input::text_field(ui, &mut modal.title, "Ticket title");

    ui.add_space(10.0);
    ui.label(egui::RichText::new("Description").strong().color(muted));
    ui.add_space(4.0);
    input::text_area(ui, &mut modal.description, "What needs doing?", 4);

    ui.add_space(12.0);
    ui.horizontal(|ui| {
        let can_save = !modal.title.trim().is_empty();
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
    ui.label(egui::RichText::new("Notes").strong().size(15.0));
    ui.add_space(6.0);
    note::render_section(ui, modal, ticket_id, &mut out.events);
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
