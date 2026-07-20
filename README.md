# Dev Dashboard

**A single, self-owned home for your development work.** One native desktop window that pulls
your tasks, quick notes, todos, and local git repositories into one calm, digestible place —
so the state of your work lives somewhere other than your head. Built in Rust, backed by a
local PostgreSQL database. Your data never leaves your machine.

<p align="center">
  <img src="static/screenshots/tasks/board.png" alt="The Tasks board — a Jira-like board of stages and tickets" width="880">
</p>

> This is deliberately a **personal, self-use** tool, not a team product — no accounts, no
> sync, no cloud. It's tuned for one developer keeping their own work in order.

There's a full [screenshot gallery](static/screenshots/) of most screens. Built with `egui`/`eframe`
(UI), `tokio` (workers), and `sqlx` + PostgreSQL (storage); see [AGENTS.md](AGENTS.md) for the
architecture and the rules every contributor follows.

## Disclaimer

The majority of this project was built with AI, this is a self-use project
to assist in daily life. The bar is not incredibly high here and bugs
are acceptable. Use at your own risk.

## Prerequisites

- **Rust** (2024 edition — Rust 1.85+). Install via [rustup](https://rustup.rs).
- **Docker** — Docker Desktop (macOS/Windows) or Docker Engine (Linux), for the local
  PostgreSQL instance.

> Developed and tested on **macOS**. The app is almost entirely portable Rust, but a few
> touchpoints are hardcoded for macOS today — see [Platform support & porting](#platform-support--porting).

## Quick start

Everything is driven by the **`dev-dash`** CLI — one wrapper for building, starting the database,
launching the app, and taking screenshots.

**1. Put `dev-dash` on your `PATH`.** From the repo root, symlink the wrapper into a bin
directory (it resolves back to the repo, so it works from anywhere):

```bash
ln -sf "$(pwd)/dev-dash" /usr/local/bin/dev-dash   # or: ln -sf "$(pwd)/dev-dash" ~/bin/dev-dash
```

**2. Start the database, then launch the app:**

```bash
dev-dash db up      # start local PostgreSQL (persistent volume; creates .env from the example)
dev-dash            # release-build + launch detached; migrations run automatically at startup
```

That's it — **running `dev-dash` with no arguments is the recommended way to launch** (it's
exactly `dev-dash open`). It builds a release binary, launches the app in the background, and
hands your shell back; use `dev-dash open dev` for a debug (`cargo run`) build instead. The
in-app **Restart** button (nav footer) rebuilds and relaunches without you leaving the app.

Not sure what's available? Run **`dev-dash help`** (or `-h` / `--help`) to print the full command
reference with all options and flags.

### `dev-dash` commands

| Command | What it does |
|---------|--------------|
| `dev-dash` *(no args)*    | **Launch the app** — the recommended default; equivalent to `dev-dash open`. |
| `dev-dash open [dev]`     | Launch the app detached. Default = release; `dev` = `cargo run`. **Restart** relaunches. |
| `dev-dash help`           | Print the full command reference (also `-h` / `--help`). |
| `dev-dash build [release]`| Compile (debug by default; `release` for optimized). |
| `dev-dash bundle [copy]`  | Build a double-clickable macOS `.app` at `builds/macos/DevDashboard.app`; `copy` also installs it to `/Applications` (see below). |
| `dev-dash db up`          | Start local PostgreSQL (persistent volume; creates `.env` from the example if missing). |
| `dev-dash db down`        | Stop the database. **Data is preserved.** |
| `dev-dash db psql`        | Open a `psql` shell against the running database. |
| `dev-dash db reset`       | **DESTRUCTIVE** — wipe the volume + all data, then bring a fresh DB back up (confirmation-gated). |
| `dev-dash sandbox <cmd>`  | Isolated sandbox DB (port 5434) for verifying migrations — never touches production. See [AGENTS.md](AGENTS.md) §12. |
| `dev-dash shot VIEW OUT`  | Screenshot a `DEV_VIEW` mock screen to `OUT.png` (no database needed). |
| `dev-dash snap [OUT]`     | Screenshot the already-running app (your real data) without building or closing it. |

On first launch you'll land on the onboarding screen to create your first **profile**, then drop
into the dashboard with a left side-nav — **Tasks**, **Notes**, **Todos**, **Projects** — and an
empty workspace. See [What it does](#what-it-does) above for a tour of each, or browse the full
[screenshot gallery](static/screenshots/).

### A double-clickable app (`dev-dash bundle`)

Prefer launching from the Dock/Finder over the terminal? `dev-dash bundle` release-builds and
assembles a macOS app bundle at **`builds/macos/DevDashboard.app`** — `open` it or drag it to your
Dock. Add **`copy`** to also install it into `/Applications` so Spotlight and Launchpad find it:

```bash
dev-dash bundle          # build into builds/macos/DevDashboard.app
open builds/macos/DevDashboard.app

dev-dash bundle copy     # build + install into /Applications (find "Dev Dashboard" in Spotlight)
```

It's a **thin wrapper around your local build**, not a shippable app: the bundle's executable is a
*symlink* into `target/release/`, and it carries a *copy* of your `.env` (its launcher `cd`s into
the bundle so config loads correctly). So it works as long as this repo stays in place; re-run
`dev-dash bundle` after changing `.env`, and any later `cargo build --release` is picked up
automatically. The in-app **Restart** button relaunches it, just like `dev-dash open`.

The icon is drawn from the app's own design system (a teal tile + the "dashboard" glyph) — the
source is `static/assets/icon/AppIcon.svg`; edit it and re-run `static/scripts/icon-gen.sh` to
rebuild the assets. The **running app** embeds the icon and hands it to egui, so it shows on
*every* launch path (bundle, `dev-dash open`, `cargo run`) — not just the bundle. (egui otherwise
stamps its own default icon over the Dock on macOS.)

## The database & persistence (important)

The dashboard stores everything in PostgreSQL running in Docker. Data is kept in a
**named external volume** called `my-dev-dash-pgdata`.

### Why your data survives `docker system prune`

`docker system prune` (and even `prune --volumes`) only removes **anonymous / dangling**
volumes. Because [`docker-compose.yml`](static/docker/docker-compose.yml) declares the volume as
`external: true` with a fixed `name`, it is:

- created explicitly, once, by `./static/scripts/db-up.sh` (never auto-created anonymously), and
- never targeted by prune, because it is a named volume that is still referenced.

So you can `docker compose down`, recreate the container, upgrade the image, or run a
routine `docker system prune` — your tickets and notes stay put. The **only** command that
deletes the data is the explicit, confirmation-gated `dev-dash db reset`.


### Configuration

Everything is driven by `.env` (git-ignored). See [`.env.example`](.env.example):

- `POSTGRES_USER` / `POSTGRES_PASSWORD` / `POSTGRES_DB` — credentials for the container.
- `POSTGRES_PORT` — host port (default `5433`, to avoid clashing with a system postgres).
- `DATABASE_URL` — connection string the app uses; **must match** the values above.
- `RUST_LOG` — log verbosity (`tracing-subscriber` `EnvFilter` syntax).

## Development

```bash
cargo fmt          # format
cargo clippy       # lint (keep it clean)
dev-dash build     # compile (builds WITHOUT a running database — runtime-checked queries)
dev-dash open       # run detached; the in-app "Restart" button then rebuilds + relaunches
dev-dash open dev  # same, but a debug `cargo run` build
```

The **Restart** button (nav footer, under Refresh) exits with a sentinel code that
`dev-dash open` catches to rebuild and relaunch (prod) or re-run (dev) — handy for picking up
code changes without leaving the app.

### Sandbox database & migrations

Because this holds real data, migrations are verified against an **isolated sandbox** database —
never your production one. The sandbox is a fully separate Docker stack
([`docker-compose.sandbox.yml`](static/docker/docker-compose.sandbox.yml) + [`.env.sandbox`](.env.sandbox)):
its own project/container/volume and **host port 5434** (production stays on 5433), so it can't
collide with or touch your data.

```bash
dev-dash sandbox up        # start the sandbox DB (port 5434)
dev-dash sandbox migrate   # build + apply migrations headlessly against the sandbox, then exit
dev-dash sandbox psql      # psql shell into the sandbox
dev-dash sandbox reset     # wipe + recreate the sandbox (throwaway)
dev-dash sandbox down      # stop it
```

`dev-dash sandbox migrate` runs the app's real migration path (via the `DEVDASH_MIGRATE_CHECK`
env gate in `main.rs`) — connect, migrate, log, exit — with no window. See
[AGENTS.md](AGENTS.md) §12 for the full data-safety rules (destructive migrations require
explicit sign-off; agents may not run `dev-dash db …` against production).

## Platform support & porting

The stack is cross-platform — `egui`/`eframe`, `tokio`, `sqlx`, and Docker/PostgreSQL all run on
macOS, Linux, and Windows — and the application is **almost entirely portable Rust**. It's only
developed and tested on macOS, so a handful of touchpoints are hardcoded for it today. If you
want to run it elsewhere, these are the exact spots to generalize (nothing else in `src/` is
OS-specific):

**In the app (`src/`) — one OS-specific call:**

| What | Where | macOS today | Elsewhere |
|------|-------|-------------|-----------|
| Open a worktree in VS Code | [`src/system/projects/git.rs`](src/system/projects/git.rs) → `open_in_vscode()` | `open -a "Visual Studio Code" <path>` | `code <path>`, or `xdg-open`/`start <path>` |
| Run a project's setup script | [`src/system/projects/git.rs`](src/system/projects/git.rs) → `run_setup_script()` | `bash -c <script>` in the new worktree | any POSIX shell; on Windows swap for `cmd`/`pwsh` (or keep `bash` via Git-Bash/WSL) |

A **project's setup script** is an optional bash snippet run in each newly-created worktree
(e.g. `bun install`) so a fresh checkout is ready to work in — edit it from the project detail
page. It runs off the UI thread with a loading indicator, and a failing script is surfaced in a
modal but doesn't undo the worktree.

Everything else in the app is already portable: all **git** operations shell out to `git`
itself; the DB, UI, and workers are `sqlx`/`egui`/`tokio`; the **Add project** folder picker uses
`rfd`, which draws native dialogs on every platform; the Nunito font is embedded; and the
`{repo-parent}/.dev-dash/worktrees/{repo}/{branch}` layout is a plain path. A failed "Open in VS Code" or setup
script is already handled as a best-effort/surfaced `ProcessError` (it shows, it doesn't crash),
so the app is usable on other platforms even before you touch those calls.

**In the `dev-dash` tooling — the screenshot helpers:**

- `dev-dash build`, `db …`, `sandbox …`, and `open` are portable shell + `cargo` + Docker — they
  work anywhere the prerequisites do.
- `dev-dash shot` and `dev-dash snap` are **macOS-only**: a small `static/scripts/window-id.swift`
  CoreGraphics helper finds the app window's id **by title** (a `DEV_VIEW` mock is titled
  `Dev Dashboard [DEV: …]`, the live app plain `Dev Dashboard`, so a mock `shot` and a live `snap`
  never grab each other's window), and `screencapture -o -l` captures just that window — never the
  menu bar or dock. Because `-l <id>` grabs the window's own image even when it's occluded, no
  window-raising is needed, so these need only **Screen Recording** permission (not Accessibility).
  `pkill`/`perl` handle process + timing control. A port would swap these for the platform's
  equivalents (e.g. `wmctrl`/`xdotool` + `import`/`grim` on Linux).
- `dev-dash bundle` is **macOS-only**: it produces a `.app` bundle (`Info.plist` + `Contents/`),
  uses BSD `sed -i ''`, and its icon (`static/scripts/icon-gen.sh`) is built with the macOS
  `sips`/`iconutil` toolchain. A port would emit the platform's launcher + icon format
  instead (e.g. a `.desktop` file + PNGs on Linux) — the build + symlink + `.env`-copy logic and
  the `AppIcon.svg` source are otherwise portable.

## Troubleshooting

- **"cannot reach PostgreSQL …"** on startup — the DB isn't running. Run `dev-dash db up`.
- **Port already in use** — change `POSTGRES_PORT` in `.env` (and `DATABASE_URL` to match).
- **Docker daemon not running** — start Docker Desktop, then retry.
