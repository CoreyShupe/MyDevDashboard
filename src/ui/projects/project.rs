//! `projects::project` part UI: the card grid, project cards, the full-page detail, and the
//! add-project + confirm-delete modals.

use uuid::Uuid;

use crate::app::Bridge;
use crate::app::projects::{ProjectCard, View as ProjectsView};
use crate::app::tasks::View as TasksView;
use crate::domain::projects::GitStatus;
use crate::ui::components::confirm::{self, Choice};
use crate::ui::components::{button, card, input};
use crate::ui::theme;

use super::{ProjectsState, field, section_label, truncate, worktree};

/// Fixed card width — cards flow-wrap across the workspace at this size.
const CARD_WIDTH: f32 = 300.0;

/// Draft state for the open "add project" modal.
#[derive(Default)]
pub(super) struct NewProjectModal {
    name: String,
    path: String,
}

impl NewProjectModal {
    /// Dev-only: a pre-filled draft (a name and an already-chosen folder) so the `add-project`
    /// gallery screen shows the modal in its ready-to-submit state, without invoking the OS
    /// folder picker.
    pub(super) fn dev_sample() -> Self {
        Self {
            name: "my-dev-dashboard".to_owned(),
            path: "/Users/you/Programming/MyDevDashboard".to_owned(),
        }
    }
}

/// Which per-worktree bash script an editor/section is for. Setup and teardown are the same
/// primitive (a project-level script run inside a worktree's dir, §10) differing only in *when*
/// they run — setup on creation, teardown right before removal — and their copy, so one modal +
/// one section renderer are parameterised by this rather than duplicated (AGENTS.md §4).
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ScriptKind {
    Setup,
    Teardown,
}

impl ScriptKind {
    /// The script's current text on a project.
    fn value(self, p: &crate::domain::projects::Project) -> &str {
        match self {
            Self::Setup => &p.setup_script,
            Self::Teardown => &p.teardown_script,
        }
    }

    /// The `projects::Event` that persists this kind of script.
    fn set_event(self, id: Uuid, script: String) -> crate::app::projects::Event {
        match self {
            Self::Setup => crate::app::projects::Event::set_setup_script(id, script),
            Self::Teardown => crate::app::projects::Event::set_teardown_script(id, script),
        }
    }

    /// Section/modal title.
    fn title(self) -> &'static str {
        match self {
            Self::Setup => "Setup script",
            Self::Teardown => "Teardown script",
        }
    }

    /// One-line description for the detail-page section.
    fn section_desc(self) -> &'static str {
        match self {
            Self::Setup => {
                "Runs as a bash script inside each new worktree (e.g. `bun install`). Optional."
            }
            Self::Teardown => {
                "Runs as a bash script inside each worktree just before it's removed \
                 (e.g. `docker compose down`). Optional."
            }
        }
    }

    /// The italic placeholder shown in the section's inset when no script is set.
    fn empty_hint(self) -> &'static str {
        match self {
            Self::Setup => "No setup script — new worktrees are left as-is.",
            Self::Teardown => "No teardown script — worktrees are removed as-is.",
        }
    }

    /// The modal's body copy (parameterised by the project name).
    fn modal_desc(self, name: &str) -> String {
        match self {
            Self::Setup => format!(
                "Runs as a bash script in the working directory of every new worktree for \
                 “{name}” — e.g. `bun install`. Optional: leave it empty for no setup.",
            ),
            Self::Teardown => format!(
                "Runs as a bash script in the working directory of each worktree for “{name}” \
                 just before it's removed — e.g. `docker compose down`. Optional: leave it empty \
                 for no teardown.",
            ),
        }
    }

    /// The text-area placeholder inside the modal.
    fn placeholder(self) -> &'static str {
        match self {
            Self::Setup => "bun install",
            Self::Teardown => "docker compose down",
        }
    }
}

/// Draft state for the open "edit script" modal — which script (setup/teardown), the project being
/// edited, and the working copy of its bash text.
pub(super) struct ScriptModal {
    kind: ScriptKind,
    project_id: Uuid,
    draft: String,
}

impl ScriptModal {
    pub(super) fn new(kind: ScriptKind, project_id: Uuid, current: &str) -> Self {
        Self {
            kind,
            project_id,
            draft: current.to_owned(),
        }
    }

    pub(super) fn project_id(&self) -> Uuid {
        self.project_id
    }

