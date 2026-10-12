//! Top-down galaxy map of everywhere the commander has been (X/Z plane, like the in-game map),
//! with landmarks, your carrier and where your ships are parked.

use crate::theme::pal;
use crate::widgets::card;
use egui::{RichText, Ui};
use egui_plot::{Line, MarkerShape, Plot, PlotPoints, Points, Text};
use nexus_model::GameState;

#[derive(Default)]
pub struct MapUi {
    pub show_route: bool,
    pub show_labels: bool,
    pub show_landmarks: bool,
    pub show_assets: bool,
}

/// (name, x, z) of well-known places.
const LANDMARKS: &[(&str, f64, f64)] = &[
    ("Sol", 0.0, 0.0),
    ("Colonia", -9530.5, 19808.125),
    ("Sagittarius A*", 25.21875, 25899.96875),
    ("Beagle Point", -1111.5625, 65269.75),
    ("Jaques' first stop (Eol Prou)", -9196.0, 19756.0),
    ("Shinrarta Dezhra", 55.71875, 17.59375),
];

pub fn show(ui: &mut Ui, gs: &GameState, st: &mut MapUi) {
    ui.horizontal(|ui| {
        ui.checkbox(&mut st.show_route, "Travel path");
        ui.checkbox(&mut st.show_labels, "Hover names");
        ui.checkbox(&mut st.show_assets, "Ships & carrier");
        ui.checkbox(&mut st.show_landmarks, "Landmarks");
        ui.label(RichText::new(format!("{} systems visited · {:.0} ly travelled", gs.history.visited.len(), gs.history.total_distance_ly)).color(pal::DIM));
    });
    // Position lookup by system name (for ships/carrier we only know names).
    let pos_of = |name: &str| -> Option<[f64; 2]> {
        gs.history.visited.values().find(|v| v.name.eq_ignore_ascii_case(name)).and_then(|v| v.pos).map(|p| [p[0], p[2]])
    };
    let mut assets: Vec<([f64; 2], String)> = Vec::new();
    if st.show_assets {
        for s in gs.fleet.ships.values() {
            if Some(s.id) == gs.fleet.current_ship_id {
                continue;
            }
            if let Some(p) = s.location.system.as_deref().and_then(pos_of) {
                assets.push((p, format!("{} ({})", s.display_name(), if s.location.in_transit { "in transit" } else { "parked" })));
            }
        }
    }
    let carrier = if st.show_assets { gs.carrier.system.as_deref().and_then(pos_of).map(|p| (p, format!("{} {}", gs.carrier.name, gs.carrier.callsign))) } else { None };

    card(ui, "", |ui| {
        let visited: Vec<[f64; 2]> = gs.history.visited.values().filter_map(|v| v.pos.map(|p| [p[0], p[2]])).collect();
        let path: Vec<[f64; 2]> = gs.history.jumps.iter().filter_map(|j| j.pos.map(|p| [p[0], p[2]])).collect();
        let here = gs.location.star_pos.map(|p| [p[0], p[2]]);
        let mut names: Vec<([f64; 2], String)> = gs.history.visited.values().filter_map(|v| v.pos.map(|p| ([p[0], p[2]], v.name.clone()))).collect();
        names.extend(assets.iter().cloned());
        if let Some(c) = &carrier {
            names.push(c.clone());
        }
        if st.show_landmarks {
            names.extend(LANDMARKS.iter().map(|(n, x, z)| ([*x, *z], (*n).to_string())));
        }
        let show_labels = st.show_labels;
        let h = ui.available_height().max(400.0);
        Plot::new("galaxy")
            .data_aspect(1.0)
            .height(h)
            .show_background(true)
            .x_axis_label("X (ly)")
            .y_axis_label("Z (ly)")
            .label_formatter(move |hp| {
                let (hx, hy) = crate::widgets::hover_xy(hp);
                if !show_labels {
                    return Some(format!("{hx:.0}, {hy:.0}"));
                }
                let near = names.iter().min_by(|a, b| {
                    let da = (a.0[0] - hx).powi(2) + (a.0[1] - hy).powi(2);
                    let db = (b.0[0] - hx).powi(2) + (b.0[1] - hy).powi(2);
                    da.total_cmp(&db)
                });
                Some(near.map(|(_, n)| n.clone()).unwrap_or_default())
            })
            .show(ui, |p| {
                if st.show_landmarks {
                    for (n, x, z) in LANDMARKS {
                        p.points(Points::new(*n, PlotPoints::from(vec![[*x, *z]])).radius(4.0).color(pal::GOLD).shape(MarkerShape::Diamond));
                        p.text(Text::new(*n, egui_plot::PlotPoint::new(*x, *z + 60.0), RichText::new(*n).small().color(pal::GOLD)));
                    }
                }
                if st.show_route && path.len() > 1 {
                    p.line(Line::new("path", PlotPoints::from(path)).color(pal::ACCENT.gamma_multiply(0.45)).width(1.0));
                }
                p.points(Points::new("visited", PlotPoints::from(visited)).radius(2.0).color(pal::CYAN));
                if !assets.is_empty() {
                    let pts: Vec<[f64; 2]> = assets.iter().map(|(p, _)| *p).collect();
                    p.points(Points::new("ships", PlotPoints::from(pts)).radius(4.5).color(pal::PURPLE).shape(MarkerShape::Square));
                }
                if let Some((cp, _)) = &carrier {
                    p.points(Points::new("carrier", PlotPoints::from(vec![*cp])).radius(6.0).color(pal::GOOD).shape(MarkerShape::Asterisk));
                }
                if let Some(h) = here {
                    p.points(Points::new("you", PlotPoints::from(vec![h])).radius(6.0).color(pal::ACCENT).shape(MarkerShape::Circle));
                }
            });
    });
}
