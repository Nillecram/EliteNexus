//! The Elite Nexus component library (see theme.rs for colours and type): cards, grouped lists in the
//! style of System Settings, segmented controls, switches, capsule buttons, badges, meters.

use crate::theme::{display, pal, size};
use egui::{Align, Align2, Color32, CornerRadius, FontId, Frame, Layout, Margin, Pos2, Rect, Response, RichText, Sense, Shadow, Stroke, Ui, Vec2, WidgetInfo, WidgetType};

// ---------------------------------------------------------------------------------------------
// Surfaces

/// The card surface: a flat rounded panel one step lighter than the window.
pub fn card_frame() -> Frame {
    Frame::new().fill(pal::CARD).corner_radius(CornerRadius::same(16)).inner_margin(Margin::same(18))
}

/// A titled panel ("card").
pub fn card<R>(ui: &mut Ui, title: &str, add: impl FnOnce(&mut Ui) -> R) -> R {
    card_with(ui, title, |_| {}, add)
}

/// A card with extra widgets right-aligned in the title row.
pub fn card_with<R>(ui: &mut Ui, title: &str, title_right: impl FnOnce(&mut Ui), add: impl FnOnce(&mut Ui) -> R) -> R {
    card_impl(ui, |ui| {
        if !title.is_empty() {
            ui.horizontal(|ui| {
                ui.label(RichText::new(title).font(display(size::HEADLINE)).color(pal::TEXT));
                ui.with_layout(Layout::right_to_left(Align::Center), title_right);
            });
            ui.add_space(4.0);
        }
        add(ui)
    })
}

/// A card headed like Apple's Health summaries: a tinted icon and label, then the content.
pub fn card_tinted<R>(ui: &mut Ui, icon: &str, title: &str, tint: Color32, add: impl FnOnce(&mut Ui) -> R) -> R {
    card_impl(ui, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            ui.label(RichText::new(icon).size(15.0).color(tint));
            ui.label(RichText::new(title).font(display(size::CALLOUT)).color(tint));
        });
        ui.add_space(2.0);
        add(ui)
    })
}

fn card_impl<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    // Inside a `card_row`, the first card of each column stretches to the row's tallest card.
    let fill = ui.ctx().data_mut(|d| d.remove_temp::<f32>(ui.id().with(ROW_FILL)));
    let (inner, natural) = card_frame()
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            // Left-aligned and never justified (columns justify by default, which spreads words apart).
            ui.with_layout(Layout::top_down(Align::Min), |ui| {
                let inner = add(ui);
                let natural = ui.min_rect().height();
                if let Some(h) = fill.filter(|h| *h > natural) {
                    // (`set_min_height` would add the height below the content instead.)
                    let top = ui.min_rect().min;
                    ui.expand_to_include_rect(Rect::from_min_size(top, Vec2::new(0.0, h)));
                }
                (inner, natural)
            })
            .inner
        })
        .inner;
    if fill.is_some() {
        ui.ctx().data_mut(|d| d.insert_temp(ui.id().with(ROW_NATURAL), natural));
    }
    inner
}

const ROW_FILL: &str = "card-row-fill";
const ROW_NATURAL: &str = "card-row-natural";
/// Space between cards.
pub const GAP: f32 = 14.0;

/// Cards in a responsive grid: up to `max_cols` per row, each at least `min_width` wide, wrapping to
/// fewer columns as the window narrows. Cards in a row share the height of the tallest.
pub fn cards(ui: &mut Ui, max_cols: usize, min_width: f32, add: impl FnOnce(&mut Cards)) {
    let avail = ui.available_width();
    let cols = (((avail + GAP) / (min_width + GAP)).floor() as usize).clamp(1, max_cols.max(1));
    let width = (avail - GAP * (cols as f32 - 1.0)) / cols as f32;
    let key = ui.id().with(("cards", ui.next_auto_id()));
    let targets: Vec<f32> = ui.ctx().data(|d| d.get_temp(key)).unwrap_or_default();
    let origin = ui.cursor().min;
    let mut c = Cards { ui, cols, width, origin, key, targets, naturals: Vec::new(), index: 0, y: origin.y, row_h: 0.0, row_natural: 0.0, avail };
    add(&mut c);
    c.finish();
}

