# AGENTS.md

This file is the **binding contract** for every agent, chat, and human working in
this repository. Read it fully before writing a line of code. Follow it religiously.
If a rule is too generic to apply cleanly to your task, **stop and specify a more
precise sub-rule here first**, then continue. Do not silently deviate.

---

## 0. What this project is

A **single, self-use macOS developer dashboard** written in Rust. One place for the
owner to manage their development work in a digestible way. It builds and runs as a
**single application** (`cargo run`) backed by a local PostgreSQL database.

The first feature is an onboarding "setup profile" flow, followed by a configurable,
Jira-like **Tasks** board.

---

## 1. Approved stack (do not add to this without asking)

> **Framework rule:** You should not need any framework/crate beyond this list. If you
> think you need a new one, **ask the owner before adding it** and then record the
> decision here with a one-line justification.

| Concern            | Choice                                  | Notes |
|--------------------|-----------------------------------------|-------|
| UI framework       | `egui` + `eframe` (0.35)                | Native macOS, immediate-mode. `App::ui`/`App::logic`, `Panel`, `Modal`. |
| Async runtime      | `tokio`                                 | Workers/tasks only. |
| Database           | PostgreSQL (via Docker)                 | Local, persistent volume. |
| DB driver          | `sqlx` (0.9, runtime-checked queries)   | Builds WITHOUT a live DB. Migrations embedded. |
| Errors             | `thiserror` (enum + sub-enums)          | See §3. |
| Serialization      | `serde` / `serde_json`                  | |
| IDs                | `uuid` (v4)                             | All primary keys. |
| Time               | `chrono`                                | `created_at` / `updated_at`. |
| Logging            | `tracing` + `tracing-subscriber`        | Console diagnostics (see §3). |
| Config             | `dotenvy`                               | Load DB config from `.env`. |

**Every crate above was explicitly approved by the owner.** Adding anything else
requires a new approval + a new row here.

---

## 2. Architecture — layered AND feature-sliced

Two axes, both mandatory:

1. **Horizontal layers** — `domain/`, `system/`, `app/`, `ui/`. These enforce the
   separation of UI from system work (below). This axis never collapses.
2. **Vertical feature slices** — every feature (e.g. `tasks`, `profile`) appears as a
   **nested folder inside every layer**. The same feature name is mirrored across all
   four layers so a human can scan one feature top-to-bottom, or one layer across
   features, at a glance.

> **Separation rule:** All UI visual rendering MUST be completely separate from the
> services handling the work. The UI renders state and emits intent; it never touches
> the database, sqlx, or business logic directly. This holds *within every feature*:
> `system/<feature>/` has no egui, `ui/<feature>/` has no DB.

> **Feature-mirroring rule:** A feature is not "done" until it exists as a folder in each
> of `domain/`, `system/`, `app/`, and `ui/` (omit a layer only if the feature genuinely
> has nothing there — and say so). Do not scatter a feature's pieces under generic names;
> keep them under `<layer>/<feature>/`.

> **Composed-of-parts rule (recursive nesting):** When a feature is made of distinct parts
> — each with its own data, rules, and actions — promote those parts to their OWN nested
> module, mirrored across the same layers, instead of leaving them as flat variants/methods
> in the feature's single file. `tasks` is composed of `stage`, `ticket`, and `note`, so
> each is a module under `domain/tasks/`, `system/tasks/`, `app/tasks/`, and `ui/tasks/`.
> This recursion has no fixed depth: the structural pattern is `root { feature { part { … } } }`,
> and a part that itself splits into parts nests again. Prefer this the moment parts become
> clearly nameable (see §4) — one screenful per file beats one big file with internal sections.
>
> **Promote a part to its own FOLDER (a `<part>/` with `mod.rs` + sub-parts) as soon as it
> grows sub-parts of its own** — don't leave sibling files piling up next to it. Worked
> example in `ui/`: the ticket UI grew into cards + detail (modal & full-page) + relationships
> + notes, so it became `ui/tasks/ticket/{mod.rs, detail.rs, link.rs, note.rs}` rather than
> four `ticket_*`-ish files under `ui/tasks/`. When you promote, the sub-parts' shared state
> (e.g. `TicketModal`) often needs `pub(crate)` so the grandparent layer can still hold it —
> that's expected; keep the struct `pub(crate)` and its fields as tight as the sub-parts allow.

