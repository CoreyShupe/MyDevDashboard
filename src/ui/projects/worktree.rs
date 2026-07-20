//! `projects::worktree` part UI: worktree rows (shown on both the project detail page and the
//! ticket detail page) and the create-worktree picker modal.

use uuid::Uuid;

use crate::app::Bridge;
use crate::app::projects::{Event, ProjectCard, View as ProjectsView};
use crate::app::tasks::View as TasksView;
use crate::domain::projects::Worktree;
use crate::ui::components::confirm::{self, Choice};
use crate::ui::components::{button, card, input};
use crate::ui::theme;

use super::{ProjectsState, section_label, truncate};

/// The deferred actions a project-detail worktree row can raise while rendering, each recorded
/// here for the caller (`project::render_detail`) to act on AFTER the frame: remove-this-worktree
/// (destructive → confirmed first, §13) and open-this-worktree's-ticket (handed to the board, §2).
/// Bundled so the row/section fns don't grow an out-param each (keeps them under clippy's arg cap).
#[derive(Default)]
pub(super) struct RowActions {
    pub(super) remove: Option<Uuid>,
    pub(super) open_ticket: Option<Uuid>,
}

/// Draft state for the open "create worktree" modal (always ticket-driven).
pub(super) struct NewWorktreeModal {
    pub(super) ticket_id: Uuid,
    project_id: Option<Uuid>,
    branch: String,
}

impl NewWorktreeModal {
    pub(super) fn new(ticket_id: Uuid) -> Self {
        Self {
            ticket_id,
            project_id: None,
            branch: String::new(),
        }
    }
}

/// The right column of the project detail page: this project's LIVE worktrees only. Removed
/// (recreatable) markers are deliberately NOT shown here — they live on the ticket they originate
/// from, which owns recreation; the project view is just "what's checked out right now". Live rows
/// open in VS Code or remove.
pub(super) fn render_project_worktrees(
    ui: &mut egui::Ui,
    bridge: &Bridge,
    project_id: Uuid,
    projects: &ProjectsView,
    tasks: &TasksView,
    actions: &mut RowActions,
) {
    let muted = theme::palette().muted;
    section_label(ui, "Worktrees");
    ui.label(
        egui::RichText::new("Worktrees are created from a ticket's detail page.")
            .color(muted)
            .size(12.0),
    );
    ui.add_space(8.0);

    let repo_path = projects.project(project_id).map(|c| c.project.path.clone());
    let live: Vec<&Worktree> = projects
        .worktrees_for_project(project_id)
        .filter(|w| w.is_live())
        .collect();

    // Worktrees being provisioned in THIS project with no live row yet — a first-time create, or a
    // recreate whose marker isn't shown here. A loading card each while git + the setup script run.
    let creating_new: Vec<Uuid> = projects
        .creating_for_project(project_id)
        .filter(|tid| !live.iter().any(|w| w.ticket_id == *tid))
        .collect();

    if live.is_empty() && creating_new.is_empty() {
        ui.label(egui::RichText::new("No worktrees yet.").color(muted));
        return;
    }

    for ticket_id in &creating_new {
        let title = tasks
            .ticket(*ticket_id)
            .map(|t| t.title.as_str())
            .unwrap_or("(ticket removed)");
        setup_loading_row(ui, title);
    }

    for w in &live {
        let title = tasks
            .ticket(w.ticket_id)
            .map(|t| t.title.as_str())
            .unwrap_or("(ticket removed)");
        worktree_row(
            ui,
            bridge,
            w,
            title,
            repo_path.as_deref(),
            actions,
            projects,
        );
    }
}