/// Builder for [`cards`].
pub struct Cards<'a> {
    ui: &'a mut Ui,
    cols: usize,
    width: f32,
    origin: Pos2,
    key: egui::Id,
    targets: Vec<f32>,
    naturals: Vec<f32>,
    index: usize,
    y: f32,
    row_h: f32,
    row_natural: f32,
    avail: f32,
}

impl Cards<'_> {
    /// Adds one grid cell (normally one card).
    pub fn add(&mut self, add: impl FnOnce(&mut Ui)) {
        let col = self.index % self.cols;
        if col == 0 && self.index > 0 {
            self.end_row();
        }
        let row = self.naturals.len();
        let x = self.origin.x + col as f32 * (self.width + GAP);
        let rect = Rect::from_min_size(Pos2::new(x, self.y), Vec2::new(self.width, f32::INFINITY));
        let mut child = self.ui.new_child(egui::UiBuilder::new().id_salt(self.key.with(self.index)).max_rect(rect).layout(Layout::top_down(Align::Min)));
        let target = self.targets.get(row).copied().unwrap_or(0.0);
        let id = child.id();
        child.ctx().data_mut(|d| d.insert_temp(id.with(ROW_FILL), target));
        add(&mut child);
        let natural = child.ctx().data_mut(|d| {
            d.remove_temp::<f32>(id.with(ROW_FILL));
            d.remove_temp::<f32>(id.with(ROW_NATURAL))
        });
        self.row_natural = self.row_natural.max(natural.unwrap_or(0.0));
        self.row_h = self.row_h.max(child.min_rect().height());
        self.index += 1;
    }

    fn end_row(&mut self) {
        self.naturals.push(self.row_natural);
        self.y += self.row_h + GAP;
        self.row_h = 0.0;
        self.row_natural = 0.0;
    }

    fn finish(mut self) {
        if self.index == 0 {
            return;
        }
        self.end_row();
        let bottom = self.y - GAP;
        self.ui.advance_cursor_after_rect(Rect::from_min_max(self.origin, Pos2::new(self.origin.x + self.avail, bottom)));
        let changed = self.naturals.len() != self.targets.len() || self.naturals.iter().zip(&self.targets).any(|(a, b)| (a - b).abs() > 0.5);
        if changed {
            let n = self.naturals;
            self.ui.ctx().data_mut(|d| d.insert_temp(self.key, n));
            self.ui.ctx().request_repaint();
        }
    }
}

/// Two panes side by side (`left_frac` of the width for the first) when there is room for both at
/// `min_width`, otherwise stacked.
pub fn split(ui: &mut Ui, left_frac: f32, min_width: f32, left: impl FnOnce(&mut Ui), right: impl FnOnce(&mut Ui)) {
    let avail = ui.available_width();
    if avail < min_width * 2.0 + GAP {
        left(ui);
        ui.add_space(GAP - ui.spacing().item_spacing.y);
        right(ui);
        return;
    }
    let lw = ((avail - GAP) * left_frac).max(min_width);
    let rw = avail - GAP - lw;
    let origin = ui.cursor().min;
    // Panes get the parent's height, so a pane can fill it (lists with their own scroll area).
    let h = ui.available_height();
    let key = ui.id().with(("split", ui.next_auto_id()));
    let mut a = ui.new_child(egui::UiBuilder::new().id_salt(key.with(0)).max_rect(Rect::from_min_size(origin, Vec2::new(lw, h))).layout(Layout::top_down(Align::Min)));
    left(&mut a);
    let mut b = ui.new_child(
        egui::UiBuilder::new().id_salt(key.with(1)).max_rect(Rect::from_min_size(Pos2::new(origin.x + lw + GAP, origin.y), Vec2::new(rw, h))).layout(Layout::top_down(Align::Min)),
    );
    right(&mut b);
    let bottom = a.min_rect().bottom().max(b.min_rect().bottom());
    ui.advance_cursor_after_rect(Rect::from_min_max(origin, Pos2::new(origin.x + avail, bottom)));
}

