//! `projects` feature — business logic, composed of one service per part (AGENTS.md §2).
//!
//! `ProjectsService` is a thin aggregate: `backend.projects.project`, `.worktree`. The shared
//! `git` module (external-command integration) sits alongside them and both parts use it.

pub mod git;
pub mod project;
pub mod worktree;

use sqlx::postgres::PgPool;

use project::ProjectService;
use worktree::WorktreeService;

/// Aggregate of the projects feature's part-services, sharing one pool.
#[derive(Clone)]
pub struct ProjectsService {
    pub project: ProjectService,
    pub worktree: WorktreeService,
}

impl ProjectsService {
    pub fn new(pool: PgPool) -> Self {
        Self {
            project: ProjectService::new(pool.clone()),
            worktree: WorktreeService::new(pool),
        }
    }
}
