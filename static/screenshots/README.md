# Screenshot gallery

A visual index of every screen in the dashboard, one folder per feature. Each image is a
`DEV_VIEW` mock render (no database, no hand-entered data) captured with `dev-dash shot`, so it
shows a real, current frame of that flow. Browse here to see what any flow looks like at a
glance.

**This is a maintained artifact** — when a flow's look changes, its screenshot must be
recaptured in the same change. The rules, the full view↔file map, and the regen commands live in
[AGENTS.md §11](../AGENTS.md). Filenames match the `DEV_VIEW` key exactly (§8).

Regenerate everything (from the repo root):

```bash
./dev-dash shot <DEV_VIEW> screenshots/<feature>/<DEV_VIEW>.png   # one view
# e.g. ./dev-dash shot projects screenshots/projects/projects.png
```

---

## home

The cross-feature **Overview** — the app's landing tab. An at-a-glance roll-up: summary tiles,
recent tickets, open todos, repos needing attention, and loose notes.

| View | `DEV_VIEW` |
|------|-----------|
| Overview, populated across every feature | `home` |
| Overview with no data yet (every section empty) | `home-empty` |

![home](home/home.png)
![home-empty](home/home-empty.png)

## profile

Onboarding + the new-profile flow (no active tab; these take over the whole window).

| View | `DEV_VIEW` |
|------|-----------|
| First-run onboarding (no profiles yet) | `onboarding` |
| "New profile" create screen (from the switcher) | `new-profile` |
| Profile picker (no active profile, others exist) | `profile-select` |

![onboarding](profile/onboarding.png)
![new-profile](profile/new-profile.png)
![profile-select](profile/profile-select.png)

## tasks

The Tasks board and the ticket/stage flows.

| View | `DEV_VIEW` |
|------|-----------|
| Board with stages + tickets (incl. a collapsed terminal stage) | `board` |
| Empty board (profile, no stages) | `board-empty` |
| Board with a search query filtering tickets across columns | `board-search` |
| Ticket detail — modal overlay | `ticket` |
| Ticket detail — full page (worktrees + notes) | `page` |
| New-ticket create modal | `create` |
| Edit-stage modal (terminal toggle) | `stage-edit` |

![board](tasks/board.png)
![board-empty](tasks/board-empty.png)
![board-search](tasks/board-search.png)
![ticket](tasks/ticket.png)
![page](tasks/page.png)
![create](tasks/create.png)
![stage-edit](tasks/stage-edit.png)

## notes

Quick uncategorized capture + its filing actions.

| View | `DEV_VIEW` |
|------|-----------|
| Notes list (rows w/ Make Todo · Create Ticket · Add To Ticket) | `notes` |
| Empty notes | `notes-empty` |
| "Add to ticket" search picker | `notes-file` |

![notes](notes/notes.png)
![notes-empty](notes/notes-empty.png)
![notes-file](notes/notes-file.png)

## todos

Quick tasks (completed todos are hidden).

| View | `DEV_VIEW` |
|------|-----------|
| Todos list (open items; the mock's done one is hidden) | `todos` |
| Empty todos | `todos-empty` |

![todos](todos/todos.png)
![todos-empty](todos/todos-empty.png)

## projects

Local repositories + their git worktrees.

| View | `DEV_VIEW` |
|------|-----------|
| Projects grid (pullable / up-to-date / out-of-sync / no-origin cards) | `projects` |
| Empty projects | `projects-empty` |
| Projects grid mid git-status refresh (spinners) | `projects-loading` |
| Projects grid with a one-click Pull in flight ("Pulling…") | `projects-pulling` |
| Add-project modal (folder picker + name) | `add-project` |
| Project detail (setup + teardown scripts + metadata + live/removed worktrees) | `project` |
| Edit-setup-script modal (per-worktree bash, run on create) | `setup-script` |
| Edit-teardown-script modal (per-worktree bash, run on remove) | `teardown-script` |
| Ticket detail with a worktree mid-setup (spinner) | `worktree-creating` |
| Project detail with a live worktree being removed ("Removing…" spinner) | `worktree-removing` |

![projects](projects/projects.png)
![projects-empty](projects/projects-empty.png)
![projects-loading](projects/projects-loading.png)
![projects-pulling](projects/projects-pulling.png)
![add-project](projects/add-project.png)
![project](projects/project.png)
![setup-script](projects/setup-script.png)
![teardown-script](projects/teardown-script.png)
![worktree-creating](projects/worktree-creating.png)
![worktree-removing](projects/worktree-removing.png)

## shell

Cross-cutting overlays that aren't tied to one tab.

| View | `DEV_VIEW` |
|------|-----------|
| Blocking error modal (retryable DB error) | `error` |
| Error modal for a failed external command (raw stderr shown) | `error-output` |
| Pre-first-snapshot loading screen | `loading` |
| Destructive-action confirmation (delete ticket) | `confirm-delete` |

![error](shell/error.png)
![error-output](shell/error-output.png)
![loading](shell/loading.png)
![confirm-delete](shell/confirm-delete.png)
