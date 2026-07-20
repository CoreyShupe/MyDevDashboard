//! `profile` feature sub-root: its `Event`, `View`, and `handle()`.

use crate::domain::profile::Profile;
use crate::error::DbError;
use crate::system::Backend;
use crate::system::profile::ProfileService;

use super::bridge::Emitter;
use super::event::UiEvent;

/// Intent for the profile / onboarding feature.
#[derive(Debug, Clone)]
pub enum Event {
    Create { display_name: String },
}

impl Event {
    pub fn create(display_name: String) -> Self {
        Self::Create { display_name }
    }
}

// Lets UI call `bridge.send(profile::Event::create(name))` without naming the root enum.
impl From<Event> for UiEvent {
    fn from(event: Event) -> Self {
        UiEvent::Profile(event)
    }
}

/// The profile feature's slice of the rendered snapshot.
#[derive(Debug, Clone, Default)]
pub struct View {
    pub profile: Option<Profile>,
}

impl View {
    pub async fn load(service: &ProfileService) -> Result<Self, DbError> {
        Ok(Self {
            profile: service.current().await?,
        })
    }

    pub fn is_onboarded(&self) -> bool {
        self.profile.is_some()
    }
}

/// Sub-root dispatch for the profile feature.
pub async fn handle(backend: &Backend, emitter: &Emitter, event: Event) {
    match event {
        Event::Create { display_name } => create(backend, emitter, &display_name).await,
    }
}

async fn create(backend: &Backend, emitter: &Emitter, display_name: &str) {
    // No seeding (AGENTS.md §5): onboarding only writes the profile. The owner lands on an
    // empty board and creates their own stages/tickets via the board's own creation flow.
    match backend.profile.create(display_name).await {
        Ok(_) => emitter.snapshot(backend).await,
        Err(e) => emitter.error(&e),
    }
}