```
src/
├── main.rs              Boot sequence only.
├── config.rs            Shared: env/config -> ConfigError.
├── error.rs             Shared: AppError + typed sub-errors (§3).
│
├── domain/              Pure data types. No I/O. Serde-able. One folder per feature.
│   ├── mod.rs
│   ├── profile/         Profile.
│   ├── tasks/           mod.rs + parts: stage.rs (Stage), ticket.rs (Ticket), note.rs (Note).
│   └── notes/           Note — an uncategorized (unfiled) note. Single concept, like profile.
│
├── system/             "System functionality": DB + business logic. No egui, ever.
│   ├── mod.rs               `Backend` = aggregate of every feature's service.
│   ├── db.rs               Shared: pool creation + migrations.
│   ├── profile/            ProfileService.
│   ├── tasks/              mod.rs `TasksService` = { stage, ticket, note } part-services.
│   └── notes/              NotesService — CRUD for the `uncategorized_notes` table.
│
├── app/                The BRIDGE + orchestration root. Root dispatch lives here.
│   ├── mod.rs              Re-exports.
│   ├── bridge.rs           `Bridge` (UI handle) + `Emitter` (worker->UI) + `Repainter`.
│   ├── event.rs            ROOT `UiEvent` / `AppMessage` that WRAP feature enums.
│   ├── state.rs            ROOT `ViewData` composed of each feature's `View`.
│   ├── worker.rs           ROOT dispatcher: routes a UiEvent to the owning feature.
│   ├── profile/            profile::{Event, View, handle()}  — the feature "sub-root".
│   ├── tasks/              mod.rs dispatches to parts: stage/ticket/note::{Command, handle()}.
│   └── notes/              notes::{Event, View, handle()}. `FileIntoTicket` reaches into tasks.
│
└── ui/                 PURE rendering. No DB. One folder per feature + the shell + kit.
    ├── mod.rs              `DashboardApp` (eframe): shell nav, workspace, error modal.
    ├── theme.rs            Design system: palette, fonts, visuals, radii, frames, grid (§7).
    ├── components/         Shared component kit: input.rs, button.rs, card.rs (§7).
    ├── dev.rs              Dev-only `DEV_VIEW` screen overrides for visual review (§8).
    ├── profile/            Onboarding "setup profile" screen + its transient UI state.
    ├── tasks/              mod.rs board + part renderers: stage.rs, ticket.rs, note.rs, modal.rs.
    └── notes/              Notes tab: composer + note rows + the "Add to ticket" picker.
```
(`assets/fonts/Nunito.ttf` — SIL OFL, embedded via `include_bytes!`. Not a crate.)

### The dispatch pattern: `root { feature { part { action } } }`
This is THE pattern; use it for every new feature, part, and action. Each level's node is
thin — it only names the level below and hands off; per-action logic lives at the leaf.
- **Root** (`app/worker.rs`, `app/event.rs`): `UiEvent` is a thin enum — `ReloadAll` plus
  one variant per feature wrapping that feature's own `Event`. The worker hands off; it never
  contains per-action logic.
- **Feature** (`app/<feature>/mod.rs`): its `Event` wraps one `Command` per part; `handle()`
  routes to the owning part's `handle()`.
- **Part** (`app/<feature>/<part>.rs`): owns its `Command` enum + constructors + a `handle()`
  that performs the action via its service. This is the leaf where logic lives.
- Adding an action → add a variant to the right part's `Command` + an arm in that part's
  `handle()`. You touch only the leaf; the feature and root are untouched. Adding a whole new
  part → add a module under each layer's feature folder + one delegating arm at the feature.

### Non-negotiable boundaries
- `ui/` depends on `app/` (for `Bridge`, `UiEvent`, `ViewData`, feature `Event`/`View`) and
  `domain/`. It MUST NOT import `system/`, `sqlx`, or spawn async work. **No DB in `ui/`.**
- `system/` MUST NOT import `egui`/`eframe` or anything in `ui/`.
- The **only** channel of communication is `app/`: UI → worker via `UiEvent` (non-blocking);
  worker → UI via `AppMessage` (snapshot / feature message / error) + a repaint nudge.
- **Cross-feature reach is allowed.** A feature handler gets `&Backend` (all services) and
  may call another feature's service when a genuine cross-feature interaction calls for it.
  Keep such reaches deliberate and commented. (There are none today — onboarding used to seed
  the board, but seeding is now banned; see §5.)
- The UI thread MUST NEVER block on I/O. All DB/async work happens on the tokio worker.

