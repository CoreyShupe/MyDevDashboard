//! Git (and editor-launch) integration for the `projects` feature.
//!
//! This is the ONE place the app shells out to external commands. Two very different failure
//! policies live here, on purpose (AGENTS.md §10):
//!
//! - **Status reads are best-effort.** [`status`] never errors: a non-repo, a missing remote,
//!   or a `git fetch` that can't reach the network simply leaves fields empty/false. A single
//!   broken project must never fail the whole snapshot.
//! - **Explicit actions surface errors.** [`worktree_add`], [`worktree_remove`],
//!   [`run_setup_script`], and [`open_in_vscode`] return a typed [`ProcessError`] the UI shows in
//!   a modal, because the owner asked for the action and needs to know precisely why it didn't
//!   happen.
//!
//! Committing / pushing are deliberately NOT here — the owner runs those by hand so the exact
//! commands are theirs to control (AGENTS.md §10). The ONE exception is a constrained
//! [`pull_rebase`]: a `git pull --rebase origin <branch>` offered only on the shared branches
//! (`main`/`develop`) that are behind — a safe fast-forward the owner shouldn't have to leave the
//! app for. We otherwise only read state and manage worktrees. SSH keys are assumed loaded by the
//! time the app runs.

use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use tokio::process::Command;
use tokio::task::JoinSet;

use crate::domain::projects::GitStatus;
use crate::error::ProcessError;

/// How long to wait for `git fetch` before falling back to local refs (offline / slow remote).
const FETCH_TIMEOUT: Duration = Duration::from_secs(8);

/// Compute the live git status for every `path` concurrently, preserving input order. Used to
/// build the projects grid without serialising N (possibly network-bound) fetches.
pub async fn statuses(paths: Vec<String>) -> Vec<GitStatus> {
    let mut set = JoinSet::new();
    for (index, path) in paths.iter().cloned().enumerate() {
        set.spawn(async move { (index, status(&path).await) });
    }
    let mut out = vec![GitStatus::default(); paths.len()];
    while let Some(joined) = set.join_next().await {
        if let Ok((index, git)) = joined {
            out[index] = git;
        }
    }
    out
}

/// A best-effort, never-failing snapshot of one repository's git state.
///
/// Order matters for cost: bail out immediately if the path isn't a repo, and only attempt the
/// (network) fetch once we know we have a real repo to compare.
pub async fn status(path: &str) -> GitStatus {
    if read(path, &["rev-parse", "--is-inside-work-tree"])
        .await
        .as_deref()
        != Some("true")
    {
        return GitStatus::default(); // is_repo = false; nothing else is meaningful
    }

    let origin_url = read(path, &["remote", "get-url", "origin"]).await;
    // `--abbrev-ref HEAD` returns "HEAD" when detached — treat that as "no branch".
    let branch = read(path, &["rev-parse", "--abbrev-ref", "HEAD"])
        .await
        .filter(|b| b != "HEAD");

    // Fetch first (best effort) so ahead/behind reflect the real remote; fall back silently to
    // the last-known local refs if it times out or fails (offline is normal).
    let fetched = fetch(path).await;

    // `status --porcelain` prints one line per change; empty output (exit 0) means clean. A
    // failed command leaves us unsure, so we conservatively treat it as NOT clean.
    let clean = matches!(read(path, &["status", "--porcelain"]).await, Some(s) if s.is_empty());

    // `A...B --left-right --count` → "<left>\t<right>"; with `@{u}...HEAD` left = behind,
    // right = ahead. The command fails when there's no upstream — then there's nothing to be
    // ahead/behind of.
    let (has_upstream, ahead, behind) = match read(
        path,
        &["rev-list", "--left-right", "--count", "@{upstream}...HEAD"],
    )
    .await
    {
        Some(counts) => {
            let mut it = counts.split_whitespace();
            let behind = it.next().and_then(|n| n.parse().ok()).unwrap_or(0);
            let ahead = it.next().and_then(|n| n.parse().ok()).unwrap_or(0);
            (true, ahead, behind)
        }
        None => (false, 0, 0),
    };

    GitStatus {
        is_repo: true,
        origin_url,
        branch,
        clean,
        has_upstream,
        ahead,
        behind,
        fetched,
        // Stamped by `ProjectService::refresh_statuses` when this lands in the cache — the read
        // itself doesn't know the wall-clock time, and keeping it here avoids a `Utc::now` per repo.
        checked_at: None,
    }
}

/// The currently checked-out branch (`None` when detached / not a repo). A cheap, local read —
/// no `git fetch` — used to gate the one-click pull on the real current branch.
pub async fn current_branch(path: &str) -> Option<String> {
    read(path, &["rev-parse", "--abbrev-ref", "HEAD"])
        .await
        .filter(|b| b != "HEAD")
}

