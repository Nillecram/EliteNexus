//! Galaxy map: a pannable, zoomable map of the Milky Way, oriented like the in-game map (galactic
//! centre up), with your travels, the active route, your carrier and ships, and landmarks.
//! Drag to pan, scroll or pinch to zoom, double-click to zoom in, click a system for details.
//!
//! The galaxy itself is painted procedurally (a barred spiral fitted to Elite's coordinates), so no
//! game artwork is bundled.

use crate::pages::navigation::Waypoint;
use crate::theme::{display, pal, size};
use egui::{Align2, Color32, CornerRadius, FontId, Pos2, Rect, Sense, Stroke, Ui, Vec2, pos2, vec2};
use egui_phosphor::regular as icon;
use nexus_model::GameState;
use std::sync::Arc;

/// (name, x, z) of well-known places.
const LANDMARKS: &[(&str, f64, f64)] = &[
    ("Sol", 0.0, 0.0),
    ("Shinrarta Dezhra", 55.71875, 17.59375),
    ("Colonia", -9530.5, 19808.125),
    ("Sagittarius A*", 25.21875, 25899.96875),
    ("Beagle Point", -1111.5625, 65269.75),
    ("Eol Prou RS-T d3-94", -9196.0, 19756.0),
];

/// World area covered by the background image (x and z, ly) and the galactic centre.
const GX: (f64, f64) = (-46_000.0, 46_000.0);
const GZ: (f64, f64) = (-20_000.0, 72_000.0);
const CORE: (f64, f64) = (25.2, 25_900.0);
const TEX: usize = 1024;
/// Deep-space backdrop behind the galaxy.
const SPACE: Color32 = Color32::from_rgb(9, 10, 16);
/// Scale limits, screen points per light year.
const MIN_SCALE: f64 = 0.004;
const MAX_SCALE: f64 = 40.0;

pub enum MapAction {
    /// Open Navigation with this system as the destination.
    PlotTo(String),
}

#[derive(Clone, Debug, PartialEq)]
enum PickKind {
    Here,
    Visited { visits: u32, last: nexus_journal::Timestamp },
    Route,
    Carrier,
    Ship,
    Landmark,
}

#[derive(Clone, Debug)]
struct Pick {
    name: String,
    detail: String,
    pos: [f64; 2],
    kind: PickKind,
}

pub struct MapUi {
    pub show_route: bool,
    pub show_labels: bool,
    pub show_landmarks: bool,
    pub show_assets: bool,
    /// Camera centre (x, z in ly) and scale (screen points per ly).
    center: [f64; 2],
    scale: f64,
    /// Where the camera is flying to (centre, scale).
    flight: Option<([f64; 2], f64)>,
    selected: Option<Pick>,
    search: String,
    show_layers: bool,
    framed: bool,
    image: Arc<parking_lot::Mutex<Option<egui::ColorImage>>>,
    painting: bool,
    texture: Option<egui::TextureHandle>,
    /// Open on the whole galaxy instead of your travels.
    start_wide: bool,
}

impl MapUi {
    /// `start_wide`: open on the whole galaxy instead of your travels.
    pub fn new(start_wide: bool) -> Self {
        MapUi { start_wide, ..Default::default() }
    }
}

impl Default for MapUi {
    fn default() -> Self {
        MapUi {
            show_route: true,
            show_labels: true,
            show_landmarks: true,
            show_assets: true,
            center: [0.0, 0.0],
            scale: 0.5,
            flight: None,
            selected: None,
            search: String::new(),
            show_layers: false,
            framed: false,
            image: Arc::default(),
            painting: false,
            texture: None,
            start_wide: false,
        }
    }
}

struct Camera {
    rect: Rect,
    center: [f64; 2],
    scale: f64,
}

impl Camera {
    fn to_screen(&self, p: [f64; 2]) -> Pos2 {
        let c = self.rect.center();
        pos2(c.x + ((p[0] - self.center[0]) * self.scale) as f32, c.y - ((p[1] - self.center[1]) * self.scale) as f32)
    }
    fn to_world(&self, s: Pos2) -> [f64; 2] {
        let c = self.rect.center();
        [self.center[0] + f64::from(s.x - c.x) / self.scale, self.center[1] - f64::from(s.y - c.y) / self.scale]
    }
}