/// Paints a floating "glass" surface: a lifted tint with a soft shadow and a light rim, brighter at
/// the top edge (sidebar, toolbar capsules, banners).
pub fn glass(p: &egui::Painter, rect: Rect, radius: u8, fill: Color32) {
    let r = CornerRadius::same(radius);
    p.add(Shadow { offset: [0, 8], blur: 28, spread: 0, color: Color32::from_black_alpha(90) }.as_shape(rect, r));
    p.rect_filled(rect, r, fill);
    p.rect_stroke(rect, r, Stroke::new(1.0, Color32::from_white_alpha(16)), egui::StrokeKind::Inside);
    // Specular rim along the top edge.
    let top = Rect::from_min_max(rect.min, Pos2::new(rect.max.x, rect.min.y + f32::from(radius).max(8.0)));
    p.with_clip_rect(top).rect_stroke(rect, r, Stroke::new(1.0, Color32::from_white_alpha(22)), egui::StrokeKind::Inside);
}

// ---------------------------------------------------------------------------------------------
// Grouped lists (System Settings style): rows in one rounded box, separated by hairlines.

/// A grouped list. Add rows through the `Group`; hairlines go between them.
pub fn group<R>(ui: &mut Ui, add: impl FnOnce(&mut Group) -> R) -> R {
    Frame::new()
        .fill(pal::CARD)
        .corner_radius(CornerRadius::same(12))
        .inner_margin(Margin::symmetric(16, 4))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.with_layout(Layout::top_down(Align::Min), |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                let mut g = Group { ui, rows: 0 };
                add(&mut g)
            })
            .inner
        })
        .inner
}

pub struct Group<'a> {
    pub ui: &'a mut Ui,
    rows: usize,
}

impl Group<'_> {
    fn separator(&mut self) {
        if self.rows > 0 {
            let rect = self.ui.available_rect_before_wrap();
            let y = rect.top();
            self.ui.painter().hline(rect.x_range(), y, Stroke::new(1.0, pal::SEPARATOR));
        }
        self.rows += 1;
    }

    /// A row: title (+ optional secondary detail underneath) on the left, `trailing` on the right.
    pub fn row<R>(&mut self, title: &str, detail: &str, trailing: impl FnOnce(&mut Ui) -> R) -> R {
        self.separator();
        let ui = &mut *self.ui;
        ui.add_space(9.0);
        let r = ui
            .horizontal(|ui| {
                ui.set_min_height(26.0);
                let full = ui.available_width();
                // Trailing control first (right-to-left), then the text in the remaining width.
                let r = ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let r = trailing(ui);
                    let rest = ui.available_width();
                    ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                        ui.set_max_width(rest);
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 1.0;
                            ui.add(egui::Label::new(RichText::new(title).size(size::BODY).color(pal::TEXT)).wrap());
                            if !detail.is_empty() {
                                ui.add(egui::Label::new(RichText::new(detail).size(size::FOOTNOTE).color(pal::DIM)).wrap());
                            }
                        });
                    });
                    r
                });
                let _ = full;
                r.inner
            })
            .inner;
        ui.add_space(9.0);
        r
    }

    /// A row with a value on the right.
    pub fn value(&mut self, title: &str, value: impl Into<RichText>) {
        let v = value.into();
        self.row(title, "", |ui| {
            ui.add(egui::Label::new(v.color_if_default(pal::DIM)).truncate());
        });
    }

    /// A row with a switch on the right. Returns true when flipped.
    pub fn switch(&mut self, on: &mut bool, title: &str, detail: &str) -> bool {
        self.row(title, detail, |ui| switch(ui, on).changed())
    }

    /// Free content as a row (wraps).
    pub fn content<R>(&mut self, add: impl FnOnce(&mut Ui) -> R) -> R {
        self.separator();
        self.ui.add_space(9.0);
        let r = add(self.ui);
        self.ui.add_space(9.0);
        r
    }
}

trait ColorIfDefault {
    fn color_if_default(self, c: Color32) -> RichText;
}

impl ColorIfDefault for RichText {
    fn color_if_default(self, c: Color32) -> RichText {
        // RichText has no getter for its colour; values passed as plain text get the secondary colour.
        self.color(c)
    }
}

