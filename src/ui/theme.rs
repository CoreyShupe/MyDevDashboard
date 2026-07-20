//! The design system: the single source of truth for colors, fonts, spacing, corner
//! radii, and shared frames. NO feature file may hardcode a color or a raw egui `Frame`
//! for a surface/input — they compose from here and from `ui::components` (AGENTS.md §7).
//!
//! Vibe: soft dark, teal accent, Nunito, bubbly (generous rounding), sparing borders,
//! solid opaque surfaces over a faint grid background.

use egui::epaint::Shadow;
use egui::{Color32, CornerRadius, Frame, Margin, Stroke, Style};

/// A cohesive set of colors. One instance today (`DARK`); more can be added behind
/// `palette()` if we ever support light mode.
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    /// Window/panel base — the grid background paints faint lines over this.
    pub bg: Color32,
    /// Grid line color: only slightly lighter than `bg`, for subtle texture.
    pub grid_line: Color32,
    /// Solid, opaque surface for cards/columns/modals that sit over the grid.
    pub surface: Color32,
    /// Slightly lighter surface for inputs and inset areas.
    pub surface_alt: Color32,
    /// Hover state for interactive surfaces.
    pub surface_hover: Color32,
    pub text: Color32,
    pub muted: Color32,
    /// Primary accent (teal): buttons, focus, selection.
    pub accent: Color32,
    /// Soft accent used as the selection/highlight fill (replaces egui's harsh blue).
    pub accent_soft: Color32,
    /// Readable text color on top of the accent.
    pub on_accent: Color32,
    pub danger: Color32,
    /// Subtle hairline for separators (used sparingly; most surfaces have no border).
    pub hairline: Color32,
}

/// The soft-dark, teal palette.
pub const DARK: Palette = Palette {
    bg: Color32::from_rgb(0x17, 0x19, 0x1F),
    grid_line: Color32::from_rgb(0x20, 0x23, 0x2B),
    surface: Color32::from_rgb(0x23, 0x27, 0x30),
    surface_alt: Color32::from_rgb(0x2B, 0x30, 0x3B),
    surface_hover: Color32::from_rgb(0x33, 0x39, 0x46),
    text: Color32::from_rgb(0xE7, 0xEA, 0xF0),
    muted: Color32::from_rgb(0x97, 0x9E, 0xAD),
    accent: Color32::from_rgb(0x17, 0xC3, 0xB2),
    accent_soft: Color32::from_rgb(0x1A, 0x39, 0x37),
    on_accent: Color32::from_rgb(0x06, 0x1A, 0x18),
    danger: Color32::from_rgb(0xE5, 0x67, 0x7A),
    hairline: Color32::from_rgb(0x2E, 0x33, 0x3F),
};

/// The active palette.
pub const fn palette() -> Palette {
    DARK
}

/// Material Icons glyphs (Private Use Area codepoints). Use these instead of raw emoji,
/// which egui's bundled fonts don't reliably cover. Compose with text via `format!`.
pub mod icon {
    pub const ADD: char = '\u{e145}'; // add
    pub const DELETE: char = '\u{e872}'; // delete
    pub const EDIT: char = '\u{e3c9}'; // edit
    pub const REFRESH: char = '\u{e5d5}'; // refresh
    pub const CLOSE: char = '\u{e5cd}'; // close
    pub const DASHBOARD: char = '\u{e871}'; // dashboard (nav: Tasks)
    pub const SAVE: char = '\u{e161}'; // save
    pub const WARNING: char = '\u{e002}'; // warning
    pub const DRAG: char = '\u{e945}'; // drag_indicator (6-dot ticket drag handle)
    pub const PARENT: char = '\u{e5d8}'; // arrow_upward (jump to parent ticket)
    pub const CHILD: char = '\u{e5da}'; // subdirectory_arrow_right (child ticket)
    pub const UNLINK: char = '\u{e16f}'; // link_off (detach from parent)
    pub const EXPAND: char = '\u{e5d0}'; // fullscreen (expand ticket to full page)
    pub const BACK: char = '\u{e5c4}'; // arrow_back (return from full page to board)
}

