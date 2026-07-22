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
(parts: `project`, `worktree`), `todos`. The **same feature name appears in every layer**. (One
exception: `home` — the cross-feature Overview tab — is a **UI-only** feature that purely
aggregates the others' already-loaded `ViewData`, so it exists solely as `ui/home/` with nothing
in `domain`/`system`/`app`, §2.) All
DB/business logic lives behind a `*Service` in `system/`; `app/` is the only UI↔system channel;
`ui/` never touches the DB. The **one** place the app shells out to external commands (git, the
editor launcher, and a project's per-worktree setup script) is `system/projects/git.rs` (§10).
Full tree + dispatch pattern in §2.

**Where's the data?** In Postgres, reached only through `system/<feature>/`. Schema is in
`static/migrations/NNNN_*.sql`. Everything is scoped to the **active profile** (§9).

**Commands.** The `./dev-dash` wrapper and `cargo clippy` are pre-approved in
`.claude/settings.json` and run without a prompt; a bare `cargo build`/`cargo run` is **not**
allowlisted (it will prompt) — use the wrapper.

| To… | Run |
|-----|-----|
| Compile-check while iterating | `cargo clippy` |
| Build | `./dev-dash build dev` (debug) / `./dev-dash build prod` (release)  ⟵ **not** `cargo build` |
| Screenshot a mock screen | `./dev-dash shot VIEW static/tmp/screenshots/NAME.png` |
| Screenshot the LIVE running app (owner's real data) | `./dev-dash snap [static/tmp/screenshots/live.png]` |
| Launch the app detached (dev by default; `prod` for release + Restart relaunch) | `./dev-dash open [prod]` |
| Build + install a double-clickable macOS `.app` to `/Applications` (`mac bundle` builds only) | `./dev-dash mac [copy]` |
| One-shot macOS setup: require Docker running → `db up` → build + install the `.app` | `./dev-dash bootstrap mac` |
| Database up / down / wipe+restart / shell | `./dev-dash db up` · `db down` · `db reset` · `db psql` |

`VIEW` ∈ `home · home-empty · onboarding · new-profile · profile-select · board · board-empty ·
board-search · ticket · page · ticket-back · create · stage-edit · confirm-delete · notes · notes-empty ·
notes-file · todos · todos-empty · projects · projects-empty · projects-loading ·
projects-pulling · add-project · project · setup-script · teardown-script · create-worktree ·
create-worktree-fresh · worktree-recreate-as · worktree-creating · worktree-removing · loading ·
error · error-output` (defined
in `ui/dev.rs`; see §8). Every one has a committed screenshot under `static/screenshots/` (§11).
**Never edit `dev-dash` itself** (trust boundary, §6).

**Before you're done:** `cargo fmt` → `cargo clippy` (clean) → `./dev-dash build dev` → **screenshot
every screen you touched** (§8). No unit tests (§6). If you added a crate/feature/table/error
variant/module, update **this file + `README.md`** in the same change (§6).

**Never:** `.unwrap()`/`.expect()` in app code (§3) · seed data (§5) · let anything escape its
profile (§9) · hardcode a color/frame outside `ui/theme.rs`+`ui/components/` (§7) · import
`system/`/`sqlx` from `ui/` (§2) · delete/remove data without a confirmation (§13).

---

## 0. What this project is

A **single, self-use macOS developer dashboard** written in Rust. One place for the
owner to manage their development work in a digestible way. It builds and runs as a
**single application** backed by a local PostgreSQL database.

Onboarding creates a **profile**. Profiles are self-contained workspaces the owner switches
between (via the nav switcher) — everything belongs to exactly one and they never mix (§9).
Inside the active profile, the first nav tab is a cross-feature **Home / Overview** — an
at-a-glance roll-up of the whole workspace (summary tiles, the most recently-touched active
tickets, open todos you can check off inline, repositories needing attention, and loose notes),
each pointing into the tab that owns it. The rest: a configurable, Jira-like **Tasks** board
(stages → tickets → notes;
stages reorder by dragging their grip, and can be marked **terminal** in the edit-stage modal —
an end state like "Complete"/"Cancelled" that collapses to a ticket count and is hidden from
"Add to ticket"; a header **search** box filters tickets by title/description across every
column, revealing matches even in collapsed terminal stages); a **Notes** tab for quick,
uncategorized capture (which can later become a
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
| File dialogs       | `rfd` (0.15)                            | Native folder/file picker (macOS `NSOpenPanel`). "Add project" uses `pick_folder()` so the repo path is chosen, not typed — a folder is required by construction. |
| Async runtime      | `tokio`                                 | Workers/tasks only. Features incl. `process` — the `projects` feature shells out to `git`/the editor launcher/a project's setup script off-thread (§10); no git *library* crate is used. |
| Database           | PostgreSQL (via Docker)                 | Local, persistent volume. |
| DB driver          | `sqlx` (0.9, runtime-checked queries)   | Builds WITHOUT a live DB. Migrations embedded. |
| Errors             | `thiserror` (enum + sub-enums)          | See §3. |
| Serialization      | `serde` / `serde_json`                  | |
| IDs                | `uuid` (v4)                             | All primary keys. |
| Time               | `chrono`                                | `created_at` / `updated_at`. |
| Logging            | `tracing` + `tracing-subscriber`        | Console + a per-run file log at `~/.dev-dash/log.txt` (see §3). |
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
├── logging.rs           Shared: tracing setup — console + per-run `~/.dev-dash/log.txt` (§3).
│
├── domain/              Pure data types. No I/O. Serde-able. One folder per feature.
│   ├── mod.rs
│   ├── profile/         Profile (+ ProfileView — the persisted per-profile last-viewed page).
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
│   │                       external-command boundary (git reads/worktree ops + editor launch +
│   │                       per-worktree setup-/teardown-script run, §10).
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
│   │                       `View` = projects (with live GitStatus) + all worktrees + the
│   │                       in-flight worktree-provision set (`creating`). Off-loop worktree
│   │                       create/recreate (+ setup script) and remove (+ teardown script)
│   │                       spawned here (§10).
│   └── todos/              todos::{Event, View, handle()} — add / set_done / delete.
│
└── ui/                 PURE rendering. No DB. One folder per feature + the shell + kit.
    ├── mod.rs              `DashboardApp` (eframe): shell nav, workspace, error modal.
    ├── theme.rs            Design system: palette, fonts, visuals, radii, frames, grid (§7).
    ├── components/         Shared component kit: input.rs, button.rs, card.rs, dnd.rs (§7).
    ├── dev.rs              Dev-only `DEV_VIEW` screen overrides for visual review (§8).
    ├── home/               UI-ONLY feature: the cross-feature Overview/Home tab. Aggregates every
    │                       other feature's `ViewData` slice into summary tiles + recent-work
    │                       lists; emits navigation intents (`HomeOutcome`) + existing feature
    │                       events. No `domain`/`system`/`app` (nothing to persist — it's a view).
    ├── profile/            Onboarding "setup profile" screen + its transient UI state.
    ├── tasks/              mod.rs board (+ live ticket SEARCH box) + part renderers: stage.rs,
    │                       ticket.rs, note.rs, modal.rs.
    ├── notes/              Notes tab: composer + note rows + the "Add to ticket" picker.
    ├── projects/           Projects tab: card grid, project detail page (setup- + teardown-script
    │                       sections + live worktree rows), worktree loading/rows + add-project /
    │                       create-worktree / edit-script modals (one modal for both scripts).
    └── todos/              Todos tab: composer + open-todo rows (done checkbox + delete).
```
(`static/assets/fonts/Nunito.ttf` — SIL OFL, embedded via `include_bytes!`. Not a crate.)

### Top-level layout (non-`src/` files)
Everything the app embeds or shells out to lives under **`static/`** so the repo root stays
lean: `static/assets/` (embedded fonts + `icon/` — the app-icon `AppIcon.svg` source + generated
`AppIcon.icns` and the embedded `AppIcon-512.png`, §14), `static/migrations/` (embedded via `sqlx::migrate!`), `static/docker/` (both
compose files), `static/scripts/` (the `db-*`/`sandbox-db` helpers + `window-id.swift`, the
screenshot window-id helper — §8, + `bundle-macos.sh` the macOS `.app` bundler & `icon-gen.sh` the
icon generator — §14), `static/screenshots/` (the committed gallery, §11), and `static/tmp/`
(gitignored scratch, §8). Build OUTPUT that is not a source input lives at the repo root under
**`builds/`** (gitignored): `dev-dash mac bundle` writes `builds/macos/DevDashboard.app` there (§14).
The `dev-dash` wrapper and the source `include_bytes!`/`sqlx::migrate!` paths point into `static/`;
moving any of these means updating those references + `.claude/settings.json` in the same change.
The project's Claude memory lives at **`.claude/CLAUDE.md`** (auto-loaded) and just imports this
file via `@../AGENTS.md` — this file, at the repo root, remains the source of truth.

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
  picker) — mirroring the notes→create-ticket coordination. The **reverse** also exists: a worktree
  row on the **project detail** has an "Open ticket" button that raises an open-ticket request
  (`ProjectsState::take_pending_open_ticket`), which the shell hands to the **board**
  (`BoardState::open_ticket`) — opening the detail OVER the current tab (see ticket navigation
  below), no longer forcing a jump to Tasks.
- The UI thread MUST NEVER block on I/O. All DB/async work happens on the tokio worker.

### Ticket navigation (tickets are the most-featured model — opening one behaves the same everywhere)
Every **ticket link** in the app — board cards, parent/child quick-links, the Home overview, a
project worktree's "Open ticket" — funnels through one gesture (`ui::tasks::ticket_open_from`):
**left-click opens the quick modal, right-click opens the full page.** All those call sites carry a
`ui::tasks::TicketOpen` (Modal/Page) rather than a bare id, so the presentation is chosen at the
click, not hard-coded per site.
- **The detail is tab-independent.** The full-page detail renders as a workspace TAKEOVER over
  whatever tab is active (the shell checks `BoardState::has_expanded_ticket()` before routing to the
  active tab), and the modal floats as an overlay over any tab. So a ticket opened from Home/Projects
  shows in place and returns you there — features never force-switch to Tasks to show a ticket.
- **Real back-navigation.** `BoardState` keeps a `back_stack`. Opening a ticket from OUTSIDE a detail
  (`open_ticket`) clears it; following a link from WITHIN a detail (`navigate_to`) pushes the current
  ticket; **"Back"** (`go_back`) pops to the previous ticket — restoring its presentation — or, when
  the stack is empty, closes the detail and returns to the tab underneath. "Expand" (`expand_current`)
  pushes too, so Back returns to the modal. A left-click inside a detail continues in the current
  presentation (page stays page); a right-click always forces the page. The X/backdrop closes the
  whole detail (clears the stack), and **clicking a nav tab** also closes it (`close_detail`) — since
  the detail renders OVER the active tab, switching tabs must drop it or the tab click looks dead.
  `reconcile` prunes back-entries for tickets deleted elsewhere.

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
  worktrees (bad path, not-a-repo, duplicate worktree, branch name that escapes the worktree root);
  `ProcessError` is an external command
  (git / the editor launcher) failing to spawn or exiting non-zero — kept separate on purpose (§10).
- `#[error("...")]` messages state **what** failed and, where actionable, **how to fix**.
- Errors crossing into the UI become a `UserFacingError { title, detail, remediation, retryable,
  output }` so the modal can render a clear, actionable message. The conversion lives in `error.rs`
  (`from_app_error`). **`output` carries an external command's raw stderr verbatim** — a
  `ProcessError::Exited` (git / a setup script / the editor launch failing) puts its `stderr` here,
  and the modal shows it in a monospace, scrollable block. So any subprocess failure surfaces
  *exactly* what the process said (e.g. `bun: command not found`, git refusing a dirty worktree),
  not a paraphrase; keep this — don't collapse a process error into a bare message.

### Example shape (illustrative)
```rust
#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("cannot reach PostgreSQL at {target}. Is the database running? Try `./static/scripts/db-up.sh`")]
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
  without a live database. All schema changes go through a numbered file in `static/migrations/`.
- **IDs & time:** `uuid::Uuid` (v4) for PKs; `chrono::DateTime<Utc>` for timestamps.
- **Logging:** use `tracing` spans/events. Log every handled error at the boundary with
  enough context to fix it. Never log secrets or full connection strings with passwords.
  `crate::logging::init()` (called first in `main`) sends tracing to the console AND to a **per-run
  file log** at `~/.dev-dash/log.txt`, TRUNCATED each launch so it holds only the current run —
  this is the prod-debug trail (in a release build the app has no console). It captures app
  diagnostics plus **subprocess output**: `git::run_setup_script` logs a header (cwd + the exact
  script) BEFORE running, then the full stdout/stderr and exit status after, so you can see whether
  a project's setup script (e.g. `bun install`) ran and what it printed. When you shell out to a
  process whose output matters for debugging, log it the same way (header first, then output).
  **Level discipline keeps the default log an activity trail, not noise:** log meaningful,
  low-frequency actions at `info` — every explicit git command (`git::run`), the worktree
  create/adopt/remove lifecycle, and profile/project create/switch/delete; log high-frequency or
  best-effort things at `debug` (per-project `git fetch`, the worker's per-event dispatch trace) so
  they're off by default but available with `RUST_LOG=my_dev_dashboard=debug`; a destructive
  cascade (profile delete) logs at `warn`. Follow this when adding logs — a new state-changing
  action is usually an `info` line; a per-frame/per-snapshot thing is `debug` or nothing.
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
  pinned** (`name: my-dev-dash` in `static/docker/docker-compose.yml`, mirrored by `COMPOSE_PROJECT_NAME` in
  `static/scripts/_common.sh`) so a directory rename can't orphan the container. See `README.md` for setup.
- Helper scripts in `static/scripts/` (`db-up`, `db-down`, `db-reset`, `db-psql`) wrap the common
  operations; run them via the allowlisted wrapper — **`./dev-dash db {up,down,reset,psql}`** —
  not a bare `docker compose`. `db-up`/`db-down`/`db-reset` share `start_db`/`stop_db` helpers
  in `_common.sh`; **`db reset` = down → wipe volume → up** (leaves a fresh, running DB; the app
  migrates on next launch). Extend these rather than documenting manual `docker` steps.
- Migrations live in `static/migrations/NNNN_name.sql`, applied automatically at app startup via
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
   warnings) → `./dev-dash build dev`. Use `./dev-dash build dev` (or `prod`), **never a bare
   `cargo build`** (not allowlisted → prompts; the wrapper is). No tests (above).
4. **Update AGENTS.md + `README.md` in the same change** whenever you add/alter a crate,
   feature, part, table, migration, error variant, module, command, `DEV_VIEW`, or design token.
   This is the maintain-this-file rule at the top — treat it as part of "done".
5. Prefer small, obvious code over cleverness. This is a personal tool — clarity wins.

### The `dev-dash` wrapper (build + screenshots)

`./dev-dash` is the trusted, pre-approved entry point for building and for the whole
build → launch → capture → kill screenshot dance (it handles the macOS gotchas: `sleep` is blocked
so it uses `perl` timing). Prefer it over hand-rolling `screencapture`. **Both `shot` and `snap`
capture ONLY the app window — never the macOS menu bar (top) or dock (bottom), which can leak other
apps/notifications (opsec).** They do this by resolving the app's window id via
`static/scripts/window-id.swift` (a CoreGraphics window-list lookup) and handing it to
`screencapture -o -l`, so the drop shadow is dropped too and the frame is a clean window rectangle.
Because `-l <id>` grabs the window's own image even when it's occluded, **no window-raising is
needed** — so this path needs only **Screen Recording** permission, NOT Accessibility.
**`shot` vs. `snap` target the right window by TITLE (opsec):** a `DEV_VIEW` run titles its window
`Dev Dashboard [DEV: <view>]` (set in `main.rs`), the live app is plain `Dev Dashboard`;
`window-id.swift` takes a `dev`|`live` arg so `shot` (dev) captures only the mock and `snap` (live)
only the running app — a mock shot can't leak the owner's live data and a live snap can't grab a
stray mock, even with both on screen at once. If no matching window is found it warns and falls back
to a full-screen grab, so a capture never silently produces nothing.

```bash
./dev-dash build dev                              # compile debug (allowlisted; use instead of cargo build); `prod` for release
./dev-dash shot VIEW static/tmp/screenshots/NAME.png     # capture one DEV_VIEW screen, then Read the PNG
./dev-dash snap [static/tmp/screenshots/live.png]        # capture the ALREADY-RUNNING app (real data)
./dev-dash open [prod]                            # launch detached; loops on Restart (see below)
```

The in-app **"Restart"** button (nav footer, under Refresh) exits with `RESTART_EXIT_CODE`
(**86**, in `src/main.rs` — chosen clear of reserved bands: 0–2, sysexits 64–78, Rust panic 101,
128+signal). `dev-dash open` runs the app in a loop that catches exactly that code and
**re-runs `cargo run`** (dev, the default) or **rebuilds + relaunches** (`prod`); any other exit
(incl. a normal close) ends the loop. Keep the `86` in `dev-dash`'s `open` loop in sync with the
constant. **The Restart button only exists in dev/debug builds** — it's gated on
`cfg!(debug_assertions)`, so release builds (the `.app` bundle, `dev-dash open prod`) omit it
entirely, because a release/Finder launch has no relaunch loop to catch the exit code (§14).

> **`dev-dash` is a trust boundary — do not edit it casually.** `.claude/settings.json`
> allowlists the safe read-only-ish subcommands (`build`/`shot`/`snap`/`sandbox`) to run without
> prompting, and `dev-dash` internally runs `swift`/`screencapture`/`pkill`/`perl` (none
> individually allowlisted); `db`/`open`/`bootstrap` are outright **denied** (they hit the prod DB,
> §12). Because the allowlisted subcommands are auto-approved, **editing the wrapper is
> deliberately gated**: the settings `ask` rules force a confirmation prompt on
> `Edit(dev-dash)`/`Write(dev-dash)` even under auto-accept. Never remove that guard, widen the
> allowlist to the underlying system tools, or allowlist a subcommand that reaches the prod DB.

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

> **Button hit-area rule:** A button's clickable/hover area must match its **visible** boundary.
> `egui::Ui::columns` (used by the detail pages) makes each column `top_down_justified`, which
> stretches a bare widget to the full column width — for a **frame-less** button (transparent fill:
> `ghost`/`link`/`danger`/`icon`) that means only the text shows but the whole empty column half is
> clickable. So those roles route through `button::add_hugging`, which re-adds them in a
> non-justified sub-scope (a no-op outside a justified layout) to size them to their content.
> **Filled** buttons (`primary`/`secondary`/`compact_primary`) are left as-is: their fill *is* the
> boundary, so full-width is fine and sometimes deliberate (e.g. the Notes actions). If you add a
> new frame-less button role, hug it too; don't place a bare frame-less button in a justified column
> without it.

> **Drag-and-drop rule:** All drag-and-drop goes through `ui/components/dnd.rs` so it behaves
> consistently. A dragged item lifts onto a floating layer and follows the pointer **from the
> exact grab point** — never re-centre it on the cursor. Use `dnd::drag_ghost` in the
> `is_being_dragged` branch (tickets and stage columns both do). Payload types are per-feature
> and DISTINCT so drop targets can tell them apart (tickets use `Uuid`; stage reorder uses a
> `StageDrag` newtype). **What is grabbable differs by item:** a **ticket card is draggable in
> its entirety** (one `click_and_drag` over the whole card — click opens the detail, drag
> reorders; the 6-dot grip is now only a visual affordance), whereas a **stage column** is
> dragged by its grip alone. Don't reintroduce a grip-only drag zone on tickets.

### Baked-in aesthetic decisions (do not fight these)
- **Framework:** stay on `egui` — it themes fully. Do NOT add a UI framework or an egui
  theme/component crate: they lag egui's version (e.g. `catppuccin-egui` capped at egui 0.30
  while we're on 0.35) and wouldn't match this look. We own the theme.
- **Palette:** soft-dark, **teal** accent. One `Palette` (`theme::DARK`).
- **Font:** Nunito (rounded, un-blocky), embedded from `static/assets/fonts/Nunito.ttf`.
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

**How:** `./dev-dash shot VIEW static/tmp/screenshots/NAME.png`, then Read the PNG (see §6 for the
wrapper). Write shots into **`static/tmp/screenshots/`** (gitignored scratch, kept via `.gitkeep`) so
you can open them in the IDE. Capture each affected `VIEW` (e.g. both `ticket` and `page` for a
detail-view change).

> **Clean up your scratch shots before you're done.** Any PNG you write into
> `static/tmp/screenshots/` is throwaway verification scrap — delete the ones you created once
> you've looked at them (leave `.gitkeep` and the owner's `live.png` alone). It's gitignored so
> it won't be committed, but don't leave stale scratch lying around. This is separate from the
> committed gallery under `static/screenshots/` (§11), which you DO keep up to date.

> **"Look at my app" is a protocol.** When the owner says *look at my app / see what I'm
> seeing / take a screenshot of what's open*, they mean the **already-running** instance with
> their real data — screenshot it with `./dev-dash snap [OUT]` (default `static/tmp/screenshots/live.png`)
> and Read it. Unlike `shot`, `snap` does NOT build, launch, or close anything; it just captures
> the live window (matched by its plain `Dev Dashboard` title, so a stray `DEV_VIEW` mock is never
> grabbed). (Errors if the app isn't running — tell them to `dev-dash open`.)

**`DEV_VIEW` screens** (in `ui/dev.rs`): the app injects mock in-memory state — no DB, no
seeding — so a screen renders instantly and works with the DB down; while set, worker snapshots
are ignored. The wrapper passes `VIEW` through as `DEV_VIEW`. Available:

| `VIEW` | Screen |
|--------|--------|
| `home`        | The cross-feature Overview, populated across every feature |
| `home-empty`  | The Overview with an active profile but no data (every section empty) |
| `onboarding`  | First-run: create your first profile |
| `new-profile` | "New profile" create screen (switcher top-left) over existing profiles |
| `profile-select` | Profile picker: no active profile but others exist (post-delete / reselect) |
| `board`          | Populated Tasks board (profiles "Work"/"Personal") |
| `board-empty`    | Tasks board with no stages (empty state) |
| `ticket`         | Ticket detail modal |
| `page`           | Ticket detail, full-page (expanded) |
| `ticket-back`    | Ticket detail modal drilled in from another ticket — shows the "← Back" affordance |
| `create`         | New-ticket create modal (with stage picker) |
| `stage-edit`     | Edit-stage modal (name + terminal toggle + delete) |
| `confirm-delete` | Destructive-action confirmation (delete ticket) — the shared confirm modal |
| `notes`          | Notes tab, populated |
| `notes-empty`    | Notes tab with no notes (empty state) |
| `notes-file`     | Notes tab with the "Add to ticket" picker open |
| `todos`          | Todos tab: open tasks (the mock's one done todo is hidden) |
| `todos-empty`    | Todos tab with nothing to do (empty state) |
| `board-search`   | Tasks board with a search query active, filtering tickets across every column |
| `projects`       | Projects tab: card grid (up-to-date / out-of-sync / no-origin states) |
| `projects-empty` | Projects tab with no projects (empty state) |
| `projects-loading` | Projects tab mid-refresh — cards + header show the git-status spinner |
| `projects-pulling` | Projects tab with a one-click Pull in flight — the card's "Pulling…" spinner |
| `add-project`    | The "add project" modal over the grid (native folder picker + name) |
| `project`        | A project's full-page detail (metadata + setup + teardown scripts + worktrees) |
| `setup-script`   | The "edit setup script" modal over the project detail (per-worktree bash, run on create) |
| `teardown-script`| The "edit teardown script" modal over the project detail (per-worktree bash, run on remove) |
| `create-worktree` | Create-worktree picker with the branch dropdown open (existing branches + "New branch…") |
| `create-worktree-fresh` | Create-worktree picker for a ticket with no branches yet — the branch picker is a plain text field |
| `worktree-recreate-as` | Recreate a removed marker under a NEW branch — the non-destructive per-worktree branch switch |
| `worktree-creating` | Ticket detail with a worktree mid-provision — its setup-script spinner |
| `worktree-removing` | Project detail with a live worktree being removed — its "Removing…" spinner |
| `loading`        | The pre-first-snapshot loading screen (spinner before any data arrives) |
| `error`          | The error modal (retryable DB outage — Retry/Dismiss) |
| `error-output`   | The error modal for a failed external command — shows the process's raw stderr in a monospace block |

(The `board`/`ticket`/`page` mocks also carry projects + worktrees, so the ticket detail's
worktree section renders under `DEV_VIEW=ticket`/`page`; one mock project carries both a setup and
a teardown script so the `project`/`setup-script`/`teardown-script` views render it populated. The
`*-empty` views (incl.
`home-empty`) share one `dev::mock_empty()` — a profile with no feature data — differing only by
active tab. The `home` view uses `dev::mock_home()` = the `board` mock + loose notes, so all four
Overview tiles and every section render with content.)

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

- **Exactly one — or zero — active profiles.** `profiles.is_active` (partial unique index → at
  most one true) marks it; `ProfileService::set_active` flips it atomically with
  `UPDATE profiles SET is_active = (id = $1)`. `create()` makes the new profile active. `active()`
  returns ONLY the explicitly-active row (`WHERE is_active`), or `None` — it deliberately does
  NOT fall back to the oldest profile. So "no active profile" is a real state: first run (no
  profiles) OR right after the active profile was deleted.
- **Deleting a profile** (`ProfileService::delete`, from the nav switcher's "Delete current
  profile", behind a confirmation — §13) removes the row; the cascade below wipes its ENTIRE
  workspace. It does NOT activate another profile — the owner chooses next on the picker (below).
  Deleting every profile is how you wipe the DB. (On-disk worktree folders survive, like project
  delete; the create guard in §10 adopts them if a worktree is ever remade.)
- **Scoping columns.** Top-level tables carry `profile_id … REFERENCES profiles(id) ON DELETE
  CASCADE` (`stages`, `uncategorized_notes`, `projects`, `todos`). Nested entities inherit their profile
  through their parent rather than duplicating the column: **tickets** via their `stage` (list
  joins `stages`), **ticket-notes** via their `ticket`, **worktrees** via their `project` (lists
  join `projects`). Deleting a profile cascades its whole workspace (stages→tickets→notes/worktrees,
  uncategorized_notes, projects→worktrees, todos) in one `DELETE`.
- **Resolving the active profile.** Handlers that create profile-scoped rows call
  `app::profile::active_id(&Backend) -> Result<Uuid, AppError>` (a `ProfileError::NoActive` if
  none) so the UI never threads a profile id through events. `ViewData::load` scopes the board,
  notes, projects, and todos to `profile.active_id()`; a fresh snapshot after any change reloads
  the active profile's data.
- **Last-viewed page is per profile.** `profiles.last_view` (TEXT, migration 0009, default
  `'tasks'`) records which workspace page each profile was on, so **switching profiles or
  relaunching restores where the owner left off** rather than always landing on Tasks. It maps to
  the domain `ProfileView` enum (`Home`/`Tasks`/`Notes`/`Todos`/`Projects`; `from_db` degrades any
  unknown value to the default `Tasks` so a stray string never breaks nav), which the UI `Tab` converts
  to/from. Writing it is a **quiet write-through**: clicking a nav tab sends
  `profile::Event::set_last_view(view)` → `ProfileService::set_last_view`, which does an `UPDATE`
  and **emits NO snapshot** (the UI already shows the tab — re-snapshotting every tab click would
  needlessly reload the whole workspace). Reading it back: the shell sets `active_tab` from
  `profile.active.last_view` on first load and on every profile switch (the same branch that
  resets transient state). A freshly-created profile has `last_view = 'tasks'` (the `INSERT` omits
  the column → DEFAULT), matching where onboarding drops you.
- **UI.** The nav shows a **profile switcher** (`ui/profile::render_switcher`, `SwitcherStyle::Nav`)
  — switch profiles, pick "New profile" (→ the new-profile onboarding flow), or "Delete current
  profile" (→ the shell's delete confirmation). The onboarding screen has three modes
  (`OnboardingMode::{FirstRun, NewProfile, Reselect}`); **new-profile mode** shows a top-left
  escape hatch — a **Back** link and a compact switcher — so you can leave without creating one
  (Back and picking any profile, including the current one, both exit; reported via
  `SwitcherOutcome::selected_current`). **First-run has no escape** (no profile to return to).
  **Reselect** is the "no active profile but others exist" state (e.g. just deleted the active
  one): the shell shows it (via `DashboardApp::render` choosing FirstRun vs Reselect on the
  profiles list) with a switcher to open one + a create field, but no Back. The shell resets
  transient board/notes/projects/todos state when the active profile changes so one profile's
  open modals don't bleed into another — and in that same branch restores `active_tab` from the
  new profile's persisted `last_view` (above).

---

## 10. Projects & worktrees

> **Points-at-repos rule:** A **project is an existing local repository path** (profile-scoped,
> §9). This tool NEVER clones — the owner enters a path they already have; `create` validates
> the path exists and is a git repo (else a typed `ProjectError`). Only durable identity
> (name + path) is stored; git facts are read live (below), never persisted **to the DB**.

**Git is computed by shelling out, cached for the session, refreshed on open + on demand.**
Origin URL, current branch, clean/dirty, and ahead/behind are computed in
`system/projects/git.rs` and travel in the `View` as a `GitStatus`. Because a status read
**fetches first** (`git fetch`, bounded by a timeout) it can be network-bound, so it must NOT
run on every snapshot — otherwise every stage/note/todo mutation pays N git fetches and the whole
app feels slow. Instead:

- **`ProjectService` holds a session cache** (`Arc<Mutex<HashMap<path, GitStatus>>>`, shared
  across `Backend` clones). `cached_statuses` reads it (never shells out) and is what
  `projects::View::load` — and therefore every snapshot — uses. `refresh_statuses` is the ONLY
  thing that shells out; it fetches all paths concurrently, stamps each `GitStatus.checked_at`,
  and writes the cache.
- **The fetch is offloaded, never blocking.** `app::projects::spawn_git_refresh` claims a CAS
  guard (`ProjectService::begin_refresh`, so concurrent refreshes don't pile up) and **`tokio::spawn`s**
  the fetch — so it never delays the Postgres snapshot, the worker's event loop, or the UI thread.
  When it lands it clears the flag (`end_refresh`) and emits a fresh snapshot. Callers snapshot
  immediately after kicking it, so the tab shows a loading state right away. The `refreshing` flag
  travels in `projects::View`.
- **A git refetch runs on:** (1) once on open — `Worker::refresh` kicks `spawn_git_refresh` then
  emits the DB snapshot immediately (git fills in a moment later); (2) on demand — the
  **"Refresh git"** button on the Projects grid + project detail page → `projects::Event::refresh_status()`
  → `project::Command::RefreshStatus`; (3) after **create project**, so a newly-added repo's status
  loads; (4) after a **Pull** (below), which refetches just that one project's path inline (via
  `refresh_statuses`, not the background `spawn_git_refresh`). Nothing else fetches. (The global nav
  "Refresh"/`ReloadAll` reloads the DB + reads the git cache; it does NOT refetch git.)
- **The UI shows load/checked state, never implies "live".** While `refreshing`, the grid header,
  each card's badge, and the detail page show a spinner. Otherwise the grid header shows
  "Checked HH:MM" (most recent `checked_at`) and the detail page shows it per project;
  `checked_at = None` (a profile whose projects were never fetched) reads as "not checked", not
  "not a repo". Separately, the whole app shows a **loading screen (not onboarding)** until the
  first snapshot arrives (`DashboardApp::loaded`) — an empty `ViewData` looks identical to "no
  profile", so without the gate a slow DB connect would flash the first-run flow.

Status reads stay **best-effort**: a non-repo / missing remote / failed fetch degrade to empty
fields and NEVER fail the snapshot; a failed fetch falls back to local refs and flags
`fetched = false`. "Up to date" = real repo + clean + in sync with upstream.

**Git is the only tool; the owner drives history.** Committing / pushing are done by the owner
**by hand** — the app never runs them, so the exact commands stay theirs. The app reads status,
manages worktrees, and offers **one** constrained history-changing action (below). Assume SSH keys
are loaded by the time the app runs.

**One-click Pull (the single exception).** On the **Projects view only**, a project whose current
branch is a shared integration branch (**`main`/`develop`**) and is **behind its upstream** with a
**clean** working tree gets a **Pull** button (grid card header, left of the git badge; and the
detail page header). It runs exactly `git pull --rebase origin <branch>` (`git::pull_rebase`) then
refetches **only that project's** status (`ProjectService::pull` → `project::Command::Pull`, settled
like a worktree op). The gate is `GitStatus::can_pull` (domain), and the service re-reads the branch
live and refuses anything but `main`/`develop` (`ProjectError::NotPullable`) so a stale card can't
drive a pull on a feature branch. Feature branches, dirty trees, and push/commit stay manual — the
owner drives those. A dirty tree makes `--rebase` refuse; that surfaces as a `ProcessError`, nothing
is silently rewritten. This is the ONLY place the app rewrites/advances refs.

**Worktrees.** A worktree lives OUTSIDE the repo, in a dev-dash-managed tree under the repo's
**parent** directory: **`{repo-parent}/.dev-dash/worktrees/{repo}/{branch}`** (the path convention
— never deviate; `{repo}` is the repo's own dir name, `{branch}` is the branch, so a slashed
branch nests). dev-dash owns this tree because worktrees are surfaced and driven from the
dashboard. A worktree is tied **1:1 to a ticket** within a project:

- **Ticket-driven creation only.** The only create entry point is a ticket's detail page
  (§2 coordination). A ticket may have **at most one worktree per project**; `worktree::create`
  rejects a duplicate (`ProjectError::WorktreeExists`).
- **Branch is chosen PER worktree.** `worktree::create` uses the requested branch verbatim; a
  ticket's worktrees may sit on **different** branches when project scope diverges (there is no
  longer a forced single "ticket branch"). The **branch picker** (`ui/projects/worktree::branch_picker`,
  used by the create + recreate-as modals) is dropdown-first: it lists the ticket's existing branch
  names — sourced from OUR worktree rows, NOT `git branch` — plus a trailing "New branch…" entry that
  reveals a text field to type a fresh one; picking an existing branch again is the way back out of
  typing mode. When the ticket has no branches yet it degrades to a plain text field. The create
  modal pre-selects the ticket's first existing branch as a convenience. Different projects are
  different repos, so each new branch still gets its own `-b` creation on `worktree add`.
- **The resolved path can't escape.** Before touching git or disk, `worktree::provision` builds the
  path with `domain::projects::worktree::checked_worktree_path`, which resolves `.`/`..` LEXICALLY
  (pure — no filesystem/symlink I/O) and refuses any name whose resolved path isn't exactly
  `{worktree_root}/{name}` — a `..` traversal or absolute component that would climb out of (or
  rewrite) the repo's worktree root. Such a name is rejected as `ProjectError::InvalidBranch` rather
  than allowed to drive a `git`/`rm`/editor call on an unintended path. (This is a defense-in-depth
  path guard, not a full git-ref validator: it catches traversal, while genuinely malformed refs —
  `~`, `^`, `:`, spaces — are literal path chars that stay in-root here and are caught by git itself
  on `worktree add`.) The raw `worktree_path` builder is still used for DISPLAY and for operating on
  already-validated stored names.
- **Adopt an existing folder.** `worktree::provision` checks the target path FIRST: if the folder
  already exists it skips `git worktree add` (and branch creation) entirely and just (re)creates
  the tracking row. This is deliberate — deleting a profile or project drops worktree ROWS via the
  cascade but leaves the on-disk folders (§9), so a later create at the same path must adopt what's
  there rather than let git error on an existing path. We trust the structure was set up for us.
- **Delete leaves a marker.** Removing a worktree (via the UI, or the cleanup on ticket delete)
  runs `git worktree remove` (NOT forced — a dirty tree makes git refuse, and that surfaces so
  nothing is lost) and sets `removed_at`, keeping the row as a **historical marker** of the
  branch name. It can be **recreated** from that marker (same branch + folder) — **only from the
  ticket it originates from.** Recreation re-runs the setup script (removal deleted the folder, so
  the provision is fresh — see below). Like create, a UI-driven removal is **off-loop with a
  "waiting" state**: `app::projects::spawn_worktree_remove` guards it per-worktree-id via
  `WorktreeService::{begin,end}_busy` (a double-click is a no-op) and the row swaps its buttons for
  a "Removing…" spinner (`projects::View::busy`) until `settle_reload` lands (§8 `worktree-removing`).
- **Recreate under a NEW branch (non-destructive branch switch).** A marker also offers "Recreate
  as…" (`worktree::recreate_as` → `worktree::Command::RecreateAs`), which recreates it onto a
  branch chosen in the branch picker instead of its original one. It affects **only that worktree**
  — the ticket's other worktrees are untouched — and the marker's **old git branch is never
  deleted** (only the row's `name`/`branch` are repointed). This is how a ticket's scope change in
  one project is absorbed without disturbing the rest. It's marker-only (to switch a LIVE worktree,
  remove it first, then recreate-as) and, like the same-branch recreate, runs through
  `spawn_worktree_recreate` (now taking an `Option<branch>`) with the fresh-provision setup run. The
  modal is projects-owned; the ticket detail raises the request via the shell (§2), mirroring create.
- **Markers are ticket-only in the UI.** The project detail lists **live worktrees only** (what's
  checked out right now); removed markers show exclusively on their originating ticket's detail,
  which owns recreation. Don't surface markers on the project page.
- **Reconcile against disk.** Before building a projects snapshot, `worktree::reconcile` flips
  any live worktree whose folder has vanished (owner deleted it outside the app) to a marker, so
  counts stay honest and it stays recreatable.
- **Ticket delete cleans up.** Deleting a ticket first best-effort-removes its live worktree
  folders (`remove_all_for_ticket`) before the rows cascade away, avoiding orphaned folders.

**Setup script (per project, run on worktree creation).** A project carries an optional
`setup_script` (a `TEXT` column on `projects`, empty = none) — a bash script run in the working
directory of every **freshly-provisioned** worktree, so a new checkout is ready to work in (e.g.
`bun install`). Edited via a modal on the project detail (`SetSetupScript` → `set_setup_script`).
The detail page reads as full-width bands — a `Repository | Git status` metadata row, then the two
scripts as a matched `Setup | Teardown` pair, then the worktrees full-width, then delete — so setup
and teardown sit side-by-side rather than stacked (`render_script_section` renders either, by kind).
Rules:
- **Runs on a fresh provision only** — a first-time create OR a recreate (removal deleted the
  folder), never on an ADOPTED existing folder (that was already set up). `git::run_setup_script`
  is the boundary — a thin wrapper over the shared `git::run_script` (which teardown also uses):
  `bash -c <script>` in the worktree dir, a typed `ProcessError` on non-zero exit.
  Because it's `bash -c`, the script needs **no shebang** — a `#!/usr/bin/env bash` first line would
  be an inert comment (bash is already the interpreter), so don't put one in examples/placeholders.
  It also logs a header (cwd + the exact script + the PATH used) and the full captured stdout/stderr
  + exit status to the per-run file log (`~/.dev-dash/log.txt`, §3), so a run that hangs or fails
  early is diagnosable there; on failure the modal shows the stderr verbatim too (§3 `output`).
- **PATH is resolved from the owner's login shell.** A GUI/Finder launch starts with only the
  minimal launchd PATH (`/usr/bin:/bin:…`), and a plain `bash -c` sources no rc — so tools installed
  under `~/.bun/bin`, `~/.cargo/bin`, Homebrew, nvm, etc. would be "command not found". Before
  running, `git::login_shell_path` runs the owner's `$SHELL` as a **login + interactive** shell
  (which sources `~/.zshrc`/`~/.zprofile` where those PATH exports live) and reads back `$PATH`,
  which is then set on the `bash -c` child. Best-effort: if it can't resolve, the script inherits
  the app's PATH (and the header logs which was used). This is the *only* command that needs the
  owner's PATH; git/editor launches use absolute-ish tools already on the minimal PATH.
- **Provisioning is off-loop + shows a loading state.** git-add + the (possibly slow) setup script
  never run on the worker's event loop — `app::projects::spawn_worktree_create` /
  `spawn_worktree_recreate` spawn them, guarded per-`(project, ticket)` by
  `WorktreeService::{begin,end}_create` (a double-click is a no-op). The in-flight set rides in
  `projects::View::creating`; the ticket/project detail shows a "Setting up… running setup script"
  spinner and the worktree is NOT presented as ready until it lands.
- **A setup-script failure is NON-fatal.** The worktree is still created and tracked; the setup
  error is surfaced in the modal (via `Emitter::settle_reload`, which always re-snapshots so the
  worktree appears AND the error shows) so the owner can fix the script and re-run in place. Only a
  git/DB provisioning failure is fatal (no worktree). Setup is therefore run as a SEPARATE step
  (`worktree::run_setup`) AFTER `create`/`recreate` return `(Worktree, fresh)`, not inside
  `provision`, so it can't roll back an already-created worktree.

**Teardown script (per project, run on worktree removal).** The mirror of the setup script: a
project carries an optional `teardown_script` (a `TEXT` column on `projects`, empty = none) — a bash
script run in the working directory of a worktree **right before it is removed**, so removal cleans
up whatever setup (or the owner) stood up (e.g. `docker compose down`, stop a dev server). Edited via
the SAME editor component on the project detail (`SetTeardownScript` → `set_teardown_script`), shown
beside the setup script as the right half of the script pair. Same mechanics as setup, differing
only in timing:
- **Runs on removal only, on a LIVE worktree** — `worktree::run_teardown` loads the row, skips a
  marker (its folder is already gone) and an empty script, then runs `git::run_teardown_script`
  (the other thin wrapper over the shared `git::run_script`) in the worktree dir. Same `bash -c`,
  no-shebang, login-shell-PATH, and run-log-header behaviour as setup.
- **Runs BEFORE `git worktree remove`** — the folder must still exist. `spawn_worktree_remove` runs
  teardown first, then the git remove, both off the worker loop under the existing "Removing…" busy
  guard (§13 confirm still applies).
- **A teardown failure is NON-fatal** (like setup): the removal proceeds regardless (a broken
  teardown can't strand a worktree the owner asked to remove), and the teardown error is surfaced
  only when the git remove itself succeeded — a git refusal (dirty tree, which leaves the worktree
  live) takes precedence as the actionable failure. Run as a SEPARATE step from `remove`, so it
  never blocks it.

**Open in VS Code.** A worktree row's "Open in VS Code" launches `open -a "Visual Studio Code"
<path>` off the worker thread (`app::projects::spawn_worktree_open`). It changes no state, but the
click still gets immediate feedback: the same per-worktree `busy` guard shows an "Opening…" spinner
on the row, and `settle_reload` clears it (and surfaces a launch error) when the launch returns. It
lives beside git in `system/projects/git.rs` (with the editor launch and the setup-/teardown-script
runner — the non-git external commands) and rolls up as `ProcessError` like git does.

**Schema.** `projects` (profile-scoped, cascades; carries `setup_script` + `teardown_script`) + `worktrees` (a churny, lightweight table:
`project_id`, `ticket_id`, `name`, `branch`, `removed_at`, with `UNIQUE(project_id, ticket_id)`
and a partial-unique on active `(project_id, name)`). Both cascade from their parents. Keep the
worktrees table lean — it takes common hard creates/deletes.

---

## 11. Screenshot gallery (keep it current)

> **The gallery is a maintained artifact, not a scratch dump.** `static/screenshots/` is a committed,
> browsable record of what every screen looks like — one folder per feature, one PNG per
> `DEV_VIEW`. The owner reviews flows here. **A screen whose look changed with STALE pixels in
> the gallery is a bug in your change.** (This is distinct from `static/tmp/screenshots/`, the
> gitignored scratch you capture into while iterating, §8.)

**Layout.** `static/screenshots/<feature>/<DEV_VIEW>.png` — the filename is exactly the `DEV_VIEW` key
(§8), so the mapping is 1:1 and unambiguous. `static/screenshots/README.md` is the index (per-feature
tables + inline thumbnails); regenerate/extend it alongside the images.

| Folder | Screens (`DEV_VIEW`) |
|--------|----------------------|
| `home/`     | `home`, `home-empty` |
| `profile/`  | `onboarding`, `new-profile`, `profile-select` |
| `tasks/`    | `board`, `board-empty`, `board-search`, `ticket`, `page`, `ticket-back`, `create`, `stage-edit` |
| `notes/`    | `notes`, `notes-empty`, `notes-file` |
| `todos/`    | `todos`, `todos-empty` |
| `projects/` | `projects`, `projects-empty`, `projects-loading`, `projects-pulling`, `add-project`, `project`, `setup-script`, `teardown-script`, `create-worktree`, `create-worktree-fresh`, `worktree-recreate-as`, `worktree-creating`, `worktree-removing` |
| `shell/`    | `error`, `error-output`, `loading`, `confirm-delete` (cross-cutting, not tied to a tab) |

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
- **New screen/feature** → add the `DevView` + mock (§8), create `static/screenshots/<feature>/` if
  new, capture the PNG(s), and add them to `static/screenshots/README.md`.
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
./dev-dash shot <DEV_VIEW> static/screenshots/<feature>/<DEV_VIEW>.png
# e.g. ./dev-dash shot projects static/screenshots/projects/projects.png
```

Then **Read the PNG** to confirm it rendered what you intended (§8) before reporting done. The
canonical list of every view lives in the §8 table; this gallery must mirror it exactly.

---

## 12. Production data & migrations (this is the owner's live utility)

> **This app now holds the owner's REAL, general-purpose data.** A careless migration can
> destroy or orphan it. Treat every schema change as production-affecting.

**Destructive migrations need sign-off — ask FIRST.** Any migration that *may* cause data loss
or make data inaccessible must be **discussed with the owner before you write or apply it**.
Non-exhaustive "must ask" list:
- `DROP TABLE` / `DROP COLUMN`, or renaming a table/column (a rename is a drop+add to old code);
- a type change that can't round-trip the existing values;
- `ADD COLUMN … NOT NULL` without a `DEFAULT` on a populated table;
- destructive data backfills / `UPDATE`/`DELETE` in a migration;
- removing or renaming anything the app still reads.

**Additive migrations are fine to add + verify without asking** — a new table, a new column with
a default (or nullable), a new index. When in doubt, ask.

**Verify against the SANDBOX, never production.** There are two totally separate DB stacks:

| | Production (the owner's data) | Sandbox (yours, for verification) |
|---|---|---|
| Compose | `static/docker/docker-compose.yml` | `static/docker/docker-compose.sandbox.yml` |
| Env | `.env` (owner's, git-ignored) | `.env.sandbox` |
| Project / container / volume | `my-dev-dash*` | `devdash-sandbox*` |
| Host port | 5433 | **5434** |
| Driven by | `dev-dash db …` (**DENIED** to agents) | `dev-dash sandbox …` |

- **What binds the data is the volume + project name, NOT the file location.** The compose files
  live under `static/docker/`; the `db-*` scripts reach them via the `compose()` helper in
  `_common.sh` (which passes `-f static/docker/docker-compose.yml`), and `sandbox-db.sh` points at
  `static/docker/docker-compose.sandbox.yml`. The production volume stays `external` + named
  `my-dev-dash-pgdata` and the project name pinned `my-dev-dash` — those two facts, not the path,
  keep the owner's data attached. **Never change the volume name, the `external: true` flag, or the
  pinned project name**, and never point the scripts at a different compose file/volume — any of
  those would orphan the owner's data. Moving the compose file itself is fine *as long as the
  scripts' `-f` path and those invariants move with it in the same change* (as this layout did).
- **Never touch the production stack.** Do not run `dev-dash db …`, do not `dev-dash open`
  (both hit the real DB — they're denied in `.claude/settings.json`).
- **Verify with `dev-dash sandbox migrate`.** It brings up the sandbox (5434), builds, and runs
  the app's real migration path headlessly via the `DEVDASH_MIGRATE_CHECK` gate in `main.rs`
  (connect → migrate → log → exit, no window). Confirm the log's `target` is `localhost:5434`.
  Other subcommands: `dev-dash sandbox {up|down|reset|psql|url}`. The sandbox script hard-refuses
  any `DATABASE_URL` that doesn't target the sandbox port.
- Screenshots (`dev-dash shot`) use DEV_VIEW mocks and touch **no** database — safe anytime.

**Agent permissions** (`.claude/settings.json`) enforce this: only `dev-dash build|shot|snap|
sandbox` and `cargo fmt|clippy` are allowed; `dev-dash db`, `dev-dash open`, and `dev-dash
bootstrap` are denied (bootstrap starts the prod DB stack, §14). Drive the sandbox through
`dev-dash sandbox`, not the raw `static/scripts/sandbox-db.sh`.

---

## 13. Destructive actions need confirmation

> **Every action that deletes or removes data/files is confirmed before it fires.** The owner
> holds real data (§12), so a delete must never be one stray click. New destructive actions
> follow this from day one.

**Transformative actions are NOT destructive — do NOT confirm them.** Turning a note into a
ticket, a note into a todo, filing a note onto a ticket, or completing a todo all delete/replace
the source as part of *becoming something else*. They fire immediately; a confirmation there
would be noise.

**One shared modal.** `ui/components/confirm::destructive(ctx, id_salt, title, body, confirm_label)
-> Choice` renders the single, consistent confirmation (red warning title, body, Delete/Cancel;
backdrop/Escape = Cancel). Never hand-roll a confirm dialog — use this so they all look alike.

**The pattern.** The feature's UI state holds an `Option<Id>` "pending confirm" slot. The danger
button *sets the slot* (it does NOT send the event); an overlay renderer shows `confirm::destructive`
while the slot is set; `Choice::Confirmed` sends the real event and clears the slot, `Cancelled`
just clears it. `reconcile` clears a slot whose entity vanished from the snapshot. Currently
gated: **delete ticket, delete stage, delete todo, remove worktree, delete project, delete
profile.** Cross-feature ones (remove-worktree raised from the ticket detail) route the request
to the owning feature via the shell, exactly like the create-worktree hand-off (§2).

---

## 14. macOS app bundle (`dev-dash mac`) & bootstrap

> **The bundle is a thin wrapper around the release build, not a redistributable.** This is a
> self-use tool (§0) tied to a local Postgres and the repo's own `target/`. `dev-dash mac bundle`
> just gives the owner a **double-clickable `.app`** (Dock/Spotlight/Finder) instead of a
> terminal launch — it is NOT a signed, self-contained, shippable artifact.

macOS packaging lives under the **`mac`** command group (OS-scoped, so other platforms can grow
their own later): `dev-dash mac` runs `copy` by default — it release-builds + assembles the bundle
**and** installs it into `/Applications` in one go. `./dev-dash mac bundle` is the build-only
variant: `static/scripts/bundle-macos.sh` release-builds and assembles
**`builds/macos/DevDashboard.app`** (gitignored output, §2 top-level layout) WITHOUT installing;
`dev-dash mac copy` (the default) adds the `/Applications` install. Structure:

```
builds/macos/DevDashboard.app/Contents/
├── Info.plist                    CFBundleExecutable = the launcher script; CFBundleIconFile =
│                                 AppIcon (CFBundleIdentifier io.github.coreyshupe.devdashboard;
│                                 version from Cargo.toml).
├── MacOS/
│   ├── DevDashboard              Launcher SCRIPT (the bundle executable).
│   └── my-dev-dashboard          SYMLINK → target/release/<bin> (absolute; NOT a copy).
└── Resources/
    ├── AppIcon.icns              COPIED from static/assets/icon/ (Dock/Finder icon).
    └── .env                      COPIED from the repo's .env at bundle time.
```

Three deliberate choices (do not "fix" them into a self-contained app):
- **Executable is a symlink, not a copied binary.** An absolute symlink to
  `target/release/<bin>` keeps the bundle tiny and means a later `cargo build --release` is
  picked up with no re-bundle. It ties the bundle to this repo's checkout — intended.
- **A launcher script, not the binary, is `CFBundleExecutable`.** Finder launches apps with
  `cwd=/`, but the app resolves `.env` via `dotenvy` from the working directory (§ config). So
  the launcher resolves its own location, `cd`s into `Contents/Resources` (where `.env` is
  copied), then `exec`s the symlinked binary — that `cd` is what makes config load. It does
  **NOT** loop on the Restart exit code (86): relaunch doesn't work from a Finder-launched
  bundle, so the **Restart button is compiled out of release builds entirely** (gated on
  `cfg!(debug_assertions)`, §8/footer) — Restart-relaunch stays a `dev-dash open` (dev/debug)
  feature. The symlink still means an external `cargo build --release` is picked up on the next
  launch.
- **`.env` is copied into the bundle.** The bundle carries its own config snapshot; editing the
  repo `.env` afterward needs a re-bundle (or edit `Contents/Resources/.env` directly). If no
  repo `.env` exists at bundle time the script warns (the app would error until one is present).

**App icon.** The icon is **self-drawn from the design system** (§7) — there is no external/stock
art. `static/assets/icon/AppIcon.svg` is the editable source: the teal `accent` tile (bubbly
rounded square) with the Material "dashboard" glyph recreated as four rounded panels (the same
motif shipped in `MaterialIcons-Regular.ttf`, Apache-2.0). `static/scripts/icon-gen.sh` rasterizes
it with `sips` (NOT QuickLook `qlmanage`, which flattens the SVG's transparency onto opaque white
— an ugly border; `sips` renders SVG natively AND keeps alpha) and, via `sips`/`iconutil`, packs every size into
`static/assets/icon/AppIcon.icns` **and** emits `AppIcon-512.png` for the app to embed;
**re-run it after editing the SVG**. All three (`AppIcon.svg`/`.icns`/`-512.png`) are committed.

The icon reaches the running app **two ways**, both needed:
- **`.icns` in the bundle** — the static Finder/`/Applications` icon of the `.app` file. The
  bundler copies it into `Contents/Resources/` (self-healing: regenerates it if missing).
- **Embedded PNG set via eframe** — `src/main.rs` `include_bytes!`s `AppIcon-512.png` and passes
  it to `ViewportBuilder::with_icon`. This is REQUIRED, not redundant: without an icon eframe
  loads a **default egui icon** and on macOS applies it at runtime via `setApplicationIconImage`,
  which overrides even the bundle's `.icns`. Handing eframe our icon makes the app own its Dock
  icon on **every** launch path (bundle, `cargo run`, `dev-dash open`). On a decode failure we
  pass an empty `IconData`, which eframe treats as "no icon" (leaving the OS default) instead of
  forcing its own. This uses only eframe's **public** API (`eframe::icon_data::from_png_bytes`);
  `image` is eframe's own dependency, so **no new crate** is added — the approved stack (§1)
  is untouched. Don't "simplify" this to a raw NSImage-from-PNG path: eframe decodes to raw RGBA
  on purpose to dodge a macOS libpng SIGBUS bug.

`dev-dash mac` touches **no** database — it only builds + copies files, so it's safe (it isn't on
the agent allowlist, so it prompts, but unlike `dev-dash db`/`open` it's not *denied*, §12). The
default **`dev-dash mac copy`** installs the bundle into `/Applications` (so Spotlight/Launchpad
find it) — `cp -R` preserves the absolute binary symlink, and it nudges LaunchServices (`touch`)
so Finder refreshes the icon; if `/Applications` isn't writable it fails loudly with a
`sudo cp -R` hint rather than half-installing. Both `bundle-macos.sh` and `icon-gen.sh` live in
`static/scripts/` and, like the other scripts, are edit-gated (§6).

**`dev-dash bootstrap mac`** is a one-shot machine setup for the owner: it **requires Docker to be
running** (errors out with a "start Docker Desktop" hint if not — it deliberately does NOT launch
Docker itself), then runs `db up`, `mac bundle`, and `mac copy` in sequence, leaving a running DB
and an installed `.app`. Because it starts the **production** DB stack it is **denied to agents**
(`Bash(./dev-dash bootstrap:*)` in `.claude/settings.json`, alongside `db`/`open`, §12) — never run
it. `bootstrap` is OS-scoped: only `mac`/`macos` is wired up today, and an unknown/absent target
errors rather than guessing, leaving room for other platforms' flows later.
