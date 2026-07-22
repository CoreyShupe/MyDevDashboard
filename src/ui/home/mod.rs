//! `home` feature UI: the **Overview** page — a cross-feature, at-a-glance roll-up.
//!
//! This is the app's namesake "dashboard": one calm screen answering "what needs my attention
//! right now" without hunting through tabs. It's a PURE aggregation of the other features'
//! already-loaded slices (`ViewData`), so — unlike every other feature — it has **nothing** in
//! `domain`/`system`/`app` (AGENTS.md §2 feature-mirroring rule allows omitting a layer a feature
//! genuinely has no work in). It never queries; it reads the snapshot the worker already built and
//! emits the same feature `Event`s / navigation intents everything else does.
//!
//! It surfaces:
//!   - **stat tiles** — active tickets, open todos, loose notes, repos needing attention (each
//!     clickable → its tab),
//!   - **pick up where you left off** — the most recently-touched active tickets (click → detail),
//!   - **open todos** — check one off inline (sends `todos::Event::set_done`),
//!   - **repositories needing attention** — dirty / ahead / behind repos,
//!   - **loose notes** — a preview of unfiled captures.
//!
//! Cross-feature navigation (switch tab / open a ticket) is returned as a [`HomeOutcome`] for the
//! shell to act on — the same hand-off pattern the Notes tab uses (AGENTS.md §2); inline mutations
//! (checking off a todo) go straight out as events.

use chrono::{Local, Timelike};
use uuid::Uuid;

use crate::app::{Bridge, ViewData};
use crate::domain::projects::GitStatus;
use crate::ui::Tab;
use crate::ui::components::card;
use crate::ui::theme;

/// How many rows each list section shows before deferring to its "view all" link.
const LIST_CAP: usize = 5;
/// The loose-notes preview is shorter — notes rows are taller (wrapped bodies).
const NOTES_CAP: usize = 3;

/// What the Overview asks the shell to do after rendering. Navigation and opening a ticket live
/// on the board / shell, so — like the Notes tab's create-ticket hand-off (§2) — Home reports the
/// intent rather than reaching across features itself.
#[derive(Default)]
pub struct HomeOutcome {
    /// Switch to this tab (a stat tile or a "view all" link was clicked).
    pub goto: Option<Tab>,
    /// Open this ticket's detail (a "pick up where you left off" row was clicked) — left-click
    /// opens the modal, right-click the full page (the shared ticket-link gesture).
    pub open_ticket: Option<(Uuid, crate::ui::tasks::TicketOpen)>,
}

/// Transient UI state for the Overview. It holds nothing today (the page is a pure projection of
/// the snapshot), but it exists so the tab matches every other feature's `*State` shape and has a
/// home for any future buffers.
#[derive(Default)]
pub struct HomeState;