/// Corner radii (px). Bubbly = generous.
pub mod radius {
    pub const INPUT: u8 = 10;
    pub const BUTTON: u8 = 12;
    pub const CARD: u8 = 14;
    pub const WINDOW: u8 = 16;
}

/// Install fonts + theme into the egui context. Call once at startup.
pub fn install(ctx: &egui::Context) {
    install_fonts(ctx);

    let p = palette();
    ctx.all_styles_mut(|style| {
        apply_text_styles(style);
        apply_spacing(style);
        apply_visuals(style, p);
    });
    // Force dark so our dark palette is the one in effect.
    ctx.set_theme(egui::ThemePreference::Dark);
}

fn install_fonts(ctx: &egui::Context) {
    use egui::FontFamily;

    let mut fonts = egui::FontDefinitions::default();
    let embed = |fonts: &mut egui::FontDefinitions, name: &str, bytes: &'static [u8]| {
        fonts.font_data.insert(
            name.to_owned(),
            std::sync::Arc::new(egui::FontData::from_static(bytes)),
        );
    };
    // SemiBold as the base weight — Regular Nunito reads too thin on a dark theme.
    embed(
        &mut fonts,
        "nunito",
        include_bytes!("../../assets/fonts/Nunito-SemiBold.ttf"),
    );
    embed(
        &mut fonts,
        "nunito-bold",
        include_bytes!("../../assets/fonts/Nunito-Bold.ttf"),
    );
    // Material Icons — real, meaningful action icons (see the `icon` module). Codepoints
    // live in the Private Use Area, so this never shadows normal text glyphs.
    embed(
        &mut fonts,
        "material-icons",
        include_bytes!("../../assets/fonts/MaterialIcons-Regular.ttf"),
    );

    // Proportional: Nunito SemiBold first; append Material Icons last so icon codepoints
    // resolve via fallback anywhere text is drawn.
    {
        let proportional = fonts.families.entry(FontFamily::Proportional).or_default();
        proportional.insert(0, "nunito".to_owned());
        proportional.push("material-icons".to_owned());
    }

    // A named "bold" family for headings: Bold first, then the whole proportional stack (so
    // it still falls back to SemiBold, emoji, and Material Icons for glyphs Bold lacks).
    let mut bold_stack = vec!["nunito-bold".to_owned()];
    if let Some(proportional) = fonts.families.get(&FontFamily::Proportional) {
        bold_stack.extend(proportional.iter().cloned());
    }
    fonts
        .families
        .insert(FontFamily::Name("nunito-bold".into()), bold_stack);

    ctx.set_fonts(fonts);
}

fn apply_text_styles(style: &mut Style) {
    use egui::{FontFamily, FontId, TextStyle};
    let proportional = FontFamily::Proportional;
    let bold = FontFamily::Name("nunito-bold".into());
    style.text_styles = [
        (TextStyle::Heading, FontId::new(23.0, bold)),
        (TextStyle::Body, FontId::new(15.5, proportional.clone())),
        (TextStyle::Button, FontId::new(15.5, proportional.clone())),
        (TextStyle::Small, FontId::new(12.5, proportional)),
        (
            TextStyle::Monospace,
            FontId::new(13.0, FontFamily::Monospace),
        ),
    ]
    .into();
}

fn apply_spacing(style: &mut Style) {
    style.spacing.item_spacing = egui::vec2(8.0, 8.0);
    style.spacing.button_padding = egui::vec2(14.0, 8.0);
    style.spacing.window_margin = Margin::same(16);
    style.spacing.menu_margin = Margin::same(10);
    style.spacing.interact_size.y = 30.0;
}

