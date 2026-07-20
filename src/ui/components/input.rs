//! Bubbly, rounded text inputs. Soft filled background, no harsh border, teal focus ring.

use egui::{Response, StrokeKind, TextEdit, Ui};

use crate::ui::theme;

/// A single-line rounded text field that fills the available width.
pub fn text_field(ui: &mut Ui, buffer: &mut String, hint: &str) -> Response {
    text_field_sized(ui, buffer, hint, f32::INFINITY)
}

/// A single-line rounded text field of a fixed width (e.g. the header "add stage" box).
pub fn text_field_sized(ui: &mut Ui, buffer: &mut String, hint: &str, width: f32) -> Response {
    single(ui, buffer, hint, width, false)
}

/// Like [`text_field`], but drawn with an accent outline when `changed` is true — used to
/// flag a form field that has been edited but not yet saved.
pub fn text_field_marked(ui: &mut Ui, buffer: &mut String, hint: &str, changed: bool) -> Response {
    single(ui, buffer, hint, f32::INFINITY, changed)
}

/// A multi-line rounded text area.
pub fn text_area(ui: &mut Ui, buffer: &mut String, hint: &str, rows: usize) -> Response {
    multi(ui, buffer, hint, rows, false)
}

/// Like [`text_area`], but drawn with an accent outline when `changed` is true (edited-but-
/// unsaved).
pub fn text_area_marked(
    ui: &mut Ui,
    buffer: &mut String,
    hint: &str,
    rows: usize,
    changed: bool,
) -> Response {
    multi(ui, buffer, hint, rows, changed)
}

fn single(ui: &mut Ui, buffer: &mut String, hint: &str, width: f32, changed: bool) -> Response {
    render(ui, changed, |ui| {
        ui.add(
            TextEdit::singleline(buffer)
                .frame(egui::Frame::default())
                .hint_text(hint)
                .desired_width(width),
        )
    })
}

fn multi(ui: &mut Ui, buffer: &mut String, hint: &str, rows: usize, changed: bool) -> Response {
    render(ui, changed, |ui| {
        ui.add(
            TextEdit::multiline(buffer)
                .frame(egui::Frame::default())
                .hint_text(hint)
                .desired_rows(rows)
                .desired_width(f32::INFINITY),
        )
    })
}

/// Wrap a frameless `TextEdit` in the soft input frame. Draws a teal ring on focus; when the
/// field is unfocused but `changed`, a subtler teal outline flags the pending edit.
fn render(ui: &mut Ui, changed: bool, add: impl FnOnce(&mut Ui) -> Response) -> Response {
    let out = theme::input_frame().show(ui, add);
    let resp = out.inner;
    // Focus takes precedence (crisper 1.5px ring); an unfocused-but-edited field still gets a
    // marker so the user can see, at a glance, which fields differ from what's saved.
    let stroke = if resp.has_focus() {
        Some(egui::Stroke::new(1.5, theme::palette().accent))
    } else if changed {
        Some(egui::Stroke::new(1.0, theme::palette().accent))
    } else {
        None
    };
    if let Some(stroke) = stroke {
        ui.painter().rect_stroke(
            out.response.rect,
            egui::CornerRadius::same(theme::radius::INPUT),
            stroke,
            StrokeKind::Inside,
        );
    }
    resp
}
