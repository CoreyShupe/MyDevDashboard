//! `tasks::stage` part UI: the add-stage control, columns, headers, and the edit-stage modal.
//!
//! Stage editing is a full modal (name + "terminal stage" toggle + delete), not an inline
//! rename — editing a stage is a first-class activity. A **terminal** stage (e.g. "Complete",
//! "Cancelled") collapses its column to a ticket COUNT with a "View tickets" button to reveal
//! the cards (so they can be dragged back out); tickets can always be dragged IN.

use uuid::Uuid;

use crate::app::Bridge;
use crate::app::tasks::{Event, View as TasksView};
use crate::domain::tasks::Stage;
use crate::ui::components::{button, card, dnd, input};
use crate::ui::theme;

use super::BoardState;

/// Draft state for the open "edit stage" modal. Only one is open at a time (board-wide).
pub(super) struct StageModal {
    pub(super) id: Uuid,
    pub(super) name: String,
    pub(super) terminal: bool,
}

/// Drag-and-drop payload for reordering stages. A distinct type from the ticket payload
/// (`Uuid`) so a column can tell a stage-reorder drop apart from a ticket-move drop.
#[derive(Clone, Copy)]
struct StageDrag(Uuid);

impl BoardState {
    /// The "add stage" field + button. Self-contained (its own row), so it can live in the
    /// board header or centered in the empty state unchanged.
    pub(super) fn render_add_stage(&mut self, ui: &mut egui::Ui, bridge: &Bridge) {
        ui.horizontal(|ui| {
            let response =
                input::text_field_sized(ui, &mut self.new_stage_name, "New stage name", 180.0);
            let entered = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            let clicked = button::primary(ui, &format!("{} Add", theme::icon::ADD)).clicked();

            if (clicked || entered) && !self.new_stage_name.trim().is_empty() {
                bridge.send(Event::create_stage(self.new_stage_name.trim().to_owned()));
                self.new_stage_name.clear();
            }
        });
    }

    /// One stage column: solid card with a header, ticket cards (or, for a terminal stage, a
    /// count), and the new-ticket control. Two drag interactions live here:
    ///   • the header grip drags THIS column to reorder it (floating copy, like ticket cards),
    ///   • the whole column is a drop target for a ticket from another stage OR another column
    ///     being reordered onto this slot.
    pub(super) fn render_stage_column(
        &mut self,
        ui: &mut egui::Ui,
        bridge: &Bridge,
        stage: &Stage,
        view: &TasksView,
    ) {
        let drag_id = egui::Id::new(("stage_drag", stage.id));

        // While this column is being dragged, lift the WHOLE card onto a floating layer that
        // follows the pointer from the grab point (shared `dnd::drag_ghost`), then return early.
        if ui.ctx().is_being_dragged(drag_id) {
            egui::DragAndDrop::set_payload(ui.ctx(), StageDrag(stage.id));
            dnd::drag_ghost(ui, drag_id, |ui| {
                self.render_stage_card(ui, bridge, stage, view);
            });
            return;
        }

        // Normal layout: render the card (capturing the grip rect), then wire the drag source
        // (only the grip) and the drop target (the whole column).
        let egui::InnerResponse {
            inner: grip_rect,
            response,
        } = ui.scope(|ui| self.render_stage_card(ui, bridge, stage, view));

        ui.interact(grip_rect, drag_id, egui::Sense::drag())
            .on_hover_cursor(egui::CursorIcon::Grab);

        // Drop detection is GEOMETRIC (pointer-in-rect) since the cards inside occlude the
        // column's own response. Two payload kinds: a ticket (`Uuid`) or a stage (`StageDrag`).
        let from_other_stage = |id: &Uuid| view.ticket(*id).map(|t| t.stage_id) != Some(stage.id);
        let ctx = ui.ctx();
        let pointer_over = ctx
            .pointer_interact_pos()
            .is_some_and(|p| response.rect.contains(p));

        let ticket_incoming = pointer_over
            && egui::DragAndDrop::payload::<Uuid>(ctx).is_some_and(|d| from_other_stage(&d));
        let stage_incoming = pointer_over
            && egui::DragAndDrop::payload::<StageDrag>(ctx).is_some_and(|d| d.0 != stage.id);

        // Highlight this column as a valid drop target for either drag.
        if ticket_incoming || stage_incoming {
            ui.painter().rect_stroke(
                response.rect,
                egui::CornerRadius::same(theme::radius::CARD),
                egui::Stroke::new(2.0, theme::palette().accent),
                egui::StrokeKind::Inside,
            );
        }

        // On release over this column, take whichever payload is present and act on it.
        //
        // We must PEEK (non-consuming `payload::<T>`) to choose the type before taking: egui's
        // `take_payload::<T>` removes the payload from storage and *then* downcasts, so calling
        // it for the wrong type (e.g. `::<Uuid>` while a `StageDrag` is in flight) silently
        // discards the payload — which is exactly why stage reorder never fired.
        if pointer_over && ui.input(|i| i.pointer.any_released()) {
            if egui::DragAndDrop::payload::<Uuid>(ctx).is_some() {
                if let Some(dragged) = egui::DragAndDrop::take_payload::<Uuid>(ctx)
                    && from_other_stage(&dragged)
                {
                    bridge.send(Event::move_ticket(*dragged, stage.id));
                }
            } else if let Some(dragged) = egui::DragAndDrop::take_payload::<StageDrag>(ctx)
                && dragged.0 != stage.id
            {
                // Insert the dragged stage at the drop target's ORIGINAL index in the
                // post-removal list. This gives the direction-aware behaviour: a stage dragged
                // leftward onto the target lands to its LEFT; dragged rightward, to its RIGHT.
                let mut ids: Vec<Uuid> = view.stages.iter().map(|s| s.id).collect();
                let target = ids.iter().position(|&x| x == stage.id).unwrap_or(ids.len());
                ids.retain(|&x| x != dragged.0);
                ids.insert(target.min(ids.len()), dragged.0);
                bridge.send(Event::reorder_stages(ids));
            }
        }
    }