/// One LIVE worktree row on the project detail page (keyed by its ticket): Open in VS Code +
/// Remove, or the setup spinner while it's mid-provision. A Remove click is recorded into `remove`
/// for confirmation. (Markers aren't shown on the project page — they live on their ticket.)
fn worktree_row(
    ui: &mut egui::Ui,
    bridge: &Bridge,
    w: &Worktree,
    ticket_title: &str,
    repo_path: Option<&str>,
    actions: &mut RowActions,
    projects: &ProjectsView,
) {
    let muted = theme::palette().muted;
    card::inset(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(egui::RichText::new(ticket_title).strong());
        ui.label(
            egui::RichText::new(format!("{} {}", theme::icon::BRANCH, w.branch))
                .color(muted)
                .size(12.5),
        );
        if let Some(repo) = repo_path {
            let path = w.path_in(repo);
            ui.label(
                egui::RichText::new(truncate(&path.to_string_lossy(), 52))
                    .color(muted)
                    .size(11.0),
            );
        }
        ui.add_space(6.0);
        // While this worktree is being (re)created, its setup script may still be running — show
        // the spinner and no actions until it lands (AGENTS.md §10).
        if projects.is_creating(w.project_id, w.ticket_id) {
            setup_indicator(ui);
        } else {
            worktree_actions(
                ui,
                bridge,
                w,
                &mut actions.remove,
                projects,
                Some(&mut actions.open_ticket),
            );
        }
    });
    ui.add_space(6.0);
}

/// The ticket detail's worktree section: this ticket's worktrees across projects, plus the
/// "Create worktree" affordance. Returns true if creation was requested (the shell owns the
/// picker, so it opens it — AGENTS.md §2 cross-feature coordination).
pub(crate) fn render_ticket_worktrees(
    ui: &mut egui::Ui,
    bridge: &Bridge,
    ticket_id: Uuid,
    projects: &ProjectsView,
    remove: &mut Option<Uuid>,
) -> bool {
    let muted = theme::palette().muted;
    let worktrees: Vec<&Worktree> = projects.worktrees_for_ticket(ticket_id).collect();

    // First-time creations in a project this ticket has no row in yet — a loading card each while
    // git + the setup script run (a recreate keeps its marker row and gets the in-row spinner).
    let creating_new: Vec<Uuid> = projects
        .creating_for_ticket(ticket_id)
        .filter(|pid| !worktrees.iter().any(|w| w.project_id == *pid))
        .collect();

    if worktrees.is_empty() && creating_new.is_empty() {
        ui.label(egui::RichText::new("No worktrees for this ticket yet.").color(muted));
    } else {
        for pid in &creating_new {
            let project_name = projects
                .project(*pid)
                .map(|c| c.project.name.clone())
                .unwrap_or_else(|| "(project removed)".to_owned());
            setup_loading_row(ui, &project_name);
        }
        for w in &worktrees {
            let project_name = projects
                .project(w.project_id)
                .map(|c| c.project.name.clone())
                .unwrap_or_else(|| "(project removed)".to_owned());
            card::inset(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(project_name).strong());
                    if !w.is_live() {
                        ui.label(egui::RichText::new("removed").color(muted).size(11.0));
                    }
                });
                ui.label(
                    egui::RichText::new(format!("{} {}", theme::icon::BRANCH, w.branch))
                        .color(muted)
                        .size(12.5),
                );
                ui.add_space(6.0);
                // Spinner while this worktree is being (re)created (its setup may still be running);
                // otherwise the usual Open/Remove/Recreate actions (§10).
                if projects.is_creating(w.project_id, w.ticket_id) {
                    setup_indicator(ui);
                } else {
                    // On the ticket detail you're already on the ticket → no "Open ticket" button.
                    worktree_actions(ui, bridge, w, remove, projects, None);
                }
            });
            ui.add_space(6.0);
        }
    }

    ui.add_space(4.0);
    if projects.projects.is_empty() {
        ui.label(
            egui::RichText::new("Add a project on the Projects tab to create a worktree.")
                .color(muted)
                .size(12.0),
        );
        return false;
    }
    button::ghost(ui, &format!("{} Create worktree", theme::icon::ADD)).clicked()
}

