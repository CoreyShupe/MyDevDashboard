//! `projects::worktree` part UI: worktree rows (shown on both the project detail page and the
//! ticket detail page) and the create-worktree picker modal.

use uuid::Uuid;

use crate::app::Bridge;
use crate::app::projects::{Event, ProjectCard, View as ProjectsView};
use crate::app::tasks::View as TasksView;
use crate::domain::projects::Worktree;
use crate::ui::components::{button, card, input};
use crate::ui::theme;

use super::{ProjectsState, section_label, truncate};

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

/// The right column of the project detail page: this project's worktrees, live first, then
/// removed (recreatable) markers. Live rows open in VS Code or remove; markers recreate.
pub(super) fn render_project_worktrees(
    ui: &mut egui::Ui,
    bridge: &Bridge,
    project_id: Uuid,
    projects: &ProjectsView,
    tasks: &TasksView,
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
    let all: Vec<&Worktree> = projects.worktrees_for_project(project_id).collect();
    if all.is_empty() {
        ui.label(egui::RichText::new("No worktrees yet.").color(muted));
        return;
    }

    for w in all.iter().filter(|w| w.is_live()) {
        let title = tasks
            .ticket(w.ticket_id)
            .map(|t| t.title.as_str())
            .unwrap_or("(ticket removed)");
        worktree_row(ui, bridge, w, title, repo_path.as_deref());
    }

    let markers: Vec<&&Worktree> = all.iter().filter(|w| !w.is_live()).collect();
    if !markers.is_empty() {
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new("Removed — recreatable")
                .color(muted)
                .size(12.0),
        );
        ui.add_space(4.0);
        for w in markers {
            let title = tasks
                .ticket(w.ticket_id)
                .map(|t| t.title.as_str())
                .unwrap_or("(ticket removed)");
            worktree_row(ui, bridge, w, title, repo_path.as_deref());
        }
    }
}

/// One worktree row on the project detail page (keyed by its ticket). Live rows show Open +
/// Remove; markers show Recreate.
fn worktree_row(
    ui: &mut egui::Ui,
    bridge: &Bridge,
    w: &Worktree,
    ticket_title: &str,
    repo_path: Option<&str>,
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
        worktree_actions(ui, bridge, w);
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
) -> bool {
    let muted = theme::palette().muted;
    let worktrees: Vec<&Worktree> = projects.worktrees_for_ticket(ticket_id).collect();

    if worktrees.is_empty() {
        ui.label(egui::RichText::new("No worktrees for this ticket yet.").color(muted));
    } else {
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
                worktree_actions(ui, bridge, w);
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

/// The action row for a worktree: Open + Remove when live, Recreate when a marker.
fn worktree_actions(ui: &mut egui::Ui, bridge: &Bridge, w: &Worktree) {
    ui.horizontal(|ui| {
        if w.is_live() {
            if button::secondary(
                ui,
                &format!("{} Open in VS Code", theme::icon::OPEN_EXTERNAL),
            )
            .clicked()
            {
                bridge.send(Event::open_worktree(w.id));
            }
            if button::danger(ui, &format!("{} Remove", theme::icon::DELETE)).clicked() {
                bridge.send(Event::remove_worktree(w.id));
            }
        } else if button::secondary(ui, &format!("{} Recreate", theme::icon::ADD)).clicked() {
            bridge.send(Event::recreate_worktree(w.id));
        }
    });
}

impl ProjectsState {
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
        // "Recreate" action in the lists, so they're excluded here).
        let eligible: Vec<&ProjectCard> = projects
            .projects
            .iter()
            .filter(|c| {
                !projects
                    .worktrees_for_ticket(ticket_id)
                    .any(|w| w.project_id == c.project.id)
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