### Data flow (one direction each way)
```
UI (egui, main thread)  --UiEvent-->  Worker (root) --> feature::handle --> part::handle --> Backend --> Postgres
UI (egui, main thread)  <-AppMessage-- Emitter <------------------------------ part::handle <-- Backend <-- Postgres
```
The worker emits a fresh `ViewData` snapshot after any state-changing event. The UI never
mutates domain state locally except for transient input buffers (text fields).

---

## 3. Error handling

> **No-panic rule:** NEVER use `.unwrap()` or `.expect()` in application code. Always
> resolve the bad path. You MAY `?`-propagate ("yeet") **only when the bubbled-up flow
> still makes sense** and is ultimately handled at a boundary that logs or shows it.
> The single allowed panic is a documented, unrecoverable boot failure in `main.rs`
> (e.g. the async runtime itself cannot be created) — and it must `tracing::error!`
> exactly what failed and how to fix it before exiting.

> **Distinct-error rule:** Every distinct, *known* failure gets its own distinct,
> typed variant carrying context, so it is obvious **where** and **what** went wrong.
> No stringly-typed catch-alls for known cases.

> **User-aware rule:** Every error surfaced to the owner must be handled in the most
> user-aware way possible: either (a) log *blatantly* to the console exactly what broke
> and what is needed to fix it, or (b) show a modal dialog that alerts them. DB/boot
> problems the owner must fix (e.g. "start the database") get BOTH.

### Structure
- One top-level `AppError` enum in `error.rs`.
- Domain-specific **sub-error** enums (`ConfigError`, `DbError`, `TaskError`, …), each a
  `#[from]` variant of `AppError`. Sub-errors carry structured fields, not just strings.
- `#[error("...")]` messages state **what** failed and, where actionable, **how to fix**.
- Errors crossing into the UI become a `UserFacingError { title, detail, remediation }`
  so the modal can render a clear, actionable message. The conversion lives in `app/`.

### Example shape (illustrative)
```rust
#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("cannot reach PostgreSQL at {target}. Is the database running? Try `./scripts/db-up.sh`")]
    Connect { target: String, #[source] source: sqlx::Error },
    #[error("migration failed while applying `{migration}`: {source}")]
    Migrate { migration: String, #[source] source: sqlx::migrate::MigrateError },
    #[error("{entity} `{id}` not found")]
    NotFound { entity: &'static str, id: String },
    #[error("query `{context}` failed: {source}")]
    Query { context: &'static str, #[source] source: sqlx::Error },
}
```
Add a new variant the moment a new *known* failure mode appears. Do not reuse a vague
variant for a distinct cause.

---

## 4. Coding standards

> **Readability-over-speed rule:** Prioritize human readability over finishing quickly.
> When a construct starts to accumulate many cases, and those cases fall into clearly
> nameable categories, **abstract into levels** (nested enums, per-category handlers,
> sub-modules) instead of extending the flat list. Don't keep growing one big `match`/enum
> over things that obviously group. This mirrors the sub-error pattern in §3.
>
> Applied examples in this codebase:
> - `UiEvent` (root) is a thin enum: `ReloadAll` + one variant per feature. `tasks::Event`
>   wraps one `Command` per part (`stage::Command`, `ticket::Command`, `note::Command`) —
>   not one flat list of `CreateStage`/`RenameStage`/`CreateTicket`/… variants.
> - Dispatch mirrors the modules: `worker` → `tasks::handle` → `stage::handle`. Each level
>   only names the level below. Add an action inside the owning part's `handle()`.
> - `tasks` being *composed of parts* (§2) is the same rule applied to modules, not just
>   enums: `stage`/`ticket`/`note` are folders across every layer, not sections in one file.
>
> Rule of thumb: if you're about to add the 4th+ sibling variant/arm/method and they clearly
> partition into groups, introduce the grouping (enum level AND module) first, then add your
> case inside it.

- **Edition:** Rust 2024. Format with `cargo fmt`. Keep `cargo clippy` clean (no new warnings).
- **No panics:** see §3. This includes `unwrap`/`expect`/`panic!`/`unreachable!`/array
  indexing that can go out of bounds / integer ops that can overflow on bad input.
- **Naming:** `snake_case` items, `CamelCase` types, `SCREAMING_SNAKE_CASE` consts.
  Services are `NounService`; events are imperative (`CreateTicket`); messages are nouns.
