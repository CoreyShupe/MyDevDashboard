//! Bubbly, rounded text inputs. Soft filled background, no harsh border, teal focus ring.

use egui::{Response, StrokeKind, TextEdit, Ui};

use crate::ui::theme;

/// A single-line rounded text field that fills the available width.
pub fn text_field(ui: &mut Ui, buffer: &mut String, hint: &str) -> Response {
    text_field_sized(ui, buffer, hint, f32::INFINITY)
}

/// A single-line rounded text field of a fixed width (e.g. the header "add stage" box).
pub fn text_field_sized(ui: &mut Ui, buffer: &mut String, hint: &str, width: f32) -> Response {
    render(ui, |ui| {
        ui.add(
            TextEdit::singleline(buffer)
                .frame(egui::Frame::default())
                .hint_text(hint)
                .desired_width(width),
        )
    })
}

/// A multi-line rounded text area.
pub fn text_area(ui: &mut Ui, buffer: &mut String, hint: &str, rows: usize) -> Response {
    render(ui, |ui| {
        ui.add(
            TextEdit::multiline(buffer)
                .frame(egui::Frame::default())
                .hint_text(hint)
                .desired_rows(rows)
                .desired_width(f32::INFINITY),
        )
    })
}

/// Wrap a frameless `TextEdit` in the soft input frame, drawing a teal ring on focus.
fn render(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> Response) -> Response {
    let out = theme::input_frame().show(ui, add);
    let resp = out.inner;
    if resp.has_focus() {
        ui.painter().rect_stroke(
            out.response.rect,
            egui::CornerRadius::same(theme::radius::INPUT),
            egui::Stroke::new(1.5, theme::palette().accent),
            StrokeKind::Inside,
        );
    }
    resp
}
