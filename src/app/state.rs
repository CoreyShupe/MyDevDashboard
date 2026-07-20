//! The ROOT immutable snapshot the UI renders, composed of each feature's `View`.
//!
//! The UI treats this as read-only: it renders it and emits `UiEvent`s to request
//! changes. The worker replaces it wholesale on the next `Snapshot`.

use crate::error::AppError;
use crate::system::Backend;

use super::{notes, profile, projects, tasks, todos};

/// Aggregate view state. Add a field here when you add a feature with a `View`.
#[derive(Debug, Clone, Default)]
pub struct ViewData {
    pub profile: profile::View,
    pub tasks: tasks::View,
    pub notes: notes::View,
    pub projects: projects::View,
    pub todos: todos::View,
}

impl ViewData {
    /// Build a full snapshot by asking each feature to load its slice.
    pub async fn load(backend: &Backend) -> Result<Self, AppError> {
        let profile = profile::View::load(&backend.profile).await?;
        // Everything is scoped to the active profile (AGENTS.md §9). Load the board + notes for
        // it; on first run (no active profile) there's nothing to load.
        let (tasks, notes, projects, todos) = if let Some(profile_id) = profile.active_id() {
            (
                tasks::View::load(&backend.tasks, profile_id).await?,
                notes::View::load(&backend.notes, profile_id).await?,
                projects::View::load(&backend.projects, profile_id).await?,
                todos::View::load(&backend.todos, profile_id).await?,
            )
        } else {
            (
                tasks::View::default(),
                notes::View::default(),
                projects::View::default(),
                todos::View::default(),
            )
        };
        Ok(Self {
            profile,
            tasks,
            notes,
            projects,
            todos,
        })
    }

    /// Whether onboarding has been completed.
    pub fn has_profile(&self) -> bool {
        self.profile.is_onboarded()
    }
}