    pub(super) fn script_mut(&mut self) -> &mut String {
        &mut self.draft
    }
}

impl ProjectsState {
    /// The grid: a header (title + "Add project" + "Refresh" + last-checked time) and the
    /// wrapping cards, or an empty state.
    pub(super) fn render_grid(
        &mut self,
        ui: &mut egui::Ui,
        bridge: &Bridge,
        projects: &ProjectsView,
    ) {
        let muted = theme::palette().muted;

        ui.horizontal(|ui| {
            ui.heading("Projects");
            ui.add_space(16.0);
            if button::primary(ui, &format!("{} Add project", theme::icon::ADD)).clicked() {
                self.adding = Some(NewProjectModal::default());
            }
            ui.add_space(6.0);
            if button::secondary(ui, &format!("{} Refresh git", theme::icon::REFRESH)).clicked() {
                bridge.send(crate::app::projects::Event::refresh_status());
            }
            ui.add_space(8.0);
            // While a refresh runs, a spinner (git is fetched in the background, off this thread);
            // otherwise the status is a session-cached snapshot, so say when it was last checked
            // rather than implying it's live.
            if projects.refreshing {
                ui.add(egui::Spinner::new().size(14.0).color(muted));
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new("Fetching git status…")
                        .color(muted)
                        .size(12.5),
                );
            } else if let Some(label) = git_checked_label(projects) {
                ui.label(egui::RichText::new(label).color(muted).size(12.5));
            }
        });
        ui.add_space(10.0);