fn apply_visuals(style: &mut Style, p: Palette) {
    let v = &mut style.visuals;
    v.dark_mode = true;
    v.override_text_color = Some(p.text);
    v.panel_fill = p.bg;
    v.faint_bg_color = p.surface_alt;
    v.extreme_bg_color = p.surface_alt; // TextEdit background base

    // Windows / menus (also used by modals unless overridden).
    v.window_fill = p.surface;
    v.window_stroke = Stroke::NONE;
    v.window_corner_radius = CornerRadius::same(radius::WINDOW);
    v.window_shadow = soft_shadow();
    v.menu_corner_radius = CornerRadius::same(radius::BUTTON);

    // Selection/highlight — teal, NOT the default harsh blue.
    v.selection.bg_fill = p.accent_soft;
    v.selection.stroke = Stroke::new(1.0, p.accent);
    v.hyperlink_color = p.accent;
    // Pointing-hand cursor over anything clickable (egui defaults to no cursor change).
    v.interact_cursor = Some(egui::CursorIcon::PointingHand);

    // Widget states. Rounded, soft fills, no harsh borders.
    let r = CornerRadius::same(radius::INPUT);
    let w = &mut v.widgets;

    w.noninteractive.bg_fill = p.bg;
    w.noninteractive.weak_bg_fill = p.bg;
    w.noninteractive.bg_stroke = Stroke::new(1.0, p.hairline); // subtle separators only
    w.noninteractive.fg_stroke = Stroke::new(1.0, p.text);
    w.noninteractive.corner_radius = r;

    for wv in [&mut w.inactive, &mut w.open] {
        wv.bg_fill = p.surface_alt;
        wv.weak_bg_fill = p.surface_alt;
        wv.bg_stroke = Stroke::NONE;
        wv.fg_stroke = Stroke::new(1.0, p.text);
        wv.corner_radius = r;
    }

    w.hovered.bg_fill = p.surface_hover;
    w.hovered.weak_bg_fill = p.surface_hover;
    w.hovered.bg_stroke = Stroke::NONE;
    w.hovered.fg_stroke = Stroke::new(1.0, p.text);
    w.hovered.corner_radius = r;
    w.hovered.expansion = 0.0;

    w.active.bg_fill = p.surface_hover;
    w.active.weak_bg_fill = p.surface_hover;
    w.active.bg_stroke = Stroke::new(1.0, p.accent);
    w.active.fg_stroke = Stroke::new(1.0, p.text);
    w.active.corner_radius = r;
    w.active.expansion = 0.0;
}

/// A soft drop shadow for cards/modals — depth without borders.
pub fn soft_shadow() -> Shadow {
    Shadow {
        offset: [0, 4],
        blur: 18,
        spread: 0,
        color: Color32::from_black_alpha(90),
    }
}

/// A solid, opaque surface frame (cards, columns, modals). Covers the grid fully.
pub fn surface_frame() -> Frame {
    let p = palette();
    Frame::default()
        .fill(p.surface)
        .corner_radius(CornerRadius::same(radius::CARD))
        .inner_margin(Margin::same(14))
        .shadow(soft_shadow())
}

/// A raised inset surface (one level lighter than [`surface_frame`]) for cards nested
/// inside a card — e.g. ticket cards inside a column, note rows inside the modal. Solid,
/// so it still fully covers whatever is behind it.
pub fn inset_frame() -> Frame {
    let p = palette();
    Frame::default()
        .fill(p.surface_alt)
        .corner_radius(CornerRadius::same(radius::BUTTON))
        .inner_margin(Margin::same(12))
}

/// A soft rounded input frame (wraps a frameless `TextEdit`).
pub fn input_frame() -> Frame {
    let p = palette();
    Frame::default()
        .fill(p.surface_alt)
        .corner_radius(CornerRadius::same(radius::INPUT))
        .inner_margin(Margin::symmetric(10, 8))
}

/// Paint the faint grid texture across `rect`. Call before adding content so it sits
/// behind everything; solid surfaces then cover it where cards overflow.
pub fn paint_grid(painter: &egui::Painter, rect: egui::Rect) {
    let p = palette();
    let stroke = Stroke::new(1.0, p.grid_line);
    const STEP: f32 = 26.0;

    let mut x = rect.left();
    while x < rect.right() {
        painter.line_segment(
            [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
            stroke,
        );
        x += STEP;
    }
    let mut y = rect.top();
    while y < rect.bottom() {
        painter.line_segment(
            [egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)],
            stroke,
        );
        y += STEP;
    }
}

/// Paint the grid across the WHOLE window on the background layer, so it reads as an
/// infinite backdrop (edge to edge, under every panel) rather than a boxed-in rectangle.
/// Panels that want the grid to show through must use a transparent fill.
pub fn paint_background(ctx: &egui::Context) {
    let painter = ctx.layer_painter(egui::LayerId::background());
    paint_grid(&painter, ctx.content_rect());
}
