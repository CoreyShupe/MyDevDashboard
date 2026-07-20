//! `tasks::ticket` create flow: the "new ticket" **modal** (stage + title + description +
//! optional first note).
//!
//! This is deliberately a modal rather than the old inline column form — the same creation
//! flow is meant to be reused elsewhere in the app, and a modal composes anywhere. The
//! "+ New ticket" affordance in each stage column opens it pre-targeted at that column; the
//! Notes tab opens it pre-filled with a captured note as the first note (see
//! [`BoardState::open_new_ticket_from_note`]). Because it can be opened without a column in
//! hand, the modal carries its own **stage picker** rather than a fixed stage id.

use uuid::Uuid;

use crate::app::Bridge;
use crate::app::notes;
use crate::app::tasks::{Event, View as TasksView};
use crate::ui::components::{button, input};
use crate::ui::theme;

use crate::ui::tasks::BoardState;

/// Draft state for the open "new ticket" modal. Only one is open at a time (board-wide).
pub(crate) struct NewTicketModal {
    /// Stage the ticket will be created in. `None` until one is picked (possible when the
    /// modal is opened without a column context and the board has no stages yet).
    stage_id: Option<Uuid>,
    title: String,
    description: String,
    /// Optional first note captured alongside the ticket.
    note: String,
    /// When the modal was opened from an uncategorized note, its id — so that note can be
    /// removed from the Notes tab once the ticket is successfully created.
    source_note_id: Option<Uuid>,
}

impl NewTicketModal {
    fn new(stage_id: Uuid) -> Self {
        Self {
            stage_id: Some(stage_id),
            title: String::new(),
            description: String::new(),
            note: String::new(),
            source_note_id: None,
        }
    }

    /// Open pre-filled from a captured note: the note body becomes the first note, and the
    /// stage defaults to `default_stage` (typically the board's first column). Creating the
    /// ticket will consume the source note.
    fn from_note(note_id: Uuid, body: String, default_stage: Option<Uuid>) -> Self {
        Self {
            stage_id: default_stage,
            title: String::new(),
            description: String::new(),
            note: body,
            source_note_id: Some(note_id),
        }
    }
}

impl BoardState {
    /// The "+ New ticket" affordance in a stage column. Opens the create modal for `stage_id`.
    pub(in crate::ui::tasks) fn render_new_ticket(&mut self, ui: &mut egui::Ui, stage_id: Uuid) {
        if button::ghost(ui, &format!("{} New ticket", theme::icon::ADD)).clicked() {
            self.new_ticket = Some(NewTicketModal::new(stage_id));
        }
    }

    /// Open the create modal pre-filled from an uncategorized note (Notes tab → "Create
    /// Ticket"). Defaults the stage to the board's first column; the picker lets the owner
    /// change it. `pub(crate)` so the app shell can drive it across features.
    pub(crate) fn open_new_ticket_from_note(
        &mut self,
        note_id: Uuid,
        body: String,
        view: &TasksView,
    ) {
        let default_stage = view.stages.first().map(|s| s.id);
        self.new_ticket = Some(NewTicketModal::from_note(note_id, body, default_stage));
    }

    /// Dev-only: open the create modal directly for review (see `ui::dev`).
    pub(crate) fn dev_open_new_ticket(&mut self, stage_id: Uuid) {
        self.new_ticket = Some(NewTicketModal::new(stage_id));
    }

    /// Render the create-ticket modal overlay, if open. Called alongside the other overlays.
    /// Needs the tasks `view` for the stage picker.
    pub(crate) fn render_create_modal(
        &mut self,
        ctx: &egui::Context,
        bridge: &Bridge,
        view: &TasksView,
    ) {
        let Some(draft) = self.new_ticket.as_mut() else {
            return;
        };
        let muted = theme::palette().muted;
        let mut submit = false;
        let mut cancel = false;

        let response = egui::Modal::new(egui::Id::new("new_ticket_modal"))
            .frame(theme::surface_frame())
            .show(ctx, |ui| {
                ui.set_min_width(460.0);
                ui.heading("New ticket");
                ui.add_space(10.0);

                ui.label(egui::RichText::new("Stage").strong().color(muted));
                ui.add_space(4.0);
                if view.stages.is_empty() {
                    ui.label(
                        egui::RichText::new("No stages yet — add one on the Tasks board first.")
                            .color(muted),
                    );
                } else {
                    stage_combo(ui, view, &mut draft.stage_id);
                }

                ui.add_space(10.0);
                ui.label(egui::RichText::new("Title").strong().color(muted));
                ui.add_space(4.0);
                input::text_field(ui, &mut draft.title, "Ticket title");

                ui.add_space(10.0);
                ui.label(egui::RichText::new("Description").strong().color(muted));
                ui.add_space(4.0);
                input::text_area(
                    ui,
                    &mut draft.description,
                    "What needs doing? (optional)",
                    3,
                );

                ui.add_space(10.0);
                ui.label(egui::RichText::new("First note").strong().color(muted));
                ui.add_space(4.0);
                input::text_area(ui, &mut draft.note, "Add an initial note (optional)", 2);

                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    let can_add = !draft.title.trim().is_empty() && draft.stage_id.is_some();
                    submit = button::primary_enabled(ui, "Create ticket", can_add).clicked();
                    cancel = button::secondary(ui, "Cancel").clicked();
                });
            });

        // Backdrop / escape also cancels.
        if response.should_close() {
            cancel = true;
        }

        if submit
            && !draft.title.trim().is_empty()
            && let Some(stage_id) = draft.stage_id
        {
            let note = Some(draft.note.trim().to_owned()).filter(|n| !n.is_empty());
            bridge.send(Event::create_ticket(
                stage_id,
                draft.title.trim().to_owned(),
                draft.description.trim().to_owned(),
                note,
            ));
            // If this ticket was born from a captured note, consume that note now that the
            // creation has been dispatched (submit is only reachable with a valid title +
            // stage, so the create will land).
            if let Some(note_id) = draft.source_note_id {
                bridge.send(notes::Event::delete(note_id));
            }
            self.new_ticket = None;
        } else if cancel {
            self.new_ticket = None;
        }
    }
}

/// Combo box to pick which stage a new ticket lands in.
fn stage_combo(ui: &mut egui::Ui, view: &TasksView, selected: &mut Option<Uuid>) {
    let current_name = selected
        .and_then(|id| view.stages.iter().find(|s| s.id == id))
        .map(|s| s.name.as_str())
        .unwrap_or("Select a stage");

    egui::ComboBox::from_id_salt("new_ticket_stage")
        .selected_text(current_name)
        .show_ui(ui, |ui| {
            for stage in &view.stages {
                if ui
                    .selectable_label(*selected == Some(stage.id), &stage.name)
                    .clicked()
                {
                    *selected = Some(stage.id);
                }
            }
        });
}
