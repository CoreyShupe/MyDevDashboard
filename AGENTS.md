# AGENTS.md

This file is the **binding contract** for every agent, chat, and human working in
this repository. It is meant to be your **one-stop shop**: read it fully and you should not
need to re-read the whole project to know its shape, where things live, or which commands to
run. Follow it religiously. If a rule is too generic to apply cleanly to your task, **stop and
specify a more precise sub-rule here first**, then continue. Do not silently deviate.

> **Maintain this file — it is part of every change, not optional.** AGENTS.md only stays a
> trustworthy one-stop shop if each change updates it in the SAME commit. Whenever you add or
> change a crate, feature, part, table, migration, error variant, module, command, `DEV_VIEW`,
> design token, or any rule/workflow, update the relevant section here (and `README.md` where
> user-facing). If you discover something in this file is stale, wrong, or
> made you run the wrong command / look in the wrong place, **fix it here first**, then proceed.
> A change that leaves AGENTS.md out of date is incomplete. New agents: treat editing this file
> as normal, expected work — future-you relies on it.

---

## Quick reference (skim this first, then §0–§9)

**Shape.** Two axes (§2): horizontal layers `domain → system → app → ui`, crossed with vertical
feature slices — `profile`, `tasks` (parts: `stage`, `ticket`, `note`), `notes`, `projects`
(parts: `project`, `worktree`), `todos`. The **same feature name appears in every layer**. All
DB/business logic lives behind a `*Service` in `system/`; `app/` is the only UI↔system channel;
`ui/` never touches the DB. The **one** place the app shells out to external commands (git, the
editor launcher) is `system/projects/git.rs` (§10). Full tree + dispatch pattern in §2.

**Where's the data?** In Postgres, reached only through `system/<feature>/`. Schema is in
`migrations/NNNN_*.sql`. Everything is scoped to the **active profile** (§9).

**Commands.** The `./dev-dash` wrapper and `cargo clippy` are pre-approved in
`.claude/settings.json` and run without a prompt; a bare `cargo build`/`cargo run` is **not**
allowlisted (it will prompt) — use the wrapper.