        if projects.projects.is_empty() {
            ui.vertical_centered(|ui| {
                ui.add_space(48.0);
                ui.heading("No projects yet");
                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new(
                        "Add a repository you already have on disk with \"Add project\" above — \
                         pick its folder and give it a name.",
                    )
                    .color(muted),
                );
            });
            return;
        }

        // The card is signalled after layout (labels inside occlude the frame's own response),
        // exactly like a ticket card — collect the target then open it after the loop.
        let mut open: Option<Uuid> = None;
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    for c in &projects.projects {
                        let live = projects.live_count_for_project(c.project.id);
                        match render_card(ui, c, live, projects.refreshing) {
                            CardAction::Open => open = Some(c.project.id),
                            CardAction::Pull => {
                                bridge.send(crate::app::projects::Event::pull_project(c.project.id))
                            }
                            CardAction::None => {}
                        }
                    }
                });
            });
        if let Some(id) = open {
            self.open_project = Some(id);
        }
    }

    /// The full-page project detail: metadata + git status on the left, worktrees on the right.
    pub(super) fn render_project_page(
        &mut self,
        ui: &mut egui::Ui,
        bridge: &Bridge,
        id: Uuid,
        projects: &ProjectsView,
        tasks: &TasksView,
    ) {
        let Some(c) = projects.project(id) else {
            return;
        };

        ui.horizontal(|ui| {
            if button::link(ui, &format!("{} Back", theme::icon::BACK)).clicked() {
                self.open_project = None;
            }
        });
        ui.add_space(10.0);

        let mut delete = false;
        let mut edit_script: Option<ScriptKind> = None;
        let mut wt_actions = worktree::RowActions::default();
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.heading(&c.project.name);
                    ui.add_space(10.0);
                    git_badge(ui, &c.git, projects.refreshing);
                    ui.add_space(10.0);
                    if button::ghost(ui, &format!("{} Refresh git", theme::icon::REFRESH)).clicked()
                    {
                        bridge.send(crate::app::projects::Event::refresh_status());
                    }
                    // Same one-click Pull as the card, for a shared branch that's behind (§10).
                    if c.pulling {
                        ui.add_space(6.0);
                        pulling_indicator(ui);
                    } else if !projects.refreshing && c.git.can_pull() {
                        ui.add_space(6.0);
                        if button::compact_primary(ui, &format!("{} Pull", theme::icon::PULL))
                            .clicked()
                        {
                            bridge.send(crate::app::projects::Event::pull_project(id));
                        }
                    }
                });
                ui.add_space(12.0);

                // The page reads as full-width horizontal bands, not scattered boxes: a metadata
                // band (Repository | Git status), then the two scripts as a matched pair (Setup |
                // Teardown — the create/remove counterparts, §10), then the worktrees full-width,
                // then the danger delete at the foot. Each paired band is a 2-column row with a
                // wide gap; both scripts optional, last edit-click wins if both somehow fire.
                let gap = 40.0;

                ui.spacing_mut().item_spacing.x = gap;
                ui.columns(2, |cols| {
                    cols[0].spacing_mut().item_spacing.x = 8.0;
                    render_repository(&mut cols[0], c);
                    cols[1].spacing_mut().item_spacing.x = 8.0;
                    render_git_status(&mut cols[1], c, projects.refreshing);
                });

                ui.add_space(18.0);
                ui.spacing_mut().item_spacing.x = gap;
                ui.columns(2, |cols| {
                    cols[0].spacing_mut().item_spacing.x = 8.0;
                    if render_script_section(&mut cols[0], c, ScriptKind::Setup) {
                        edit_script = Some(ScriptKind::Setup);
                    }
                    cols[1].spacing_mut().item_spacing.x = 8.0;
                    if render_script_section(&mut cols[1], c, ScriptKind::Teardown) {
                        edit_script = Some(ScriptKind::Teardown);
                    }
                });

                ui.add_space(18.0);
                worktree::render_project_worktrees(
                    ui,
                    bridge,
                    id,
                    projects,
                    tasks,
                    &mut wt_actions,
                );

                ui.add_space(18.0);
                delete = button::danger(ui, &format!("{} Delete project", theme::icon::DELETE))
                    .clicked();
            });

        if delete {
            self.confirm_delete_project = Some(id);
        }
        if let Some(kind) = edit_script {
            let current = projects
                .project(id)
                .map(|c| kind.value(&c.project).to_owned())
                .unwrap_or_default();
            self.editing_script = Some(ScriptModal::new(kind, id, &current));
        }
        if let Some(worktree_id) = wt_actions.remove {
            self.confirm_remove_worktree = Some(worktree_id);
        }
        if let Some(ticket_id) = wt_actions.open_ticket {
            // Hand off to the shell → board (cross-feature): open this worktree's ticket detail.
            self.pending_open_ticket = Some(ticket_id);
        }
    }

    /// The "add project" modal overlay.
    pub(super) fn render_add_project_modal(&mut self, ctx: &egui::Context, bridge: &Bridge) {
        let Some(draft) = self.adding.as_mut() else {
            return;
        };
        let muted = theme::palette().muted;
        let mut submit = false;
        let mut cancel = false;

        let response = egui::Modal::new(egui::Id::new("add_project_modal"))
            .frame(theme::surface_frame())
            .show(ctx, |ui| {
                ui.set_min_width(460.0);
                ui.heading("Add project");
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(
                        "Point at a git repository you already have on disk. This never clones — \
                         it just tracks the path.",
                    )
                    .color(muted)
                    .size(12.5),
                );
                ui.add_space(12.0);

                ui.label(egui::RichText::new("Name").strong().color(muted));
                ui.add_space(4.0);
                input::text_field(ui, &mut draft.name, "e.g. My Dev Dashboard");

                ui.add_space(10.0);
                ui.label(
                    egui::RichText::new("Repository folder")
                        .strong()
                        .color(muted),
                );
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    if button::secondary(ui, &format!("{} Choose folder…", theme::icon::PATH))
                        .clicked()
                    {
                        // Native macOS folder picker: only a directory can be selected, so the
                        // chosen path is always a folder (the create service still verifies it's
                        // a git repo). Blocking is fine — egui runs on the main thread and the OS
                        // dialog is modal.
                        if let Some(dir) = rfd::FileDialog::new()
                            .set_title("Choose a repository folder")
                            .pick_folder()
                        {
                            draft.path = dir.display().to_string();
                        }
                    }
                    if draft.path.trim().is_empty() {
                        ui.label(
                            egui::RichText::new("No folder selected")
                                .color(muted)
                                .size(12.5),
                        );
                    } else {
                        ui.label(
                            egui::RichText::new(truncate(&draft.path, 46))
                                .color(theme::palette().text)
                                .size(12.5),
                        )
                        .on_hover_text(&draft.path);
                    }
                });

                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    let can_add = !draft.name.trim().is_empty() && !draft.path.trim().is_empty();
                    submit = button::primary_enabled(ui, "Add project", can_add).clicked();
                    cancel = button::secondary(ui, "Cancel").clicked();
                });
            });

        if response.should_close() {
            cancel = true;
        }

        if submit && !draft.name.trim().is_empty() && !draft.path.trim().is_empty() {
            bridge.send(crate::app::projects::Event::create_project(
                draft.name.trim().to_owned(),
                draft.path.trim().to_owned(),
            ));
            self.adding = None;
        } else if cancel {
            self.adding = None;
        }
    }

    /// The confirm-delete-project modal overlay (shared confirm component, §13).
    pub(super) fn render_confirm_delete_modal(
        &mut self,
        ctx: &egui::Context,
        bridge: &Bridge,
        projects: &ProjectsView,
    ) {
        let Some(id) = self.confirm_delete_project else {
            return;
        };
        let name = projects
            .project(id)
            .map(|c| c.project.name.clone())
            .unwrap_or_default();
        let body = format!(
            "Remove “{name}” from your dashboard? This only forgets it here — the repository on \
             disk (and any worktree folders inside it) are left untouched. Its worktree records \
             are discarded."
        );
        match confirm::destructive(
            ctx,
            ("delete_project", id),
            "Delete project",
            &body,
            "Delete",
        ) {
            Choice::Confirmed => {
                bridge.send(crate::app::projects::Event::delete_project(id));
                self.confirm_delete_project = None;
                self.open_project = None; // also leave the now-gone detail page
            }
            Choice::Cancelled => self.confirm_delete_project = None,
            Choice::Pending => {}
        }
    }

    /// The "edit script" modal: a multi-line bash editor for a project's setup OR teardown script
    /// (which one is carried on the open `ScriptModal`, §10). Save persists it (even when emptied —
    /// that clears it); Cancel discards.
    pub(super) fn render_script_modal(
        &mut self,
        ctx: &egui::Context,
        bridge: &Bridge,
        projects: &ProjectsView,
    ) {
        let Some(draft) = self.editing_script.as_mut() else {
            return;
        };
        let kind = draft.kind;
        let project_id = draft.project_id();
        let name = projects
            .project(project_id)
            .map(|c| c.project.name.clone())
            .unwrap_or_default();
        let muted = theme::palette().muted;
        let mut save = false;
        let mut cancel = false;

        let response = egui::Modal::new(egui::Id::new(("script_modal", project_id)))
            .frame(theme::surface_frame())
            .show(ctx, |ui| {
                ui.set_min_width(560.0);
                ui.heading(kind.title());
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(kind.modal_desc(&name))
                        .color(muted)
                        .size(12.5),
                );
                ui.add_space(12.0);
                // No shebang in the placeholder: the script is executed with `bash -c <script>`
                // (system::projects::git::run_script), so a `#!/usr/bin/env bash` line would just
                // be an inert comment — bash is already the interpreter.
                input::text_area(ui, draft.script_mut(), kind.placeholder(), 12);
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    save = button::primary(ui, &format!("{} Save", theme::icon::SAVE)).clicked();
                    cancel = button::secondary(ui, "Cancel").clicked();
                });
            });

        if response.should_close() {
            cancel = true;
        }

        if save {
            bridge.send(kind.set_event(project_id, draft.script_mut().clone()));
            self.editing_script = None;
        } else if cancel {
            self.editing_script = None;
        }
    }
}