/// Run `git pull --rebase origin <branch>` in `repo`. An explicit, owner-requested action, so it
/// surfaces a typed [`ProcessError`] on failure (unlike best-effort status reads). Only ever
/// invoked for the shared branches gated by [`crate::domain::projects::GitStatus::can_pull`]; a
/// dirty tree or a diverged history makes git refuse, and that refusal surfaces so nothing is
/// silently rewritten (AGENTS.md §10).
pub async fn pull_rebase(repo: &str, branch: &str) -> Result<(), ProcessError> {
    let args = vec![
        "-C".to_owned(),
        repo.to_owned(),
        "pull".to_owned(),
        "--rebase".to_owned(),
        "origin".to_owned(),
        branch.to_owned(),
    ];
    run(&args, "pulling the latest changes").await.map(|_| ())
}

/// Whether a local branch already exists in the repo (drives `-b` vs. plain checkout on add).
pub async fn branch_exists(repo: &str, branch: &str) -> bool {
    read(
        repo,
        &[
            "show-ref",
            "--verify",
            "--quiet",
            &format!("refs/heads/{branch}"),
        ],
    )
    .await
    .is_some()
}

/// Create a worktree at `path`. When `new_branch`, creates `branch` (`-b`); otherwise checks
/// out the existing `branch`. Git creates intermediate directories (`.dev-dash/worktrees/{repo}/…`).
pub async fn worktree_add(
    repo: &str,
    path: &Path,
    branch: &str,
    new_branch: bool,
) -> Result<(), ProcessError> {
    let path = path.to_string_lossy().into_owned();
    let mut args = vec![
        "-C".to_owned(),
        repo.to_owned(),
        "worktree".to_owned(),
        "add".to_owned(),
    ];
    if new_branch {
        args.push("-b".to_owned());
        args.push(branch.to_owned());
        args.push(path);
    } else {
        args.push(path);
        args.push(branch.to_owned());
    }
    run(&args, "adding a worktree").await.map(|_| ())
}

/// Remove a worktree's on-disk folder via git. NOT forced: if the working tree has uncommitted
/// changes git refuses, and that refusal surfaces to the owner (so nothing is silently lost) —
/// they commit by hand, then retry.
pub async fn worktree_remove(repo: &str, path: &Path) -> Result<(), ProcessError> {
    let args = vec![
        "-C".to_owned(),
        repo.to_owned(),
        "worktree".to_owned(),
        "remove".to_owned(),
        path.to_string_lossy().into_owned(),
    ];
    run(&args, "removing a worktree").await.map(|_| ())
}

/// Run a project's **setup script** as bash, with `dir` (the new worktree) as the working
/// directory — the app's convenience for getting a fresh worktree ready (e.g. `bun install`).
/// An explicit, owner-configured action, so a non-zero exit surfaces a typed [`ProcessError`]
/// carrying the script's own stderr (unlike best-effort status reads). Callers only invoke this
/// for a non-empty script; an empty one means "no setup" and never shells out (AGENTS.md §10).
pub async fn run_setup_script(dir: &Path, script: &str) -> Result<(), ProcessError> {
    // Resolve the owner's real login-shell PATH so tools they installed via their shell profile
    // (`bun` in `~/.bun/bin`, `cargo`, Homebrew, pnpm, …) are found. Without this a script launched
    // from a Finder/`.app` process only sees the minimal launchd PATH and fails with
    // "command not found" (see `login_shell_path`). Best-effort: `None` → inherit the app's PATH.
    let shell_path = login_shell_path().await;
    // Log a HEADER before running, so if the script hangs or exits early the run log
    // (`~/.dev-dash/log.txt`, see `crate::logging`) still records exactly what was about to run
    // and where — the prod-debug trail for "did my setup script even start?". The PATH is logged
    // too, since a "command not found" is almost always a PATH problem.
    tracing::info!(
        "setup script starting\n\
         ── setup script ──────────────────────────────────────────────\n\
         dir: {}\n\
         PATH: {}\n\
         {}\n\
         ──────────────────────────────────────────────────────────────",
        dir.display(),
        shell_path
            .as_deref()
            .unwrap_or("(app default — could not resolve the login-shell PATH)"),
        script.trim_end(),
    );
    let mut command = Command::new("bash");
    command.arg("-c").arg(script).current_dir(dir);
    if let Some(path) = &shell_path {
        command.env("PATH", path);
    }
    let output = command
        .output()
        .await
        .map_err(|source| ProcessError::Spawn {
            program: "bash".to_owned(),
            source,
        })?;
    // Capture the full output into the run log regardless of outcome (stdout AND stderr, labelled),
    // so the owner can see what the script actually printed — not just the failure tail.
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let code = output.status.code();
    if output.status.success() {
        tracing::info!(
            "setup script finished (exit {code:?})\n\
             ── setup output ──────────────────────────────────────────────\n\
             [stdout]\n{stdout}\n[stderr]\n{stderr}\n\
             ──────────────────────────────────────────────────────────────",
        );
        Ok(())
    } else {
        tracing::error!(
            "setup script FAILED (exit {code:?})\n\
             ── setup output ──────────────────────────────────────────────\n\
             [stdout]\n{stdout}\n[stderr]\n{stderr}\n\
             ──────────────────────────────────────────────────────────────",
        );
        Err(ProcessError::Exited {
            program: "bash".to_owned(),
            context: "running the project setup script",
            stderr: stderr_of(&output.stderr),
        })
    }
}