pub fn show(ui: &mut Ui, gs: &GameState, st: &mut MapUi, route: &[Waypoint]) -> Option<MapAction> {
    let mut action = None;
    let avail = ui.available_rect_before_wrap();
    let rect = Rect::from_min_max(avail.min, pos2(avail.max.x, avail.max.y.max(avail.min.y + 420.0)));
    let resp = ui.allocate_rect(rect, Sense::click_and_drag());
    let here = gs.location.star_pos.map(|p| [p[0], p[2]]);
    ensure_galaxy(ui.ctx(), st);

    // Gather what to draw.
    let pos_of = |name: &str| -> Option<[f64; 2]> { gs.history.visited.values().find(|v| v.name.eq_ignore_ascii_case(name)).and_then(|v| v.pos).map(|p| [p[0], p[2]]) };
    let mut picks: Vec<Pick> = Vec::new();
    for v in gs.history.visited.values() {
        if let Some(p) = v.pos {
            picks.push(Pick { name: v.name.clone(), detail: String::new(), pos: [p[0], p[2]], kind: PickKind::Visited { visits: v.visits, last: v.last } });
        }
    }
    let route_pts: Vec<(usize, [f64; 2], &str)> = route.iter().enumerate().filter_map(|(i, w)| w.pos.map(|p| (i, [p[0], p[2]], w.system.as_str()))).collect();
    if st.show_route {
        for (i, p, name) in &route_pts {
            picks.push(Pick { name: (*name).to_string(), detail: format!("Route stop {}", i + 1), pos: *p, kind: PickKind::Route });
        }
    }
    let mut carrier = None;
    let mut ships: Vec<([f64; 2], Vec<String>)> = Vec::new();
    if st.show_assets {
        if gs.carrier.owned()
            && let Some(p) = gs.carrier.system.as_deref().and_then(pos_of)
        {
            carrier = Some(p);
            picks.push(Pick { name: format!("{} {}", gs.carrier.name, gs.carrier.callsign), detail: gs.carrier.system.clone().unwrap_or_default(), pos: p, kind: PickKind::Carrier });
        }
        for s in gs.fleet.ships.values() {
            if Some(s.id) == gs.fleet.current_ship_id {
                continue;
            }
            if let Some(p) = s.location.system.as_deref().and_then(pos_of) {
                let state = if s.location.in_transit { "in transit" } else { "parked" };
                match ships.iter_mut().find(|(q, _)| *q == p) {
                    Some((_, names)) => names.push(s.display_name()),
                    None => ships.push((p, vec![s.display_name()])),
                }
                picks.push(Pick { name: s.display_name(), detail: format!("{} · {}", s.location.system.clone().unwrap_or_default(), state), pos: p, kind: PickKind::Ship });
            }
        }
    }
    if st.show_landmarks {
        for (n, x, z) in LANDMARKS {
            picks.push(Pick { name: (*n).to_string(), detail: String::new(), pos: [*x, *z], kind: PickKind::Landmark });
        }
    }
    if let Some(h) = here {
        picks.push(Pick { name: gs.location.system.clone(), detail: gs.location.situation(), pos: h, kind: PickKind::Here });
    }

    // First view: frame your travels.
    if !st.framed {
        st.framed = true;
        let mut pts: Vec<[f64; 2]> = picks.iter().filter(|p| !matches!(p.kind, PickKind::Landmark)).map(|p| p.pos).collect();
        if st.start_wide {
            pts = vec![[GX.0 + 6000.0, GZ.0 + 4000.0], [GX.1 - 6000.0, GZ.1 - 4000.0]];
        }
        let (c, s) = fit(&pts, rect).unwrap_or(([0.0, 0.0], 0.5));
        st.center = c;
        st.scale = s;
    }

    // Input: drag to pan, scroll / pinch to zoom around the cursor, double-click to zoom in.
    let mut cam = Camera { rect, center: st.center, scale: st.scale };
    if resp.dragged() {
        let d = resp.drag_delta();
        cam.center[0] -= f64::from(d.x) / cam.scale;
        cam.center[1] += f64::from(d.y) / cam.scale;
        st.flight = None;
    }
    if let Some(hover) = resp.hover_pos() {
        let (scroll, pinch) = ui.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
        let mut factor = f64::from(pinch) * (f64::from(scroll) / 300.0).exp();
        if resp.double_clicked() {
            factor *= 2.5;
        }
        if (factor - 1.0).abs() > 1e-4 {
            let before = cam.to_world(hover);
            cam.scale = (cam.scale * factor).clamp(MIN_SCALE, MAX_SCALE);
            let after = cam.to_world(hover);
            cam.center[0] += before[0] - after[0];
            cam.center[1] += before[1] - after[1];
            st.flight = None;
        }
    }
    // Smooth camera flights (locate, search, fit).
    if let Some((tc, ts)) = st.flight {
        let k = 0.18;
        cam.center[0] += (tc[0] - cam.center[0]) * k;
        cam.center[1] += (tc[1] - cam.center[1]) * k;
        cam.scale *= (ts / cam.scale).powf(k);
        let close = ((tc[0] - cam.center[0]).abs() * cam.scale) < 0.5 && ((tc[1] - cam.center[1]).abs() * cam.scale) < 0.5 && (ts / cam.scale - 1.0).abs() < 0.002;
        if close {
            cam.center = tc;
            cam.scale = ts;
            st.flight = None;
        }
        ui.ctx().request_repaint();
    }
    st.center = cam.center;
    st.scale = cam.scale;

    // Paint: space, the galaxy, a fine grid when close in, then the overlays.
    let p = ui.painter_at(rect);
    let radius = CornerRadius::same(16);
    p.rect_filled(rect, radius, SPACE);
    if let Some(tex) = &st.texture {
        let a = cam.to_world(rect.min);
        let b = cam.to_world(rect.max);
        let u = |x: f64| ((x - GX.0) / (GX.1 - GX.0)) as f32;
        let v = |z: f64| ((GZ.1 - z) / (GZ.1 - GZ.0)) as f32;
        let uv = Rect::from_min_max(pos2(u(a[0]), v(a[1])), pos2(u(b[0]), v(b[1])));
        p.add(egui::epaint::RectShape::filled(rect, radius, Color32::WHITE).with_texture(tex.id(), uv));
    }
    grid(&p, &cam);

    let dot = (cam.scale * 4.0).clamp(1.6, 4.0) as f32;
    if st.show_route {
        // Travel path: faint orange trail.
        let mut last: Option<Pos2> = None;
        for j in &gs.history.jumps {
            let Some(pp) = j.pos else { continue };
            let s = cam.to_screen([pp[0], pp[2]]);
            if let Some(l) = last
                && (rect.contains(l) || rect.contains(s))
                && l.distance(s) > 0.5
            {
                p.line_segment([l, s], Stroke::new(1.2, pal::ACCENT.gamma_multiply(0.45)));
            }
            last = Some(s);
        }
    }
    for v in gs.history.visited.values() {
        if let Some(pp) = v.pos {
            let s = cam.to_screen([pp[0], pp[2]]);
            if rect.expand(4.0).contains(s) {
                p.circle_filled(s, dot, Color32::from_rgb(120, 200, 255).gamma_multiply(0.85));
            }
        }
    }
    // The active route: dashed cyan line with stop markers, the next stop highlighted.
    if st.show_route && !route_pts.is_empty() {
        let mut prev = here.map(|h| cam.to_screen(h));
        for (n, (_, wp, _)) in route_pts.iter().enumerate() {
            let s = cam.to_screen(*wp);
            if let Some(a) = prev {
                p.extend(egui::Shape::dashed_line(&[a, s], Stroke::new(2.0, pal::CYAN), 6.0, 4.0));
            }
            let r = if n == 0 { 5.5 } else { 4.0 };
            p.circle(s, r, if n == 0 { pal::CYAN } else { SPACE }, Stroke::new(2.0, pal::CYAN));
            prev = Some(s);
        }
    }
    // Declutter: the blue dot keeps its place, pins step aside, labels never overlap.
    let mut spots: Vec<Pos2> = Vec::new();
    let mut labels: Vec<Rect> = Vec::new();
    let here_s = here.map(|h| cam.to_screen(h));
    if let Some(h) = here_s {
        spots.push(h);
        labels.push(Rect::from_center_size(h, Vec2::splat(22.0)));
    }
    let mut pins: Vec<(Pos2, &str, Color32, String)> = Vec::new();
    if let Some(c) = carrier {
        pins.push((cam.to_screen(c), icon::ANCHOR, pal::GOOD, gs.carrier.name.clone()));
    }
    if st.show_route
        && let Some((_, last, name)) = route_pts.last()
    {
        pins.push((cam.to_screen(*last), icon::FLAG_CHECKERED, pal::TEAL, (*name).to_string()));
    }
    for (sp, names) in &ships {
        let label = if names.len() == 1 { names[0].clone() } else { format!("{} ships", names.len()) };
        pins.push((cam.to_screen(*sp), icon::ROCKET, pal::PURPLE, label));
    }
    let mut drawn: Vec<(Pos2, &str, Color32, String)> = Vec::new();
    for (s, glyph, color, name) in pins {
        let mut at = s;
        let mut k = 0;
        while spots.iter().any(|q| q.distance(at) < 24.0) && k < 8 {
            k += 1;
            at = s + vec2(26.0 * k as f32, 0.0);
        }
        spots.push(at);
        if at != s {
            p.line_segment([s, at], Stroke::new(1.0, Color32::from_white_alpha(90)));
        }
        drawn.push((at, glyph, color, name));
    }
    if st.show_landmarks {
        for (n, x, z) in LANDMARKS {
            let s = cam.to_screen([*x, *z]);
            if rect.expand(60.0).contains(s) {
                let d = 5.0;
                p.add(egui::Shape::convex_polygon(vec![s + vec2(0.0, -d), s + vec2(d, 0.0), s + vec2(0.0, d), s + vec2(-d, 0.0)], pal::GOLD, Stroke::new(1.0, Color32::from_black_alpha(120))));
                if st.show_labels {
                    label(&p, &mut labels, s + vec2(0.0, 9.0), n, pal::GOLD);
                }
            }
        }
    }
    for (at, glyph, color, name) in &drawn {
        pin(&p, *at, glyph, *color);
        if st.show_labels {
            label(&p, &mut labels, *at + vec2(0.0, 16.0), name, pal::TEXT);
        }
    }
    if let Some(h) = here_s {
        you_are_here(&p, h);
    }

    // Hover and click: the nearest thing within reach of the pointer.
    let hovered = resp.hover_pos().and_then(|hp| nearest(&picks, &cam, hp, 14.0));
    if let Some(h) = &hovered
        && !resp.dragged()
    {
        let s = cam.to_screen(h.pos);
        p.circle_stroke(s, dot + 5.0, Stroke::new(2.0, Color32::WHITE));
        tooltip(&p, s, h, here);
    }
    if resp.clicked() {
        st.selected = hovered.cloned();
    }

    // Floating controls (glass): search on the left, map controls on the right, scale bottom-left.
    let mut ui_over = ui.new_child(egui::UiBuilder::new().max_rect(rect.shrink(14.0)).layout(egui::Layout::top_down(egui::Align::Min)));
    let ui2 = &mut ui_over;
    ui2.horizontal(|ui| {
        if let Some(target) = search_box(ui, st, gs) {
            st.selected = Some(target.clone());
            st.flight = Some((target.pos, st.scale.max(0.6)));
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            if round_button(ui, icon::STACK, "Layers").clicked() {
                st.show_layers = !st.show_layers;
            }
            if round_button(ui, icon::CORNERS_OUT, "Show all your travels").clicked() {
                let pts: Vec<[f64; 2]> = picks.iter().filter(|p| !matches!(p.kind, PickKind::Landmark)).map(|p| p.pos).collect();
                if let Some(f) = fit(&pts, rect) {
                    st.flight = Some(f);
                }
            }
            if round_button(ui, icon::GLOBE_HEMISPHERE_WEST, "Whole galaxy").clicked() {
                st.flight = fit(&[[GX.0 + 6000.0, GZ.0 + 4000.0], [GX.1 - 6000.0, GZ.1 - 4000.0]], rect);
            }
            if let Some(h) = here
                && round_button(ui, icon::NAVIGATION_ARROW, "Where you are").clicked()
            {
                st.flight = Some((h, st.scale.max(1.0)));
            }
            if round_button(ui, icon::MINUS, "Zoom out").clicked() {
                st.flight = Some((st.center, (st.scale / 2.0).max(MIN_SCALE)));
            }
            if round_button(ui, icon::PLUS, "Zoom in").clicked() {
                st.flight = Some((st.center, (st.scale * 2.0).min(MAX_SCALE)));
            }
        });
    });
    if st.show_layers {
        ui2.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
            glass_panel(ui, 230.0, |ui| {
                ui.label(egui::RichText::new("Show on the map").font(display(size::FOOTNOTE)).color(pal::DIM));
                crate::widgets::toggle(ui, &mut st.show_route, "Travels and route");
                crate::widgets::toggle(ui, &mut st.show_assets, "Carrier and ships");
                crate::widgets::toggle(ui, &mut st.show_landmarks, "Landmarks");
                crate::widgets::toggle(ui, &mut st.show_labels, "Names");
            });
        });
    }
    scale_bar(&p, &cam);
    legend(&p, rect, gs, st);

    // Selection card, bottom left.
    if let Some(sel) = st.selected.clone() {
        let card = Rect::from_min_size(pos2(rect.left() + 14.0, rect.bottom() - 14.0 - 136.0), vec2(300.0, 136.0));
        let mut cu = ui.new_child(egui::UiBuilder::new().max_rect(card).layout(egui::Layout::top_down(egui::Align::Min)));
        glass_panel(&mut cu, 300.0, |ui| {
            ui.horizontal(|ui| {
                ui.add(egui::Label::new(egui::RichText::new(&sel.name).font(display(size::HEADLINE)).color(pal::TEXT)).truncate());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if crate::widgets::icon_button(ui, icon::X, "Close").clicked() {
                        st.selected = None;
                    }
                });
            });
            let mut lines = Vec::new();
            match &sel.kind {
                PickKind::Visited { visits, last } => lines.push(format!("{} visit{} · last {}", visits, if *visits == 1 { "" } else { "s" }, last.to_local_string("%d %b %Y"))),
                _ if !sel.detail.is_empty() => lines.push(sel.detail.clone()),
                _ => {}
            }
            if let Some(h) = here
                && sel.kind != PickKind::Here
            {
                lines.push(format!("{:.1} ly from you", ((sel.pos[0] - h[0]).powi(2) + (sel.pos[1] - h[1]).powi(2)).sqrt()));
            }
            for l in lines {
                ui.label(egui::RichText::new(l).size(size::FOOTNOTE).color(pal::DIM));
            }
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if crate::widgets::button(ui, &format!("{} Copy name", icon::COPY)).clicked() {
                    ui.ctx().copy_text(sel.name.clone());
                }
                let system = match sel.kind {
                    PickKind::Carrier | PickKind::Ship => sel.detail.split(" · ").next().unwrap_or_default().to_string(),
                    _ => sel.name.clone(),
                };
                if sel.kind != PickKind::Here && !system.is_empty() && crate::widgets::primary_button(ui, "Plot route").clicked() {
                    action = Some(MapAction::PlotTo(system));
                }
            });
        });
    }
    action
}

