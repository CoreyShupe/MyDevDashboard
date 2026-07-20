//! UI layer: PURE egui rendering, sliced by feature. No DB, no sqlx, no business logic.
//!
//! `ui/mod.rs` is the app shell: it owns the eframe `App`, the left nav, the error modal,
//! and routes the workspace to the active feature's UI (`ui/<feature>/`). All visuals come
//! from `ui::theme` + `ui::components` — no hardcoded colors here (AGENTS.md §2, §7).

pub mod components;
pub mod theme;

mod dev;
mod notes;
mod profile;
mod projects;
mod tasks;
mod todos;

use std::rc::Rc;

use uuid::Uuid;

use crate::app::{AppMessage, Bridge, UiEvent, ViewData};
use crate::error::UserFacingError;

use components::button;

/// Which left-nav tab is active. One variant per feature surfaced in the workspace.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Tasks,
    Notes,
    Projects,
    Todos,
}

/// The eframe application shell. Owns UI state only; system state lives behind the `Bridge`.
pub struct DashboardApp {
    bridge: Bridge,
    data: Rc<ViewData>,
    active_tab: Tab,

    // Per-feature UI state.
    onboarding: profile::OnboardingState,
    board: tasks::BoardState,
    notes: notes::NotesState,
    projects: projects::ProjectsState,
    todos: todos::TodosState,

    // Shell-level overlay.
    error: Option<UserFacingError>,

    // True while the "New profile" flow is showing (from the switcher). Renders the onboarding
    // create screen over the app until a profile is created or the user switches away.
    new_profile_flow: bool,
    // The active profile last rendered — used to reset transient board/notes state when the
    // owner switches profiles (so one profile's open modals don't bleed into another).
    active_profile_id: Option<Uuid>,

    // False until the first worker snapshot lands. Until then we show a loading screen rather
    // than flashing onboarding — an empty default `ViewData` looks identical to "no profile".
    loaded: bool,

    // When a `DEV_VIEW` override is active, mock state is injected and worker snapshots are
    // ignored so the forced screen stays put (AGENTS.md §8).
    dev_mode: bool,
}

impl DashboardApp {
    pub fn new(bridge: Bridge) -> Self {
        let mut app = Self {
            bridge,
            data: Rc::new(ViewData::default()),
            active_tab: Tab::Tasks,
            onboarding: profile::OnboardingState::default(),
            board: tasks::BoardState::default(),
            notes: notes::NotesState::default(),
            projects: projects::ProjectsState::default(),
            todos: todos::TodosState::default(),
            error: None,
            new_profile_flow: false,
            active_profile_id: None,
            loaded: false,
            dev_mode: false,
        };
        app.apply_dev_overrides();
        app
    }

