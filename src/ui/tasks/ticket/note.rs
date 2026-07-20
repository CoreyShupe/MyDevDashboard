//! `tasks::note` part UI: the notes list + entry, rendered inside the ticket modal.

use uuid::Uuid;

use crate::app::tasks::Event;
use crate::ui::components::{button, card, input};
use crate::ui::theme;

use super::detail::TicketModal;

/// Render the notes list and the "add note" input. Collects intent into `events`.
pub(super) fn render_section(
    ui: &mut egui::Ui,
    modal: &mut TicketModal,
    ticket_id: Uuid,
    events: &mut Vec<Event>,
) {
    let muted = theme::palette().muted;

    if !modal.notes_loaded {
        ui.label(egui::RichText::new("Loading notes…").color(muted));
    } else if modal.notes.is_empty() {
        ui.label(egui::RichText::new("No notes yet.").color(muted));
    } else {
        egui::ScrollArea::vertical()
            .id_salt(("notes_scroll", ticket_id))
            .max_height(180.0)
            .show(ui, |ui| {
                for note in &modal.notes {
                    card::inset(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.label(&note.body);
                        ui.label(
                            egui::RichText::new(
                                note.created_at.format("%Y-%m-%d %H:%M").to_string(),
                            )
                            .color(muted)
                            .size(11.0),
                        );
                    });
                    ui.add_space(6.0);
                }
            });
    }

    ui.add_space(6.0);
    input::text_area(ui, &mut modal.new_note, "Add a note…", 2);
    ui.add_space(6.0);
    let can_add = !modal.new_note.trim().is_empty();
    if button::primary_enabled(ui, "Add note", can_add).clicked() {
        events.push(Event::add_note(ticket_id, modal.new_note.trim().to_owned()));
    }
}