/// Camera that fits `pts` into `rect` with a margin.
fn fit(pts: &[[f64; 2]], rect: Rect) -> Option<([f64; 2], f64)> {
    let first = pts.first()?;
    let (mut x0, mut x1, mut z0, mut z1) = (first[0], first[0], first[1], first[1]);
    for p in pts {
        x0 = x0.min(p[0]);
        x1 = x1.max(p[0]);
        z0 = z0.min(p[1]);
        z1 = z1.max(p[1]);
    }
    let w = (x1 - x0).max(60.0);
    let h = (z1 - z0).max(60.0);
    let s = (f64::from(rect.width() - 160.0) / w).min(f64::from(rect.height() - 160.0) / h).clamp(MIN_SCALE, MAX_SCALE);
    Some(([(x0 + x1) / 2.0, (z0 + z1) / 2.0], s))
}

fn nearest<'a>(picks: &'a [Pick], cam: &Camera, at: Pos2, reach: f32) -> Option<&'a Pick> {
    let rank = |k: &PickKind| match k {
        PickKind::Here => 0,
        PickKind::Carrier | PickKind::Ship => 1,
        PickKind::Route => 2,
        PickKind::Landmark => 3,
        PickKind::Visited { .. } => 4,
    };
    picks
        .iter()
        .filter_map(|p| {
            let d = cam.to_screen(p.pos).distance(at);
            (d <= reach).then_some((d + rank(&p.kind) as f32 * 2.0, p))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, p)| p)
}

