//! The channel plumbing between UI and worker.
//!
//! - `Bridge`  : the UI-side handle — send `UiEvent`, drain `AppMessage`.
//! - `Emitter` : the worker-side handle — the shared "sub-root" every feature handler
//!   uses to push results back (`snapshot` / `settle` / `send` / `error`).

use std::sync::Arc;
use std::sync::mpsc::{Receiver as StdReceiver, Sender as StdSender, channel as std_channel};

use tokio::runtime::Handle;
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};
use tracing::{error, info};

use crate::config::Config;
use crate::error::{AppError, UserFacingError};
use crate::system::Backend;

use super::event::{AppMessage, UiEvent};
use super::state::ViewData;
use super::worker::Worker;

/// A UI-thread-agnostic "please repaint" signal (constructed from `egui::Context`).
pub type Repainter = Arc<dyn Fn() + Send + Sync>;

/// Worker-side result emitter, shared by every feature handler.
///
/// Feature handlers never touch the UI channel directly; they go through these helpers so
/// snapshot-building, error logging, and repaint nudging stay in one place.
#[derive(Clone)]
pub struct Emitter {
    to_ui: StdSender<AppMessage>,
    repaint: Repainter,
}

impl Emitter {
    pub fn new(to_ui: StdSender<AppMessage>, repaint: Repainter) -> Self {
        Self { to_ui, repaint }
    }

    /// Push a message to the UI and nudge it to repaint.
    pub fn send(&self, msg: AppMessage) {
        // If the UI receiver is gone the app is closing; nothing actionable to do.
        if self.to_ui.send(msg).is_err() {
            info!("worker: UI message channel closed; dropping message");
            return;
        }
        (self.repaint)();
    }

    /// Log an error blatantly to the console AND queue it for the UI modal (AGENTS.md §3).
    pub fn error(&self, err: &AppError) {
        error!(error = %err, "operation failed");
        self.send(AppMessage::Error(UserFacingError::from_app_error(err)));
    }

    /// Build and send a fresh full snapshot (or surface a load error).
    pub async fn snapshot(&self, backend: &Backend) {
        match ViewData::load(backend).await {
            Ok(data) => self.send(AppMessage::Snapshot(data)),
            Err(e) => self.error(&e),
        }
    }

    /// Settle a fallible mutation: reload the snapshot on success, surface on failure.
    pub async fn settle(&self, backend: &Backend, result: Result<(), AppError>) {
        match result {
            Ok(()) => self.snapshot(backend).await,
            Err(e) => self.error(&e),
        }
    }
}

/// The UI-side handle: a sender for intent and a receiver for results.
pub struct Bridge {
    events: UnboundedSender<UiEvent>,
    messages: StdReceiver<AppMessage>,
}

impl Bridge {
    /// Spawn the worker onto the tokio `runtime` and return the UI-side handle.
    ///
    /// The worker connects to the database lazily, so a DB that is down at startup
    /// surfaces as an in-app modal rather than aborting boot.
    pub fn spawn(runtime: &Handle, config: Config, repaint: Repainter) -> Self {
        let (event_tx, event_rx) = unbounded_channel::<UiEvent>();
        let (msg_tx, msg_rx) = std_channel::<AppMessage>();

        let emitter = Emitter::new(msg_tx, repaint);
        let worker = Worker::new(config, emitter);
        runtime.spawn(worker.run(event_rx));

        Self {
            events: event_tx,
            messages: msg_rx,
        }
    }

    /// Emit UI intent to the worker. Accepts any feature `Event` via `Into<UiEvent>`,
    /// so call sites don't spell out the root wrapper. Non-blocking.
    pub fn send(&self, event: impl Into<UiEvent>) {
        if self.events.send(event.into()).is_err() {
            error!("cannot reach the background worker; it has stopped");
        }
    }

    /// Drain all pending worker messages (called each UI frame). Never blocks.
    pub fn drain(&self) -> Vec<AppMessage> {
        self.messages.try_iter().collect()
    }
}

/// Convenience: build a `Repainter` from an egui context. Kept here so `ui/` stays thin.
pub fn repainter_from_ctx(ctx: egui::Context) -> Repainter {
    Arc::new(move || ctx.request_repaint())
}