/// What a click on a project card resolved to: open its detail, pull it, or nothing.
enum CardAction {
    None,
    Open,
    Pull,
}

/// Render one project card. Clicking anywhere opens the detail; clicking the (conditional) Pull
/// button pulls instead.
fn render_card(
    ui: &mut egui::Ui,
    c: &ProjectCard,
    live_worktrees: usize,
    loading: bool,
) -> CardAction {
    let muted = theme::palette().muted;

    // The Pull button (when shown) lives in the header, just left of the git badge; we grab its
    // rect here so its click can be wired ON TOP of the whole-card open interaction below.
    let mut pull_rect: Option<egui::Rect> = None;
    let response = card::card(ui, |ui| {
        ui.set_width(CARD_WIDTH);
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(&c.project.name).strong().size(16.0));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Right-to-left: the badge sits furthest right; a shared branch (main/develop)
                // that's behind then gets a one-click Pull to its LEFT (§10). Hidden mid-refresh —
                // the cached status it's judged from is about to change.
                git_badge(ui, &c.git, loading);
                if c.pulling {
                    // A pull is in flight — show its own spinner, not the button, so it can't be
                    // clicked again (the system guard also refuses duplicates) and its loading is
                    // never confused with the git-status refresh spinner.
                    ui.add_space(8.0);
                    pulling_indicator(ui);
                } else if !loading && c.git.can_pull() {
                    ui.add_space(8.0);
                    pull_rect = Some(
                        button::compact_primary(ui, &format!("{} Pull", theme::icon::PULL)).rect,
                    );
                }
            });
        });
        ui.add_space(6.0);
        meta_line(ui, theme::icon::LINK_URL, c.git.origin_url.as_deref());
        meta_line(ui, theme::icon::PATH, Some(&c.project.path));
        meta_line(ui, theme::icon::BRANCH, c.git.branch.as_deref());
        ui.add_space(8.0);
        ui.label(
            egui::RichText::new(format!(
                "{} {} worktree{}",
                theme::icon::BRANCH,
                live_worktrees,
                if live_worktrees == 1 { "" } else { "s" }
            ))
            .color(muted)
            .size(12.5),
        );
    })
    .response;

    // Whole-card click opens the detail. Added FIRST so the Pull interaction (added last, hence on
    // top) wins its own sub-rect — mirroring the ticket card's drag-handle-over-open layering.
    let open = ui
        .interact(
            response.rect,
            egui::Id::new(("project_card", c.project.id)),
            egui::Sense::click(),
        )
        .on_hover_cursor(egui::CursorIcon::PointingHand);

    if let Some(rect) = pull_rect {
        let pull = ui
            .interact(
                rect,
                egui::Id::new(("project_pull", c.project.id)),
                egui::Sense::click(),
            )
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        if pull.clicked() {
            return CardAction::Pull;
        }
    }
    if open.clicked() {
        CardAction::Open
    } else {
        CardAction::None
    }
}