/// Faint grid at 1 / 10 / 100 / 1000 ly steps once the galaxy image runs out of detail.
fn grid(p: &egui::Painter, cam: &Camera) {
    if cam.scale < 0.02 {
        return;
    }
    // Pick a step that is 60–600 points on screen.
    let mut step = 1.0;
    while step * cam.scale < 70.0 {
        step *= 10.0;
    }
    let a = cam.to_world(cam.rect.min);
    let b = cam.to_world(cam.rect.max);
    let alpha = ((cam.scale - 0.02) / 0.2).clamp(0.0, 1.0) as f32;
    let stroke = Stroke::new(1.0, Color32::from_white_alpha((10.0 * alpha) as u8));
    let mut x = (a[0] / step).floor() * step;
    while x <= b[0] {
        let sx = cam.to_screen([x, 0.0]).x;
        p.vline(sx, cam.rect.y_range(), stroke);
        x += step;
    }
    let mut z = (b[1] / step).floor() * step;
    while z <= a[1] {
        let sy = cam.to_screen([0.0, z]).y;
        p.hline(cam.rect.x_range(), sy, stroke);
        z += step;
    }
}

/// The blue "you are here" dot with a white ring and a soft halo (static: no idle repaints).
fn you_are_here(p: &egui::Painter, s: Pos2) {
    p.circle_filled(s, 18.0, pal::BLUE.gamma_multiply(0.16));
    p.circle_filled(s + vec2(0.0, 1.0), 9.0, Color32::from_black_alpha(80));
    p.circle_filled(s, 8.5, Color32::WHITE);
    p.circle_filled(s, 6.5, pal::BLUE);
}

