//! Dev Dashboard — boot sequence.
//!
//! Wires config -> tokio worker -> egui. Owns nothing but the startup handshake.
//! See AGENTS.md for the architecture and rules.

// A GUI app on macOS: don't spawn a console window in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod config;
mod domain;
mod error;
mod logging;
mod system;
mod ui;

use tracing::{error, info, warn};

use app::{Bridge, repainter_from_ctx};
use config::Config;
use ui::DashboardApp;

/// Process exit code the app returns when the owner clicks "Restart". `dev-dash open` watches
/// for exactly this code and rebuilds+relaunches instead of staying down; any other exit code
/// (incl. a normal window close = 0) ends the loop.
///
/// 86 is chosen to sit clear of every conventional band so it can't be confused with a real
/// exit: not 0–2 (success / general error / shell misuse), not 64–78 (BSD `sysexits.h`), not
/// 101 (Rust panic), not 128+N (killed by signal N). The "86 it" = eject/restart slang is the
/// mnemonic. Keep this in sync with `restart_code` in `dev-dash`.
pub const RESTART_EXIT_CODE: i32 = 86;

fn main() -> eframe::Result<()> {
    logging::init();

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

    // Headless migration check (AGENTS.md §12): connect to `DATABASE_URL`, apply migrations,
    // log the outcome, and EXIT — no window. Gated by `DEVDASH_MIGRATE_CHECK`, so a normal run
    // is unaffected. `static/scripts/sandbox-db.sh migrate` uses this to verify migrations against the
    // isolated sandbox DB (never production).
    if std::env::var_os("DEVDASH_MIGRATE_CHECK").is_some() {
        migrate_check(config);
    }

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

    // A DEV_VIEW run shows a mock (never the owner's real data), so give its window a distinct
    // title. `dev-dash shot` (mock) and `dev-dash snap` (the live app) match on this title via
    // `static/scripts/window-id.swift`, so a capture can never grab the wrong window — a mock shot
    // can't leak the live app's data, and a live snap can't grab a stray mock (opsec, AGENTS §8).
    let window_title = match std::env::var("DEV_VIEW") {
        Ok(view) if !view.trim().is_empty() => format!("Dev Dashboard [DEV: {}]", view.trim()),
        _ => "Dev Dashboard".to_owned(),
    };

    // Own our Dock icon on EVERY launch path (bundle, `cargo run`, `dev-dash open`). Without an
    // icon, eframe loads a default egui icon and, on macOS, applies it via `setApplicationIconImage`
    // at runtime — which overrides even a bundle's `.icns`. So we embed our real icon and hand it
    // to eframe. On a decode failure we fall back to an empty `IconData`, which eframe treats as
    // "no icon" (leaving the OS/bundle default) rather than forcing its own. Uses only eframe's
    // public API — `image` is eframe's own dependency, so no new crate (AGENTS.md §1/§14).
    let icon = match eframe::icon_data::from_png_bytes(include_bytes!(
        "../static/assets/icon/AppIcon-512.png"
    )) {
        Ok(icon) => icon,
        Err(e) => {
            warn!(error = %e, "could not decode the embedded app icon; falling back to default");
            egui::IconData::default()
        }
    };

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(&window_title)
            .with_icon(icon)
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

/// Apply migrations against `DATABASE_URL` headlessly, then exit — the `DEVDASH_MIGRATE_CHECK`
/// path (AGENTS.md §12). Exits 0 on success, 1 if config is missing or migration fails. Never
/// opens a window. Uses its own single-thread runtime so it doesn't depend on the GUI runtime.
fn migrate_check(config: Option<Config>) -> ! {
    let Some(config) = config else {
        error!("migrate-check: no valid configuration (check DATABASE_URL); aborting");
        std::process::exit(1);
    };
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build tokio runtime for migrate-check");
    let code = runtime.block_on(async {
        match system::db::connect_and_migrate(&config).await {
            Ok(_) => {
                info!(target = %config.redacted_target(), "migrate-check: migrations applied cleanly");
                0
            }
            Err(e) => {
                error!(error = %e, "migrate-check: FAILED");
                1
            }
        }
    });
    std::process::exit(code);
}