/// The action row for a worktree: Open + Remove when live, Recreate when a marker. Open and
/// Recreate emit directly; Remove is destructive, so it only records the request into `remove`
/// (the caller confirms before it fires — AGENTS.md §13). While a slow action (remove / open) is
/// in flight on this worktree, its buttons are replaced by a "waiting" spinner (AGENTS.md §10).
///
/// `open_ticket` is `Some` only on the PROJECT detail page (where the ticket lives elsewhere): its
/// button records the worktree's ticket id so the shell can open that ticket's detail. The TICKET
/// detail passes `None` — you're already on the ticket, so "Open ticket" would be pointless there.
fn worktree_actions(
    ui: &mut egui::Ui,
    bridge: &Bridge,
    w: &Worktree,
    remove: &mut Option<Uuid>,
    projects: &ProjectsView,
    open_ticket: Option<&mut Option<Uuid>>,
) {
    if let Some(busy) = projects.is_busy(w.id) {
        spinner_row(ui, busy.label());
        return;
    }
    ui.horizontal(|ui| {
        if w.is_live() {
            if let Some(slot) = open_ticket
                && button::secondary(ui, &format!("{} Open ticket", theme::icon::DASHBOARD))
                    .clicked()
            {
                *slot = Some(w.ticket_id);
            }
            if button::secondary(
                ui,
                &format!("{} Open in VS Code", theme::icon::OPEN_EXTERNAL),
            )
            .clicked()
            {
                bridge.send(Event::open_worktree(w.id));
            }
            if button::danger(ui, &format!("{} Remove", theme::icon::DELETE)).clicked() {
                *remove = Some(w.id);
            }
        } else if button::secondary(ui, &format!("{} Recreate", theme::icon::ADD)).clicked() {
            bridge.send(Event::recreate_worktree(w.id));
        }
    });
}

/// A full loading card for a worktree being provisioned that has no row yet (a first-time create):
/// the label (project or ticket, depending on the caller) plus the setup spinner.
fn setup_loading_row(ui: &mut egui::Ui, label: &str) {
    card::inset(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(egui::RichText::new(label).strong());
        ui.add_space(6.0);
        setup_indicator(ui);
    });
    ui.add_space(6.0);
}

/// The in-flight "Setting up…" indicator shown while a worktree is being provisioned — git
/// `worktree add` plus the project's setup script (e.g. `bun install`), which can take a while.
/// Until it lands the worktree isn't presented as ready (AGENTS.md §10).
fn setup_indicator(ui: &mut egui::Ui) {
    spinner_row(ui, "Setting up… running setup script");
}

/// A small accent spinner + "waiting" label, shown in place of a worktree's actions while a slow
/// operation (provision / remove / open) is in flight (AGENTS.md §10).
fn spinner_row(ui: &mut egui::Ui, label: &str) {
    let accent = theme::palette().accent;
    ui.horizontal(|ui| {
        ui.add(egui::Spinner::new().size(14.0).color(accent));
        ui.add_space(6.0);
        ui.label(egui::RichText::new(label).color(accent).size(12.5));
    });
}

impl ProjectsState {
    /// The remove-worktree confirmation (open only while `confirm_remove_worktree` is set).
    /// Removing a worktree runs `git worktree remove` on its on-disk folder, so it's confirmed
    /// first (AGENTS.md §13). Fired from both the project detail page and (via the shell) the
    /// ticket detail.
    pub(super) fn render_remove_worktree_modal(
        &mut self,
        ctx: &egui::Context,
        bridge: &Bridge,
        projects: &ProjectsView,
    ) {
        let Some(id) = self.confirm_remove_worktree else {
            return;
        };
        let Some(w) = projects.worktrees.iter().find(|w| w.id == id) else {
            self.confirm_remove_worktree = None; // gone from the snapshot; nothing to confirm
            return;
        };
        let project = projects
            .project(w.project_id)
            .map(|c| c.project.name.clone())
            .unwrap_or_else(|| "this project".to_owned());
        let body = format!(
            "Remove the worktree for branch “{}” in {project}? This runs `git worktree remove` on \
             its folder (git refuses if it has uncommitted changes) and keeps a marker so you can \
             recreate it later. The branch itself is left alone.",
            w.branch
        );
        match confirm::destructive(
            ctx,
            ("remove_worktree", id),
            "Remove worktree",
            &body,
            "Remove",
        ) {
            Choice::Confirmed => {
                bridge.send(Event::remove_worktree(id));
                self.confirm_remove_worktree = None;
            }
            Choice::Cancelled => self.confirm_remove_worktree = None,
            Choice::Pending => {}
        }
    }

