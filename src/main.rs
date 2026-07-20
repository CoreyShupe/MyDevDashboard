//! Mac Dev Dashboard — boot sequence.
//!
//! Wires config -> tokio worker -> egui. Owns nothing but the startup handshake.
//! See AGENTS.md for the architecture and rules.

// A GUI app on macOS: don't spawn a console window in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod config;
mod domain;
mod error;
mod system;
mod ui;

use tracing::{error, info};
use tracing_subscriber::EnvFilter;

use app::{Bridge, repainter_from_ctx};
use config::Config;
use ui::DashboardApp;

fn main() -> eframe::Result<()> {
    init_tracing();

    // Config problems are logged blatantly with a fix hint (AGENTS.md §3). We still open
    // the window so the user gets a visible, actionable error rather than a silent exit.
    let config = match Config::load() {
        Ok(config) => {
            info!(target = %config.redacted_target(), "configuration loaded");
            Some(config)
        }
        Err(e) => {
            error!(error = %e, "configuration error — fix your .env (see .env.example) and restart");
            None
        }
    };

    // The tokio runtime hosts the background worker. It must outlive the UI event loop,
    // so it is kept alive in this scope for the duration of `run_native`.
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            // Unrecoverable boot failure: the async runtime itself could not start.
            error!(error = %e, "fatal: could not create the tokio runtime; the app cannot run");
            std::process::exit(1);
        }
    };
    let handle = runtime.handle().clone();

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Dev Dashboard")
            // Open maximized — the bubbly, spacious design is built for a large canvas.
            .with_maximized(true)
            .with_inner_size([1100.0, 720.0])
            .with_min_inner_size([720.0, 480.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Dev Dashboard",
        native_options,
        Box::new(move |cc| {
            // Install the design system (fonts + theme) before anything renders.
            ui::theme::install(&cc.egui_ctx);

            let repaint = repainter_from_ctx(cc.egui_ctx.clone());

            // If config failed to load, surface it via the worker path using a sentinel
            // config whose connection will fail with a clear, actionable message.
            let config = config.unwrap_or_else(|| Config {
                database_url: "postgres://invalid/unconfigured".to_owned(),
            });

            let bridge = Bridge::spawn(&handle, config, repaint);
            Ok(Box::new(DashboardApp::new(bridge)))
        }),
    )
}

fn init_tracing() {
    // Honor RUST_LOG if set; otherwise a sensible default for a personal tool.
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("my_dev_dashboard=info,warn"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .init();
}