/// Small secondary heading above a group or a block of cards.
pub fn section(ui: &mut Ui, title: &str) {
    ui.add_space(10.0);
    let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 18.0), Sense::hover());
    ui.painter().text(r.left_center() + Vec2::new(4.0, 0.0), Align2::LEFT_CENTER, title, display(size::FOOTNOTE), pal::DIM);
    ui.add_space(2.0);
}

/// Explanatory text under a group.
pub fn footnote(ui: &mut Ui, text: &str) {
    ui.add_space(2.0);
    ui.add(egui::Label::new(RichText::new(text).size(size::FOOTNOTE).color(pal::DIM)).wrap());
}

// ---------------------------------------------------------------------------------------------
// Text and figures

/// Large number with a small caption underneath.
pub fn stat(ui: &mut Ui, caption: &str, value: impl Into<String>, color: Color32) -> Response {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        ui.label(RichText::new(value.into()).font(display(24.0)).color(color));
        ui.label(RichText::new(caption).size(size::FOOTNOTE).color(pal::DIM));
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
    ui.add_space(48.0);
    ui.vertical_centered(|ui| {
        ui.label(RichText::new(icon).size(44.0).color(pal::FAINT));
        ui.add_space(8.0);
        ui.label(RichText::new(title).font(display(size::TITLE3)).color(pal::TEXT));
        ui.add_space(2.0);
        ui.scope(|ui| {
            ui.set_max_width(460.0);
            ui.add(egui::Label::new(RichText::new(hint).size(size::CALLOUT).color(pal::DIM)).wrap().halign(Align::Center));
        });
    });
    ui.add_space(24.0);
}

/// A table column heading.
pub fn th(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).font(display(size::FOOTNOTE)).color(pal::DIM));
}

// ---------------------------------------------------------------------------------------------
// Meters and tags

/// Slim rounded meter with optional overlay text.
pub fn bar(ui: &mut Ui, frac: f32, color: Color32, height: f32, text: Option<&str>) -> Response {
    let width = ui.available_width().max(40.0);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
    let p = ui.painter();
    let r = CornerRadius::same((height / 2.0).min(255.0) as u8);
    p.rect_filled(rect, r, Color32::from_white_alpha(18));
    let f = frac.clamp(0.0, 1.0);
    if f > 0.0 {
        let mut fill = rect;
        fill.set_width((rect.width() * f).max(height));
        p.rect_filled(fill, r, color);
    }
    if let Some(t) = text {
        p.text(rect.center(), Align2::CENTER_CENTER, t, display((height - 3.0).clamp(10.0, 12.0)), Color32::WHITE);
    }
    resp
}

/// A small tinted pill (status, count, tag). Never wraps: in a wrapping row the whole pill moves to
/// the next line.
pub fn badge(ui: &mut Ui, text: &str, color: Color32) -> Response {
    let galley = ui.painter().layout_no_wrap(text.to_owned(), display(size::CAPTION), color);
    let pad = Vec2::new(8.0, 3.0);
    let (rect, resp) = ui.allocate_exact_size(galley.size() + pad * 2.0, Sense::hover());
    if ui.is_rect_visible(rect) {
        ui.painter().rect_filled(rect, CornerRadius::same(255), color.gamma_multiply(0.18));
        ui.painter().galley(rect.min + pad, galley, color);
    }
    resp
}

// ---------------------------------------------------------------------------------------------
// Controls

/// Apple-style segmented control. Returns true when the value changed.
pub fn segmented<T: PartialEq + Copy>(ui: &mut Ui, value: &mut T, options: &[(T, &str)]) -> bool {
    match tabs(ui, *value, options) {
        Some(v) if v != *value => {
            *value = v;
            true
        }
        _ => false,
    }
}

