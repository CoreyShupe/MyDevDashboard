//! UI layer: PURE egui rendering, sliced by feature. No DB, no sqlx, no business logic.
//!
//! `ui/mod.rs` is the app shell: it owns the eframe `App`, the left nav, the error modal,
//! and routes the workspace to the active feature's UI (`ui/<feature>/`). All visuals come
//! from `ui::theme` + `ui::components` — no hardcoded colors here (AGENTS.md §2, §7).

pub mod components;
pub mod theme;

mod dev;
mod home;
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
use components::confirm::{self, Choice};

/// Which left-nav tab is active. One variant per feature surfaced in the workspace.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Home,
    Tasks,
    Notes,
    Projects,
    Todos,
}

impl Tab {
    /// The persisted `ProfileView` this tab records as the profile's "last viewed page" (§9).
    fn as_view(self) -> crate::domain::profile::ProfileView {
        use crate::domain::profile::ProfileView;
        match self {
            Tab::Home => ProfileView::Home,
            Tab::Tasks => ProfileView::Tasks,
            Tab::Notes => ProfileView::Notes,
            Tab::Projects => ProfileView::Projects,
            Tab::Todos => ProfileView::Todos,
        }
    }

    /// The tab to land on for a profile's persisted `ProfileView`.
    fn from_view(view: crate::domain::profile::ProfileView) -> Self {
        use crate::domain::profile::ProfileView;
        match view {
            ProfileView::Home => Tab::Home,
            ProfileView::Tasks => Tab::Tasks,
            ProfileView::Notes => Tab::Notes,
            ProfileView::Projects => Tab::Projects,
            ProfileView::Todos => Tab::Todos,
        }
    }
}

/// The eframe application shell. Owns UI state only; system state lives behind the `Bridge`.
pub struct DashboardApp {
    bridge: Bridge,
    data: Rc<ViewData>,
    active_tab: Tab,

    // Per-feature UI state.
    onboarding: profile::OnboardingState,
    home: home::HomeState,
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

