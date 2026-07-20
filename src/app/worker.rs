//! The tokio worker: the ROOT dispatcher.
//!
//! It owns the DB-backed `Backend` (connected lazily) and routes each `UiEvent` to the
//! owning feature's `handle()`. There is deliberately NO per-action logic here — that
//! lives in the feature sub-roots (`app/<feature>/`). This is the top of the
//! `root { sub-root -> feature }` pattern (AGENTS.md §2).

use tokio::sync::mpsc::UnboundedReceiver;
use tracing::info;

use crate::config::Config;
use crate::error::AppError;
use crate::system::{Backend, db};

use super::bridge::Emitter;
use super::event::UiEvent;
use super::{notes, profile, tasks};

pub struct Worker {
    config: Config,
    backend: Option<Backend>,
    emitter: Emitter,
}

impl Worker {
    pub fn new(config: Config, emitter: Emitter) -> Self {
        Self {
            config,
            backend: None,
            emitter,
        }
    }

    /// Run until the UI drops the event sender (app shutdown).
    pub async fn run(mut self, mut rx: UnboundedReceiver<UiEvent>) {
        // Initial snapshot so the UI knows whether to show onboarding.
        self.refresh().await;

        while let Some(event) = rx.recv().await {
            self.handle(event).await;
        }
        info!("worker shutting down: UI event channel closed");
    }

    /// Connect + migrate on first use; reuse the pool thereafter.
    async fn ensure_backend(&mut self) -> Result<Backend, AppError> {
        if let Some(backend) = &self.backend {
            return Ok(backend.clone());
        }
        let pool = db::connect_and_migrate(&self.config).await?;
        let backend = Backend::new(pool);
        self.backend = Some(backend.clone());
        info!("connected to database and applied migrations");
        Ok(backend)
    }

    /// Ensure the connection, then push a fresh snapshot (or surface a connection error).
    async fn refresh(&mut self) {
        match self.ensure_backend().await {
            Ok(backend) => self.emitter.snapshot(&backend).await,
            Err(e) => self.emitter.error(&e),
        }
    }

    /// ROOT dispatch: hand the event to the owning feature. Keep this tiny.
    async fn handle(&mut self, event: UiEvent) {
        let backend = match self.ensure_backend().await {
            Ok(backend) => backend,
            Err(e) => {
                self.emitter.error(&e);
                return;
            }
        };

        match event {
            UiEvent::ReloadAll => self.emitter.snapshot(&backend).await,
            UiEvent::Profile(event) => profile::handle(&backend, &self.emitter, event).await,
            UiEvent::Tasks(event) => tasks::handle(&backend, &self.emitter, event).await,
            UiEvent::Notes(event) => notes::handle(&backend, &self.emitter, event).await,
        }
    }
}