/// Segmented control used for a page's sections. Returns the segment clicked this frame, including
/// the one already selected (pages use that to reset a drill-down). The selection pill slides.
pub fn tabs<T: PartialEq + Copy>(ui: &mut Ui, value: T, options: &[(T, &str)]) -> Option<T> {
    const INSET: f32 = 2.0;
    const H: f32 = 26.0;
    let sel_font = display(size::CALLOUT);
    let font = FontId::proportional(size::CALLOUT);
    // Width from the semibold face, so segments don't shift as the selection moves.
    let text_w: Vec<f32> = options.iter().map(|(_, l)| ui.painter().layout_no_wrap((*l).to_owned(), sel_font.clone(), pal::TEXT).size().x).collect();
    let n = options.len().max(1) as f32;
    let room = ui.available_width() - 2.0 * INSET - text_w.iter().sum::<f32>();
    if room < 2.0 * n * 6.0 {
        // Too narrow for one control: wrap the choices as pills instead of overflowing.
        let mut clicked = None;
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
            for (v, label) in options {
                if pill(ui, *v == value, label).clicked() {
                    clicked = Some(*v);
                }
            }
        });
        return clicked;
    }
    let pad = (room / (2.0 * n)).clamp(6.0, 14.0);
    let widths: Vec<f32> = text_w.iter().map(|w| w + 2.0 * pad).collect();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(widths.iter().sum::<f32>() + 2.0 * INSET, H + 2.0 * INSET), Sense::hover());
    let mut clicked = None;
    let mut segs = Vec::with_capacity(options.len());
    let mut x = rect.left() + INSET;
    for (i, w) in widths.iter().enumerate() {
        let seg = Rect::from_min_size(Pos2::new(x, rect.top() + INSET), Vec2::new(*w, H));
        let r = ui.interact(seg, resp.id.with(i), Sense::click());
        if r.clicked() {
            clicked = Some(options[i].0);
        }
        segs.push((seg, r.hovered()));
        x += w;
    }
    if !ui.is_rect_visible(rect) {
        return clicked;
    }
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(9), Color32::from_white_alpha(14));
    let sel = options.iter().position(|(v, _)| *v == value);
    if let Some(i) = sel {
        let target = segs[i].0;
        let ctx = ui.ctx();
        let x0 = ctx.animate_value_with_time(resp.id.with("x0"), target.left(), 0.16);
        let x1 = ctx.animate_value_with_time(resp.id.with("x1"), target.right(), 0.16);
        let pill = Rect::from_x_y_ranges(x0..=x1, target.y_range());
        p.add(Shadow { offset: [0, 1], blur: 3, spread: 0, color: Color32::from_black_alpha(80) }.as_shape(pill, CornerRadius::same(7)));
        p.rect_filled(pill, CornerRadius::same(7), Color32::from_rgb(99, 99, 102));
    }
    for (i, ((v, label), (seg, hovered))) in options.iter().zip(&segs).enumerate() {
        let selected = *v == value;
        // Hairline separators between unselected neighbours, as in macOS.
        if i > 0 && !selected && sel != Some(i - 1) {
            p.vline(seg.left(), seg.y_range().shrink(7.0), Stroke::new(1.0, Color32::from_white_alpha(22)));
        }
        let col = if selected || *hovered { pal::TEXT } else { Color32::from_rgb(206, 206, 210) };
        p.text(seg.center(), Align2::CENTER_CENTER, *label, if selected { sel_font.clone() } else { font.clone() }, col);
    }
    clicked
}

/// A bare switch (iOS/macOS style). The response reports `changed()` when flipped.
pub fn switch(ui: &mut Ui, on: &mut bool) -> Response {
    let (rect, mut resp) = ui.allocate_exact_size(Vec2::new(38.0, 22.0), Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    resp.widget_info(|| WidgetInfo::selected(WidgetType::Checkbox, ui.is_enabled(), *on, ""));
    paint_switch(ui, rect, resp.id, *on);
    resp
}

fn paint_switch(ui: &Ui, rect: Rect, id: egui::Id, on: bool) {
    let t = ui.ctx().animate_bool_responsive(id, on);
    let bg = lerp_color(Color32::from_rgb(72, 72, 76), pal::GOOD, t);
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(11), bg);
    let x = egui::lerp((rect.left() + 11.0)..=(rect.right() - 11.0), t);
    p.circle_filled(Pos2::new(x, rect.center().y + 1.0), 9.5, Color32::from_black_alpha(50));
    p.circle_filled(Pos2::new(x, rect.center().y), 9.0, Color32::WHITE);
}

