//! `projects::project` part UI: the card grid, project cards, the full-page detail, and the
//! add-project + confirm-delete modals.

use uuid::Uuid;

use crate::app::Bridge;
use crate::app::projects::{ProjectCard, View as ProjectsView};
use crate::app::tasks::View as TasksView;
use crate::domain::projects::GitStatus;
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

impl ProjectsState {
    /// The grid: a header (title + "Add project") and the wrapping cards, or an empty state.
    pub(super) fn render_grid(&mut self, ui: &mut egui::Ui, projects: &ProjectsView) {
        let muted = theme::palette().muted;

        ui.horizontal(|ui| {
            ui.heading("Projects");
            ui.add_space(16.0);
            if button::primary(ui, &format!("{} Add project", theme::icon::ADD)).clicked() {
                self.adding = Some(NewProjectModal::default());
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
                         paste its path and give it a name.",
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
                        if render_card(ui, c, live) {
                            open = Some(c.project.id);
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
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.heading(&c.project.name);
                    ui.add_space(10.0);
                    git_badge(ui, &c.git);
                });
                ui.add_space(12.0);

                // Two full-width columns: metadata pinned left, worktrees pinned right, with a
                // wide gap between (matching the ticket full-page layout).
                ui.spacing_mut().item_spacing.x = 40.0;
                ui.columns(2, |cols| {
                    cols[0].spacing_mut().item_spacing.x = 8.0;
                    delete = render_meta(&mut cols[0], c);

                    cols[1].spacing_mut().item_spacing.x = 8.0;
                    worktree::render_project_worktrees(&mut cols[1], bridge, id, projects, tasks);
                });
            });

        if delete {
            self.confirm_delete_project = Some(id);
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
                ui.label(egui::RichText::new("Repository path").strong().color(muted));
                ui.add_space(4.0);
                input::text_field(ui, &mut draft.path, "/Users/you/Programming/your-repo");

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

    /// The confirm-delete-project modal overlay.
    pub(super) fn render_confirm_delete_modal(
        &mut self,
        ctx: &egui::Context,
        bridge: &Bridge,
        projects: &ProjectsView,
    ) {
        let Some(id) = self.confirm_delete_project else {
            return;
        };
        let p = theme::palette();
        let name = projects
            .project(id)
            .map(|c| c.project.name.clone())
            .unwrap_or_default();
        let mut confirm = false;
        let mut cancel = false;

        let response = egui::Modal::new(egui::Id::new(("delete_project", id)))
            .frame(theme::surface_frame())
            .show(ctx, |ui| {
                ui.set_max_width(440.0);
                ui.label(
                    egui::RichText::new(format!("{} Delete project", theme::icon::WARNING))
                        .heading()
                        .color(p.danger),
                );
                ui.add_space(8.0);
                ui.label(format!(
                    "Remove “{name}” from your dashboard? This only forgets it here — the \
                     repository on disk (and any worktree folders inside it) are left untouched. \
                     Its worktree records are discarded."
                ));
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    confirm =
                        button::danger(ui, &format!("{} Delete", theme::icon::DELETE)).clicked();
                    cancel = button::secondary(ui, "Cancel").clicked();
                });
            });

        if response.should_close() {
            cancel = true;
        }

        if confirm {
            bridge.send(crate::app::projects::Event::delete_project(id));
            self.confirm_delete_project = None;
            self.open_project = None;
        } else if cancel {
            self.confirm_delete_project = None;
        }
    }
}

/// Render one project card. Returns true if the card was clicked (open its detail).
fn render_card(ui: &mut egui::Ui, c: &ProjectCard, live_worktrees: usize) -> bool {
    let muted = theme::palette().muted;

    let response = card::card(ui, |ui| {
        ui.set_width(CARD_WIDTH);
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(&c.project.name).strong().size(16.0));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                git_badge(ui, &c.git);
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

    ui.interact(
        response.rect,
        egui::Id::new(("project_card", c.project.id)),
        egui::Sense::click(),
    )
    .on_hover_cursor(egui::CursorIcon::PointingHand)
    .clicked()
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

/// A compact, color-coded git badge: teal "up to date", muted sync summary, or a red "not a
/// repo". This is the at-a-glance card/header indicator.
fn git_badge(ui: &mut egui::Ui, git: &GitStatus) {
    let p = theme::palette();
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

/// Left column of the detail page: repository metadata, git status detail, and delete. Returns
/// true if "Delete project" was clicked.
fn render_meta(ui: &mut egui::Ui, c: &ProjectCard) -> bool {
    let p = theme::palette();

    section_label(ui, "Repository");
    field(ui, "Path", &c.project.path);
    field(ui, "Origin", c.git.origin_url.as_deref().unwrap_or("—"));
    field(ui, "Branch", c.git.branch.as_deref().unwrap_or("—"));

    ui.add_space(6.0);
    section_label(ui, "Git status");
    if !c.git.is_repo {
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
    }

    ui.add_space(16.0);
    button::danger(ui, &format!("{} Delete project", theme::icon::DELETE)).clicked()
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
