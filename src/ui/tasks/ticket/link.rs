//! `tasks` UI: ticket parent/child relationships, rendered inside the ticket modal.
//!
//! Shows a quick-link to the parent, a list of children (each navigable), and an inline
//! "add child ticket" form. Mutations are collected as `Event`s; navigation is signalled
//! via `navigate` so the modal can swap to another ticket after the borrow ends.

use uuid::Uuid;

use crate::app::tasks::{Event, View as TasksView};
use crate::ui::components::{button, input};
use crate::ui::theme;

use super::detail::TicketModal;

pub(super) fn render(
    ui: &mut egui::Ui,
    modal: &mut TicketModal,
    view: &TasksView,
    events: &mut Vec<Event>,
    navigate: &mut Option<Uuid>,
) {
    let muted = theme::palette().muted;
    let ticket_id = modal.ticket_id;

    // --- Parent quick-link ------------------------------------------------------
    let parent = view.ticket(ticket_id).and_then(|t| view.parent_of(t));
    if let Some(parent) = parent {
        ui.horizontal(|ui| {
            if button::link(
                ui,
                &format!("{} Parent: {}", theme::icon::PARENT, parent.title),
            )
            .clicked()
            {
                *navigate = Some(parent.id);
            }
            if button::icon(ui, theme::icon::UNLINK, "Detach from parent").clicked() {
                events.push(Event::unlink_ticket(ticket_id));
            }
        });
        ui.add_space(8.0);
    }

    // --- Children ---------------------------------------------------------------
    let children: Vec<_> = view.children_of(ticket_id).cloned().collect();
    if children.is_empty() {
        ui.label(egui::RichText::new("No child tickets yet.").color(muted));
    } else {
        for child in &children {
            if button::link(ui, &format!("{} {}", theme::icon::CHILD, child.title)).clicked() {
                *navigate = Some(child.id);
            }
        }
    }

    // --- Add child --------------------------------------------------------------
    ui.add_space(4.0);
    if !modal.adding_child {
        if button::ghost(ui, &format!("{} Add child ticket", theme::icon::ADD)).clicked() {
            modal.adding_child = true;
        }
        return;
    }

    input::text_field(ui, &mut modal.new_child_title, "Child ticket title");
    ui.add_space(4.0);
    input::text_area(ui, &mut modal.new_child_desc, "Description (optional)", 2);
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        let can_add = !modal.new_child_title.trim().is_empty();
        if button::primary_enabled(ui, "Add child", can_add).clicked() && can_add {
            events.push(Event::create_child(
                ticket_id,
                modal.new_child_title.trim().to_owned(),
                modal.new_child_desc.trim().to_owned(),
            ));
            reset_child_draft(modal);
        }
        if button::secondary(ui, "Cancel").clicked() {
            reset_child_draft(modal);
        }
    });
}

fn reset_child_draft(modal: &mut TicketModal) {
    modal.new_child_title.clear();
    modal.new_child_desc.clear();
    modal.adding_child = false;
}