/// A map pin: a coloured disc with a white rim and a glyph.
fn pin(p: &egui::Painter, s: Pos2, glyph: &str, color: Color32) {
    p.circle_filled(s + vec2(0.0, 1.5), 12.0, Color32::from_black_alpha(90));
    p.circle_filled(s, 12.0, color);
    p.circle_stroke(s, 12.0, Stroke::new(1.5, Color32::WHITE));
    p.text(s, Align2::CENTER_CENTER, glyph, FontId::proportional(13.0), Color32::WHITE);
}

/// A map label on a dark pill, skipped when it would overlap one already placed.
fn label(p: &egui::Painter, placed: &mut Vec<Rect>, top: Pos2, text: &str, color: Color32) {
    let g = p.layout_no_wrap(text.to_owned(), display(size::CAPTION), color);
    let r = Rect::from_center_size(top + vec2(0.0, g.size().y / 2.0), g.size() + vec2(10.0, 4.0));
    if placed.iter().any(|q| q.expand(2.0).intersects(r)) {
        return;
    }
    placed.push(r);
    p.rect_filled(r, CornerRadius::same(255), Color32::from_black_alpha(160));
    p.galley(r.min + vec2(5.0, 2.0), g, color);
}

fn tooltip(p: &egui::Painter, s: Pos2, pick: &Pick, here: Option<[f64; 2]>) {
    let mut sub = match &pick.kind {
        PickKind::Visited { visits, last } => format!("{} visit{} · {}", visits, if *visits == 1 { "" } else { "s" }, last.to_local_string("%d %b %Y")),
        PickKind::Landmark => "Landmark".into(),
        _ => pick.detail.clone(),
    };
    if let Some(h) = here
        && pick.kind != PickKind::Here
    {
        let d = ((pick.pos[0] - h[0]).powi(2) + (pick.pos[1] - h[1]).powi(2)).sqrt();
        sub = if sub.is_empty() { format!("{d:.1} ly away") } else { format!("{sub} · {d:.1} ly away") };
    }
    let title = p.layout_no_wrap(pick.name.clone(), display(size::CALLOUT), pal::TEXT);
    let detail = p.layout_no_wrap(sub, FontId::proportional(size::CAPTION), pal::DIM);
    let w = title.size().x.max(detail.size().x) + 24.0;
    let h = title.size().y + detail.size().y + 16.0;
    let r = Rect::from_min_size(s + vec2(14.0, -h - 10.0), vec2(w, h));
    p.add(egui::Shadow { offset: [0, 6], blur: 18, spread: 0, color: Color32::from_black_alpha(120) }.as_shape(r, CornerRadius::same(10)));
    p.rect_filled(r, CornerRadius::same(10), Color32::from_rgb(48, 48, 52));
    p.rect_stroke(r, CornerRadius::same(10), Stroke::new(1.0, Color32::from_white_alpha(20)), egui::StrokeKind::Inside);
    let th = title.size().y;
    p.galley(r.min + vec2(12.0, 8.0), title, pal::TEXT);
    p.galley(r.min + vec2(12.0, 8.0 + th), detail, pal::DIM);
}

