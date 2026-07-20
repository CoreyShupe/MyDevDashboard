//! `projects::worktree` part — domain type.

use std::path::{Component, Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// A git worktree tied 1:1 to a ticket within a project. Lives on disk OUTSIDE the repo, in a
/// dev-dash-managed tree under the repo's parent directory:
/// `{repo-parent}/.dev-dash/worktrees/{repo}/{branch}`. `branch` is shared across all of a
/// ticket's worktrees.
///
/// A row with `removed_at = None` is LIVE (its folder should exist on disk); a row with
/// `removed_at = Some(_)` is a historical marker — the worktree was removed but the branch
/// name is kept so it can be recreated (AGENTS.md §10).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, FromRow)]
pub struct Worktree {
    pub id: Uuid,
    pub project_id: Uuid,
    pub ticket_id: Uuid,
    /// On-disk worktree folder, relative to this repo's worktree root. It IS the branch, so a
    /// slashed branch (`feature/foo`) nests naturally under `.dev-dash/worktrees/{repo}/`.
    pub name: String,
    /// The git branch this worktree checks out. Shared across a ticket's worktrees.
    pub branch: String,
    /// When the worktree was removed (leaving this row as a marker). `None` = live.
    pub removed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// The shared root — under a repo's PARENT directory — holding all of dev-dash's worktrees,
/// grouped by repo name then branch: `{repo-parent}/.dev-dash/worktrees/{repo}/{branch}`. Worktrees
/// live outside the repo (never `.github/…` inside it); dev-dash owns and manages this tree since
/// worktrees are surfaced and driven from the dashboard.
pub const WORKTREES_ROOT: &str = ".dev-dash/worktrees";

impl Worktree {
    /// Whether this worktree is currently live (folder should exist) vs. a historical marker.
    pub fn is_live(&self) -> bool {
        self.removed_at.is_none()
    }

    /// The on-disk path of this worktree, given its project's repository root:
    /// `{repo-parent}/.dev-dash/worktrees/{repo}/{name}`.
    pub fn path_in(&self, repo_path: impl AsRef<Path>) -> PathBuf {
        worktree_path(repo_path, &self.name)
    }
}

/// A slow, in-flight action on an existing worktree, surfaced as a loading state in place of that
/// worktree's action row until it lands (AGENTS.md §10). Both shell out (git / the editor
/// launcher), so a click gets an immediate "waiting for it to happen" indicator rather than a
/// button that looks like nothing happened. (Create/recreate use the separate `creating` guard,
/// keyed by `(project, ticket)` since there's no worktree id yet.)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorktreeBusy {
    /// `git worktree remove` is running (it leaves a marker when it lands).
    Removing,
    /// The editor launcher (`open -a "Visual Studio Code" …`) is starting up.
    Opening,
}

impl WorktreeBusy {
    /// The "waiting" label shown next to the spinner.
    pub fn label(self) -> &'static str {
        match self {
            Self::Removing => "Removing…",
            Self::Opening => "Opening in VS Code…",
        }
    }
}

/// The directory holding all of a single repo's worktrees:
/// `{repo-parent}/.dev-dash/worktrees/{repo}`. Every worktree name resolves as a child of this.
/// Pure — no I/O.
pub fn worktree_root(repo_path: impl AsRef<Path>) -> PathBuf {
    let repo = repo_path.as_ref();
    // The repo's parent directory. If the path has no parent (e.g. "/"), fall back to the repo
    // itself so we still produce a deterministic path without panicking (§3 bans unwrap/expect).
    let base = repo.parent().unwrap_or(repo);
    // The repo's own directory name groups its worktrees; fall back to the whole path if there's
    // somehow no final component.
    let repo_name = repo.file_name().unwrap_or(repo.as_os_str());
    base.join(WORKTREES_ROOT).join(repo_name)
}

/// Build the on-disk worktree path for a repo root + worktree name (its branch). Pure — no I/O.
/// Worktrees live OUTSIDE the repo, in a dev-dash-managed tree under the repo's PARENT directory:
/// `{repo-parent}/.dev-dash/worktrees/{repo}/{name}`. `name` is the branch, so a slashed branch
/// (`feature/foo`) nests naturally.
///
/// This is the raw builder used for DISPLAY and for operating on already-validated stored names.
/// When turning UNTRUSTED input (a freshly-typed branch) into a path, use
/// [`checked_worktree_path`] so a `..` traversal can't escape the worktree root.
pub fn worktree_path(repo_path: impl AsRef<Path>, name: &str) -> PathBuf {
    worktree_root(repo_path).join(name)
}

/// Build the worktree path AND verify the name stays put: the lexically-resolved path must be
/// exactly `{worktree_root}/{name}` — no `..`/`.` climbing out of (or rewriting) the root, no
/// absolute component replacing it. Returns the path when safe, or `None` when the resolved path
/// differs from the assumed one so the caller can refuse it (`ProjectError::InvalidBranch`).
/// Pure — resolves `.`/`..` LEXICALLY only, never touching the filesystem (no symlink following,
/// no existence requirement), matching this module's no-I/O contract.
pub fn checked_worktree_path(repo_path: impl AsRef<Path>, name: &str) -> Option<PathBuf> {
    let root = worktree_root(&repo_path);
    let full = root.join(name);
    // Where the name ACTUALLY points once `.`/`..` are resolved vs. where we assumed it would.
    let resolved = resolve_lexically(&full);
    let rel = resolved.strip_prefix(resolve_lexically(&root)).ok()?;
    // Must land exactly at the assumed sub-path — anything else (escaped, or internally rewritten
    // by `..`) is rejected.
    (rel == Path::new(name)).then_some(full)
}

/// Resolve `.`/`..` in a path purely lexically — no filesystem access, no symlink resolution, no
/// requirement that the path exist. A `..` pops a preceding normal component; a leading `..` that
/// would climb above the root is KEPT (as `..`) so a prefix check against the root will fail.
fn resolve_lexically(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::CurDir => {}
            Component::ParentDir => {
                if matches!(out.components().next_back(), Some(Component::Normal(_))) {
                    out.pop();
                } else {
                    out.push("..");
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}