- **Async:** `tokio` only inside `system/`/`app/`. No blocking calls on the UI thread.
- **DB access:** only through a `*Service` in `system/`. Use `sqlx` **runtime** queries
  (`query`, `query_as`) — never the compile-time `query!` macros — so the project builds
  without a live database. All schema changes go through a numbered file in `migrations/`.
- **IDs & time:** `uuid::Uuid` (v4) for PKs; `chrono::DateTime<Utc>` for timestamps.
- **Logging:** use `tracing` spans/events. Log every handled error at the boundary with
  enough context to fix it. Never log secrets or full connection strings with passwords.
- **Comments:** match the surrounding density. Explain *why*, not *what*. Every non-trivial
  `?`-propagation chain should make its error path obvious by the types it returns.
- **Modules:** keep `ui/` files free of business logic; keep `system/` files free of egui.
  If you are tempted to cross the boundary, you are doing it wrong — route through `app/`.
- **egui 0.35 conventions:** implement `eframe::App::logic` for per-frame state sync (drain
  worker messages there — no painting) and `eframe::App::ui` for pure rendering. Side panels
  are `egui::Panel::left(id)`; blocking dialogs are `egui::Modal` (dims + traps input), not
  bare `Window`. `egui::Margin` values are `i8`.

---

## 5. Database & dev environment

> **No-seeding rule:** NEVER seed data — not in migrations, not at startup, not on
> onboarding, not anywhere. Migrations create/alter **schema only** (tables, indexes,
> constraints); they must not `INSERT` rows. The database starts empty and is populated
> **exclusively** through the app's own creation flows. This is deliberate: a personal
> tool shouldn't invent data the owner didn't ask for, and a dropped DB should be trivial
> to rebuild by hand.
>
> **Consequence — every part needs an easy from-scratch creation flow.** Because nothing
> is pre-populated, each part MUST offer an obvious, low-friction way to create its first
> item from an empty state (e.g. the empty board front-and-centers "create your first
> stage"; each column has "+ New ticket"; the ticket modal has "Add note"). When you add a
> part, add its empty-state creation affordance in the same change.

- Local PostgreSQL runs via `docker compose` with a **named, persistent volume** so data
  survives `docker system prune`. See `README.md` for full setup.
- Helper scripts in `scripts/` (`db-up`, `db-down`, `db-reset`, `db-psql`) wrap the common
  operations so setup is one command. Extend these rather than documenting manual steps.
- Migrations live in `migrations/NNNN_name.sql`, applied automatically at app startup via
  `sqlx::migrate!`. Never edit an already-applied migration; add a new one. Schema only —
  no seed rows (see the no-seeding rule above).
- Connection config comes from `.env` (see `.env.example`). Never commit real secrets.

---

## 6. Workflow expectations for agents

> **No-unit-tests rule:** Do NOT write unit tests, `#[test]` functions, `#[cfg(test)]`
> modules, or a `tests/` dir. This is a self-use tool; we do not care about unit-level
> regression coverage and won't maintain it. Verify by building and running the app. (If a
> genuinely thorny algorithm ever needs a scratch check, do it ad hoc and delete it — don't
> commit a test suite.)

1. Before coding, confirm your change respects §2 (separation) and §3 (errors).
2. If a rule doesn't fit, add a precise sub-rule to this file **before** proceeding.
3. **`cargo clippy` is the primary check tool** — use it (not `cargo check`) to compile-
   verify while iterating; it is pre-approved in `.claude/settings.json` so it runs without a
   permission prompt. Run `cargo fmt` + `cargo clippy` + `cargo build` before considering work
   done. No tests (above).
4. When you add a crate, feature, table, error variant, or module, update this file and
   the `README.md` in the same change.
5. Prefer small, obvious code over cleverness. This is a personal tool — clarity wins.

### Visual verification (screenshots on macOS)

Because this is a GUI, verify UI changes by looking at the running app, not just building.

- **Force a specific screen without a DB** via the `DEV_VIEW` env var (see `ui/dev.rs`);
  worker snapshots are ignored while it's set:
  `DEV_VIEW={onboarding|board|ticket|page|create|notes|notes-file|error}` (`ticket` = detail
  modal, `page` = the full-page detail, `create` = the new-ticket modal, `notes` = the Notes
  tab, `notes-file` = the Notes tab with the "Add to ticket" picker open).
- **Run the binary by ABSOLUTE path**, not `target/debug/…` relative — this shell's cwd
  drifts (e.g. after a `cd` for a file move), and a wrong relative path makes the launch fail
  silently, so you end up screenshotting whatever was already frontmost. If a capture shows
  the wrong app, first check the binary actually started (`pgrep -fl my-dev-dashboard`).
