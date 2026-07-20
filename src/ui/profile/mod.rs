//! `profile` feature UI: the profile switcher + the "setup / new profile" screen.
//!
//! Profiles are containers (AGENTS.md §9). Two surfaces live here:
//!   - [`render_switcher`] — a dropdown of profiles (+ "New profile") shown in the dashboard
//!     nav and, in a compact form, on the new-profile screen as an escape hatch.
//!   - [`OnboardingState::render`] — the borderless create screen, in two modes: first-run
//!     ("set up your first profile") and new-profile (adding another workspace).
//!
//! Deliberately borderless (AGENTS.md §7): inputs sit directly on the grid, no card.

use crate::app::Bridge;
use crate::app::profile::{Event, View};
use crate::ui::components::{button, input};
use crate::ui::theme;

/// Which onboarding context we're in — changes copy and whether the escape-hatch switcher shows.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum OnboardingMode {
    /// First run: no profiles exist yet.
    FirstRun,
    /// Adding another profile from the switcher (existing profiles remain to switch back to).
    NewProfile,
    /// No profile is active but others exist (e.g. the active one was just deleted): pick one to
    /// open, or create a fresh one. Like `NewProfile` but with no active profile to go "Back" to.
    Reselect,
}

/// Visual weight for the switcher — prominent in the nav, compact on the onboarding screen.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SwitcherStyle {
    Nav,
    Onboarding,
}

/// What the switcher needs its caller to do (it sends switch events itself).
#[derive(Default)]
pub struct SwitcherOutcome {
    /// User picked "New profile" (nav only) — caller should enter the new-profile flow.
    pub new_profile: bool,
    /// User switched to a *different* existing profile (a switch event was sent).
    pub switched: bool,
    /// User picked the profile that's *already* active — no switch needed, but on the
    /// new-profile screen it means "take me back to this profile" (exit the flow).
    pub selected_current: bool,
    /// User picked "Delete current profile" (nav only). Carries the profile id to delete; the
    /// caller opens a confirmation (destructive → cascades the whole workspace, §9 + §13).
    pub delete: Option<uuid::Uuid>,
}

/// A profiles dropdown: lists every profile (active one checked), and — in the nav — a
/// "New profile" entry. Switching is dispatched here; the rest is reported via the outcome.
pub fn render_switcher(
    ui: &mut egui::Ui,
    bridge: &Bridge,
    view: &View,
    style: SwitcherStyle,
) -> SwitcherOutcome {
    let p = theme::palette();
    let mut out = SwitcherOutcome::default();
    let active_name = view
        .active
        .as_ref()
        .map(|prof| prof.display_name.as_str())
        .unwrap_or("Dashboard");

    let (text_size, width) = match style {
        SwitcherStyle::Nav => (20.0, ui.available_width()),
        SwitcherStyle::Onboarding => (14.0, 200.0),
    };

    egui::ComboBox::from_id_salt("profile_switcher")
        .selected_text(
            egui::RichText::new(active_name)
                .size(text_size)
                .color(p.text),
        )
        .width(width)
        .show_ui(ui, |ui| {
            for profile in &view.profiles {
                let selected = view.active_id() == Some(profile.id);
                if ui
                    .selectable_label(selected, &profile.display_name)
                    .clicked()
                {
                    if selected {
                        // Already active — nothing to switch, but callers use this to dismiss.
                        out.selected_current = true;
                    } else {
                        bridge.send(Event::switch(profile.id));
                        out.switched = true;
                    }
                }
            }
            // "New profile" and "Delete current profile" are nav affordances; the onboarding
            // switcher is only an escape hatch back to an existing profile.
            if style == SwitcherStyle::Nav {
                ui.separator();
                if ui
                    .selectable_label(false, format!("{} New profile", theme::icon::ADD))
                    .clicked()
                {
                    out.new_profile = true;
                }
                if let Some(active_id) = view.active_id() {
                    let label = egui::RichText::new(format!(
                        "{} Delete current profile",
                        theme::icon::DELETE
                    ))
                    .color(p.danger);
                    if ui.selectable_label(false, label).clicked() {
                        out.delete = Some(active_id);
                    }
                }
            }
        });

    out
}

/// Transient input state for the create-profile screen. Lives in the UI only.
#[derive(Default)]
pub struct OnboardingState {
    name_input: String,
}

impl OnboardingState {
    /// Render the create-profile screen. Returns `true` when the caller should LEAVE the
    /// new-profile flow — i.e. the user created a profile or switched to an existing one.
    pub fn render(
        &mut self,
        bridge: &Bridge,
        ui: &mut egui::Ui,
        mode: OnboardingMode,
        view: &View,
    ) -> bool {
        let p = theme::palette();
        let mut leave = false;

        egui::CentralPanel::default()
            .frame(
                egui::Frame::group(ui.style())
                    .fill(egui::Color32::TRANSPARENT) // infinite grid shows through
                    .stroke(egui::Stroke::NONE)
                    .corner_radius(egui::CornerRadius::ZERO)
                    .inner_margin(egui::Margin::same(18)),
            )
            .show(ui, |ui| {
                // A compact switcher pinned top-left lets you pick an existing profile instead of
                // creating one. New-profile mode also gets a Back button (there's an active
                // profile to return to); Reselect has none (nothing is active after a delete).
                // First-run has neither: no profile exists yet.
                if mode == OnboardingMode::NewProfile || mode == OnboardingMode::Reselect {
                    ui.horizontal(|ui| {
                        if mode == OnboardingMode::NewProfile
                            && button::link(ui, &format!("{} Back", theme::icon::BACK)).clicked()
                        {
                            leave = true;
                        }
                        let out = render_switcher(ui, bridge, view, SwitcherStyle::Onboarding);
                        if out.switched || out.selected_current {
                            leave = true;
                        }
                    });
                }

                ui.vertical_centered(|ui| {
                    ui.add_space(120.0);
                    let (heading, sub) = match mode {
                        OnboardingMode::FirstRun => (
                            "Welcome to your Dev Dashboard",
                            "Let's set up your first profile to get started.",
                        ),
                        OnboardingMode::NewProfile => (
                            "New profile",
                            "Name a fresh, separate workspace — its own stages, tickets, and notes.",
                        ),
                        OnboardingMode::Reselect => (
                            "Choose a profile",
                            "Pick a workspace to open from the switcher above, or create a new one.",
                        ),
                    };
                    ui.label(egui::RichText::new(heading).heading());
                    ui.add_space(6.0);
                    ui.label(egui::RichText::new(sub).color(p.muted));
                    ui.add_space(28.0);

                    // No border/card — the field + button sit right on the grid.
                    ui.scope(|ui| {
                        ui.set_max_width(340.0);
                        ui.label(egui::RichText::new("Profile name").strong());
                        ui.add_space(6.0);

                        let response =
                            input::text_field(ui, &mut self.name_input, "e.g. Work or Personal");
                        let via_enter =
                            response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));

                        ui.add_space(12.0);
                        let can_submit = !self.name_input.trim().is_empty();
                        let clicked =
                            button::primary_enabled(ui, "Create profile", can_submit).clicked();

                        if can_submit && (clicked || via_enter) {
                            bridge.send(Event::create(self.name_input.trim().to_owned()));
                            self.name_input.clear();
                            leave = true; // created → back to the dashboard for the new profile
                        }
                    });
                });
            });

        leave
    }
}
