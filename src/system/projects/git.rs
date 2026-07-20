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
/// out the existing `branch`. Git creates intermediate directories (`.github/worktrees/…`).
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
    let output = Command::new("bash")
        .arg("-c")
        .arg(script)
        .current_dir(dir)
        .output()
        .await
        .map_err(|source| ProcessError::Spawn {
            program: "bash".to_owned(),
            source,
        })?;
    if output.status.success() {
        Ok(())
    } else {
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

/// Run `git fetch --quiet` with a timeout. Returns whether it succeeded (so status can note
/// the counts are fresh vs. remote); a timeout or failure is a normal offline outcome, not an
/// error to surface.
async fn fetch(path: &str) -> bool {
    let fut = Command::new("git")
        .args(["-C", path, "fetch", "--quiet"])
        .output();
    matches!(tokio::time::timeout(FETCH_TIMEOUT, fut).await, Ok(Ok(o)) if o.status.success())
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
/// carrying git's own stderr so the owner sees exactly what git said.
async fn run(args: &[String], context: &'static str) -> Result<String, ProcessError> {
    let output = Command::new("git")
        .args(args)
        .output()
        .await
        .map_err(|source| ProcessError::Spawn {
            program: "git".to_owned(),
            source,
        })?;
    if !output.status.success() {
        return Err(ProcessError::Exited {
            program: "git".to_owned(),
            context,
            stderr: stderr_of(&output.stderr),
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