impl HomeState {
    /// Render the Overview and return any navigation the owner requested.
    pub fn render_workspace(
        &mut self,
        ui: &mut egui::Ui,
        bridge: &Bridge,
        data: &ViewData,
    ) -> HomeOutcome {
        let mut outcome = HomeOutcome::default();

        self.render_header(ui, data);
        ui.add_space(16.0);

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                self.render_stat_tiles(ui, data, &mut outcome);
                ui.add_space(16.0);

                // Two balanced columns: work on the left (tickets + todos), repos & notes on the
                // right. `ui.columns` splits the available width evenly.
                ui.columns(2, |cols| {
                    self.render_recent_tickets(&mut cols[0], data, &mut outcome);
                    cols[0].add_space(14.0);
                    self.render_open_todos(&mut cols[0], bridge, data, &mut outcome);

                    self.render_projects_attention(&mut cols[1], data, &mut outcome);
                    cols[1].add_space(14.0);
                    self.render_loose_notes(&mut cols[1], data, &mut outcome);
                });
            });

        outcome
    }

    /// The greeting + date line. Time-of-day is display-only (not business logic), so reading the
    /// local clock here is fine (AGENTS.md §4 keeps *DB/async* work out of `ui/`, not a clock read).
    fn render_header(&self, ui: &mut egui::Ui, data: &ViewData) {
        let p = theme::palette();
        let now = Local::now();
        let greeting = match now.hour() {
            5..=11 => "Good morning",
            12..=17 => "Good afternoon",
            _ => "Good evening",
        };
        ui.heading(greeting);
        ui.add_space(2.0);

        let date = now.format("%A, %B %-d, %Y").to_string();
        let subtitle = match data.profile.active.as_ref() {
            Some(profile) => format!("{date}  ·  {} workspace", profile.display_name),
            None => date,
        };
        ui.label(egui::RichText::new(subtitle).color(p.muted).size(13.5));
    }

    /// A row of four clickable summary tiles. Each jumps to its feature's tab.
    fn render_stat_tiles(&self, ui: &mut egui::Ui, data: &ViewData, outcome: &mut HomeOutcome) {
        let active_tickets = data
            .tasks
            .tickets
            .iter()
            .filter(|t| !data.tasks.is_terminal(t.stage_id))
            .count();
        let open_todos = data.todos.todos.iter().filter(|t| !t.done).count();
        let loose_notes = data.notes.notes.len();
        let attention = data
            .projects
            .projects
            .iter()
            .filter(|c| attention_label(&c.git).is_some())
            .count();

        ui.columns(4, |cols| {
            if stat_tile(
                &mut cols[0],
                "active_tickets",
                theme::icon::DASHBOARD,
                active_tickets,
                "Active tickets",
                false,
            ) {
                outcome.goto = Some(Tab::Tasks);
            }
            if stat_tile(
                &mut cols[1],
                "open_todos",
                theme::icon::TODOS,
                open_todos,
                "Open todos",
                false,
            ) {
                outcome.goto = Some(Tab::Todos);
            }
            if stat_tile(
                &mut cols[2],
                "loose_notes",
                theme::icon::NOTES,
                loose_notes,
                "Loose notes",
                false,
            ) {
                outcome.goto = Some(Tab::Notes);
            }
            // The attention tile turns danger-colored when there's something to act on, so a repo
            // needing a look reads at a glance.
            if stat_tile(
                &mut cols[3],
                "repos_attention",
                theme::icon::SYNC,
                attention,
                "Repos need attention",
                attention > 0,
            ) {
                outcome.goto = Some(Tab::Projects);
            }
        });
    }

    /// "Pick up where you left off": the most recently-updated NON-terminal tickets (live work,
    /// not finished columns), newest first. Clicking a row opens that ticket's detail.
    fn render_recent_tickets(&self, ui: &mut egui::Ui, data: &ViewData, outcome: &mut HomeOutcome) {
        let mut recent: Vec<&crate::domain::tasks::Ticket> = data
            .tasks
            .tickets
            .iter()
            .filter(|t| !data.tasks.is_terminal(t.stage_id))
            .collect();
        recent.sort_by_key(|t| std::cmp::Reverse(t.updated_at));
        recent.truncate(LIST_CAP);

        section(
            ui,
            "Pick up where you left off",
            Some(Tab::Tasks),
            outcome,
            |ui, outcome| {
                if recent.is_empty() {
                    empty_hint(ui, "No active tickets — add one on the board.");
                    return;
                }
                for ticket in recent {
                    let stage = data.tasks.stage(ticket.stage_id).map(|s| s.name.as_str());
                    if let Some(open) = ticket_row(ui, &ticket.title, stage) {
                        outcome.open_ticket = Some((ticket.id, open));
                    }
                }
            },
        );
    }

    /// Open todos, checkable inline. Checking one sends `set_done` (it drops off on the next
    /// snapshot, exactly like the Todos tab).
    fn render_open_todos(
        &self,
        ui: &mut egui::Ui,
        bridge: &Bridge,
        data: &ViewData,
        outcome: &mut HomeOutcome,
    ) {
        let open: Vec<&crate::domain::todos::Todo> = data
            .todos
            .todos
            .iter()
            .filter(|t| !t.done)
            .take(LIST_CAP)
            .collect();

        section(ui, "Todos", Some(Tab::Todos), outcome, |ui, _outcome| {
            if open.is_empty() {
                empty_hint(ui, "Nothing to do — you're all caught up.");
                return;
            }
            for todo in open {
                todo_row(ui, bridge, todo);
            }
        });
    }

    /// Repositories that aren't fully in sync — dirty, ahead, or behind — so a repo needing a
    /// commit/pull is visible without opening the Projects tab. Clicking jumps there.
    fn render_projects_attention(
        &self,
        ui: &mut egui::Ui,
        data: &ViewData,
        outcome: &mut HomeOutcome,
    ) {
        let flagged: Vec<(&str, String)> = data
            .projects
            .projects
            .iter()
            .filter_map(|c| attention_label(&c.git).map(|label| (c.project.name.as_str(), label)))
            .take(LIST_CAP)
            .collect();

        section(
            ui,
            "Repositories",
            Some(Tab::Projects),
            outcome,
            |ui, outcome| {
                if data.projects.projects.is_empty() {
                    empty_hint(ui, "No projects yet — add a repository in Projects.");
                    return;
                }
                if flagged.is_empty() {
                    empty_hint(ui, "All repositories are in sync.");
                    return;
                }
                for (name, label) in flagged {
                    if project_row(ui, name, &label) {
                        outcome.goto = Some(Tab::Projects);
                    }
                }
            },
        );
    }

    /// A preview of the newest loose (unfiled) notes. Clicking the header jumps to the Notes tab
    /// where they can be filed onto a ticket or turned into a todo.
    fn render_loose_notes(&self, ui: &mut egui::Ui, data: &ViewData, outcome: &mut HomeOutcome) {
        let notes: Vec<&crate::domain::notes::Note> =
            data.notes.notes.iter().take(NOTES_CAP).collect();

        section(
            ui,
            "Loose notes",
            Some(Tab::Notes),
            outcome,
            |ui, _outcome| {
                if notes.is_empty() {
                    empty_hint(ui, "No loose notes.");
                    return;
                }
                for note in notes {
                    note_row(ui, &note.body);
                }
            },
        );
    }
}

