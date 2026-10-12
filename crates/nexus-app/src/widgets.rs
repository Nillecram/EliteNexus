//! Reusable widgets: cards, stat tiles, slim progress bars, badges.

use crate::theme::{display, pal};
use egui::{Align, Color32, CornerRadius, Frame, Layout, Margin, Response, RichText, Sense, Stroke, Ui, Vec2};

/// A titled panel ("card").
pub fn card<R>(ui: &mut Ui, title: &str, add: impl FnOnce(&mut Ui) -> R) -> R {
    card_with(ui, title, |_| {}, add)
}

/// A card with extra widgets right-aligned in the title row.
pub fn card_with<R>(ui: &mut Ui, title: &str, title_right: impl FnOnce(&mut Ui), add: impl FnOnce(&mut Ui) -> R) -> R {
    Frame::new()
        .fill(pal::CARD)
        .stroke(Stroke::new(1.0, pal::BORDER))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin::same(14))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            if !title.is_empty() {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(title.to_uppercase()).font(display(13.0)).color(pal::ACCENT).strong());
                    ui.with_layout(Layout::right_to_left(Align::Center), title_right);
                });
                ui.add_space(4.0);
            }
            add(ui)
        })
        .inner
}

/// Large number with a small caption underneath.
pub fn stat(ui: &mut Ui, caption: &str, value: impl Into<String>, color: Color32) -> Response {
    ui.vertical(|ui| {
        ui.label(RichText::new(value.into()).font(display(22.0)).color(color));
        ui.label(RichText::new(caption).small().color(pal::DIM));
    })
    .response
}

/// Thin progress bar with optional overlay text.
pub fn bar(ui: &mut Ui, frac: f32, color: Color32, height: f32, text: Option<&str>) -> Response {
    let width = ui.available_width().max(40.0);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
    let p = ui.painter();
    let r = CornerRadius::same((height / 2.0).min(6.0) as u8);
    p.rect_filled(rect, r, pal::BG0);
    let f = frac.clamp(0.0, 1.0);
    if f > 0.0 {
        let mut fill = rect;
        fill.set_width(rect.width() * f);
        p.rect_filled(fill, r, color);
    }
    if let Some(t) = text {
        p.text(rect.center(), egui::Align2::CENTER_CENTER, t, egui::FontId::proportional(height.max(11.0) - 2.0), pal::TEXT);
    }
    resp
}

/// A rounded coloured tag.
pub fn badge(ui: &mut Ui, text: &str, color: Color32) -> Response {
    Frame::new()
        .fill(color.gamma_multiply(0.18))
        .stroke(Stroke::new(1.0, color.gamma_multiply(0.6)))
        .corner_radius(CornerRadius::same(9))
        .inner_margin(Margin::symmetric(7, 1))
        .show(ui, |ui| {
            ui.label(RichText::new(text).small().color(color));
        })
        .response
}

/// Key/value line for inside a `Grid`.
pub fn kv(ui: &mut Ui, key: &str, value: impl Into<RichText>) {
    ui.label(RichText::new(key).color(pal::DIM));
    ui.label(value.into());
    ui.end_row();
}

/// Big centred hint for empty pages.
pub fn empty(ui: &mut Ui, icon: &str, title: &str, hint: &str) {
    ui.add_space(40.0);
    ui.vertical_centered(|ui| {
        ui.label(RichText::new(icon).size(42.0).color(pal::FAINT));
        ui.add_space(6.0);
        ui.label(RichText::new(title).font(display(18.0)).color(pal::DIM));
        ui.label(RichText::new(hint).color(pal::FAINT));
    });
}

pub fn page_title(ui: &mut Ui, icon: &str, title: &str, subtitle: &str) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(icon).size(26.0).color(pal::ACCENT));
        ui.vertical(|ui| {
            ui.label(RichText::new(title).font(display(24.0)).color(pal::TEXT));
            if !subtitle.is_empty() {
                ui.label(RichText::new(subtitle).color(pal::DIM));
            }
        });
    });
    ui.add_space(8.0);
}

pub fn credits(v: i64) -> String {
    format!("{} Cr", nexus_data::names::group_digits(v))
}

pub fn short_credits(v: i64) -> String {
    format!("{} Cr", nexus_data::names::compact_credits(v))
}

/// "3 m ago", "2 h ago" for a unix timestamp.
pub fn ago(ts: nexus_journal::Timestamp) -> String {
    if ts.is_zero() {
        return "never".into();
    }
    let s = (nexus_journal::Timestamp::now().0 - ts.0).max(0);
    match s {
        0..=59 => format!("{s}s ago"),
        60..=3599 => format!("{}m ago", s / 60),
        3600..=86_399 => format!("{}h {}m ago", s / 3600, (s % 3600) / 60),
        _ => format!("{}d ago", s / 86_400),
    }
}

pub fn duration(secs: i64) -> String {
    let s = secs.max(0);
    if s >= 3600 { format!("{}h {:02}m", s / 3600, (s % 3600) / 60) } else { format!("{}m {:02}s", s / 60, s % 60) }
}

/// Plot coordinates under the cursor, whichever hover variant egui_plot reports.
pub fn hover_xy(h: &egui_plot::HoverPosition<'_>) -> (f64, f64) {
    match h {
        egui_plot::HoverPosition::NearDataPoint { position, .. } | egui_plot::HoverPosition::Elsewhere { position } => (position.x, position.y),
    }
}
