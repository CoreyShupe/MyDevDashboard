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

## profile

Onboarding + the new-profile flow (no active tab; these take over the whole window).

| View | `DEV_VIEW` |
|------|-----------|
| First-run onboarding (no profiles yet) | `onboarding` |
| "New profile" create screen (from the switcher) | `new-profile` |

![onboarding](profile/onboarding.png)
![new-profile](profile/new-profile.png)

## tasks

The Tasks board and the ticket/stage flows.

| View | `DEV_VIEW` |
|------|-----------|
| Board with stages + tickets (incl. a collapsed terminal stage) | `board` |
| Empty board (profile, no stages) | `board-empty` |
| Ticket detail — modal overlay | `ticket` |
| Ticket detail — full page (worktrees + notes) | `page` |
| New-ticket create modal | `create` |
| Edit-stage modal (terminal toggle) | `stage-edit` |

![board](tasks/board.png)
![board-empty](tasks/board-empty.png)
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
| Projects grid (up-to-date / out-of-sync / no-origin cards) | `projects` |
| Empty projects | `projects-empty` |
| Project detail (metadata + live + removed worktrees) | `project` |

![projects](projects/projects.png)
![projects-empty](projects/projects-empty.png)
![project](projects/project.png)

## shell

Cross-cutting overlays that aren't tied to one tab.

| View | `DEV_VIEW` |
|------|-----------|
| Blocking error modal (retryable DB error) | `error` |

![error](shell/error.png)