/// Open a path in VS Code via the macOS launcher (robust even when the app was started from
/// Finder and `code` isn't on PATH).
pub async fn open_in_vscode(path: &Path) -> Result<(), ProcessError> {
    tracing::info!("opening in VS Code: {}", path.display());
    let output = Command::new("open")
        .arg("-a")
        .arg("Visual Studio Code")
        .arg(path)
        .output()
        .await
        .map_err(|source| ProcessError::Spawn {
            program: "open".to_owned(),
            source,
        })?;
    if output.status.success() {
        Ok(())
    } else {
        Err(ProcessError::Exited {
            program: "open".to_owned(),
            context: "opening VS Code",
            stderr: stderr_of(&output.stderr),
        })
    }
}

/// Resolve the owner's interactive-login-shell `PATH`, so a **setup script** launched from a GUI/
/// Finder process (which starts with only the minimal launchd PATH — `/usr/bin:/bin:…`) can still
/// find tools they installed under `~/.bun/bin`, `~/.cargo/bin`, Homebrew, nvm, pnpm, etc. Those
/// PATH entries live in `~/.zshrc`/`~/.zprofile`, which a plain `bash -c` never sources — so we run
/// the owner's `$SHELL` as a **login + interactive** shell (which does source them) and read back
/// `$PATH`, isolated with a marker so any prompt/rc chatter is ignored. Best-effort: `None` on any
/// failure, and the setup script then just inherits the app's PATH. macOS-only tool, so `$SHELL`
/// (zsh by default) is the right thing to consult; falls back to `/bin/zsh` if it's unset.
async fn login_shell_path() -> Option<String> {
    const MARKER: &str = "__DEVDASH_PATH__=";
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_owned());
    // `-l -i` sources the login + interactive rc files (where the PATH exports live); `-c` runs our
    // probe and exits. stdin is closed so an rc that reads input can't hang us.
    let probe = format!("printf '%s%s' '{MARKER}' \"$PATH\"");
    let output = Command::new(&shell)
        .args(["-l", "-i", "-c", &probe])
        .stdin(Stdio::null())
        .output()
        .await
        .ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Take everything after the marker (rc noise, if any, prints before it), then trim the newline.
    let path = stdout.rsplit_once(MARKER)?.1.trim().to_owned();
    (!path.is_empty()).then_some(path)
}

/// Run `git fetch --quiet` with a timeout. Returns whether it succeeded (so status can note
/// the counts are fresh vs. remote); a timeout or failure is a normal offline outcome, not an
/// error to surface.
async fn fetch(path: &str) -> bool {
    let fut = Command::new("git")
        .args(["-C", path, "fetch", "--quiet"])
        .output();
    let ok =
        matches!(tokio::time::timeout(FETCH_TIMEOUT, fut).await, Ok(Ok(o)) if o.status.success());
    // Debug, not info: a fetch runs per project on every git refresh, so keep it out of the
    // default run log (turn it on with `RUST_LOG=my_dev_dashboard=debug` when diagnosing status).
    tracing::debug!(path, ok, "git fetch");
    ok
}

/// Best-effort `git -C <path> <args>` read: trimmed stdout on success, `None` on spawn error
/// or non-zero exit. The workhorse for status reads that must never fail the snapshot.
async fn read(path: &str, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(args)
        .output()
        .await
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

/// Run a git command that must succeed, mapping spawn/exit failures to a typed [`ProcessError`]
/// carrying git's own stderr so the owner sees exactly what git said. Every such command (the
/// explicit, state-changing ones: pull, worktree add/remove) is logged to the run log — the exact
/// invocation before it runs, and its stderr on failure — so the log reads as a trail of what the
/// app actually ran (AGENTS.md §3).
async fn run(args: &[String], context: &'static str) -> Result<String, ProcessError> {
    let cmd = args.join(" ");
    tracing::info!("git {cmd}  ({context})");
    let output = Command::new("git")
        .args(args)
        .output()
        .await
        .map_err(|source| ProcessError::Spawn {
            program: "git".to_owned(),
            source,
        })?;
    if !output.status.success() {
        let stderr = stderr_of(&output.stderr);
        tracing::warn!("git {cmd} failed ({context}): {stderr}");
        return Err(ProcessError::Exited {
            program: "git".to_owned(),
            context,
            stderr,
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

/// Trimmed stderr, with a friendly stand-in when a command failed but said nothing.
fn stderr_of(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes).trim().to_owned();
    if text.is_empty() {
        "no error output".to_owned()
    } else {
        text
    }
}