/// Distance scale, bottom right, Apple Maps style.
fn scale_bar(p: &egui::Painter, cam: &Camera) {
    let target = 110.0 / cam.scale;
    let pow = 10f64.powf(target.log10().floor());
    let step = [1.0, 2.0, 5.0].iter().map(|m| m * pow).rfind(|s| *s <= target).unwrap_or(pow);
    let w = (step * cam.scale) as f32;
    let y = cam.rect.bottom() - 22.0;
    let x1 = cam.rect.right() - 20.0;
    let x0 = x1 - w;
    let s = Stroke::new(2.0, Color32::from_white_alpha(200));
    p.line_segment([pos2(x0, y), pos2(x1, y)], s);
    p.line_segment([pos2(x0, y - 4.0), pos2(x0, y + 1.0)], s);
    p.line_segment([pos2(x1, y - 4.0), pos2(x1, y + 1.0)], s);
    let text = if step >= 1000.0 { format!("{} ly", nexus_data::names::group_digits(step as i64)) } else { format!("{step} ly") };
    p.text(pos2(x1, y - 7.0), Align2::RIGHT_BOTTOM, text, display(size::CAPTION), Color32::from_white_alpha(220));
}

/// Counts in the corner: systems visited and distance flown.
fn legend(p: &egui::Painter, rect: Rect, gs: &GameState, st: &MapUi) {
    if st.selected.is_some() {
        return;
    }
    let text = format!("{} systems visited · {} ly travelled", gs.history.visited.len(), nexus_data::names::group_digits(gs.history.total_distance_ly as i64));
    let g = p.layout_no_wrap(text, FontId::proportional(size::CAPTION), pal::DIM);
    let r = Rect::from_min_size(pos2(rect.left() + 14.0, rect.bottom() - 14.0 - g.size().y - 10.0), g.size() + vec2(20.0, 10.0));
    p.rect_filled(r, CornerRadius::same(255), Color32::from_black_alpha(140));
    p.galley(r.min + vec2(10.0, 5.0), g, pal::DIM);
}

fn round_button(ui: &mut Ui, glyph: &str, tip: &str) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(36.0), Sense::click());
    let p = ui.painter();
    p.add(egui::Shadow { offset: [0, 4], blur: 14, spread: 0, color: Color32::from_black_alpha(110) }.as_shape(rect, CornerRadius::same(18)));
    p.rect_filled(rect, CornerRadius::same(18), if resp.hovered() { Color32::from_rgb(64, 64, 68) } else { Color32::from_rgb(44, 44, 48) });
    p.rect_stroke(rect, CornerRadius::same(18), Stroke::new(1.0, Color32::from_white_alpha(22)), egui::StrokeKind::Inside);
    p.text(rect.center(), Align2::CENTER_CENTER, glyph, FontId::proportional(17.0), pal::TEXT);
    resp.on_hover_text(tip)
}

