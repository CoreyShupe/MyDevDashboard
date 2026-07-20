# Mac Dev Dashboard

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

On first launch you'll land on the **Setup Profile** onboarding screen. Enter your name
and you'll drop into the dashboard with a left side-nav (**Tasks**, **Notes**) and an empty
workspace.

- **Tasks** — a configurable, Jira-like board of stages and tickets (with notes and
  parent/child relationships).
- **Notes** — a fast, list-like scratchpad for *uncategorized* notes. Jot a note at the top
  (Enter or **Add**), then file it later: **Create Ticket** turns a note into a new ticket
  (pre-filled as its first note), or **Add To Ticket** searches your tickets by title and
  attaches the note to the one you pick. Filing a note removes it from the list.

---

## The database & persistence (important)

The dashboard stores everything in PostgreSQL running in Docker. Data is kept in a
**named external volume** called `macdevdash_pgdata`.

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

| Script                   | What it does |
|--------------------------|--------------|
| `./scripts/db-up.sh`     | Create the persistent volume (if needed) and start PostgreSQL; waits until healthy. |
| `./scripts/db-down.sh`   | Stop the container. **Data is preserved.** |
| `./scripts/db-psql.sh`   | Open an interactive `psql` shell against the running DB. |
| `./scripts/db-reset.sh`  | **DESTRUCTIVE.** Delete the volume and all data (asks for confirmation). |

All scripts load `.env` automatically and are idempotent where it makes sense. They are
the intended, expandable CLI surface for managing the dev database — extend them rather
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
  domain/      Pure data types (Profile, Stage, Ticket, Note, uncategorized Note).
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
```

## Troubleshooting

- **"cannot reach PostgreSQL …"** on startup — the DB isn't running. Run `./scripts/db-up.sh`.
- **Port already in use** — change `POSTGRES_PORT` in `.env` (and `DATABASE_URL` to match).
- **Docker daemon not running** — start Docker Desktop, then retry.