- The app window opens **behind** other windows, and `screencapture` grabs the frontmost
  screen — so raise the app first (needs Screen Recording permission for the terminal/agent;
  `set frontmost` also needs Accessibility permission — if it silently fails, that's why).
- **Do not use `sleep`** (blocked in this harness) to wait for the window; use
  `perl -e 'select(undef,undef,undef,SECONDS)'`.

```bash
BIN=/Users/cs/Programming/MacDevDashboard/target/debug/my-dev-dashboard  # absolute!
cargo build            # cargo finds Cargo.toml from any subdir; the binary path is fixed

# 1. launch the screen you want, detached
DEV_VIEW=page RUST_LOG=warn "$BIN" >/dev/null 2>&1 &

# 2. wait for the window, raise it, then capture
perl -e 'select(undef,undef,undef,4)'
osascript -e 'tell application "System Events" to set frontmost of (every process whose name contains "dev-dashboard") to true'
perl -e 'select(undef,undef,undef,1.5)'
screencapture -x /tmp/shot.png     # then open/read /tmp/shot.png

# 3. IMMEDIATELY exit the app once captured — don't leave a window lingering.
pkill -f "$BIN" 2>/dev/null || true
```

(The process/window name is the Cargo package name, `my-dev-dashboard`.) `dev-dash open`
launches detached the same way for manual use.

> **`dev-dash` is a trust boundary — do not edit it casually.** `.claude/settings.json`
> allowlists `Bash(./dev-dash:*)` to run without prompting, and `dev-dash` internally runs
> `osascript`/`screencapture`/`pkill`/`perl` (none of which are individually allowlisted).
> Because running it is auto-approved, **editing it is deliberately gated**: the settings
> `ask` rules force a confirmation prompt on `Edit(dev-dash)`/`Write(dev-dash)` even under
> auto-accept. This is intentional — never remove that guard or widen the allowlist to the
> underlying system tools to "simplify" things.

---

## 7. Design system & UI components

> **Design-system rule:** ALL visuals come from `ui/theme.rs` (palette, fonts, spacing,
> corner radii, frames, grid) and `ui/components/` (input, button, card). Feature UI files
> MUST compose from these. NEVER hardcode a `Color32`, build a raw surface/input `Frame`,
> or bespoke-style a button in a feature file. Need a color? add it to `Palette`. Need a
> widget? add/extend a component.

> **Reuse-first components rule:** Add a NEW component only when no existing one can be
> adapted by a small change. Prefer extending an existing one (e.g. `text_field` grew a
> `text_field_sized` sibling rather than a new widget). One component per visual primitive.

### Baked-in aesthetic decisions (do not fight these)
- **Framework:** stay on `egui` — it themes fully. Do NOT add a UI framework or an egui
  theme/component crate: they lag egui's version (e.g. `catppuccin-egui` capped at egui 0.30
  while we're on 0.35) and wouldn't match this look. We own the theme.
- **Palette:** soft-dark, **teal** accent. One `Palette` (`theme::DARK`).
- **Font:** Nunito (rounded, un-blocky), embedded from `assets/fonts/Nunito.ttf`.
- **Bubbly:** generous corner radii (`theme::radius`). Inputs are soft filled pills with a
  teal focus ring — never a harsh 1px box.
- **No harsh blue:** selection/highlight uses `accent_soft` (teal), set in `theme.rs`.
- **Sparing borders:** most surfaces have NO border. Separate with fill + elevation +
  shadow, not outlines. Onboarding is deliberately borderless.
- **Infinite grid:** the grid is painted on the window background layer, edge-to-edge
  (`theme::paint_background`). Panels over it use a transparent fill so it shows through.
- **Solid over grid:** anything floating over the grid MUST be a solid, opaque surface so
  the grid never bleeds through — use `components::card::card` (elevation 1) or `::inset`
  (elevation 2, one shade lighter for nested cards). Elevation ladder: grid → card → inset.

### When you add a feature's UI
Compose from the kit; add an empty-state creation flow (§5); if a needed component or color
doesn't exist, extend `theme`/`components` in the same change (and update this section).

---

## 8. Visual review workflow (for agents)

This is a GUI app; verify UI changes by looking at them, not by guessing.

> **Always verify UI changes with a screenshot before reporting them done.** This is a
> standing expectation, not an optional extra — after any change that affects what a screen
> looks like, capture the relevant `DEV_VIEW` and actually look at the image. The capture
> tooling below is pre-approved (`Bash(./dev-dash:*)` in `.claude/settings.json`), so it runs
> without a permission prompt — there is no reason to skip it.

### The one-liner: `dev-dash shot VIEW OUT`
`dev-dash shot` encapsulates the whole build → launch → raise → capture → kill flow, so this
is the primary way to get a screenshot. It's the trusted wrapper (see the `dev-dash` trust-
boundary note in §6) — prefer it over hand-rolling the `screencapture` dance.