/// A switch with its label to the right (inline use). The response reports `changed()` when flipped.
pub fn toggle(ui: &mut Ui, on: &mut bool, label: &str) -> Response {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        let (rect, mut resp) = ui.allocate_exact_size(Vec2::new(38.0, 22.0), Sense::click());
        let text = ui.add(egui::Label::new(RichText::new(label).color(pal::TEXT)).sense(Sense::click()));
        if resp.clicked() || text.clicked() {
            *on = !*on;
            resp.mark_changed();
        }
        resp.widget_info(|| WidgetInfo::selected(WidgetType::Checkbox, ui.is_enabled(), *on, label));
        paint_switch(ui, rect, resp.id, *on);
        resp
    })
    .inner
}

fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let l = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * t).round() as u8;
    Color32::from_rgb(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()))
}

/// The main action of a view: a filled accent capsule.
pub fn primary_button(ui: &mut Ui, text: &str) -> Response {
    ui.add(
        egui::Button::new(RichText::new(text).font(display(size::CALLOUT)).color(pal::ON_ACCENT))
            .fill(pal::ACCENT)
            .corner_radius(CornerRadius::same(255))
            .min_size(Vec2::new(64.0, 30.0)),
    )
}

/// A secondary action: a grey capsule.
pub fn button(ui: &mut Ui, text: &str) -> Response {
    ui.add(egui::Button::new(RichText::new(text).size(size::CALLOUT).color(pal::TEXT)).corner_radius(CornerRadius::same(255)).min_size(Vec2::new(0.0, 30.0)))
}

/// A frameless icon button with a tooltip.
pub fn icon_button(ui: &mut Ui, icon: &str, tip: &str) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(30.0), Sense::click());
    if resp.hovered() {
        ui.painter().circle_filled(rect.center(), 15.0, Color32::from_white_alpha(14));
    }
    let col = if resp.hovered() { pal::TEXT } else { pal::DIM };
    ui.painter().text(rect.center(), Align2::CENTER_CENTER, icon, FontId::proportional(17.0), col);
    resp.on_hover_text(tip)
}

/// A selectable pill (filters): accent when selected.
pub fn pill(ui: &mut Ui, selected: bool, text: &str) -> Response {
    let (fill, col) = if selected { (pal::ACCENT, pal::ON_ACCENT) } else { (pal::CARD_HI, pal::TEXT) };
    let font = if selected { display(size::CALLOUT) } else { FontId::proportional(size::CALLOUT) };
    ui.add(egui::Button::new(RichText::new(text).font(font).color(col)).fill(fill).corner_radius(CornerRadius::same(255)).min_size(Vec2::new(0.0, 30.0)))
}

/// A rounded suggestion chip.
pub fn chip(ui: &mut Ui, text: &str) -> Response {
    ui.add(egui::Button::new(RichText::new(text).size(size::CALLOUT).color(pal::TEXT)).fill(pal::CARD).corner_radius(CornerRadius::same(255)).min_size(Vec2::new(0.0, 32.0)))
}

/// Collapsible "› Options" disclosure; returns true while open.
pub fn disclosure(ui: &mut Ui, open: &mut bool, label: &str) -> bool {
    let caret = if *open { egui_phosphor::regular::CARET_DOWN } else { egui_phosphor::regular::CARET_RIGHT };
    if ui.add(egui::Button::new(RichText::new(format!("{caret}  {label}")).size(size::CALLOUT).color(pal::DIM)).frame(false)).clicked() {
        *open = !*open;
    }
    *open
}

/// A capsule search field with a magnifying-glass icon.
pub fn search_field(ui: &mut Ui, text: &mut String, hint: &str, width: f32) -> Response {
    Frame::new()
        .fill(pal::FIELD)
        .stroke(Stroke::new(1.0, pal::SEPARATOR))
        .corner_radius(CornerRadius::same(255))
        .inner_margin(Margin { left: 12, right: 10, top: 0, bottom: 0 })
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                ui.label(RichText::new(egui_phosphor::regular::MAGNIFYING_GLASS).size(14.0).color(pal::DIM));
                ui.add_sized(
                    [width - 40.0, 28.0],
                    egui::TextEdit::singleline(text).frame(Frame::new().inner_margin(Margin::symmetric(0, 4))).hint_text(hint).vertical_align(Align::Center),
                )
            })
            .inner
        })
        .inner
}

// ---------------------------------------------------------------------------------------------
// Formatting

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
