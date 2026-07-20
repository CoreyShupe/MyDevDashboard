//! `notes` feature UI: the Notes tab — a fast, list-like capture surface.
//!
//! Layout (deliberately roomy, not compact — this is a scratchpad you scan and act on):
//!   - a **composer** at the top: a single-line "New note" field + Add (Enter also adds),
//!   - a scrollable list of **note rows**, newest first. Each row shows the wrapped note
//!     body (given generous height so it stays readable) with two actions stacked on the
//!     right: **Create Ticket** (opens the ticket create modal pre-filled) and **Add To
//!     Ticket** (opens a search picker to file it onto an existing ticket).
//!
//! Pure rendering (AGENTS.md §2): mutations go out as `notes::Event`s. "Create Ticket"
//! can't be done here alone (it drives the board's create modal), so it's surfaced to the
//! app shell via [`NotesOutcome`].

use uuid::Uuid;

use crate::app::Bridge;
use crate::app::notes::{Event, View as NotesView};
use crate::app::tasks::View as TasksView;
use crate::domain::notes::Note;
use crate::ui::components::{button, card, input};
use crate::ui::theme;

/// What the Notes tab needs the shell to do after rendering (things it can't do itself).
#[derive(Default)]
pub struct NotesOutcome {
    /// The owner asked to turn this note (id, body) into a new ticket — the shell opens the
    /// board's create modal pre-filled.
    pub create_ticket_from: Option<(Uuid, String)>,
}

/// Transient UI state for the Notes tab. Lives in the UI only.
#[derive(Default)]
pub struct NotesState {
    /// Composer buffer for the "New note" field.
    new_note: String,
    /// The open "Add to ticket" picker, if any.
    add_to_ticket: Option<AddToTicketModal>,
}

/// Draft state for the "Add to ticket" search picker.
struct AddToTicketModal {
    note_id: Uuid,
    note_body: String,
    /// Ticket-title search query.
    search: String,
}

impl AddToTicketModal {
    fn new(note: &Note) -> Self {
        Self {
            note_id: note.id,
            note_body: note.body.clone(),
            search: String::new(),
        }
    }
}

impl NotesState {
    /// Dev-only: open the "Add to ticket" picker directly for review (see `ui::dev`).
    pub(crate) fn dev_open_add_to_ticket(&mut self, note: &Note) {
        self.add_to_ticket = Some(AddToTicketModal::new(note));
    }

