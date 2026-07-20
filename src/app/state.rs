//! The ROOT immutable snapshot the UI renders, composed of each feature's `View`.
//!
//! The UI treats this as read-only: it renders it and emits `UiEvent`s to request
//! changes. The worker replaces it wholesale on the next `Snapshot`.

use crate::error::AppError;
use crate::system::Backend;

use super::{profile, tasks};

/// Aggregate view state. Add a field here when you add a feature with a `View`.
#[derive(Debug, Clone, Default)]
pub struct ViewData {
    pub profile: profile::View,
    pub tasks: tasks::View,
}

impl ViewData {
    /// Build a full snapshot by asking each feature to load its slice.
    pub async fn load(backend: &Backend) -> Result<Self, AppError> {
        let profile = profile::View::load(&backend.profile).await?;
        // Only load the board once onboarding is done; keeps first-run cheap.
        let tasks = if profile.is_onboarded() {
            tasks::View::load(&backend.tasks).await?
        } else {
            tasks::View::default()
        };
        Ok(Self { profile, tasks })
    }

    /// Whether onboarding has been completed.
    pub fn has_profile(&self) -> bool {
        self.profile.is_onboarded()
    }
}