```bash
./dev-dash shot ticket tmp/screenshots/ticket.png   # VIEW = onboarding|board|ticket|page|create|notes|notes-file|error
./dev-dash shot page   tmp/screenshots/page.png
```

**Compile-verify with `./dev-dash build`, not a bare `cargo build`.** `./dev-dash build` is
part of the same pre-approved wrapper (`Bash(./dev-dash:*)` in `.claude/settings.json`), so it
runs without a permission prompt; `cargo build` is not allowlisted and will prompt. Use
`cargo clippy` while iterating (it *is* allowlisted — see §6) and `./dev-dash build` for the
build step.

Write shots into the in-project **`tmp/screenshots/`** folder (`tmp/` is the gitignored local
scratch dir; the `screenshots/` subfolder is kept via `.gitkeep`, its contents ignored).
Keeping them in-repo means you can open them in the IDE, not hunt through the system `/tmp`.
Then Read the PNG to review it. Capture every view your change touches (e.g. both `ticket`
and `page` for a detail-view change).

> **Make the feature actually visible in the mock.** A screenshot only verifies what the
> mock state exercises. If your change only shows up with certain data (e.g. the notes cap
> needs >2 notes, an overflow needs a long string), enrich `ui/dev.rs` so the mock produces
> it — e.g. `mock_notes()` returns 5 notes so the modal's 2-note cap and "N earlier notes"
> line both render. An empty mock that hides the feature is a failed verification.

### Jump straight to a screen — `DEV_VIEW`
`ui/dev.rs` injects MOCK in-memory state (no DB, no data entry, no seeding) so any screen
renders instantly. Set the env var when launching:

```bash
DEV_VIEW=board      cargo run   # populated Tasks board
DEV_VIEW=onboarding cargo run   # setup-profile screen
DEV_VIEW=ticket     cargo run   # board with a ticket modal open
DEV_VIEW=create     cargo run   # board with the new-ticket create modal open
DEV_VIEW=notes      cargo run   # the Notes tab, populated
DEV_VIEW=notes-file cargo run   # the Notes tab with the "Add to ticket" picker open
DEV_VIEW=error      cargo run   # the error modal
```

While a `DEV_VIEW` is active the app ignores worker snapshots, so the forced screen stays
put and it works even with the database down. **When you add a new screen/feature, add a
mock + `DevView` variant to `ui/dev.rs`** so it stays reviewable. Never wire dev mocks into
a normal run — it's gated solely by the env var.

### Capture the screen for review
The app is a native window; capture it with macOS `screencapture` (requires Screen
Recording permission for the controlling terminal). Bring the window to the front first:

```bash
# 1. Launch (background), e.g. straight to the board:
DEV_VIEW=board RUST_LOG=warn cargo run   # run as a background task

# 2. Bring the window to the front (its process name contains "dev-dashboard"):
osascript -e 'tell application "System Events" to set frontmost of (first process whose name contains "dev-dashboard") to true'

# 3. Capture to a file, then view it:
screencapture -x /path/to/scratchpad/shot.png
```

Notes: `timeout` isn't on macOS — bound a foreground run with
`perl -e 'alarm shift; exec @ARGV' 25 cargo run` if needed. Stop a background run when done.

---

_Last updated as part of: initial scaffold + onboarding/profile + Tasks board (feature-sliced,
composed of parts; no seeding; no unit tests) + design system (theme + component kit) +
ticket detail split (two-column page / capped modal notes) + handle-only card drag + new-ticket
modal (now with a stage picker; title/description/first note, replacing the inline form) +
Notes tab (uncategorized notes: quick capture, "Create Ticket" reusing the create modal, and
"Add to ticket" search picker — a full feature slice + `uncategorized_notes` table); clippy as
primary check; `./dev-dash build` for the build step; mandatory screenshot verification via
`dev-dash shot`._