    /// Inject mock state for a `DEV_VIEW` override, if one is set. No-op on a normal run.
    fn apply_dev_overrides(&mut self) {
        let Some(view) = dev::DevView::from_env() else {
            return;
        };
        self.dev_mode = true;
        self.loaded = true; // mock state stands in for a snapshot; never show the loading screen
        match view {
            dev::DevView::Onboarding => {} // default state (no profiles) shows first-run onboarding
            dev::DevView::NewProfile => {
                self.data = Rc::new(dev::mock_board()); // existing profiles to switch back to
                self.new_profile_flow = true;
            }
            dev::DevView::Board => self.data = Rc::new(dev::mock_board()),
            dev::DevView::Ticket => self.dev_open_ticket(false),
            dev::DevView::Page => self.dev_open_ticket(true),
            dev::DevView::Create => {
                let data = dev::mock_board();
                if let Some(stage) = data.tasks.stages.first() {
                    self.board.dev_open_new_ticket(stage.id);
                }
                self.data = Rc::new(data);
            }
            dev::DevView::StageEdit => {
                let data = dev::mock_board();
                // Open the edit modal on the terminal "Complete" stage so the toggle shows on.
                if let Some(stage) = data.tasks.stages.iter().find(|s| s.terminal) {
                    self.board.dev_open_stage_edit(stage);
                }
                self.data = Rc::new(data);
            }
            dev::DevView::Error => self.error = Some(dev::mock_error()),
            dev::DevView::Notes => {
                self.data = Rc::new(dev::mock_notes_view());
                self.active_tab = Tab::Notes;
            }
            dev::DevView::NotesFile => {
                let data = dev::mock_notes_view();
                if let Some(note) = data.notes.notes.first() {
                    self.notes.dev_open_add_to_ticket(note);
                }
                self.data = Rc::new(data);
                self.active_tab = Tab::Notes;
            }
            dev::DevView::Projects => {
                self.data = Rc::new(dev::mock_board());
                self.active_tab = Tab::Projects;
            }
            dev::DevView::ProjectsLoading => {
                let mut data = dev::mock_board();
                data.projects.refreshing = true; // git fetch in flight → cards show spinners
                self.data = Rc::new(data);
                self.active_tab = Tab::Projects;
            }
            dev::DevView::Loading => self.loaded = false, // show the pre-snapshot loading screen
            dev::DevView::AddProject => {
                self.data = Rc::new(dev::mock_board());
                self.projects.dev_open_add_project();
                self.active_tab = Tab::Projects;
            }
            dev::DevView::Project => {
                let data = dev::mock_board();
                if let Some(card) = data.projects.projects.first() {
                    self.projects.dev_open_project(card.project.id);
                }
                self.data = Rc::new(data);
                self.active_tab = Tab::Projects;
            }
            dev::DevView::Todos => {
                self.data = Rc::new(dev::mock_board());
                self.active_tab = Tab::Todos;
            }
            dev::DevView::BoardEmpty => {
                self.data = Rc::new(dev::mock_empty());
                self.active_tab = Tab::Tasks;
            }
            dev::DevView::NotesEmpty => {
                self.data = Rc::new(dev::mock_empty());
                self.active_tab = Tab::Notes;
            }
            dev::DevView::TodosEmpty => {
                self.data = Rc::new(dev::mock_empty());
                self.active_tab = Tab::Todos;
            }
            dev::DevView::ProjectsEmpty => {
                self.data = Rc::new(dev::mock_empty());
                self.active_tab = Tab::Projects;
            }
        }
        tracing::warn!(
            ?view,
            "DEV_VIEW override active — worker snapshots are ignored"
        );
    }

    /// Dev-only: load the mock board and open a child ticket (so the parent quick-link is
    /// exercised), as either the modal or the full-page (`expanded`) presentation.
    fn dev_open_ticket(&mut self, expanded: bool) {
        let data = dev::mock_board();
        let target = data
            .tasks
            .tickets
            .iter()
            .find(|t| t.parent_id.is_some())
            .or_else(|| data.tasks.tickets.first());
        if let Some(ticket) = target {
            self.board.dev_open(ticket, expanded);
        }
        self.data = Rc::new(data);
    }

    /// Apply any messages the worker produced since the last frame.
    fn drain_messages(&mut self) {
        if self.dev_mode {
            return; // keep the forced mock screen; don't let real snapshots overwrite it
        }
        for msg in self.bridge.drain() {
            match msg {
                AppMessage::Snapshot(data) => {
                    self.loaded = true; // real state has arrived; leave the loading screen
                    let data = Rc::new(data);
                    // Switching profiles swaps the whole workspace — drop transient board/notes
                    // state so one profile's open modals/buffers don't carry into another.
                    let active = data.profile.active_id();
                    if active != self.active_profile_id {
                        self.board = tasks::BoardState::default();
                        self.notes = notes::NotesState::default();
                        self.projects = projects::ProjectsState::default();
                        self.todos = todos::TodosState::default();
                        self.active_profile_id = active;
                    }
                    // Let the board / projects close a modal or detail whose entity vanished.
                    self.board.reconcile(&data.tasks);
                    self.projects.reconcile(&data.projects);
                    self.data = data;
                }
                // Feature-specific messages route to the owning feature's UI state.
                AppMessage::Tasks(message) => self.board.apply_message(&message),
                AppMessage::Error(err) => self.error = Some(err),
            }
        }
    }

