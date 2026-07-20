//! `projects::worktree` part — domain type.

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// A git worktree tied 1:1 to a ticket within a project. Lives on disk at
/// `{repo}/.github/worktrees/{name}`. `branch` is shared across all of a ticket's worktrees.
///
/// A row with `removed_at = None` is LIVE (its folder should exist on disk); a row with
/// `removed_at = Some(_)` is a historical marker — the worktree was removed but the branch
/// name is kept so it can be recreated (AGENTS.md §10).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, FromRow)]
pub struct Worktree {
    pub id: Uuid,
    pub project_id: Uuid,
    pub ticket_id: Uuid,
    /// On-disk folder name (a filesystem-safe rendering of the branch).
    pub name: String,
    /// The git branch this worktree checks out. Shared across a ticket's worktrees.
    pub branch: String,
    /// When the worktree was removed (leaving this row as a marker). `None` = live.
    pub removed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// The fixed sub-path, under a repo, that holds all of its worktrees.
pub const WORKTREES_SUBDIR: &str = ".github/worktrees";

impl Worktree {
    /// Whether this worktree is currently live (folder should exist) vs. a historical marker.
    pub fn is_live(&self) -> bool {
        self.removed_at.is_none()
    }

    /// The on-disk path of this worktree, given its project's repository root:
    /// `{repo}/.github/worktrees/{name}`.
    pub fn path_in(&self, repo_path: impl AsRef<Path>) -> PathBuf {
        worktree_path(repo_path, &self.name)
    }
}

/// Build the on-disk worktree path for a given repo root + worktree name. Pure — no I/O.
pub fn worktree_path(repo_path: impl AsRef<Path>, name: &str) -> PathBuf {
    repo_path.as_ref().join(WORKTREES_SUBDIR).join(name)
}

/// Render a branch name as a filesystem-safe folder name (e.g. `feature/foo` → `feature-foo`).
/// Keeps the worktree directory tidy and free of nested paths.
pub fn folder_name_for_branch(branch: &str) -> String {
    let mapped: String = branch
        .trim()
        .chars()
        .map(|c| match c {
            '/' | '\\' | ' ' | ':' => '-',
            other => other,
        })
        .collect();
    mapped.trim_matches('-').to_owned()
}
