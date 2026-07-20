# Dev Dashboard

A self-use macOS developer dashboard, written in Rust. One place to manage your dev
work in a digestible way — starting with a configurable, Jira-like **Tasks** board.

Built with `egui`/`eframe` (UI), `tokio` (workers), `sqlx` + PostgreSQL (storage).
See [AGENTS.md](AGENTS.md) for the architecture and the rules every contributor follows.

---

## Prerequisites

- **Rust** (2024 edition — Rust 1.85+). Install via [rustup](https://rustup.rs).
- **Docker Desktop for macOS** (for the local PostgreSQL instance).

---

## Quick start

```bash
# 1. Configure environment (creates .env from the example on first db-up).
cp .env.example .env          # optional — db-up.sh does this for you if missing

# 2. Start a local PostgreSQL with a PERSISTENT volume (see below).
./scripts/db-up.sh

# 3. Build & run the dashboard. Migrations run automatically at startup.
cargo run
```

On first launch you'll land on the onboarding screen to create your first **profile**. Then
you'll drop into the dashboard with a left side-nav (**Tasks**, **Notes**, **Todos**,
**Projects**) and an empty workspace.

- **Profiles** — self-contained workspaces you switch between from the switcher at the top of
  the nav. Each profile has its **own** stages, tickets, and notes — they never mix. "New
  profile" (in the switcher) walks you through creating another and switches you into it.
- **Tasks** — a configurable, Jira-like board of stages and tickets (with notes and
  parent/child relationships), scoped to the active profile. Drag a stage's grip to reorder
  columns; edit a stage to mark it **terminal** (an end state like "Complete"/"Cancelled") —
  terminal columns collapse to a ticket count with a "View tickets" toggle and are excluded
  from the Notes "Add to ticket" picker.
- **Notes** — a fast, list-like scratchpad for *uncategorized* notes. Jot a note at the top
  (Enter or **Add**), then file it later: **Create Ticket** turns a note into a new ticket
  (pre-filled as its first note), **Make Todo** turns it into a quick todo, or **Add To Ticket**
  searches your tickets by title and attaches the note to the one you pick. Filing a note removes
  it from the list.
- **Todos** — a fast, list-like scratchpad for quick tasks you just need to remember, without
  the ceremony of a ticket. Jot one at the top (Enter or **Add**); check its box to complete it
  (**completed todos are hidden**), or delete it. Works just like Notes.
- **Projects** — local repositories you already have on disk (this never clones — you paste a
  path). Each project is a card showing its name, origin URL, path, current branch, a live
  up-to-date badge, and its worktree count. Open a project for a full page with its metadata and
  its git **worktrees** (at `{repo}/.github/worktrees/{name}`), each openable in VS Code. A
  worktree is created from a **ticket** — "Create worktree" on a ticket picks a project and a
  branch; a ticket keeps the **same branch across every project** it has a worktree in, so you
  can work the same change in parallel across repos. Removing a worktree cleans its folder but
  keeps a marker so you can recreate it later. You run all commits/pushes/pulls yourself.

---

## The database & persistence (important)

The dashboard stores everything in PostgreSQL running in Docker. Data is kept in a
**named external volume** called `my-dev-dash-pgdata`.

### Why your data survives `docker system prune`

`docker system prune` (and even `prune --volumes`) only removes **anonymous / dangling**
volumes. Because [`docker-compose.yml`](docker-compose.yml) declares the volume as
`external: true` with a fixed `name`, it is:

- created explicitly, once, by `./scripts/db-up.sh` (never auto-created anonymously), and
- never targeted by prune, because it is a named volume that is still referenced.

So you can `docker compose down`, recreate the container, upgrade the image, or run a
routine `docker system prune` — your tickets and notes stay put. The **only** command that
deletes the data is the explicit, confirmation-gated `./scripts/db-reset.sh`.

### Helper scripts

| Script (or `dev-dash db …`)          | What it does |
|--------------------------------------|--------------|
| `db-up.sh`   (`dev-dash db up`)      | Create the persistent volume (if needed) and start PostgreSQL; waits until healthy. |
| `db-down.sh` (`dev-dash db down`)    | Stop the container. **Data is preserved.** |
| `db-psql.sh` (`dev-dash db psql`)    | Open an interactive `psql` shell against the running DB. |
| `db-reset.sh`(`dev-dash db reset`)   | **DESTRUCTIVE.** Down → wipe the volume (all data) → up, leaving a fresh running DB (asks for confirmation). |

Each script is also exposed through the `dev-dash db <cmd>` wrapper (handy since `dev-dash` is
the one command agents run without a prompt). All load `.env` automatically and are idempotent
where it makes sense. They are the intended, expandable CLI surface for managing the dev
database — extend them rather
than documenting manual `docker` commands.

### Configuration

Everything is driven by `.env` (git-ignored). See [`.env.example`](.env.example):

- `POSTGRES_USER` / `POSTGRES_PASSWORD` / `POSTGRES_DB` — credentials for the container.
- `POSTGRES_PORT` — host port (default `5433`, to avoid clashing with a system postgres).
- `DATABASE_URL` — connection string the app uses; **must match** the values above.
- `RUST_LOG` — log verbosity (`tracing-subscriber` `EnvFilter` syntax).

---

## Project layout

```
src/
  main.rs      Boot sequence: config -> tokio worker -> egui.
  error.rs     All typed errors (thiserror). See AGENTS.md §3.
  config.rs    Env/config loading.
  domain/      Pure data types (Profile, Stage, Ticket, Note, uncategorized Note, Project, Worktree, Todo).
  system/      DB pool + services (business logic). No UI here.
  app/         The bridge: events, view-model, tokio worker.
  ui/          Pure egui rendering. No DB here.
    theme.rs     Design system: palette, fonts, radii, frames, grid.
    components/  Shared component kit (input, button, card).
migrations/    Numbered SQL migrations, applied at startup (schema only — no seed data).
assets/fonts/  Embedded Nunito font (SIL OFL).
scripts/       DB lifecycle helpers.
```

The look is a self-owned design system (soft-dark, teal, Nunito, bubbly, infinite grid
backdrop) built on egui — see [AGENTS.md](AGENTS.md) §7. All feature UI composes from
`ui/theme.rs` + `ui/components/`; no hardcoded colors or bespoke widgets in feature files.

**Hard rule:** the UI never touches the database directly, and the system layer never
imports egui. They communicate only through channels in `app/`. See [AGENTS.md](AGENTS.md).

---

## Development

```bash
cargo fmt          # format
cargo clippy       # lint (keep it clean)
cargo build        # builds WITHOUT a running database (runtime-checked queries)
cargo run          # run the app (needs the DB up)
./dev-dash open    # run detached; the in-app "Restart" button then rebuilds + relaunches
```

The **Restart** button (nav footer, under Refresh) exits with a sentinel code that
`./dev-dash open` catches to rebuild and relaunch (prod) or re-run (dev) — handy for picking up
code changes without leaving the app.

### Sandbox database & migrations

Because this holds real data, migrations are verified against an **isolated sandbox** database —
never your production one. The sandbox is a fully separate Docker stack
([`docker-compose.sandbox.yml`](docker-compose.sandbox.yml) + [`.env.sandbox`](.env.sandbox)):
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

## Troubleshooting

- **"cannot reach PostgreSQL …"** on startup — the DB isn't running. Run `./scripts/db-up.sh`.
- **Port already in use** — change `POSTGRES_PORT` in `.env` (and `DATABASE_URL` to match).
- **Docker daemon not running** — start Docker Desktop, then retry.
