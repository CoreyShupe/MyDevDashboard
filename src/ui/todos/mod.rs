//! `todos` feature UI: the Todos tab — a fast, list-like task surface, twin to the Notes tab.
//!
//! Layout (mirrors Notes on purpose):
//!   - a **composer** at the top: a single-line "New todo" field + Add (Enter also adds),
//!   - a scrollable list of **open todo rows**. Each row has a **done checkbox** on the left,
//!     the wrapped body, and a **delete** action on the right.
//!
//! **Completed todos are hidden**: checking a todo off marks it done (which persists) and it
//! drops out of the list on the next snapshot — the list only ever shows what's left to do.
//!
//! Pure rendering (AGENTS.md §2): mutations go out as `todos::Event`s. No overlays — every
//! action is inline, so there's nothing for the shell to coordinate.

use crate::app::Bridge;
use crate::app::todos::{Event, View as TodosView};
use crate::domain::todos::Todo;
use crate::ui::components::{button, card, input};
use crate::ui::theme;

/// Transient UI state for the Todos tab. Lives in the UI only.
#[derive(Default)]
pub struct TodosState {
    /// Composer buffer for the "New todo" field.
    new_todo: String,
}

impl TodosState {
    /// The Todos workspace: composer + todo list.
    pub fn render_workspace(&mut self, ui: &mut egui::Ui, bridge: &Bridge, view: &TodosView) {
        let muted = theme::palette().muted;

        ui.heading("Todos");
        ui.add_space(10.0);

        self.render_composer(ui, bridge);
        ui.add_space(14.0);

        // Completed todos are hidden — the list is only what's still open.
        let open: Vec<&Todo> = view.todos.iter().filter(|t| !t.done).collect();
        if open.is_empty() {
            ui.add_space(24.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new("Nothing to do — jot a quick task above to get started.")
                        .color(muted),
                );
            });
            return;
        }

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for todo in open {
                    render_todo_row(ui, bridge, todo);
                    ui.add_space(10.0);
                }
            });
    }

    /// Top composer: single-line field + Add button. Enter also submits, and focus returns to
    /// the field afterwards so tasks can be rattled off in quick succession (like Notes).
    fn render_composer(&mut self, ui: &mut egui::Ui, bridge: &Bridge) {
        card::card(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                let add_w = 96.0;
                let field_w = (ui.available_width() - add_w - 8.0).max(120.0);
                let response = input::text_field_sized(ui, &mut self.new_todo, "New todo", field_w);
                let via_enter =
                    response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));

                let can_add = !self.new_todo.trim().is_empty();
                let clicked = button::primary_enabled(ui, "Add", can_add).clicked();

                if can_add && (clicked || via_enter) {
                    bridge.send(Event::add(self.new_todo.trim().to_owned()));
                    self.new_todo.clear();
                    response.request_focus(); // keep typing without reaching for the mouse
                }
            });
        });
    }
}

/// A single OPEN todo row: done checkbox (left), wrapped body + timestamp (center), delete
/// (right). Checking the box completes the todo, which hides it on the next snapshot.
fn render_todo_row(ui: &mut egui::Ui, bridge: &Bridge, todo: &Todo) {
    let muted = theme::palette().muted;

    card::card(ui, |ui| {
        ui.set_width(ui.available_width());
        const ROW_MIN_H: f32 = 40.0;
        const CHECK_W: f32 = 28.0;
        const DELETE_W: f32 = 40.0;
        const GAP: f32 = 12.0;

        ui.horizontal_top(|ui| {
            // Zero the row's item-spacing so the columns sum to EXACTLY the card width (like the
            // note rows); each column re-sets its own inner spacing where needed.
            ui.spacing_mut().item_spacing.x = 0.0;
            let text_w = (ui.available_width() - CHECK_W - DELETE_W - GAP * 2.0).max(120.0);

            // Left: the done checkbox. Checking it completes the todo (and hides it next frame).
            ui.allocate_ui_with_layout(
                egui::vec2(CHECK_W, ROW_MIN_H),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.set_min_width(CHECK_W);
                    let mut done = todo.done;
                    if ui.add(egui::Checkbox::new(&mut done, "")).changed() {
                        bridge.send(Event::set_done(todo.id, done));
                    }
                },
            );
            ui.add_space(GAP);

            // Center: the body (wrapped) with a subtle timestamp.
            ui.allocate_ui_with_layout(
                egui::vec2(text_w, ROW_MIN_H),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.set_min_width(text_w);
                    ui.add(egui::Label::new(egui::RichText::new(&todo.body).size(15.0)).wrap());
                    ui.add_space(6.0);
                    ui.label(
                        egui::RichText::new(todo.created_at.format("%Y-%m-%d %H:%M").to_string())
                            .color(muted)
                            .size(11.0),
                    );
                },
            );
            ui.add_space(GAP);

            // Right: delete, hugging the card's right edge.
            ui.allocate_ui_with_layout(
                egui::vec2(DELETE_W, ROW_MIN_H),
                egui::Layout::right_to_left(egui::Align::Min),
                |ui| {
                    if button::icon(ui, theme::icon::DELETE, "Delete todo").clicked() {
                        bridge.send(Event::delete(todo.id));
                    }
                },
            );
        });
    });
}