/// A floating glass panel of the given width.
fn glass_panel(ui: &mut Ui, width: f32, add: impl FnOnce(&mut Ui)) {
    egui::Frame::new()
        .fill(Color32::from_rgba_unmultiplied(40, 40, 44, 236))
        .stroke(Stroke::new(1.0, Color32::from_white_alpha(22)))
        .corner_radius(CornerRadius::same(14))
        .inner_margin(egui::Margin::same(14))
        .shadow(egui::Shadow { offset: [0, 8], blur: 24, spread: 0, color: Color32::from_black_alpha(120) })
        .show(ui, |ui| {
            ui.set_width(width - 28.0);
            ui.with_layout(egui::Layout::top_down(egui::Align::Min), add);
        });
}

/// Search field over the map: type part of a visited system, landmark, ship or carrier name.
fn search_box(ui: &mut Ui, st: &mut MapUi, gs: &GameState) -> Option<Pick> {
    let mut chosen = None;
    let out = egui::Frame::new()
        .fill(Color32::from_rgba_unmultiplied(40, 40, 44, 236))
        .stroke(Stroke::new(1.0, Color32::from_white_alpha(22)))
        .corner_radius(CornerRadius::same(18))
        .inner_margin(egui::Margin { left: 12, right: 12, top: 0, bottom: 0 })
        .shadow(egui::Shadow { offset: [0, 4], blur: 14, spread: 0, color: Color32::from_black_alpha(110) })
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(icon::MAGNIFYING_GLASS).size(15.0).color(pal::DIM));
                ui.add_sized([220.0, 36.0], egui::TextEdit::singleline(&mut st.search).frame(egui::Frame::new()).hint_text("Search your galaxy").vertical_align(egui::Align::Center))
            })
            .inner
        });
    let q = st.search.trim().to_lowercase();
    if q.len() < 2 {
        return None;
    }
    let mut hits: Vec<Pick> = gs
        .history
        .visited
        .values()
        .filter(|v| v.name.to_lowercase().contains(&q))
        .filter_map(|v| v.pos.map(|p| Pick { name: v.name.clone(), detail: String::new(), pos: [p[0], p[2]], kind: PickKind::Visited { visits: v.visits, last: v.last } }))
        .chain(LANDMARKS.iter().filter(|(n, _, _)| n.to_lowercase().contains(&q)).map(|(n, x, z)| Pick { name: (*n).to_string(), detail: String::new(), pos: [*x, *z], kind: PickKind::Landmark }))
        .collect();
    hits.sort_by_key(|h| (!h.name.to_lowercase().starts_with(&q), h.name.len()));
    hits.truncate(6);
    let below = out.response.rect;
    egui::Area::new(ui.id().with("map-search"))
        .fixed_pos(below.left_bottom() + vec2(0.0, 6.0))
        .order(egui::Order::Foreground)
        .show(ui.ctx(), |ui| {
            glass_panel(ui, below.width().max(280.0), |ui| {
                if hits.is_empty() {
                    ui.label(egui::RichText::new("No matching places").color(pal::DIM));
                }
                for h in &hits {
                    let r = ui.add(egui::Button::new(egui::RichText::new(&h.name).color(pal::TEXT)).frame(false));
                    if r.clicked() {
                        chosen = Some(h.clone());
                    }
                }
            });
        });
    if chosen.is_some() {
        st.search.clear();
    }
    chosen
}

/// Starts painting the galaxy on a worker thread the first time, then uploads it as a texture.
fn ensure_galaxy(ctx: &egui::Context, st: &mut MapUi) {
    if st.texture.is_some() {
        return;
    }
    if let Some(img) = st.image.lock().take() {
        st.texture = Some(ctx.load_texture("galaxy", img, egui::TextureOptions::LINEAR));
        return;
    }
    if !st.painting {
        st.painting = true;
        let slot = st.image.clone();
        let ctx = ctx.clone();
        let _ = std::thread::Builder::new().name("galaxy-paint".into()).spawn(move || {
            *slot.lock() = Some(galaxy_image());
            ctx.request_repaint();
        });
    }
}

/// Hash-based value noise in [0, 1].
fn noise(x: f64, y: f64) -> f64 {
    fn h(i: i64, j: i64) -> f64 {
        let mut n = (i.wrapping_mul(374_761_393) ^ j.wrapping_mul(668_265_263)) as u64;
        n = (n ^ (n >> 13)).wrapping_mul(1_274_126_177);
        ((n ^ (n >> 16)) & 0xffff) as f64 / 65535.0
    }
    let (xi, yi) = (x.floor() as i64, y.floor() as i64);
    let (xf, yf) = (x - x.floor(), y - y.floor());
    let s = |t: f64| t * t * (3.0 - 2.0 * t);
    let (u, v) = (s(xf), s(yf));
    let a = h(xi, yi) + (h(xi + 1, yi) - h(xi, yi)) * u;
    let b = h(xi, yi + 1) + (h(xi + 1, yi + 1) - h(xi, yi + 1)) * u;
    a + (b - a) * v
}