    /// Left side-nav (solid surface) + workspace (grid background).
    fn render_shell(&mut self, ui: &mut egui::Ui, data: &ViewData) {
        let p = theme::palette();

        egui::Panel::left(egui::Id::new("nav"))
            .exact_size(210.0)
            .resizable(false)
            .frame(
                egui::Frame::group(ui.style())
                    .fill(p.surface)
                    .stroke(egui::Stroke::NONE)
                    .corner_radius(egui::CornerRadius::ZERO)
                    .inner_margin(egui::Margin::same(14)),
            )
            .show(ui, |ui| {
                ui.add_space(6.0);
                // Profile switcher (replaces the old static name heading): switch profiles or
                // start a new one. Everything below is scoped to the active profile (§9).
                if profile::render_switcher(
                    ui,
                    &self.bridge,
                    &data.profile,
                    profile::SwitcherStyle::Nav,
                )
                .new_profile
                {
                    self.new_profile_flow = true;
                }
                ui.add_space(18.0);

                // Nav tabs. Add a tab per feature as they land.
                self.nav_item(ui, Tab::Tasks, &format!("{} Tasks", theme::icon::DASHBOARD));
                self.nav_item(ui, Tab::Notes, &format!("{} Notes", theme::icon::NOTES));
                self.nav_item(ui, Tab::Todos, &format!("{} Todos", theme::icon::TODOS));
                self.nav_item(
                    ui,
                    Tab::Projects,
                    &format!("{} Projects", theme::icon::PROJECTS),
                );

                // Footer: manual refresh (re-pulls state from the DB) + restart. In a
                // bottom-up layout the first item added sits lowest, so Restart lands UNDER
                // Refresh. Restart exits with `RESTART_EXIT_CODE`, which `dev-dash open`
                // catches to rebuild + relaunch (a plain run just exits).
                ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
                    ui.add_space(12.0);
                    if button::ghost(ui, &format!("{} Restart", theme::icon::RESTART)).clicked() {
                        std::process::exit(crate::RESTART_EXIT_CODE);
                    }
                    if button::ghost(ui, &format!("{} Refresh", theme::icon::REFRESH)).clicked() {
                        self.bridge.send(UiEvent::ReloadAll);
                    }
                });
            });

        egui::CentralPanel::default()
            .frame(
                egui::Frame::group(ui.style())
                    .fill(egui::Color32::TRANSPARENT) // let the infinite grid show through
                    .stroke(egui::Stroke::NONE)
                    .corner_radius(egui::CornerRadius::ZERO)
                    .inner_margin(egui::Margin::same(18)),
            )
            .show(ui, |ui| match self.active_tab {
                Tab::Tasks => {
                    self.board
                        .render_workspace(ui, &self.bridge, &data.tasks, &data.projects)
                }
                Tab::Notes => {
                    let outcome = self.notes.render_workspace(ui, &self.bridge, &data.notes);
                    // "Create Ticket" from a note drives the board's create modal, so the
                    // shell coordinates it across features (AGENTS.md §2). The modal renders
                    // as an overlay, so it appears over the Notes tab without switching tabs.
                    if let Some((note_id, body)) = outcome.create_ticket_from {
                        self.board
                            .open_new_ticket_from_note(note_id, body, &data.tasks);
                    }
                }
                Tab::Projects => {
                    self.projects
                        .render_workspace(ui, &self.bridge, &data.projects, &data.tasks)
                }
                Tab::Todos => self.todos.render_workspace(ui, &self.bridge, &data.todos),
            });
    }

    /// A full-width, rounded nav row; teal-tinted when selected.
    fn nav_item(&mut self, ui: &mut egui::Ui, tab: Tab, label: &str) {
        let p = theme::palette();
        let selected = self.active_tab == tab;
        let text_color = if selected { p.accent } else { p.text };
        let fill = if selected {
            p.accent_soft
        } else {
            egui::Color32::TRANSPARENT
        };
        let button = egui::Button::new(egui::RichText::new(label).size(15.0).color(text_color))
            .fill(fill)
            .corner_radius(egui::CornerRadius::same(theme::radius::INPUT));
        if ui.add_sized([ui.available_width(), 36.0], button).clicked() {
            self.active_tab = tab;
        }
    }

    /// The full-window loading screen shown before the first snapshot lands. A centered spinner
    /// (which self-animates by requesting repaints) over the grid backdrop, so a slow DB connect
    /// or the initial git warm reads as "loading", never as an empty first-run app.
    fn render_loading(&self, ui: &mut egui::Ui) {
        let p = theme::palette();
        ui.vertical_centered(|ui| {
            ui.add_space(ui.available_height() * 0.42);
            ui.add(egui::Spinner::new().size(34.0).color(p.accent));
            ui.add_space(14.0);
            ui.label(
                egui::RichText::new("Loading your workspace…")
                    .color(p.muted)
                    .size(14.0),
            );
        });
    }

    /// A blocking error alert: dims the app and traps input (AGENTS.md §3).
    ///
    /// For retryable (database) errors it offers a **Retry** button that re-attempts the
    /// connection + reload rather than dismissing to an empty app. A failed operation never
    /// clears the last good snapshot, so retrying can't lose data.
    fn render_error_modal(&mut self, ctx: &egui::Context) {
        let Some(err) = self.error.clone() else {
            return;
        };
        let p = theme::palette();
        let mut dismiss = false;
        let mut retry = false;
        let response = egui::Modal::new(egui::Id::new("error_modal"))
            .frame(theme::surface_frame())
            .show(ctx, |ui| {
                ui.set_max_width(420.0);
                ui.label(
                    egui::RichText::new(format!("{} {}", theme::icon::WARNING, err.title))
                        .heading()
                        .color(p.danger),
                );
                ui.add_space(6.0);
                ui.label(&err.detail);
                ui.add_space(10.0);
                ui.label(egui::RichText::new("How to fix it").strong().color(p.muted));
                ui.label(&err.remediation);
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    if err.retryable {
                        if button::primary(ui, &format!("{} Retry", theme::icon::REFRESH)).clicked()
                        {
                            retry = true;
                        }
                        if button::secondary(ui, "Dismiss").clicked() {
                            dismiss = true;
                        }
                    } else if button::primary(ui, "Got it").clicked() {
                        dismiss = true;
                    }
                });
            });

        // A retryable error must NOT be auto-dismissed by clicking the backdrop/Escape —
        // that would silently drop back to an empty app. Only explicit buttons close it.
        let closed_by_backdrop = response.should_close() && !err.retryable;
        if retry {
            self.error = None;
            self.bridge.send(UiEvent::ReloadAll); // reconnect + reload
        } else if dismiss || closed_by_backdrop {
            self.error = None;
        }
    }
}