    /// The create-worktree picker (ticket-driven). Picks an eligible project (one with no
    /// worktree for this ticket yet); the branch is locked to the ticket's shared branch if it
    /// already has one, otherwise entered here (AGENTS.md §10).
    pub(super) fn render_create_worktree_modal(
        &mut self,
        ctx: &egui::Context,
        bridge: &Bridge,
        projects: &ProjectsView,
        tasks: &TasksView,
    ) {
        let Some(draft) = self.creating_worktree.as_mut() else {
            return;
        };
        let muted = theme::palette().muted;
        let ticket_id = draft.ticket_id;
        let ticket_title = tasks
            .ticket(ticket_id)
            .map(|t| t.title.clone())
            .unwrap_or_default();

        // Branch is a ticket-level choice, shared across its worktrees.
        let shared_branch = projects
            .worktrees_for_ticket(ticket_id)
            .next()
            .map(|w| w.branch.clone());
        // Eligible = projects with no worktree row for this ticket at all (markers get their own
        // "Recreate" action in the lists, so they're excluded here), and none currently being
        // provisioned for it (its loading row is already showing).
        let eligible: Vec<&ProjectCard> = projects
            .projects
            .iter()
            .filter(|c| {
                !projects
                    .worktrees_for_ticket(ticket_id)
                    .any(|w| w.project_id == c.project.id)
                    && !projects.is_creating(c.project.id, ticket_id)
            })
            .collect();

        let mut submit = false;
        let mut cancel = false;

        let response = egui::Modal::new(egui::Id::new(("create_worktree", ticket_id)))
            .frame(theme::surface_frame())
            .show(ctx, |ui| {
                ui.set_min_width(460.0);
                ui.heading("Create worktree");
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(format!("For ticket: {ticket_title}"))
                        .color(muted)
                        .size(13.0),
                );
                ui.add_space(12.0);

                if eligible.is_empty() {
                    ui.label(
                        egui::RichText::new(
                            "Every project already has a worktree for this ticket. Recreate a \
                             removed one from the list instead.",
                        )
                        .color(muted),
                    );
                } else {
                    ui.label(egui::RichText::new("Project").strong().color(muted));
                    ui.add_space(4.0);
                    project_combo(ui, &eligible, &mut draft.project_id);

                    ui.add_space(10.0);
                    ui.label(egui::RichText::new("Branch").strong().color(muted));
                    ui.add_space(4.0);
                    if let Some(branch) = &shared_branch {
                        ui.label(egui::RichText::new(format!(
                            "{} {branch}",
                            theme::icon::BRANCH
                        )));
                        ui.label(
                            egui::RichText::new(
                                "Shared across this ticket's worktrees (chosen with the first).",
                            )
                            .color(muted)
                            .size(12.0),
                        );
                    } else {
                        input::text_field(ui, &mut draft.branch, "e.g. feature/short-description");
                    }
                }

                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    let branch_ok = shared_branch.is_some() || !draft.branch.trim().is_empty();
                    let can_create =
                        !eligible.is_empty() && draft.project_id.is_some() && branch_ok;
                    submit = button::primary_enabled(ui, "Create worktree", can_create).clicked();
                    cancel = button::secondary(ui, "Cancel").clicked();
                });
            });

        if response.should_close() {
            cancel = true;
        }

        // End the `draft` borrow by copying out what we need before mutating `self`.
        let chosen = draft.project_id;
        let typed_branch = draft.branch.trim().to_owned();

        if submit && let Some(project_id) = chosen {
            let branch = shared_branch.unwrap_or(typed_branch);
            if !branch.is_empty() {
                bridge.send(Event::create_worktree(project_id, ticket_id, branch));
                self.creating_worktree = None;
            }
        } else if cancel {
            self.creating_worktree = None;
        }
    }
}

/// Combo box to pick which eligible project a new worktree lands in.
fn project_combo(ui: &mut egui::Ui, eligible: &[&ProjectCard], selected: &mut Option<Uuid>) {
    let current = selected
        .and_then(|id| eligible.iter().find(|c| c.project.id == id))
        .map(|c| c.project.name.as_str())
        .unwrap_or("Select a project");

    egui::ComboBox::from_id_salt("create_worktree_project")
        .selected_text(current)
        .show_ui(ui, |ui| {
            for c in eligible {
                if ui
                    .selectable_label(*selected == Some(c.project.id), &c.project.name)
                    .clicked()
                {
                    *selected = Some(c.project.id);
                }
            }
        });
}