    // A profile pending a delete confirmation, if any. Deleting cascades the whole workspace
    // (§9), so it's confirmed at the shell (the switcher that triggers it lives in the nav).
    confirm_delete_profile: Option<Uuid>,

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
            home: home::HomeState,
            board: tasks::BoardState::default(),
            notes: notes::NotesState::default(),
            projects: projects::ProjectsState::default(),
            todos: todos::TodosState::default(),
            error: None,
            new_profile_flow: false,
            active_profile_id: None,
            confirm_delete_profile: None,
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
            dev::DevView::Home => {
                self.data = Rc::new(dev::mock_home());
                self.active_tab = Tab::Home;
            }
            dev::DevView::HomeEmpty => {
                self.data = Rc::new(dev::mock_empty());
                self.active_tab = Tab::Home;
            }
            dev::DevView::Onboarding => {} // default state (no profiles) shows first-run onboarding
            dev::DevView::NewProfile => {
                self.data = Rc::new(dev::mock_board()); // existing profiles to switch back to
                self.new_profile_flow = true;
            }
            dev::DevView::ProfileSelect => self.data = Rc::new(dev::mock_reselect()),
            dev::DevView::ConfirmDelete => {
                let data = dev::mock_board();
                if let Some(ticket) = data.tasks.tickets.first() {
                    self.board.dev_open_delete_ticket_confirm(ticket.id);
                }
                self.data = Rc::new(data);
            }
            dev::DevView::Board => self.data = Rc::new(dev::mock_board()),
            dev::DevView::BoardSearch => {
                self.board.dev_set_search("drag");
                self.data = Rc::new(dev::mock_board());
            }
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
            dev::DevView::ErrorOutput => self.error = Some(dev::mock_error_output()),
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
            dev::DevView::ProjectsPulling => {
                let mut data = dev::mock_board();
                // A one-click Pull in flight on the pullable card → its "Pulling…" spinner shows.
                if let Some(card) = data.projects.projects.first_mut() {
                    card.pulling = true;
                }
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
            dev::DevView::SetupScript => {
                let data = dev::mock_board();
                // The first project (my-dev-dashboard) carries a setup script in the mock.
                if let Some(card) = data.projects.projects.first() {
                    self.projects
                        .dev_open_setup_script(card.project.id, &card.project.setup_script);
                }
                self.data = Rc::new(data);
                self.active_tab = Tab::Projects;
            }
            dev::DevView::TeardownScript => {
                let data = dev::mock_board();
                // The first project (my-dev-dashboard) also carries a teardown script in the mock.
                if let Some(card) = data.projects.projects.first() {
                    self.projects
                        .dev_open_teardown_script(card.project.id, &card.project.teardown_script);
                }
                self.data = Rc::new(data);
                self.active_tab = Tab::Projects;
            }
            dev::DevView::WorktreeCreating => {
                let mut data = dev::mock_board();
                // Open the child ticket (as dev_open_ticket does) and mark a worktree as being
                // provisioned in a project it has no worktree in yet, so its setup spinner shows.
                let target = data
                    .tasks
                    .tickets
                    .iter()
                    .find(|t| t.parent_id.is_some())
                    .or_else(|| data.tasks.tickets.first())
                    .cloned();
                if let Some(ticket) = target {
                    let existing: Vec<uuid::Uuid> = data
                        .projects
                        .worktrees
                        .iter()
                        .filter(|w| w.ticket_id == ticket.id)
                        .map(|w| w.project_id)
                        .collect();
                    let pid = data
                        .projects
                        .projects
                        .iter()
                        .map(|c| c.project.id)
                        .find(|id| !existing.contains(id));
                    if let Some(pid) = pid {
                        data.projects.creating = vec![(pid, ticket.id)];
                    }
                    self.board.dev_open(&ticket, true);
                }
                self.data = Rc::new(data);
            }
            dev::DevView::WorktreeRemoving => {
                let mut data = dev::mock_board();
                // Open a project that has a live worktree and mark that worktree as being removed,
                // so its row shows the "Removing…" spinner in place of the Open/Remove buttons.
                let target = data
                    .projects
                    .worktrees
                    .iter()
                    .find(|w| w.is_live())
                    .map(|w| (w.project_id, w.id));
                if let Some((project_id, worktree_id)) = target {
                    data.projects.busy =
                        vec![(worktree_id, crate::domain::projects::WorktreeBusy::Removing)];
                    self.projects.dev_open_project(project_id);
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
                        self.home = home::HomeState;
                        self.board = tasks::BoardState::default();
                        self.notes = notes::NotesState::default();
                        self.projects = projects::ProjectsState::default();
                        self.todos = todos::TodosState::default();
                        self.active_profile_id = active;
                        // Land on the profile's last-viewed page (§9) — this fires on first load
                        // (None -> Some) and on every profile switch. No active profile → the
                        // default (Tasks) is irrelevant (onboarding/picker is shown instead).
                        self.active_tab = data
                            .profile
                            .active
                            .as_ref()
                            .map(|p| Tab::from_view(p.last_view))
                            .unwrap_or(Tab::Tasks);
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
                // start a new one, or delete the current one. Everything below is scoped to the
                // active profile (§9).
                let switch = profile::render_switcher(
                    ui,
                    &self.bridge,
                    &data.profile,
                    profile::SwitcherStyle::Nav,
                );
                if switch.new_profile {
                    self.new_profile_flow = true;
                }
                if let Some(id) = switch.delete {
                    // Confirm before wiping the whole workspace (§13); the modal renders at the
                    // shell level so it sits over everything.
                    self.confirm_delete_profile = Some(id);
                }
                ui.add_space(18.0);

                // Nav tabs. Add a tab per feature as they land. Home (the cross-feature Overview)
                // sits at the top as the natural landing point.
                self.nav_item(ui, Tab::Home, &format!("{} Home", theme::icon::HOME));
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
                // Refresh. Restart is a DEV-ONLY affordance: it exits with `RESTART_EXIT_CODE`,
                // which the `dev-dash open` dev loop catches to re-run. Release builds (the .app
                // bundle) can't relaunch, so the button is omitted there entirely — the short-
                // circuit means `button::ghost` is never even rendered in release.
                ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
                    ui.add_space(12.0);
                    if cfg!(debug_assertions)
                        && button::ghost(ui, &format!("{} Restart", theme::icon::RESTART)).clicked()
                    {
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
                Tab::Home => {
                    // The Overview aggregates every feature's slice; it reports navigation intents
                    // (switch tab / open a ticket) for the shell to act on — like the Notes
                    // create-ticket hand-off (AGENTS.md §2). Opening a ticket switches to Tasks and
                    // opens its detail; the modal then renders in the overlay pass below.
                    let outcome = self.home.render_workspace(ui, &self.bridge, data);
                    if let Some(tab) = outcome.goto {
                        self.active_tab = tab;
                        self.bridge
                            .send(crate::app::profile::Event::set_last_view(tab.as_view()));
                    }
                    if let Some(ticket_id) = outcome.open_ticket
                        && let Some(ticket) = data.tasks.ticket(ticket_id)
                    {
                        self.active_tab = Tab::Tasks;
                        self.board.open_ticket_modal(&self.bridge, ticket);
                    }
                }
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
            // Remember this as the active profile's last-viewed page so a relaunch / profile
            // switch lands back here (§9). Quiet write-through — no snapshot (see profile::handle).
            self.bridge
                .send(crate::app::profile::Event::set_last_view(tab.as_view()));
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

    /// The delete-profile confirmation (open only while `confirm_delete_profile` is set). Deleting
    /// cascades the entire workspace (§9), so this is the last gate before it fires. After the
    /// delete lands the next snapshot has no active profile, so the shell shows the picker /
    /// onboarding — the owner is never silently dropped into another profile.
    fn render_delete_profile_modal(
        &mut self,
        ctx: &egui::Context,
        view: &crate::app::profile::View,
    ) {
        let Some(id) = self.confirm_delete_profile else {
            return;
        };
        let Some(profile) = view.profiles.iter().find(|p| p.id == id) else {
            self.confirm_delete_profile = None; // already gone
            return;
        };
        let body = format!(
            "Delete the “{}” profile and EVERYTHING in it? This permanently removes its stages, \
             tickets, notes, projects, worktree records, and todos — it can't be undone. (Your \
             repositories on disk are left untouched.)",
            profile.display_name
        );
        match confirm::destructive(
            ctx,
            ("delete_profile", id),
            "Delete profile",
            &body,
            "Delete profile",
        ) {
            Choice::Confirmed => {
                self.bridge.send(crate::app::profile::Event::delete(id));
                self.confirm_delete_profile = None;
            }
            Choice::Cancelled => self.confirm_delete_profile = None,
            Choice::Pending => {}
        }
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
                // Widen when there's command output to show, so log lines don't wrap awkwardly.
                ui.set_max_width(if err.output.is_some() { 620.0 } else { 420.0 });
                ui.label(
                    egui::RichText::new(format!("{} {}", theme::icon::WARNING, err.title))
                        .heading()
                        .color(p.danger),
                );
                ui.add_space(6.0);
                ui.label(&err.detail);
                // Raw stderr from a failed external command (git / a setup script / the editor
                // launch), shown verbatim in a monospace, scrollable block so the owner sees
                // exactly what the process said — not a paraphrase (§3).
                if let Some(out) = err
                    .output
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                {
                    ui.add_space(10.0);
                    ui.label(
                        egui::RichText::new("Command output")
                            .strong()
                            .color(p.muted)
                            .size(12.5),
                    );
                    ui.add_space(4.0);
                    components::card::inset(ui, |ui| {
                        ui.set_width(ui.available_width());
                        // Shrink to the output's height (so short output isn't a tall empty box),
                        // but cap at 220px and scroll beyond that for a long log.
                        egui::ScrollArea::vertical()
                            .max_height(220.0)
                            .auto_shrink([false, true])
                            .show(ui, |ui| {
                                ui.label(egui::RichText::new(out).monospace().size(12.0));
                            });
                    });
                }
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

        // Three top-level screens: onboarding when no profile is active (first-run when none
        // exist, else a picker to reopen/create — e.g. after deleting the active one), the "new
        // profile" create screen (opened from the switcher), or the dashboard.
        if !data.has_profile() {
            let mode = if data.profile.profiles.is_empty() {
                profile::OnboardingMode::FirstRun
            } else {
                profile::OnboardingMode::Reselect
            };
            self.onboarding
                .render(&self.bridge, ui, mode, &data.profile);
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
            // A worktree row's "Open ticket" (projects) opens that ticket's detail on the board —
            // the reverse of the create-worktree hand-off (§2). Consumed before the board renders
            // its overlays so the detail shows the same frame; switch to Tasks so the board (and
            // the Expand-to-full-page path) is the backdrop.
            if let Some(ticket_id) = self.projects.take_pending_open_ticket()
                && let Some(ticket) = data.tasks.ticket(ticket_id)
            {
                self.active_tab = Tab::Tasks;
                self.board.open_ticket_modal(&self.bridge, ticket);
            }
            self.board
                .render_overlays(&ctx, &self.bridge, &data.tasks, &data.projects);
            self.board
                .render_create_modal(&ctx, &self.bridge, &data.tasks);
            self.board
                .render_stage_modal(&ctx, &self.bridge, &data.tasks);
            // Destructive confirmations for the board (delete ticket / delete stage), §13.
            self.board
                .render_confirmations(&ctx, &self.bridge, &data.tasks);
            self.notes.render_overlays(&ctx, &self.bridge, &data.tasks);
            // A "Create worktree" request raised by the open ticket detail is handed to the
            // projects UI, which owns the picker (cross-feature coordination, AGENTS.md §2).
            if let Some(ticket_id) = self.board.take_pending_worktree() {
                self.projects.open_create_worktree(ticket_id);
            }
            // Likewise a "Remove worktree" request — the projects UI owns the confirmation (§13).
            if let Some(worktree_id) = self.board.take_pending_remove_worktree() {
                self.projects.request_remove_worktree(worktree_id);
            }
            self.projects
                .render_overlays(&ctx, &self.bridge, &data.projects, &data.tasks);
        }
        // Deleting a profile is triggered from the nav switcher; its confirmation sits above
        // everything and stays valid across the has-profile / onboarding boundary (§9, §13).
        self.render_delete_profile_modal(&ctx, &data.profile);
        self.render_error_modal(&ctx);
    }
}