impl eframe::App for DashboardApp {
    /// The window base color — the grid backdrop paints over this. Pin it to `bg` so the
    /// transparent workspace doesn't inherit the lighter window/surface fill.
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        theme::palette().bg.to_normalized_gamma_f32()
    }

    /// Per-frame state sync only — no painting (egui 0.35 convention, AGENTS.md §4).
    fn logic(&mut self, _ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain_messages();
    }

    /// Pure rendering. The root `ui` has no frame; panels below supply their own.
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // Independent handle so we can read the snapshot while mutating UI buffers.
        let data = Rc::clone(&self.data);
        // Modals attach to the context (as overlays), independent of the root `ui`.
        let ctx = ui.ctx().clone();

        // Infinite grid backdrop, painted under every panel (panels below are transparent).
        theme::paint_background(&ctx);

        // Until the first snapshot arrives, show a loading screen — NOT onboarding. An empty
        // default `ViewData` is indistinguishable from "no profile", so without this gate a slow
        // DB connect would flash the first-run flow before real data lands. Errors still surface
        // (the modal renders below), so a failed connect isn't hidden behind the spinner.
        if !self.loaded {
            self.render_loading(ui);
            self.render_error_modal(&ctx);
            return;
        }

        // Three top-level screens: first-run onboarding (no profiles), the "new profile" create
        // screen (opened from the switcher), or the dashboard for the active profile.
        if !data.has_profile() {
            self.onboarding.render(
                &self.bridge,
                ui,
                profile::OnboardingMode::FirstRun,
                &data.profile,
            );
        } else if self.new_profile_flow {
            let leave = self.onboarding.render(
                &self.bridge,
                ui,
                profile::OnboardingMode::NewProfile,
                &data.profile,
            );
            if leave {
                self.new_profile_flow = false;
            }
        } else {
            self.render_shell(ui, &data);
        }

        // Overlays render last so they sit on top of whatever is behind them. Suppressed while a
        // full-screen onboarding flow is up (no board/notes behind them to act on).
        if data.has_profile() && !self.new_profile_flow {
            self.board
                .render_overlays(&ctx, &self.bridge, &data.tasks, &data.projects);
            self.board
                .render_create_modal(&ctx, &self.bridge, &data.tasks);
            self.board
                .render_stage_modal(&ctx, &self.bridge, &data.tasks);
            self.notes.render_overlays(&ctx, &self.bridge, &data.tasks);
            // A "Create worktree" request raised by the open ticket detail is handed to the
            // projects UI, which owns the picker (cross-feature coordination, AGENTS.md §2).
            if let Some(ticket_id) = self.board.take_pending_worktree() {
                self.projects.open_create_worktree(ticket_id);
            }
            self.projects
                .render_overlays(&ctx, &self.bridge, &data.projects, &data.tasks);
        }
        self.render_error_modal(&ctx);
    }
}
