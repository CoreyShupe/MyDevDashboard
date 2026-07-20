//! `profile` feature sub-root: its `Event`, `View`, and `handle()`.
//!
//! Profiles are the top-level containers (AGENTS.md §9). This sub-root also exposes
//! [`active_id`], the shared way other features resolve "which profile am I acting in" — the
//! stage/notes create handlers call it so the UI never has to thread a profile id through
//! every event.

use uuid::Uuid;

use crate::domain::profile::{Profile, ProfileView};
use crate::error::{AppError, DbError, ProfileError};
use crate::system::Backend;
use crate::system::profile::ProfileService;

use super::bridge::Emitter;
use super::event::UiEvent;

/// Intent for the profile feature.
#[derive(Debug, Clone)]
pub enum Event {
    /// Create a new profile (also becomes the active one).
    Create { display_name: String },
    /// Switch which profile is active.
    Switch { id: Uuid },
    /// Delete a profile and its entire workspace (cascades). Destructive → the UI confirms first.
    Delete { id: Uuid },
    /// Record the active profile's last-viewed workspace page (§9). A quiet write-through — it
    /// does NOT re-snapshot (the UI already shows the tab), so a tab switch stays cheap.
    SetLastView { view: ProfileView },
}

impl Event {
    pub fn create(display_name: String) -> Self {
        Self::Create { display_name }
    }
    pub fn switch(id: Uuid) -> Self {
        Self::Switch { id }
    }
    pub fn delete(id: Uuid) -> Self {
        Self::Delete { id }
    }
    pub fn set_last_view(view: ProfileView) -> Self {
        Self::SetLastView { view }
    }
}

// Lets UI call `bridge.send(profile::Event::switch(id))` without naming the root enum.
impl From<Event> for UiEvent {
    fn from(event: Event) -> Self {
        UiEvent::Profile(event)
    }
}

/// The profile feature's slice of the rendered snapshot: every profile (for the switcher)
/// plus which one is active (whose workspace is shown).
#[derive(Debug, Clone, Default)]
pub struct View {
    pub profiles: Vec<Profile>,
    pub active: Option<Profile>,
}

impl View {
    pub async fn load(service: &ProfileService) -> Result<Self, DbError> {
        Ok(Self {
            profiles: service.list().await?,
            active: service.active().await?,
        })
    }

    /// Onboarding is complete once at least one profile exists (so one is active).
    pub fn is_onboarded(&self) -> bool {
        self.active.is_some()
    }

    /// The active profile's id, if any.
    pub fn active_id(&self) -> Option<Uuid> {
        self.active.as_ref().map(|p| p.id)
    }
}

/// Resolve the active profile's id, or a typed [`ProfileError::NoActive`]. The shared way for
/// any feature handler to scope a create to the current profile (AGENTS.md §9).
pub async fn active_id(backend: &Backend) -> Result<Uuid, AppError> {
    backend
        .profile
        .active()
        .await?
        .map(|p| p.id)
        .ok_or_else(|| ProfileError::NoActive.into())
}

/// Sub-root dispatch for the profile feature. The create/switch/delete actions settle to a fresh
/// snapshot, which reloads the (now differently-scoped) workspace for the active profile.
/// `SetLastView` is the exception: a quiet write-through with NO snapshot (see below).
pub async fn handle(backend: &Backend, emitter: &Emitter, event: Event) {
    let result = match event {
        // No seeding (AGENTS.md §5): creating a profile writes only the profile (and makes it
        // active). The owner lands on that profile's empty board and builds it up themselves.
        Event::Create { display_name } => backend.profile.create(&display_name).await.map(|_| ()),
        Event::Switch { id } => backend.profile.set_active(id).await,
        // Cascades to the whole workspace; leaves no active profile, so the next snapshot lands
        // the owner on the picker/onboarding (AGENTS.md §9).
        Event::Delete { id } => backend.profile.delete(id).await,
        // Persist where the owner is, scoped to the active profile. Deliberately does NOT emit a
        // snapshot: the UI already reflects the new tab, and re-snapshotting on every tab click
        // would needlessly reload the whole workspace. Only a failure is surfaced.
        Event::SetLastView { view } => {
            let result = match active_id(backend).await {
                Ok(id) => backend.profile.set_last_view(id, view).await,
                Err(e) => Err(e),
            };
            if let Err(e) = result {
                emitter.error(&e);
            }
            return;
        }
    };
    emitter.settle(backend, result).await;
}
