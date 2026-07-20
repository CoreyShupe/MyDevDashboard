//! `projects` feature — domain types, composed of parts (AGENTS.md §2).
//!
//! Each part (`project`, `worktree`) owns its type here and is mirrored in
//! `system/projects/`, `app/projects/`, and `ui/projects/`.

pub mod project;
pub mod worktree;

pub use project::{GitStatus, Project};
pub use worktree::{Worktree, WorktreeBusy};