| To… | Run |
|-----|-----|
| Compile-check while iterating | `cargo clippy` |
| Build | `./dev-dash build`  ⟵ **not** `cargo build` |
| Screenshot a mock screen | `./dev-dash shot VIEW tmp/screenshots/NAME.png` |
| Screenshot the LIVE running app (owner's real data) | `./dev-dash snap [tmp/screenshots/live.png]` |
| Launch the app detached (in-app Restart rebuilds/relaunches) | `./dev-dash open [dev]` |
| Database up / down / wipe+restart / shell | `./dev-dash db up` · `db down` · `db reset` · `db psql` |

`VIEW` ∈ `onboarding · new-profile · board · board-empty · ticket · page · create · stage-edit ·
notes · notes-empty · notes-file · todos · todos-empty · projects · projects-empty · project ·
error` (defined in `ui/dev.rs`; see §8). Every one has a committed screenshot under
`screenshots/` (§11). **Never edit `dev-dash` itself** (trust boundary, §6).

**Before you're done:** `cargo fmt` → `cargo clippy` (clean) → `./dev-dash build` → **screenshot
every screen you touched** (§8). No unit tests (§6). If you added a crate/feature/table/error
variant/module, update **this file + `README.md`** in the same change (§6).

**Never:** `.unwrap()`/`.expect()` in app code (§3) · seed data (§5) · let anything escape its
profile (§9) · hardcode a color/frame outside `ui/theme.rs`+`ui/components/` (§7) · import
`system/`/`sqlx` from `ui/` (§2).

---

## 0. What this project is

A **single, self-use macOS developer dashboard** written in Rust. One place for the
owner to manage their development work in a digestible way. It builds and runs as a
**single application** backed by a local PostgreSQL database.

Onboarding creates a **profile**. Profiles are self-contained workspaces the owner switches
between (via the nav switcher) — everything belongs to exactly one and they never mix (§9).
Inside the active profile: a configurable, Jira-like **Tasks** board (stages → tickets → notes;
stages reorder by dragging their grip, and can be marked **terminal** in the edit-stage modal —
an end state like "Complete"/"Cancelled" that collapses to a ticket count and is hidden from
"Add to ticket"); a **Notes** tab for quick, uncategorized capture (which can later become a
ticket or be filed onto one); a **Projects** tab — local repositories (never cloned) shown as
cards with live git status, each opening to a detail page of its git **worktrees**, which are
created per-ticket to enable parallel work on different branches (§10); and a **Todos** tab —
quick tasks to remember (twin to Notes, plus a done-checkbox; completed todos are hidden). A note
can be turned into a todo, a ticket, or filed onto an existing ticket.

---

## 1. Approved stack (do not add to this without asking)

> **Framework rule:** You should not need any framework/crate beyond this list. If you
> think you need a new one, **ask the owner before adding it** and then record the
> decision here with a one-line justification.

| Concern            | Choice                                  | Notes |
|--------------------|-----------------------------------------|-------|
| UI framework       | `egui` + `eframe` (0.35)                | Native macOS, immediate-mode. `App::ui`/`App::logic`, `Panel`, `Modal`. |
| Async runtime      | `tokio`                                 | Workers/tasks only. Features incl. `process` — the `projects` feature shells out to `git`/the editor launcher off-thread (§10); no git *library* crate is used. |
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
│   ├── notes/           Note — an uncategorized (unfiled) note. Single concept, like profile.
│   ├── projects/        mod.rs + parts: project.rs (Project + GitStatus), worktree.rs (Worktree).
│   └── todos/           Todo — a quick task (body + `done`). Single concept, like notes.
│
├── system/             "System functionality": DB + business logic. No egui, ever.
│   ├── mod.rs               `Backend` = aggregate of every feature's service.
│   ├── db.rs               Shared: pool creation + migrations.
│   ├── profile/            ProfileService.
│   ├── tasks/              mod.rs `TasksService` = { stage, ticket, note } part-services.
│   ├── notes/              NotesService — CRUD for the `uncategorized_notes` table.
│   ├── projects/           mod.rs `ProjectsService` = { project, worktree }; git.rs = the ONE
│   │                       external-command boundary (git reads/worktree ops + editor launch, §10).
│   └── todos/              TodosService — CRUD + `set_done` for the `todos` table.
│
├── app/                The BRIDGE + orchestration root. Root dispatch lives here.
│   ├── mod.rs              Re-exports.
│   ├── bridge.rs           `Bridge` (UI handle) + `Emitter` (worker->UI) + `Repainter`.
│   ├── event.rs            ROOT `UiEvent` / `AppMessage` that WRAP feature enums.
│   ├── state.rs            ROOT `ViewData` composed of each feature's `View`.
│   ├── worker.rs           ROOT dispatcher: routes a UiEvent to the owning feature.
│   ├── profile/            profile::{Event, View, handle()}  — the feature "sub-root".
│   ├── tasks/              mod.rs dispatches to parts: stage/ticket/note::{Command, handle()}.
│   ├── notes/              notes::{Event, View, handle()}. `FileIntoTicket`/`FileIntoTodo` reach out.
│   ├── projects/           mod.rs dispatches to parts: project/worktree::{Command, handle()}.
│   │                       `View` = projects (with live GitStatus) + all worktrees.
│   └── todos/              todos::{Event, View, handle()} — add / set_done / delete.
│
└── ui/                 PURE rendering. No DB. One folder per feature + the shell + kit.
    ├── mod.rs              `DashboardApp` (eframe): shell nav, workspace, error modal.
    ├── theme.rs            Design system: palette, fonts, visuals, radii, frames, grid (§7).
    ├── components/         Shared component kit: input.rs, button.rs, card.rs, dnd.rs (§7).
    ├── dev.rs              Dev-only `DEV_VIEW` screen overrides for visual review (§8).
    ├── profile/            Onboarding "setup profile" screen + its transient UI state.
    ├── tasks/              mod.rs board + part renderers: stage.rs, ticket.rs, note.rs, modal.rs.
    ├── notes/              Notes tab: composer + note rows + the "Add to ticket" picker.
    ├── projects/           Projects tab: card grid, project detail page, worktree rows +
    │                       add-project / create-worktree modals.
    └── todos/              Todos tab: composer + open-todo rows (done checkbox + delete).
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
  Keep such reaches deliberate and commented. Examples today: `notes::FileIntoTicket` adds a
  ticket note then deletes the uncategorized note, and `notes::FileIntoTodo` adds a todo then
  deletes the note; the `stage`/`notes`/`project`/`todo` create handlers
  call `app::profile::active_id(backend)` to scope new rows to the active profile (§9); deleting
  a ticket first calls `projects::worktree::remove_all_for_ticket` so its worktree folders aren't
  orphaned (§10). In the **UI**, the ticket detail renders the projects worktree section and
  raises a "create worktree" request that the shell hands to the projects UI (which owns the
  picker) — mirroring the notes→create-ticket coordination.
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
- Domain-specific **sub-error** enums (`ConfigError`, `DbError`, `TaskError`, `ProfileError`,
  `ProjectError`, `ProcessError`, …), each a `#[from]` variant of `AppError`. Sub-errors carry
  structured fields, not just strings. `ProjectError` is domain-rule refusals for projects/
  worktrees (bad path, not-a-repo, duplicate worktree); `ProcessError` is an external command
  (git / the editor launcher) failing to spawn or exiting non-zero — kept separate on purpose (§10).
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

- Local PostgreSQL runs via `docker compose` with a **named, persistent volume**
  (`my-dev-dash-pgdata`) so data survives `docker system prune`. The compose **project name is
  pinned** (`name: my-dev-dash` in `docker-compose.yml`, mirrored by `COMPOSE_PROJECT_NAME` in
  `scripts/_common.sh`) so a directory rename can't orphan the container. See `README.md` for setup.
- Helper scripts in `scripts/` (`db-up`, `db-down`, `db-reset`, `db-psql`) wrap the common
  operations; run them via the allowlisted wrapper — **`./dev-dash db {up,down,reset,psql}`** —
  not a bare `docker compose`. `db-up`/`db-down`/`db-reset` share `start_db`/`stop_db` helpers
  in `_common.sh`; **`db reset` = down → wipe volume → up** (leaves a fresh, running DB; the app
  migrates on next launch). Extend these rather than documenting manual `docker` steps.
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
3. **Compile-check with `cargo clippy`** (not `cargo check`) while iterating — it's pre-approved
   so it runs without a prompt. **Before done:** `cargo fmt` → `cargo clippy` (clean, no new
   warnings) → `./dev-dash build`. Use `./dev-dash build`, **never a bare `cargo build`** (not
   allowlisted → prompts; the wrapper is). No tests (above).
4. **Update AGENTS.md + `README.md` in the same change** whenever you add/alter a crate,
   feature, part, table, migration, error variant, module, command, `DEV_VIEW`, or design token.
   This is the maintain-this-file rule at the top — treat it as part of "done".
5. Prefer small, obvious code over cleverness. This is a personal tool — clarity wins.

### The `dev-dash` wrapper (build + screenshots)

`./dev-dash` is the trusted, pre-approved entry point for building and for the whole
build → launch → raise → capture → kill screenshot dance (it handles the macOS gotchas: the
window opens behind others and needs Screen Recording + Accessibility permission; `sleep` is
blocked so it uses `perl` timing). Prefer it over hand-rolling `screencapture`.

```bash
./dev-dash build                                  # compile (allowlisted; use instead of cargo build)
./dev-dash shot VIEW tmp/screenshots/NAME.png     # capture one DEV_VIEW screen, then Read the PNG
./dev-dash snap [tmp/screenshots/live.png]        # capture the ALREADY-RUNNING app (real data)
./dev-dash open [dev]                             # launch detached; loops on Restart (see below)
```

The in-app **"Restart"** button (nav footer, under Refresh) exits with `RESTART_EXIT_CODE`
(**86**, in `src/main.rs` — chosen clear of reserved bands: 0–2, sysexits 64–78, Rust panic 101,
128+signal). `dev-dash open` runs the app in a loop that catches exactly that code and
**rebuilds + relaunches** (prod) or **re-runs `cargo run`** (dev); any other exit (incl. a
normal close) ends the loop. Keep the `86` in `dev-dash`'s `open` loop in sync with the constant.

> **`dev-dash` is a trust boundary — do not edit it casually.** `.claude/settings.json`
> allowlists `Bash(./dev-dash:*)` to run without prompting, and `dev-dash` internally runs
> `osascript`/`screencapture`/`pkill`/`perl` (none individually allowlisted). Because running it
> is auto-approved, **editing it is deliberately gated**: the settings `ask` rules force a
> confirmation prompt on `Edit(dev-dash)`/`Write(dev-dash)` even under auto-accept. Never remove
> that guard or widen the allowlist to the underlying system tools to "simplify" things.

---

## 7. Design system & UI components

> **Design-system rule:** ALL visuals come from `ui/theme.rs` (palette, fonts, spacing,
> corner radii, frames, grid) and `ui/components/` (input, button, card, dnd). Feature UI files
> MUST compose from these. NEVER hardcode a `Color32`, build a raw surface/input `Frame`,
> or bespoke-style a button in a feature file. Need a color? add it to `Palette`. Need a
> widget? add/extend a component.

> **Reuse-first components rule:** Add a NEW component only when no existing one can be
> adapted by a small change. Prefer extending an existing one (e.g. `text_field` grew a
> `text_field_sized` sibling rather than a new widget). One component per visual primitive.

> **Drag-and-drop rule:** All drag-and-drop goes through `ui/components/dnd.rs` so it behaves
> consistently. A dragged item lifts onto a floating layer and follows the pointer **from the
> exact grab point** — never re-centre it on the cursor. Use `dnd::drag_ghost` in the
> `is_being_dragged` branch (tickets and stage columns both do). Payload types are per-feature
> and DISTINCT so drop targets can tell them apart (tickets use `Uuid`; stage reorder uses a
> `StageDrag` newtype).

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

> **Always screenshot UI changes before reporting them done** — capture every screen your
> change touches and actually look at the image. The tooling is pre-approved (§6), so there's
> no reason to skip it.

**How:** `./dev-dash shot VIEW tmp/screenshots/NAME.png`, then Read the PNG (see §6 for the
wrapper). Write shots into **`tmp/screenshots/`** (gitignored scratch, kept via `.gitkeep`) so
you can open them in the IDE. Capture each affected `VIEW` (e.g. both `ticket` and `page` for a
detail-view change).

> **"Look at my app" is a protocol.** When the owner says *look at my app / see what I'm
> seeing / take a screenshot of what's open*, they mean the **already-running** instance with
> their real data — screenshot it with `./dev-dash snap [OUT]` (default `tmp/screenshots/live.png`)
> and Read it. Unlike `shot`, `snap` does NOT build, launch, or close anything; it just raises
> and captures the live window. (Errors if the app isn't running — tell them to `dev-dash open`.)

**`DEV_VIEW` screens** (in `ui/dev.rs`): the app injects mock in-memory state — no DB, no
seeding — so a screen renders instantly and works with the DB down; while set, worker snapshots
are ignored. The wrapper passes `VIEW` through as `DEV_VIEW`. Available:

| `VIEW` | Screen |
|--------|--------|
| `onboarding`  | First-run: create your first profile |
| `new-profile` | "New profile" create screen (switcher top-left) over existing profiles |
| `board`          | Populated Tasks board (profiles "Work"/"Personal") |
| `board-empty`    | Tasks board with no stages (empty state) |
| `ticket`         | Ticket detail modal |
| `page`           | Ticket detail, full-page (expanded) |
| `create`         | New-ticket create modal (with stage picker) |
| `stage-edit`     | Edit-stage modal (name + terminal toggle + delete) |
| `notes`          | Notes tab, populated |
| `notes-empty`    | Notes tab with no notes (empty state) |
| `notes-file`     | Notes tab with the "Add to ticket" picker open |
| `todos`          | Todos tab: open tasks (the mock's one done todo is hidden) |
| `todos-empty`    | Todos tab with nothing to do (empty state) |
| `projects`       | Projects tab: card grid (up-to-date / out-of-sync / no-origin states) |
| `projects-empty` | Projects tab with no projects (empty state) |
| `project`        | A project's full-page detail (metadata + live + removed worktrees) |
| `error`          | The error modal |

(The `board`/`ticket`/`page` mocks also carry projects + worktrees, so the ticket detail's
worktree section renders under `DEV_VIEW=ticket`/`page`. The `*-empty` views share one
`dev::mock_empty()` — a profile with no feature data — differing only by active tab.)

**When you add a screen/feature, add a `DevView` variant + mock to `ui/dev.rs`** (and a row
above) so it stays reviewable, then capture its screenshot into the gallery (§11). Dev mocks are
gated solely by the env var — never wire them into a normal run.

> **Make the feature actually visible in the mock.** A screenshot only verifies what the mock
> exercises. If your change shows only with certain data (a long string to force wrapping, >N
> items to trip a cap), enrich the mock so it renders — an empty mock that hides the feature is
> a failed verification.

---

## 9. Profiles are containers (everything belongs to one)

> **Containment rule:** A **profile is a self-contained workspace.** Every user-created entity
> — stages, tickets, ticket-notes, uncategorized notes, and anything added later — belongs to
> exactly ONE profile, and profiles NEVER mix. Switching profiles swaps the entire workspace.
> When you add any new kind of user data, it MUST be scoped to a profile in the same change.

How it's enforced (keep new data consistent with this):

- **Exactly one active profile.** `profiles.is_active` (partial unique index → at most one true)
  marks it; `ProfileService::set_active` flips it atomically with
  `UPDATE profiles SET is_active = (id = $1)`. `create()` makes the new profile active. `active()`
  returns it (falling back to the oldest profile so a pre-multi-profile DB still resolves).
- **Scoping columns.** Top-level tables carry `profile_id … REFERENCES profiles(id) ON DELETE
  CASCADE` (`stages`, `uncategorized_notes`, `projects`, `todos`). Nested entities inherit their profile
  through their parent rather than duplicating the column: **tickets** via their `stage` (list
  joins `stages`), **ticket-notes** via their `ticket`, **worktrees** via their `project` (lists
  join `projects`). Deleting a profile cascades its whole workspace.
- **Resolving the active profile.** Handlers that create profile-scoped rows call
  `app::profile::active_id(&Backend) -> Result<Uuid, AppError>` (a `ProfileError::NoActive` if
  none) so the UI never threads a profile id through events. `ViewData::load` scopes the board,
  notes, projects, and todos to `profile.active_id()`; a fresh snapshot after any change reloads
  the active profile's data.
- **UI.** The nav shows a **profile switcher** (`ui/profile::render_switcher`, `SwitcherStyle::Nav`)
  — switch profiles or pick "New profile" (→ the new-profile onboarding flow). The onboarding
  screen has two modes (`OnboardingMode::{FirstRun, NewProfile}`); **new-profile mode** shows a
  top-left escape hatch — a **Back** link and a compact switcher — so you can leave without
  creating one (Back and picking any profile, including the current one, both exit; the switcher
  reports this via `SwitcherOutcome::selected_current`). **First-run has no escape** (no profile
  to return to). The shell resets transient board/notes/projects/todos state when the active
  profile changes so one profile's open modals don't bleed into another.

---

## 10. Projects & worktrees

> **Points-at-repos rule:** A **project is an existing local repository path** (profile-scoped,
> §9). This tool NEVER clones — the owner enters a path they already have; `create` validates
> the path exists and is a git repo (else a typed `ProjectError`). Only durable identity
> (name + path) is stored; git facts are read live (below), never persisted.

**Git is read live, never stored.** Origin URL, current branch, clean/dirty, and ahead/behind
are computed at snapshot time in `system/projects/git.rs` and travel in the `View` as a
`GitStatus` — so a card can never show stale git data. Status reads are **best-effort**: a
non-repo / missing remote / failed fetch degrade to empty fields and NEVER fail the snapshot.
The status **fetches first** (bounded by a timeout) then compares; if the fetch can't connect it
**falls back to local refs** and flags `fetched = false`. "Up to date" = real repo + clean +
in sync with upstream.

**Git is the only tool; the owner drives history.** Committing / pushing / pulling are done by
the owner **by hand** — the app never runs them, so the exact commands stay theirs. The app only
reads status and manages worktrees. Assume SSH keys are loaded by the time the app runs.

**Worktrees.** A worktree lives at **`{repo}/.github/worktrees/{name}`** (the path convention —
never deviate) and is tied **1:1 to a ticket** within a project:

- **Ticket-driven creation only.** The only create entry point is a ticket's detail page
  (§2 coordination). A ticket may have **at most one worktree per project**; `worktree::create`
  rejects a duplicate (`ProjectError::WorktreeExists`).
- **One shared branch per ticket.** The branch is chosen once (on the ticket's first worktree)
  and **reused** for every later worktree of that ticket, in any project — so a ticket's work
  sits on the same branch name everywhere. Different projects are different repos, so each still
  gets its own `-b` branch creation.
- **Delete leaves a marker.** Removing a worktree (via the UI, or the cleanup on ticket delete)
  runs `git worktree remove` (NOT forced — a dirty tree makes git refuse, and that surfaces so
  nothing is lost) and sets `removed_at`, keeping the row as a **historical marker** of the
  branch name. It can be **recreated** from that marker (same branch + folder) from the ticket or
  project detail.
- **Reconcile against disk.** Before building a projects snapshot, `worktree::reconcile` flips
  any live worktree whose folder has vanished (owner deleted it outside the app) to a marker, so
  counts stay honest and it stays recreatable.
- **Ticket delete cleans up.** Deleting a ticket first best-effort-removes its live worktree
  folders (`remove_all_for_ticket`) before the rows cascade away, avoiding orphaned folders.

**Open in VS Code.** A worktree row's "Open in VS Code" launches `open -a "Visual Studio Code"
<path>` off the worker thread. It changes no state, so its handler does **not** snapshot — it
only surfaces an error on failure. This is the one non-git external command; it lives beside git
in `system/projects/git.rs` and rolls up as `ProcessError` like git does.

**Schema.** `projects` (profile-scoped, cascades) + `worktrees` (a churny, lightweight table:
`project_id`, `ticket_id`, `name`, `branch`, `removed_at`, with `UNIQUE(project_id, ticket_id)`
and a partial-unique on active `(project_id, name)`). Both cascade from their parents. Keep the
worktrees table lean — it takes common hard creates/deletes.

---

## 11. Screenshot gallery (keep it current)

> **The gallery is a maintained artifact, not a scratch dump.** `screenshots/` is a committed,
> browsable record of what every screen looks like — one folder per feature, one PNG per
> `DEV_VIEW`. The owner reviews flows here. **A screen whose look changed with STALE pixels in
> the gallery is a bug in your change.** (This is distinct from `tmp/screenshots/`, the
> gitignored scratch you capture into while iterating, §8.)

**Layout.** `screenshots/<feature>/<DEV_VIEW>.png` — the filename is exactly the `DEV_VIEW` key
(§8), so the mapping is 1:1 and unambiguous. `screenshots/README.md` is the index (per-feature
tables + inline thumbnails); regenerate/extend it alongside the images.

| Folder | Screens (`DEV_VIEW`) |
|--------|----------------------|
| `profile/`  | `onboarding`, `new-profile` |
| `tasks/`    | `board`, `board-empty`, `ticket`, `page`, `create`, `stage-edit` |
| `notes/`    | `notes`, `notes-empty`, `notes-file` |
| `todos/`    | `todos`, `todos-empty` |
| `projects/` | `projects`, `projects-empty`, `project` |
| `shell/`    | `error` (cross-cutting overlays, not tied to a tab) |

**The invariant (must always hold):**
1. **Every `DEV_VIEW` has a mock AND a committed screenshot.** Adding a `DevView` variant
   without capturing its PNG, or vice-versa, is incomplete.
2. **Every user-facing view, flow, and meaningful data variation has a `DEV_VIEW`.** Empty vs.
   populated, and distinct states (e.g. a git card's up-to-date / out-of-sync / no-origin) must
   be reachable and captured — add a variant (e.g. `*-empty`) when one is missing. A state you
   can't screenshot is a state you can't review.
3. **Mock data must exercise the thing.** The mock has to actually render the feature/variation
   the screenshot is meant to show (the §8 "make it visible in the mock" rule).

**When you touch UI, in the SAME change:**
- **New screen/feature** → add the `DevView` + mock (§8), create `screenshots/<feature>/` if
  new, capture the PNG(s), and add them to `screenshots/README.md`.
- **Changed look/layout/copy of an existing flow** → recapture every affected view's PNG (both
  presentations where relevant, e.g. `ticket` *and* `page`) so the gallery matches `main`.
- **Recapture views whose visible BACKGROUND changed, not just the view you edited.** Many
  screens render a modal/picker/overlay over a tab that still shows behind it — so a change to
  that tab makes those overlay shots stale too. Concretely: an edit to the notes rows means
  recapturing `notes-file` (the "Add to ticket" picker sits over the notes list), a board change
  means recapturing `ticket`/`create`/`stage-edit` (all overlay the board), etc. Scan for every
  view that shows the thing you changed anywhere in frame.
- **New data variation** (empty state, error, a new status) → add a `DEV_VIEW` for it and capture
  it; don't rely on an existing shot to "sort of" cover it.
- **Removed screen** → delete its `DevView`, its PNG, and its gallery/table rows.

**Regenerate** (from the repo root; the wrapper is pre-approved, §6):

```bash
./dev-dash shot <DEV_VIEW> screenshots/<feature>/<DEV_VIEW>.png
# e.g. ./dev-dash shot projects screenshots/projects/projects.png
```

Then **Read the PNG** to confirm it rendered what you intended (§8) before reporting done. The
canonical list of every view lives in the §8 table; this gallery must mirror it exactly.