/// The in-flight "Pulling…" indicator shown where the Pull button would be (card header + detail
/// header) while a `git pull --rebase` runs. A self-contained left-to-right group so it renders
/// the same inside a right-to-left card header as it does in the left-to-right detail header.
fn pulling_indicator(ui: &mut egui::Ui) {
    let accent = theme::palette().accent;
    ui.horizontal(|ui| {
        ui.add(egui::Spinner::new().size(13.0).color(accent));
        ui.add_space(4.0);
        ui.label(egui::RichText::new("Pulling…").color(accent).size(12.5));
    });
}

/// A muted `icon + value` line; shows an em-dash when the value is absent.
fn meta_line(ui: &mut egui::Ui, glyph: char, value: Option<&str>) {
    let muted = theme::palette().muted;
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(glyph.to_string())
                .color(muted)
                .size(13.0),
        );
        let text = value
            .map(|v| truncate(v, 42))
            .unwrap_or_else(|| "—".to_owned());
        ui.label(egui::RichText::new(text).color(muted).size(12.5));
    });
}

/// A compact, color-coded git badge: a spinner while a refresh is in flight, then teal "up to
/// date", a muted sync summary, or a red "not a repo". This is the at-a-glance card/header
/// indicator. `loading` shows the spinner regardless of the (about-to-change) status.
fn git_badge(ui: &mut egui::Ui, git: &GitStatus, loading: bool) {
    let p = theme::palette();
    if loading {
        ui.add(egui::Spinner::new().size(13.0).color(p.muted));
        ui.add_space(4.0);
        ui.label(egui::RichText::new("checking…").color(p.muted).size(12.0));
        return;
    }
    if git.checked_at.is_none() {
        // Never fetched this session (e.g. just switched to this profile) — don't claim "not a
        // repo" when we simply haven't looked yet.
        ui.label(egui::RichText::new("not checked").color(p.muted).size(12.0));
        return;
    }
    if !git.is_repo {
        ui.label(egui::RichText::new("not a repo").color(p.danger).size(12.0));
        return;
    }
    if git.up_to_date() {
        ui.label(
            egui::RichText::new(format!("{} up to date", theme::icon::CHECK))
                .color(p.accent)
                .size(12.0),
        );
        return;
    }
    ui.label(
        egui::RichText::new(format!("{} {}", theme::icon::SYNC, sync_summary(git)))
            .color(p.muted)
            .size(12.0),
    );
}