/// A titled section card with an optional "view all →" link in its header (which sets
/// `outcome.goto`). The body closure renders the list rows into an already-open card.
fn section(
    ui: &mut egui::Ui,
    title: &str,
    view_all: Option<Tab>,
    outcome: &mut HomeOutcome,
    body: impl FnOnce(&mut egui::Ui, &mut HomeOutcome),
) {
    card::card(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(title).strong().size(15.0));
            if let Some(tab) = view_all {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if crate::ui::components::button::link(ui, "View all").clicked() {
                        outcome.goto = Some(tab);
                    }
                });
            }
        });
        ui.add_space(4.0);
        ui.separator();
        ui.add_space(8.0);
        body(ui, outcome);
    });
}

/// One clickable summary tile: a big count over a label, with an icon. Returns true when clicked.
/// `alert` tints the number danger-colored (used when the count means "act on me").
fn stat_tile(
    ui: &mut egui::Ui,
    salt: &'static str,
    glyph: char,
    value: usize,
    label: &str,
    alert: bool,
) -> bool {
    let p = theme::palette();
    let number_color = if alert && value > 0 {
        p.danger
    } else {
        p.accent
    };
    let resp = card::card(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(glyph.to_string())
                    .size(15.0)
                    .color(p.muted),
            );
            ui.label(
                egui::RichText::new(value.to_string())
                    .size(30.0)
                    .strong()
                    .color(number_color),
            );
        });
        ui.add_space(2.0);
        ui.label(egui::RichText::new(label).color(p.muted).size(12.5));
    })
    .response;

    // The frame itself doesn't sense clicks, so overlay a click-sense over its rect (the content
    // is non-interactive text, so there's nothing to steal the click).
    let click = ui.interact(
        resp.rect,
        egui::Id::new(("home_stat", salt)),
        egui::Sense::click(),
    );
    if click.hovered() {
        ui.painter().rect_stroke(
            resp.rect,
            egui::CornerRadius::same(theme::radius::CARD),
            egui::Stroke::new(1.5, p.accent),
            egui::StrokeKind::Inside,
        );
    }
    click
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
}

