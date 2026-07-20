//! Run logging (AGENTS.md §3).
//!
//! Writes the current run's logs — app diagnostics AND captured subprocess output such as a
//! project's **setup script** — to a master file at `~/.dev-dash/log.txt`, in addition to the
//! console. The file is TRUNCATED at startup, so it always holds THIS run only: a quick, single
//! place to answer "what just happened" (e.g. exactly what a setup script ran and printed). This
//! ships in production on purpose — in a release build the app has no console (`windows_subsystem
//! = "windows"`), so the file is the only durable trail for debugging a real run.

use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use tracing_subscriber::EnvFilter;
use tracing_subscriber::prelude::*;

/// A cloneable `io::Write` over ONE shared file handle. The `fmt` layer asks for a fresh writer
/// per event (via a `Fn() -> W` `MakeWriter`), so every event writes to the same file, serialized
/// by the mutex. A poisoned lock drops the line rather than panicking — logging must never crash
/// the app (AGENTS.md §3).
#[derive(Clone)]
struct SharedFile(Arc<Mutex<fs::File>>);

impl Write for SharedFile {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self.0.lock() {
            Ok(mut f) => f.write(buf),
            Err(_) => Ok(buf.len()), // poisoned: pretend-consume rather than error the logger
        }
    }
    fn flush(&mut self) -> std::io::Result<()> {
        match self.0.lock() {
            Ok(mut f) => f.flush(),
            Err(_) => Ok(()),
        }
    }
}

/// `~/.dev-dash/log.txt`, or `None` if `$HOME` is unset.
fn log_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join(".dev-dash").join("log.txt"))
}

/// Install tracing: always to the console; additionally to `~/.dev-dash/log.txt` (truncated to
/// this run) when it can be opened. Any file error degrades to console-only with an `eprintln!`
/// warning — logging setup must never abort boot.
pub fn init() {
    // Honor RUST_LOG if set; otherwise a sensible default for a personal tool.
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("my_dev_dashboard=info,warn"));

    let console = tracing_subscriber::fmt::layer().with_target(false);

    // `Option<Layer>` is itself a `Layer` (a no-op when `None`), so the file layer simply drops
    // out if the log file can't be opened.
    let file = open_log_file().map(|shared| {
        tracing_subscriber::fmt::layer()
            .with_target(false)
            .with_ansi(false) // no ANSI colour codes in the file
            .with_writer(move || shared.clone())
    });

    tracing_subscriber::registry()
        .with(filter)
        .with(console)
        .with(file)
        .init();
}

/// Create `~/.dev-dash/`, (re)create the truncated log file, and write a run banner at the top.
/// Returns the shared handle for the tracing file layer, or `None` on any I/O error.
fn open_log_file() -> Option<SharedFile> {
    let path = log_path()?;
    if let Some(dir) = path.parent()
        && let Err(e) = fs::create_dir_all(dir)
    {
        eprintln!("dev-dash: could not create log dir {}: {e}", dir.display());
        return None;
    }
    let mut file = match fs::File::create(&path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("dev-dash: could not open log file {}: {e}", path.display());
            return None;
        }
    };
    // Best-effort run banner at the top of the file; a write error here isn't fatal.
    let _ = writeln!(
        file,
        "======================================================================\n\
         dev-dash run — {} — v{} — pid {}\n\
         ======================================================================",
        chrono::Utc::now().to_rfc3339(),
        env!("CARGO_PKG_VERSION"),
        std::process::id(),
    );
    Some(SharedFile(Arc::new(Mutex::new(file))))
}
