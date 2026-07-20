//! `projects` feature UI: the Projects tab — a grid of repository cards, a full-page project
//! detail (metadata + worktrees), and the modals for adding a project and creating a worktree.
//!
//! Rendering is split across part files as `impl ProjectsState`:
//!   - `project`  — the grid, project cards, the detail page, add-project + confirm-delete modals.
//!   - `worktree` — worktree rows (project detail + ticket detail) and the create-worktree modal.
//!
//! All visuals come from `ui::theme` + `ui::components` (AGENTS.md §7). This file never touches
//! the DB — it renders `app::projects::View` and emits `projects::Event`s (AGENTS.md §2).

mod project;
mod worktree;

use uuid::Uuid;

use crate::app::Bridge;
use crate::app::projects::View as ProjectsView;
use crate::app::tasks::View as TasksView;

use project::{NewProjectModal, SetupScriptModal};
use worktree::NewWorktreeModal;

// The ticket detail (tasks feature) delegates its worktree section here — creation is
// ticket-driven, so the ticket page shows this ticket's worktrees and asks to create one.
pub(crate) use worktree::render_ticket_worktrees;

/// All transient UI state for the Projects tab. Lives in the UI only.
#[derive(Default)]
pub struct ProjectsState {
    /// The open "add project" modal, if any.
    adding: Option<NewProjectModal>,
    /// The project whose detail page has taken over the workspace, if any.
    open_project: Option<Uuid>,
    /// The open "edit setup script" modal (a project's per-worktree bash script), if any.
    editing_setup_script: Option<SetupScriptModal>,
    /// A project pending a delete confirmation, if any.
    confirm_delete_project: Option<Uuid>,
    /// A worktree pending a remove confirmation, if any (destructive → confirmed first, §13).
    /// Set both from the project detail page and, via the shell, from the ticket detail.
    confirm_remove_worktree: Option<Uuid>,
    /// The open "create worktree" modal (driven from a ticket), if any.
    creating_worktree: Option<NewWorktreeModal>,
    /// Set when a worktree row's "Open ticket" is clicked — the shell drains this and asks the
    /// board to open that ticket's detail (cross-feature, the reverse of create-worktree, §2).
    pending_open_ticket: Option<Uuid>,
}

impl ProjectsState {
    /// The Projects workspace: the detail page when a project is open, otherwise the card grid.
    pub fn render_workspace(
        &mut self,
        ui: &mut egui::Ui,
        bridge: &Bridge,
        projects: &ProjectsView,
        tasks: &TasksView,
    ) {
        if let Some(id) = self.open_project {
            if projects.project(id).is_some() {
                self.render_project_page(ui, bridge, id, projects, tasks);
                return;
            }
            self.open_project = None; // it vanished; fall back to the grid
        }
        self.render_grid(ui, bridge, projects);
    }

    /// Projects overlays: add-project, confirm-delete, and the create-worktree picker. Rendered
    /// from the app shell alongside the other features' overlays.
    pub fn render_overlays(
        &mut self,
        ctx: &egui::Context,
        bridge: &Bridge,
        projects: &ProjectsView,
        tasks: &TasksView,
    ) {
        self.render_add_project_modal(ctx, bridge);
        self.render_setup_script_modal(ctx, bridge, projects);
        self.render_confirm_delete_modal(ctx, bridge, projects);
        self.render_remove_worktree_modal(ctx, bridge, projects);
        self.render_create_worktree_modal(ctx, bridge, projects, tasks);
    }

    /// Close a stale detail page / confirmation if its project disappeared in the latest
    /// snapshot (mirrors the board's `reconcile`).
    pub fn reconcile(&mut self, projects: &ProjectsView) {
        if self
            .open_project
            .is_some_and(|id| projects.project(id).is_none())
        {
            self.open_project = None;
        }
        if self
            .confirm_delete_project
            .is_some_and(|id| projects.project(id).is_none())
        {
            self.confirm_delete_project = None;
        }
        // Close the setup-script editor if its project vanished from the snapshot.
        if self
            .editing_setup_script
            .as_ref()
            .is_some_and(|m| projects.project(m.project_id()).is_none())
        {
            self.editing_setup_script = None;
        }
        // Drop a pending remove-confirmation if the worktree is gone or already a marker.
        if self
            .confirm_remove_worktree
            .is_some_and(|id| !projects.worktrees.iter().any(|w| w.id == id && w.is_live()))
        {
            self.confirm_remove_worktree = None;
        }
    }

    /// Open the create-worktree picker for a ticket. Called by the app shell when the ticket
    /// detail (tasks feature) requests it (AGENTS.md §2 cross-feature coordination).
    pub fn open_create_worktree(&mut self, ticket_id: Uuid) {
        self.creating_worktree = Some(NewWorktreeModal::new(ticket_id));
    }

    /// Open the remove-worktree confirmation for `id`. Called by the app shell when the ticket
    /// detail asks to remove one (cross-feature, §2 + §13); the project detail page sets it
    /// directly. The projects overlays own the actual confirmation modal.
    pub fn request_remove_worktree(&mut self, worktree_id: Uuid) {
        self.confirm_remove_worktree = Some(worktree_id);
    }

    /// Take a pending "open this worktree's ticket" request raised by a worktree row on the project
    /// detail. The shell hands the ticket to the board's detail view (cross-feature, §2).
    pub fn take_pending_open_ticket(&mut self) -> Option<Uuid> {
        self.pending_open_ticket.take()
    }

    /// Dev-only: open a project's detail page directly for review (see `ui::dev`).
    pub fn dev_open_project(&mut self, project_id: Uuid) {
        self.open_project = Some(project_id);
    }

    /// Dev-only: open the "add project" modal, pre-filled, for review (see `ui::dev`).
    pub fn dev_open_add_project(&mut self) {
        self.adding = Some(NewProjectModal::dev_sample());
    }

    /// Dev-only: open a project's detail page with the setup-script editor open, for review.
    pub fn dev_open_setup_script(&mut self, project_id: Uuid, current: &str) {
        self.open_project = Some(project_id);
        self.editing_setup_script = Some(SetupScriptModal::new(project_id, current));
    }
}

/// Truncate a string to at most `max` chars, appending an ellipsis if cut. (Local copy so the
/// projects UI doesn't reach into the tasks module for it.)
pub(super) fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_owned();
    }
    let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

/// A muted section heading used inside the project detail columns.
pub(super) fn section_label(ui: &mut egui::Ui, text: &str) {
    ui.label(egui::RichText::new(text).strong().size(15.0));
    ui.add_space(6.0);
}

/// A labelled metadata field (muted caption above a value) for the detail page.
pub(super) fn field(ui: &mut egui::Ui, label: &str, value: &str) {
    let muted = crate::ui::theme::palette().muted;
    ui.label(egui::RichText::new(label).color(muted).size(12.0));
    ui.label(value);
    ui.add_space(8.0);
}