/// A full-width script panel on the detail page — used for both the setup script (left column) and
/// the teardown script (right column, atop the worktrees), selected by `kind`. Shows the current
/// script (or a muted "no script" hint) and an Edit button. Returns true if Edit/Add was clicked.
fn render_script_section(ui: &mut egui::Ui, c: &ProjectCard, kind: ScriptKind) -> bool {
    let p = theme::palette();
    let script = kind.value(&c.project);
    let script = script.trim();
    let noun = kind.title().to_lowercase(); // "setup script" / "teardown script"

    section_label(ui, kind.title());
    ui.label(
        egui::RichText::new(kind.section_desc())
            .color(p.muted)
            .size(12.0),
    );
    ui.add_space(8.0);

    card::inset(ui, |ui| {
        ui.set_width(ui.available_width());
        if script.is_empty() {
            ui.label(
                egui::RichText::new(kind.empty_hint())
                    .color(p.muted)
                    .italics(),
            );
        } else {
            // Show the script verbatim in monospace so it reads as code, not prose.
            ui.label(egui::RichText::new(script).monospace().size(12.5));
        }
    });
    ui.add_space(8.0);

    let label = if script.is_empty() {
        format!("{} Add {noun}", theme::icon::EDIT)
    } else {
        format!("{} Edit {noun}", theme::icon::EDIT)
    };
    button::secondary(ui, &label).clicked()
}

/// The repository identity — the left half of the top metadata band (path / origin / branch).
fn render_repository(ui: &mut egui::Ui, c: &ProjectCard) {
    section_label(ui, "Repository");
    field(ui, "Path", &c.project.path);
    field(ui, "Origin", c.git.origin_url.as_deref().unwrap_or("—"));
    field(ui, "Branch", c.git.branch.as_deref().unwrap_or("—"));
}

/// The git-status detail — the right half of the top metadata band. `refreshing` swaps it for a
/// loading line while a background fetch is in flight.
fn render_git_status(ui: &mut egui::Ui, c: &ProjectCard, refreshing: bool) {
    let p = theme::palette();

    section_label(ui, "Git status");
    if refreshing {
        ui.horizontal(|ui| {
            ui.add(egui::Spinner::new().size(14.0).color(p.muted));
            ui.add_space(4.0);
            ui.label(egui::RichText::new("Fetching git status…").color(p.muted));
        });
    } else if c.git.checked_at.is_none() {
        ui.label(
            egui::RichText::new("Not checked yet — Refresh to load git status.").color(p.muted),
        );
    } else if !c.git.is_repo {
        ui.label(egui::RichText::new("This path is not a git repository.").color(p.danger));
    } else {
        let working = if c.git.clean {
            "Working tree clean".to_owned()
        } else {
            "Uncommitted changes".to_owned()
        };
        ui.label(&working);
        let upstream = if !c.git.has_upstream {
            "No upstream tracking branch".to_owned()
        } else if c.git.ahead == 0 && c.git.behind == 0 {
            "In sync with upstream".to_owned()
        } else {
            format!("Upstream: {}", sync_summary(&c.git))
        };
        ui.label(egui::RichText::new(upstream).color(p.muted));
        ui.label(
            egui::RichText::new(if c.git.fetched {
                "Compared against a fresh fetch."
            } else {
                "Offline — compared against local refs. Fetch/pull yourself to refresh."
            })
            .color(p.muted)
            .size(12.0),
        );
        if let Some(at) = c.git.checked_at {
            ui.label(
                egui::RichText::new(format!(
                    "Checked {}. Refresh to re-fetch.",
                    format_checked(at)
                ))
                .color(p.muted)
                .size(12.0),
            );
        }
    }
}

/// A short "N ahead · M behind · uncommitted" phrase for an out-of-sync repo.
fn sync_summary(git: &GitStatus) -> String {
    let mut parts = Vec::new();
    if !git.clean {
        parts.push("uncommitted".to_owned());
    }
    if git.behind > 0 {
        parts.push(format!("{} behind", git.behind));
    }
    if git.ahead > 0 {
        parts.push(format!("{} ahead", git.ahead));
    }
    if parts.is_empty() {
        "out of sync".to_owned()
    } else {
        parts.join(" · ")
    }
}

/// A muted "Checked HH:MM" header label from the most recent `checked_at` across projects.
/// `None` when there are no projects, or none checked yet (the header omits it then).
fn git_checked_label(projects: &ProjectsView) -> Option<String> {
    let latest = projects
        .projects
        .iter()
        .filter_map(|c| c.git.checked_at)
        .max()?;
    Some(format!("Checked {}", format_checked(latest)))
}

/// Format a UTC check time in the owner's local time, compact (HH:MM). The status is a
/// session-cached read, so the UI shows when it was taken rather than implying it's live.
fn format_checked(at: chrono::DateTime<chrono::Utc>) -> String {
    at.with_timezone(&chrono::Local).format("%H:%M").to_string()
}