fn fbm(x: f64, y: f64) -> f64 {
    let mut sum = 0.0;
    let mut amp = 0.5;
    let mut f = 1.0;
    for _ in 0..4 {
        sum += amp * noise(x * f, y * f);
        f *= 2.03;
        amp *= 0.5;
    }
    sum
}

/// A barred spiral matching Elite's galaxy: the core at Sagittarius A*, Sol near the rim, Colonia
/// and Beagle Point where they belong.
fn galaxy_image() -> egui::ColorImage {
    let mut px = vec![Color32::TRANSPARENT; TEX * TEX];
    let bar_angle: f64 = 0.45;
    let (bs, bc) = bar_angle.sin_cos();
    let pitch: f64 = 12.5f64.to_radians();
    let k = 1.0 / pitch.tan();
    for j in 0..TEX {
        let z = GZ.1 - (j as f64 + 0.5) / TEX as f64 * (GZ.1 - GZ.0);
        for i in 0..TEX {
            let x = GX.0 + (i as f64 + 0.5) / TEX as f64 * (GX.1 - GX.0);
            let (dx, dz) = (x - CORE.0, z - CORE.1);
            let r = (dx * dx + dz * dz).sqrt().max(1.0);
            let theta = dz.atan2(dx);
            // Bulge and bar.
            let bulge = (-(r / 2600.0).powf(1.5)).exp();
            let (u, v) = (dx * bc + dz * bs, -dx * bs + dz * bc);
            let bar = (-(u / 8200.0).powi(2) - (v / 2300.0).powi(2)).exp();
            // Logarithmic spiral arms: two major, two minor.
            let phase = theta - k * (r / 4000.0).ln();
            let major = (0.5 + 0.5 * (2.0 * phase).cos()).powi(5);
            let minor = (0.5 + 0.5 * (2.0 * phase + std::f64::consts::FRAC_PI_2).cos()).powi(7) * 0.55;
            let arms = (major + minor) * (((r - 5000.0) / 4000.0).clamp(0.0, 1.0));
            // Dust lane on the inside edge of each major arm.
            let dust = (0.5 + 0.5 * (2.0 * (phase + 0.32)).cos()).powi(9) * ((r - 4000.0) / 6000.0).clamp(0.0, 1.0);
            let disk = (-r / 13_500.0).exp() * (1.0 - ((r - 40_000.0) / 7000.0).clamp(0.0, 1.0));
            let n = fbm(x / 2600.0, z / 2600.0);
            let clumps = fbm(x / 900.0 + 31.0, z / 900.0 - 17.0);
            let arm_light = disk * (0.20 + 1.05 * arms * (0.55 + 0.9 * clumps)) * (0.75 + 0.5 * n) * (1.0 - 0.55 * dust);
            let core_light = bulge * 1.25 + bar * 0.7;
            let total = (arm_light * 1.6 + core_light).min(1.4);
            if total < 0.004 {
                continue;
            }
            // Warm core, blue-white arms, a touch of pink star-forming regions in the arms.
            let warm = (core_light / (core_light + arm_light + 1e-6)).clamp(0.0, 1.0);
            let pink = ((clumps - 0.62) * 6.0).clamp(0.0, 1.0) * arms * 0.6;
            let (cr, cg, cb) = (
                170.0 * (1.0 - warm) + 255.0 * warm,
                190.0 * (1.0 - warm) + 222.0 * warm,
                255.0 * (1.0 - warm) + 170.0 * warm,
            );
            let (cr, cg, cb) = (cr + (255.0 - cr) * pink, cg * (1.0 - 0.35 * pink), cb * (1.0 - 0.15 * pink));
            let a = (total.powf(0.8) * 255.0).clamp(0.0, 255.0);
            px[j * TEX + i] = Color32::from_rgba_unmultiplied(cr as u8, cg as u8, cb as u8, a as u8);
        }
    }
    // A sprinkle of bright stars across the disc.
    let mut seed: u64 = 0x9e37_79b9_7f4a_7c15;
    for _ in 0..9000 {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        let i = (seed % TEX as u64) as usize;
        let j = ((seed >> 20) % TEX as u64) as usize;
        let cur = px[j * TEX + i];
        if cur.a() > 20 {
            let b = (cur.a() as f32 / 255.0 * 1.6).min(1.0);
            px[j * TEX + i] = Color32::from_rgba_unmultiplied(255, 255, 255, (200.0 * b + 55.0) as u8);
        }
    }
    egui::ColorImage::new([TEX, TEX], px)
}
