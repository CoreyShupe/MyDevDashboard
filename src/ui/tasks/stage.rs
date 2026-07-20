//! `tasks::stage` part UI: the add-stage control, columns, and stage headers.

use uuid::Uuid;

use crate::app::Bridge;
use crate::app::tasks::{Event, View as TasksView};
use crate::domain::tasks::Stage;
use crate::ui::components::{button, card, input};
use crate::ui::theme;

use super::BoardState;

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

    /// One stage column: solid card holding a header, ticket cards, and the new-ticket
    /// control. The whole column is a drop target for tickets dragged from other stages.
    pub(super) fn render_stage_column(
        &mut self,
        ui: &mut egui::Ui,
        bridge: &Bridge,
        stage: &Stage,
        view: &TasksView,
    ) {
        let response = card::card(ui, |ui| {
            ui.set_width(248.0);
            self.render_stage_header(ui, bridge, stage);
            ui.add_space(8.0);

            let tickets: Vec<_> = view.tickets_for(stage.id).cloned().collect();
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
            self.render_new_ticket(ui, bridge, stage.id);
        })
        .response;

        // Drag-and-drop: a ticket dragged from another stage can be dropped anywhere on this
        // column. We detect the drop GEOMETRICALLY (pointer-in-rect) rather than via the
        // frame's hover, because the ticket cards inside occlude the column's own response.
        let from_other_stage = |id: &Uuid| view.ticket(*id).map(|t| t.stage_id) != Some(stage.id);
        let ctx = ui.ctx();
        let pointer_over = ctx
            .pointer_interact_pos()
            .is_some_and(|p| response.rect.contains(p));

        // Highlight this column as a valid drop target while a ticket from another stage
        // hovers over it.
        if pointer_over
            && let Some(dragged) = egui::DragAndDrop::payload::<Uuid>(ctx)
            && from_other_stage(&dragged)
        {
            ui.painter().rect_stroke(
                response.rect,
                egui::CornerRadius::same(theme::radius::CARD),
                egui::Stroke::new(2.0, theme::palette().accent),
                egui::StrokeKind::Inside,
            );
        }
        // On release over this column, take the payload and move the ticket here.
        if pointer_over
            && ui.input(|i| i.pointer.any_released())
            && let Some(dragged) = egui::DragAndDrop::take_payload::<Uuid>(ctx)
            && from_other_stage(&dragged)
        {
            bridge.send(Event::move_ticket(*dragged, stage.id));
        }
    }

    fn render_stage_header(&mut self, ui: &mut egui::Ui, bridge: &Bridge, stage: &Stage) {
        // Inline rename mode?
        if let Some((editing_id, buffer)) = &mut self.editing_stage
            && *editing_id == stage.id
        {
            let response = input::text_field_sized(ui, buffer, "Stage name", f32::INFINITY);
            let entered = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            let (mut save, mut cancel) = (entered, false);
            ui.horizontal(|ui| {
                save |= button::primary(ui, "Save").clicked();
                cancel = button::secondary(ui, "Cancel").clicked();
            });

            if save {
                let name = buffer.trim().to_owned();
                if !name.is_empty() {
                    bridge.send(Event::rename_stage(stage.id, name));
                }
                self.editing_stage = None;
            } else if cancel {
                self.editing_stage = None;
            }
            return;
        }

        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(&stage.name).strong().size(16.0));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if button::icon(ui, theme::icon::DELETE, "Delete stage").clicked() {
                    bridge.send(Event::delete_stage(stage.id));
                }
                if button::icon(ui, theme::icon::EDIT, "Rename stage").clicked() {
                    self.editing_stage = Some((stage.id, stage.name.clone()));
                }
            });
        });
    }
}
