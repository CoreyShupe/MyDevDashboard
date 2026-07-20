//! Rounded, bubbly buttons in the palette's roles. No harsh default styling.

use egui::{Button, Color32, CornerRadius, Response, RichText, Ui};

use crate::ui::theme;

/// Solid teal call-to-action.
pub fn primary(ui: &mut Ui, label: &str) -> Response {
    primary_enabled(ui, label, true)
}

/// Solid teal call-to-action; when `enabled` is false it stays clearly READABLE (a muted
/// filled pill with bright text) instead of egui's near-invisible faded default, and is
/// non-clickable.
pub fn primary_enabled(ui: &mut Ui, label: &str, enabled: bool) -> Response {
    let p = theme::palette();
    // Pick colors per-state so the disabled label keeps contrast; then disable the fade so
    // it isn't washed out.
    let (fill, text) = if enabled {
        (p.accent, p.on_accent)
    } else {
        (p.surface_hover, p.text)
    };
    let button = Button::new(RichText::new(label).strong().color(text))
        .fill(fill)
        .corner_radius(CornerRadius::same(theme::radius::BUTTON));

    if enabled {
        ui.add(button)
    } else {
        // Full opacity (readable), but senses only hover — never reports a click — so a
        // caller that checks `.clicked()` alone still can't submit.
        ui.add(button.sense(egui::Sense::hover()))
            .on_hover_cursor(egui::CursorIcon::NotAllowed)
    }
}

/// Compact accent pill for an inline, in-card call to action (e.g. a project card's Pull
/// button). Smaller text + tighter padding than [`primary`] so it sits neatly beside labels and
/// badges rather than towering over them.
pub fn compact_primary(ui: &mut Ui, label: &str) -> Response {
    let p = theme::palette();
    let restore = ui.spacing().button_padding;
    ui.spacing_mut().button_padding = egui::vec2(10.0, 3.0);
    let response = ui.add(
        Button::new(RichText::new(label).size(12.5).strong().color(p.on_accent))
            .fill(p.accent)
            .corner_radius(CornerRadius::same(theme::radius::BUTTON)),
    );
    ui.spacing_mut().button_padding = restore;
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Neutral filled button for secondary actions (e.g. Cancel).
pub fn secondary(ui: &mut Ui, label: &str) -> Response {
    let p = theme::palette();
    ui.add(
        Button::new(RichText::new(label).color(p.text))
            .fill(p.surface_alt)
            .corner_radius(CornerRadius::same(theme::radius::BUTTON)),
    )
}

/// Low-emphasis, frame-less button (e.g. Refresh, Close).
pub fn ghost(ui: &mut Ui, label: &str) -> Response {
    let p = theme::palette();
    ui.add(
        Button::new(RichText::new(label).color(p.muted))
            .fill(Color32::TRANSPARENT)
            .corner_radius(CornerRadius::same(theme::radius::INPUT)),
    )
}

/// Frame-less navigational link (accent-colored) — reads as clickable, for jumping to
/// another entity (e.g. a ticket's parent/child).
pub fn link(ui: &mut Ui, label: &str) -> Response {
    let p = theme::palette();
    ui.add(
        Button::new(RichText::new(label).color(p.accent))
            .fill(Color32::TRANSPARENT)
            .corner_radius(CornerRadius::same(theme::radius::INPUT)),
    )
    .on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Destructive frame-less button (e.g. Delete).
pub fn danger(ui: &mut Ui, label: &str) -> Response {
    let p = theme::palette();
    ui.add(
        Button::new(RichText::new(label).color(p.danger))
            .fill(Color32::TRANSPARENT)
            .corner_radius(CornerRadius::same(theme::radius::INPUT)),
    )
}

/// Tiny transparent icon button (e.g. edit / delete on a stage header). Takes a
/// [`theme::icon`] glyph.
pub fn icon(ui: &mut Ui, glyph: char, hover: &str) -> Response {
    let p = theme::palette();
    ui.add(
        Button::new(RichText::new(glyph.to_string()).size(16.0).color(p.muted))
            .fill(Color32::TRANSPARENT)
            .corner_radius(CornerRadius::same(8)),
    )
    .on_hover_text(hover)
}