    /// Render the column card (header + body) and return the drag-handle (grip) rect so the
    /// caller can restrict the stage-reorder drag to just that handle.
    fn render_stage_card(
        &mut self,
        ui: &mut egui::Ui,
        bridge: &Bridge,
        stage: &Stage,
        view: &TasksView,
    ) -> egui::Rect {
        let mut grip_rect = egui::Rect::NOTHING;
        card::card(ui, |ui| {
            ui.set_width(248.0);
            grip_rect = self.render_stage_header(ui, stage);
            ui.add_space(8.0);

            let tickets: Vec<_> = view.tickets_for(stage.id).cloned().collect();
            let expanded = self.viewing_terminal.contains(&stage.id);

            if stage.terminal && !expanded {
                // Collapsed end state: just a count + a way to reveal the tickets.
                self.render_terminal_summary(ui, stage.id, tickets.len());
            } else {
                egui::ScrollArea::vertical()
                    .id_salt(("stage_scroll", stage.id))
                    .max_height(440.0)
                    .show(ui, |ui| {
                        for ticket in &tickets {
                            self.render_ticket_card(ui, bridge, ticket);
                            ui.add_space(8.0);
                        }
                    });

                ui.add_space(2.0);
                if stage.terminal {
                    // Revealed terminal stage: only there to move tickets back out — no
                    // new-ticket affordance, just a way to collapse back to the count.
                    if button::ghost(ui, &format!("{} Hide tickets", theme::icon::BACK)).clicked() {
                        self.viewing_terminal.remove(&stage.id);
                    }
                } else {
                    self.render_new_ticket(ui, stage.id);
                }
            }
        });
        grip_rect
    }

    /// The collapsed terminal-stage body: a big ticket count + "View tickets" to reveal them.
    fn render_terminal_summary(&mut self, ui: &mut egui::Ui, stage_id: Uuid, count: usize) {
        let p = theme::palette();
        ui.vertical_centered(|ui| {
            ui.add_space(10.0);
            ui.label(egui::RichText::new(count.to_string()).size(34.0).strong());
            ui.label(
                egui::RichText::new(if count == 1 { "ticket" } else { "tickets" })
                    .color(p.muted)
                    .size(12.0),
            );
            ui.add_space(12.0);
            if button::secondary(ui, &format!("{} View tickets", theme::icon::EXPAND)).clicked() {
                self.viewing_terminal.insert(stage_id);
            }
            ui.add_space(6.0);
        });
    }

