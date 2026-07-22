//! `projects::project` part — domain types.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// A local repository the owner works in. `path` is an existing directory on disk — this
/// tool points at repos, it never clones them. Git facts (origin/branch/sync) are read live
/// (see [`GitStatus`]), never persisted, so a card always reflects the real repo state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, FromRow)]
pub struct Project {
    pub id: Uuid,
    pub profile_id: Uuid,
    pub name: String,
    /// Absolute path to the repository root on disk.
    pub path: String,
    /// A bash script run inside each freshly-created worktree (e.g. `bun install`) so a new
    /// worktree is ready to work in (AGENTS.md §10). Empty = no setup script.
    pub setup_script: String,
    /// A bash script run inside each worktree right before it is removed (e.g. `docker compose
    /// down`) so removal tears down whatever setup stood up (AGENTS.md §10). Empty = no teardown.
    pub teardown_script: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A read-only snapshot of a repository's git state, computed off-thread. Cached for the
/// session and refreshed on open + on demand (AGENTS.md §10) — `checked_at` records when. All
/// fields degrade gracefully: a non-repo, a missing remote, or a failed `git fetch` simply
/// leaves the relevant field empty/false rather than erroring the load.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitStatus {
    /// Whether `path` is actually a git repository. When false, the rest is meaningless.
    pub is_repo: bool,
    /// `origin` remote URL, if the repo has one.
    pub origin_url: Option<String>,
    /// Current checked-out branch (or `None` when detached / not a repo).
    pub branch: Option<String>,
    /// Whether the working tree is clean (no uncommitted changes).
    pub clean: bool,
    /// Whether the current branch tracks an upstream (so ahead/behind are meaningful).
    pub has_upstream: bool,
    /// Commits the local branch is ahead of / behind its upstream.
    pub ahead: u32,
    pub behind: u32,
    /// Whether the ahead/behind counts reflect a successful `git fetch` (fresh vs. remote) or
    /// fell back to the last-known local refs (offline / fetch failed).
    pub fetched: bool,
    /// When this status was last refreshed (a `git` read ran). `None` means it hasn't been
    /// checked yet this session — the card shows "checking…" until the first refresh lands.
    pub checked_at: Option<DateTime<Utc>>,
}

impl GitStatus {
    /// The shared integration branches on which the app offers a one-click **Pull**. Everything
    /// else (feature branches) stays the owner's to drive by hand (AGENTS.md §10).
    pub const PULLABLE_BRANCHES: [&'static str; 2] = ["main", "develop"];

    /// "Up to date" = a real repo, a clean working tree, and in sync with the upstream (or no
    /// upstream to compare against). This is what the card's up-to-date badge reflects.
    pub fn up_to_date(&self) -> bool {
        self.is_repo && self.clean && self.ahead == 0 && self.behind == 0
    }

    /// Whether `branch` is one of the shared branches a one-click Pull is offered on.
    pub fn is_pullable_branch(branch: &str) -> bool {
        Self::PULLABLE_BRANCHES.contains(&branch)
    }

    /// Whether to offer the one-click **Pull** action (`git pull --rebase origin <branch>`): a
    /// real repo on a shared branch (`main`/`develop`) that tracks an upstream, is behind it, and
    /// has a clean working tree so the rebase won't be refused. Feature branches, dirty trees, or
    /// nothing-to-pull all fall outside this — the owner drives those by hand (AGENTS.md §10).
    pub fn can_pull(&self) -> bool {
        self.is_repo
            && self.clean
            && self.has_upstream
            && self.behind > 0
            && self.branch.as_deref().is_some_and(Self::is_pullable_branch)
    }
}
