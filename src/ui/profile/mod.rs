//! `profile` feature UI: the "Setup Profile" onboarding screen + its input state.
//!
//! Deliberately borderless (AGENTS.md §7): the inputs sit directly on the grid, no card.

use crate::app::Bridge;
use crate::app::profile::Event;
use crate::ui::components::{button, input};
use crate::ui::theme;

/// Transient input state for onboarding. Lives in the UI only.
#[derive(Default)]
pub struct OnboardingState {
    name_input: String,
}

impl OnboardingState {
    /// Render the onboarding screen shown until a profile exists.
    pub fn render(&mut self, bridge: &Bridge, ui: &mut egui::Ui) {
        let p = theme::palette();
        egui::CentralPanel::default()
            .frame(
                egui::Frame::group(ui.style())
                    .fill(egui::Color32::TRANSPARENT) // infinite grid shows through
                    .stroke(egui::Stroke::NONE)
                    .corner_radius(egui::CornerRadius::ZERO)
                    .inner_margin(egui::Margin::same(18)),
            )
            .show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(140.0);
                    ui.label(egui::RichText::new("Welcome to your Dev Dashboard").heading());
                    ui.add_space(6.0);
                    ui.label(
                        egui::RichText::new("Let's set up your profile to get started.")
                            .color(p.muted),
                    );
                    ui.add_space(28.0);

                    // No border/card — the field + button sit right on the grid.
                    ui.scope(|ui| {
                        ui.set_max_width(340.0);
                        ui.label(egui::RichText::new("Your name").strong());
                        ui.add_space(6.0);

                        let response = input::text_field(ui, &mut self.name_input, "e.g. Corey");
                        let submit_via_enter =
                            response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));

                        ui.add_space(12.0);
                        let can_submit = !self.name_input.trim().is_empty();
                        let clicked =
                            button::primary_enabled(ui, "Create profile", can_submit).clicked();

                        if can_submit && (clicked || submit_via_enter) {
                            bridge.send(Event::create(self.name_input.trim().to_owned()));
                            self.name_input.clear();
                        }
                    });
                });
            });
    }
}