    /// The Notes workspace: composer + note list. Returns cross-feature intents for the shell.
    pub fn render_workspace(
        &mut self,
        ui: &mut egui::Ui,
        bridge: &Bridge,
        view: &NotesView,
    ) -> NotesOutcome {
        let mut outcome = NotesOutcome::default();
        let muted = theme::palette().muted;

        ui.heading("Notes");
        ui.add_space(10.0);

        self.render_composer(ui, bridge);
        ui.add_space(14.0);

        if view.notes.is_empty() {
            ui.add_space(24.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new("No notes yet — jot one above to get started.")
                        .color(muted),
                );
            });
            return outcome;
        }

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for note in &view.notes {
                    self.render_note_row(ui, note, &mut outcome);
                    ui.add_space(10.0);
                }
            });

        outcome
    }

    /// Top composer: single-line field + Add button. Enter also submits, and focus returns
    /// to the field afterwards so notes can be rattled off in quick succession.
    fn render_composer(&mut self, ui: &mut egui::Ui, bridge: &Bridge) {
        card::card(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                let add_w = 96.0;
                let field_w = (ui.available_width() - add_w - 8.0).max(120.0);
                let response = input::text_field_sized(ui, &mut self.new_note, "New note", field_w);
                let via_enter =
                    response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));

                let can_add = !self.new_note.trim().is_empty();
                let clicked = button::primary_enabled(ui, "Add", can_add).clicked();

                if can_add && (clicked || via_enter) {
                    bridge.send(Event::add(self.new_note.trim().to_owned()));
                    self.new_note.clear();
                    response.request_focus(); // keep typing without reaching for the mouse
                }
            });
        });
    }

    /// A single note row: wrapped body (roomy) on the left, stacked actions on the right.
    fn render_note_row(&mut self, ui: &mut egui::Ui, note: &Note, outcome: &mut NotesOutcome) {
        let muted = theme::palette().muted;

        card::card(ui, |ui| {
            ui.set_width(ui.available_width());
            // Rows are intentionally taller than the composer so there's room to write and
            // for both action buttons to sit comfortably on the right.
            const ROW_MIN_H: f32 = 76.0;
            const ACTIONS_W: f32 = 150.0;
            const GAP: f32 = 14.0;

            ui.horizontal_top(|ui| {
                let text_w = (ui.available_width() - ACTIONS_W - GAP).max(140.0);

                // Left: the note body (wrapped) + a subtle timestamp.
                ui.allocate_ui_with_layout(
                    egui::vec2(text_w, ROW_MIN_H),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        // Fill the whole text column even when the note is short, so the
                        // action buttons stay pinned to the card's right edge on every row.
                        ui.set_min_width(text_w);
                        ui.add(egui::Label::new(egui::RichText::new(&note.body).size(15.0)).wrap());
                        ui.add_space(6.0);
                        ui.label(
                            egui::RichText::new(
                                note.created_at.format("%Y-%m-%d %H:%M").to_string(),
                            )
                            .color(muted)
                            .size(11.0),
                        );
                    },
                );

                ui.add_space(GAP);

                // Right: stacked, full-width action buttons.
                ui.allocate_ui_with_layout(
                    egui::vec2(ACTIONS_W, ROW_MIN_H),
                    egui::Layout::top_down_justified(egui::Align::Min),
                    |ui| {
                        if button::secondary(ui, "Create Ticket").clicked() {
                            outcome.create_ticket_from = Some((note.id, note.body.clone()));
                        }
                        if button::secondary(ui, "Add To Ticket").clicked() {
                            self.add_to_ticket = Some(AddToTicketModal::new(note));
                        }
                    },
                );
            });
        });
    }

    /// Notes-tab overlays (the "Add to ticket" picker). Rendered from the app shell each
    /// frame, like the board's overlays. Needs the tasks `view` to search tickets by title.
    pub fn render_overlays(&mut self, ctx: &egui::Context, bridge: &Bridge, tasks: &TasksView) {
        let Some(modal) = self.add_to_ticket.as_mut() else {
            return;
        };
        let muted = theme::palette().muted;
        let mut close = false;
        let mut chosen: Option<Uuid> = None;

        let response = egui::Modal::new(egui::Id::new("add_note_to_ticket_modal"))
            .frame(theme::surface_frame())
            .show(ctx, |ui| {
                ui.set_min_width(480.0);
                ui.horizontal(|ui| {
                    ui.heading("Add to ticket");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if button::icon(ui, theme::icon::CLOSE, "Close").clicked() {
                            close = true;
                        }
                    });
                });
                ui.add_space(8.0);

                // Show the note being filed so the owner knows what they're attaching.
                ui.label(egui::RichText::new("Note").strong().color(muted));
                ui.add_space(4.0);
                card::inset(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.add(egui::Label::new(&modal.note_body).wrap());
                });

                ui.add_space(12.0);
                ui.label(egui::RichText::new("Find a ticket").strong().color(muted));
                ui.add_space(4.0);
                input::text_field(ui, &mut modal.search, "Search tickets by title…");
                ui.add_space(8.0);

                let query = modal.search.trim().to_lowercase();
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .max_height(260.0)
                    .show(ui, |ui| {
                        let mut any = false;
                        for ticket in &tasks.tickets {
                            if !query.is_empty() && !ticket.title.to_lowercase().contains(&query) {
                                continue;
                            }
                            any = true;
                            if ticket_pick_row(ui, tasks, ticket).clicked() {
                                chosen = Some(ticket.id);
                            }
                        }
                        if !any {
                            ui.label(
                                egui::RichText::new(if tasks.tickets.is_empty() {
                                    "No tickets yet — create one on the Tasks board first."
                                } else {
                                    "No tickets match that search."
                                })
                                .color(muted),
                            );
                        }
                    });
            });

        if response.should_close() {
            close = true;
        }

        if let Some(ticket_id) = chosen {
            bridge.send(Event::file_into_ticket(
                modal.note_id,
                ticket_id,
                modal.note_body.clone(),
            ));
            self.add_to_ticket = None;
        } else if close {
            self.add_to_ticket = None;
        }
    }
}

/// One selectable ticket in the "Add to ticket" search results: full-width, title with its
/// stage as a muted suffix for context.
fn ticket_pick_row(
    ui: &mut egui::Ui,
    tasks: &TasksView,
    ticket: &crate::domain::tasks::Ticket,
) -> egui::Response {
    let muted = theme::palette().muted;
    let stage = tasks
        .stages
        .iter()
        .find(|s| s.id == ticket.stage_id)
        .map(|s| s.name.as_str())
        .unwrap_or("—");

    let mut text = egui::text::LayoutJob::default();
    text.append(
        &ticket.title,
        0.0,
        egui::TextFormat {
            font_id: egui::TextStyle::Body.resolve(ui.style()),
            color: theme::palette().text,
            ..Default::default()
        },
    );
    text.append(
        &format!("   ·   {stage}"),
        0.0,
        egui::TextFormat {
            font_id: egui::TextStyle::Small.resolve(ui.style()),
            color: muted,
            ..Default::default()
        },
    );

    let resp = ui.add_sized(
        [ui.available_width(), 34.0],
        egui::Button::new(text)
            .fill(theme::palette().surface_alt)
            .corner_radius(egui::CornerRadius::same(theme::radius::INPUT)),
    );
    ui.add_space(4.0);
    resp
}