/// A recent-ticket row: an accent title link + a muted stage chip on the right. Returns true when
/// the title is clicked (open the detail).
fn ticket_row(
    ui: &mut egui::Ui,
    title: &str,
    stage: Option<&str>,
) -> Option<crate::ui::tasks::TicketOpen> {
    let p = theme::palette();
    let mut open = None;
    ui.horizontal(|ui| {
        let link = crate::ui::components::button::link(ui, &truncate(title, 42));
        open = crate::ui::tasks::ticket_open_from(&link);
        if let Some(stage) = stage {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(egui::RichText::new(stage).color(p.muted).size(12.0));
            });
        }
    });
    ui.add_space(4.0);
    open
}

/// An open-todo row: a done checkbox + the (truncated) body. Checking it completes the todo.
fn todo_row(ui: &mut egui::Ui, bridge: &Bridge, todo: &crate::domain::todos::Todo) {
    ui.horizontal(|ui| {
        let mut done = false;
        if ui.add(egui::Checkbox::new(&mut done, "")).changed() {
            bridge.send(crate::app::todos::Event::set_done(todo.id, true));
        }
        ui.add(egui::Label::new(egui::RichText::new(truncate(&todo.body, 48)).size(14.0)).wrap());
    });
    ui.add_space(4.0);
}

/// A repo-attention row: name + a muted status label (e.g. "2 behind"). Returns true if clicked.
fn project_row(ui: &mut egui::Ui, name: &str, label: &str) -> bool {
    let p = theme::palette();
    let mut clicked = false;
    ui.horizontal(|ui| {
        clicked = crate::ui::components::button::link(ui, name).clicked();
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(egui::RichText::new(label).color(p.muted).size(12.0));
        });
    });
    ui.add_space(4.0);
    clicked
}

/// A loose-note preview row: the note body, truncated to a single glanceable line.
fn note_row(ui: &mut egui::Ui, body: &str) {
    let p = theme::palette();
    ui.label(
        egui::RichText::new(truncate(body, 60))
            .color(p.text)
            .size(13.5),
    );
    ui.add_space(6.0);
}

/// A muted, italic-feeling empty hint inside a section body.
fn empty_hint(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .color(theme::palette().muted)
            .size(13.0),
    );
}

/// The short attention label for a repo, or `None` when it's up to date / not a repo. Mirrors the
/// Projects grid's notion of "needs a look": uncommitted changes, or out of sync with upstream.
fn attention_label(git: &GitStatus) -> Option<String> {
    if !git.is_repo || git.up_to_date() {
        return None;
    }
    let mut parts = Vec::new();
    if !git.clean {
        parts.push("uncommitted changes".to_owned());
    }
    if git.behind > 0 {
        parts.push(format!("{} behind", git.behind));
    }
    if git.ahead > 0 {
        parts.push(format!("{} ahead", git.ahead));
    }
    if parts.is_empty() {
        // Out of date for some other reason (defensive) — still worth flagging.
        parts.push("out of sync".to_owned());
    }
    Some(parts.join(" · "))
}

/// Truncate to at most `max` chars, appending an ellipsis when cut. Char-based so multi-byte
/// text is never sliced mid-codepoint.
fn truncate(s: &str, max: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max {
        return s.to_owned();
    }
    let cut: String = chars[..max].iter().collect();
    format!("{}…", cut.trim_end())
}
