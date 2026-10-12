//! Visual identity: palette, fonts, egui style.

use egui::{Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Margin, Shadow, Stroke, TextStyle};
use std::sync::Arc;

pub mod pal {
    use egui::Color32;
    pub const BG0: Color32 = Color32::from_rgb(8, 11, 16);
    pub const BG1: Color32 = Color32::from_rgb(13, 18, 25);
    pub const CARD: Color32 = Color32::from_rgb(18, 25, 34);
    pub const CARD_HI: Color32 = Color32::from_rgb(24, 33, 45);
    pub const ROW_ALT: Color32 = Color32::from_rgb(21, 29, 39);
    pub const BORDER: Color32 = Color32::from_rgb(34, 46, 61);
    pub const TEXT: Color32 = Color32::from_rgb(220, 228, 236);
    pub const DIM: Color32 = Color32::from_rgb(140, 154, 171);
    pub const FAINT: Color32 = Color32::from_rgb(92, 106, 124);
    pub const ACCENT: Color32 = Color32::from_rgb(255, 140, 26);
    pub const ACCENT_SOFT: Color32 = Color32::from_rgb(255, 176, 92);
    pub const ACCENT_BG: Color32 = Color32::from_rgb(58, 36, 14);
    pub const CYAN: Color32 = Color32::from_rgb(64, 200, 255);
    pub const GOOD: Color32 = Color32::from_rgb(66, 214, 132);
    pub const WARN: Color32 = Color32::from_rgb(255, 199, 77);
    pub const BAD: Color32 = Color32::from_rgb(255, 96, 96);
    pub const PURPLE: Color32 = Color32::from_rgb(182, 144, 255);
    pub const GOLD: Color32 = Color32::from_rgb(242, 201, 76);
}

/// Font family for big HUD-style numbers and headings.
pub fn display_family() -> FontFamily {
    FontFamily::Name("display".into())
}

pub fn display(size: f32) -> FontId {
    FontId::new(size, display_family())
}

/// Loads a system font if present (Windows ships Segoe UI and Bahnschrift).
fn system_font(names: &[&str]) -> Option<Vec<u8>> {
    let dir = std::env::var_os("WINDIR").map(|w| std::path::PathBuf::from(w).join("Fonts"))?;
    names.iter().find_map(|n| std::fs::read(dir.join(n)).ok())
}

pub fn install(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);

    let mut display_stack: Vec<String> = Vec::new();
    if let Some(bytes) = system_font(&["segoeui.ttf"]) {
        fonts.font_data.insert("segoe".into(), Arc::new(FontData::from_owned(bytes)));
        if let Some(f) = fonts.families.get_mut(&FontFamily::Proportional) {
            f.insert(0, "segoe".into());
        }
    }
    if let Some(bytes) = system_font(&["bahnschrift.ttf"]) {
        fonts.font_data.insert("bahnschrift".into(), Arc::new(FontData::from_owned(bytes)));
        display_stack.push("bahnschrift".into());
    }
    // Fall back to the proportional stack (incl. icons) for missing glyphs.
    let proportional = fonts.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    display_stack.extend(proportional);
    fonts.families.insert(display_family(), display_stack);
    ctx.set_fonts(fonts);

    ctx.all_styles_mut(|style| {
        style.text_styles = [
            (TextStyle::Heading, display(22.0)),
            (TextStyle::Body, FontId::proportional(14.0)),
            (TextStyle::Button, FontId::proportional(14.0)),
            (TextStyle::Small, FontId::proportional(11.5)),
            (TextStyle::Monospace, FontId::monospace(13.0)),
        ]
        .into();
        let sp = &mut style.spacing;
        sp.item_spacing = egui::vec2(8.0, 6.0);
        sp.button_padding = egui::vec2(10.0, 5.0);
        sp.interact_size = egui::vec2(40.0, 24.0);
        sp.window_margin = Margin::same(12);
        sp.menu_margin = Margin::same(8);
        sp.indent = 16.0;

        let v = &mut style.visuals;
        *v = egui::Visuals::dark();
        v.panel_fill = pal::BG1;
        v.window_fill = pal::CARD;
        v.extreme_bg_color = pal::BG0;
        v.faint_bg_color = pal::ROW_ALT;
        v.code_bg_color = pal::BG0;
        v.hyperlink_color = pal::CYAN;
        v.warn_fg_color = pal::WARN;
        v.error_fg_color = pal::BAD;
        v.window_corner_radius = CornerRadius::same(10);
        v.menu_corner_radius = CornerRadius::same(8);
        v.window_stroke = Stroke::new(1.0, pal::BORDER);
        v.window_shadow = Shadow { offset: [0, 6], blur: 24, spread: 0, color: Color32::from_black_alpha(140) };
        v.popup_shadow = Shadow { offset: [0, 4], blur: 16, spread: 0, color: Color32::from_black_alpha(120) };
        v.selection.bg_fill = pal::ACCENT_BG;
        v.selection.stroke = Stroke::new(1.0, pal::ACCENT);
        v.override_text_color = None;
        v.striped = false;

        let w = &mut v.widgets;
        let r = CornerRadius::same(6);
        w.noninteractive.bg_fill = pal::CARD;
        w.noninteractive.weak_bg_fill = pal::CARD;
        w.noninteractive.bg_stroke = Stroke::new(1.0, pal::BORDER);
        w.noninteractive.fg_stroke = Stroke::new(1.0, pal::TEXT);
        w.noninteractive.corner_radius = r;
        w.inactive.bg_fill = pal::CARD_HI;
        w.inactive.weak_bg_fill = pal::CARD_HI;
        w.inactive.bg_stroke = Stroke::new(1.0, pal::BORDER);
        w.inactive.fg_stroke = Stroke::new(1.0, pal::TEXT);
        w.inactive.corner_radius = r;
        w.hovered.bg_fill = Color32::from_rgb(33, 45, 60);
        w.hovered.weak_bg_fill = Color32::from_rgb(33, 45, 60);
        w.hovered.bg_stroke = Stroke::new(1.0, pal::ACCENT_SOFT);
        w.hovered.fg_stroke = Stroke::new(1.5, Color32::WHITE);
        w.hovered.corner_radius = r;
        w.active.bg_fill = pal::ACCENT_BG;
        w.active.weak_bg_fill = pal::ACCENT_BG;
        w.active.bg_stroke = Stroke::new(1.0, pal::ACCENT);
        w.active.fg_stroke = Stroke::new(2.0, Color32::WHITE);
        w.active.corner_radius = r;
        w.open.bg_fill = pal::CARD_HI;
        w.open.weak_bg_fill = pal::CARD_HI;
        w.open.bg_stroke = Stroke::new(1.0, pal::ACCENT);
        w.open.corner_radius = r;
    });
}