    /// The stage header: a drag grip, the name, a terminal badge (if any), and the edit button
    /// that opens the full edit-stage modal. Returns the grip's rect so the column can make it
    /// the sole drag handle for reordering.
    fn render_stage_header(&mut self, ui: &mut egui::Ui, stage: &Stage) -> egui::Rect {
        let p = theme::palette();
        let mut grip_rect = egui::Rect::NOTHING;
        ui.horizontal(|ui| {
            // Edit on the top-left.
            if button::icon(ui, theme::icon::EDIT, "Edit stage").clicked() {
                self.editing_stage = Some(StageModal {
                    id: stage.id,
                    name: stage.name.clone(),
                    terminal: stage.terminal,
                });
            }

            ui.label(egui::RichText::new(&stage.name).strong().size(16.0));
            if stage.terminal {
                ui.label(
                    egui::RichText::new(theme::icon::FLAG.to_string())
                        .color(p.accent)
                        .size(13.0),
                )
                .on_hover_text("Terminal stage");
            }

            // Drag grip on the top-right: the ONLY stage-reorder handle (the column wires the
            // drag sense onto this rect); grabbing anywhere else never starts a stage drag.
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                grip_rect = ui
                    .add(
                        egui::Label::new(
                            egui::RichText::new(theme::icon::DRAG.to_string())
                                .color(p.muted)
                                .size(16.0),
                        )
                        .selectable(false),
                    )
                    .rect;
            });
        });
        grip_rect
    }

    /// Dev-only: open the edit-stage modal directly for review (see `ui::dev`).
    pub(crate) fn dev_open_stage_edit(&mut self, stage: &Stage) {
        self.editing_stage = Some(StageModal {
            id: stage.id,
            name: stage.name.clone(),
            terminal: stage.terminal,
        });
    }

    /// The full "edit stage" modal overlay: rename, toggle terminal, or delete. Rendered from
    /// the app shell alongside the other overlays.
    pub(crate) fn render_stage_modal(
        &mut self,
        ctx: &egui::Context,
        bridge: &Bridge,
        view: &TasksView,
    ) {
        let Some(draft) = self.editing_stage.as_mut() else {
            return;
        };
        let p = theme::palette();
        let saved = view.stage(draft.id);
        let mut close = false;
        let mut submit = false;
        let mut delete = false;

        let response = egui::Modal::new(egui::Id::new(("stage_modal", draft.id)))
            .frame(theme::surface_frame())
            .show(ctx, |ui| {
                ui.set_min_width(420.0);
                ui.heading("Edit stage");
                ui.add_space(10.0);

                ui.label(egui::RichText::new("Name").strong().color(p.muted));
                ui.add_space(4.0);
                let resp = input::text_field(ui, &mut draft.name, "Stage name");
                let entered = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));

                ui.add_space(14.0);
                ui.checkbox(&mut draft.terminal, "Terminal stage");
                ui.add_space(2.0);
                ui.label(
                    egui::RichText::new(
                        "An end state (e.g. Complete, Cancelled): the column collapses to a \
                         ticket count, and its tickets are hidden from \"Add to ticket\".",
                    )
                    .color(p.muted)
                    .size(12.0),
                );

                ui.add_space(16.0);
                ui.horizontal(|ui| {
                    let can_save = !draft.name.trim().is_empty();
                    let save_label = format!("{} Save", theme::icon::SAVE);
                    submit = button::primary_enabled(ui, &save_label, can_save).clicked()
                        || (entered && can_save);
                    if button::secondary(ui, "Cancel").clicked() {
                        close = true;
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if button::danger(ui, &format!("{} Delete", theme::icon::DELETE)).clicked()
                        {
                            delete = true;
                        }
                    });
                });
            });

        // Backdrop / escape closes without saving.
        if response.should_close() {
            close = true;
        }

        if delete {
            bridge.send(Event::delete_stage(draft.id));
            self.editing_stage = None;
        } else if submit && !draft.name.trim().is_empty() {
            // Send only what actually changed, so we don't fire redundant events.
            let name = draft.name.trim().to_owned();
            if saved.map(|s| s.name.as_str()) != Some(name.as_str()) {
                bridge.send(Event::rename_stage(draft.id, name));
            }
            if saved.map(|s| s.terminal) != Some(draft.terminal) {
                bridge.send(Event::set_terminal_stage(draft.id, draft.terminal));
            }
            self.editing_stage = None;
        } else if close {
            self.editing_stage = None;
        }
    }
}
