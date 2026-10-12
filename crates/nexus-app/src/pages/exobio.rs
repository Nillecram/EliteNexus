use crate::theme::{display, pal, size};
use crate::widgets::{GAP, badge, card_tinted, cards, credits, empty, kv, short_credits, th};
use egui::{Color32, Grid, Pos2, RichText, Sense, Stroke, Ui, Vec2};
use egui_extras::{Column, TableBuilder};
use egui_phosphor::regular as icon;
use nexus_model::exobio::SampleInProgress;
use nexus_model::location::surface_distance;
use nexus_model::GameState;

pub fn show(ui: &mut Ui, gs: &GameState) {
    let pos = gs.status.as_ref().and_then(|s| s.lat_lon());
    let radius = gs.status.as_ref().and_then(|s| s.planet_radius);
    let heading = gs.status.as_ref().and_then(|s| s.heading);

    cards(ui, 2, 340.0, |c| {
        c.add(|ui| {
            card_tinted(ui, icon::PLANT, "Sampling", pal::GOOD, |ui| match &gs.exobio.current {
                Some(cur) => sampling(ui, cur, pos, radius, heading),
                None => {
                    ui.label(RichText::new("No sample in progress").font(display(size::TITLE3)).color(pal::TEXT));
                    ui.add(egui::Label::new(RichText::new("Use the genetic sampler on a plant: progress, the distance to the next valid sample and the colony range appear here live.").color(pal::DIM)).wrap());
                }
            });
        });
        c.add(|ui| unsold(ui, gs));
    });
    ui.add_space(GAP);
    bio_bodies(ui, gs);
}

fn unsold(ui: &mut Ui, gs: &GameState) {
        card_tinted(ui, icon::FLASK, "Unsold Vista Genomics data", pal::GOLD, |ui| {
            let total = gs.exobio.unsold_value();
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 22.0;
                crate::widgets::stat(ui, "estimated payout", short_credits(total), pal::GOLD);
                crate::widgets::stat(ui, "samples", gs.exobio.unsold.len().to_string(), pal::TEXT);
                crate::widgets::stat(ui, "sold so far", short_credits(gs.exobio.sold_total), pal::DIM);
            });
            ui.add_space(4.0);
            if gs.exobio.unsold.is_empty() {
                ui.label(RichText::new("Everything sold.").color(pal::DIM));
            } else {
                Grid::new("unsoldbio").max_col_width(ui.available_width()).num_columns(3).striped(true).spacing([10.0, 3.0]).show(ui, |ui| {
                    for r in gs.exobio.unsold.iter().rev() {
                        ui.label(RichText::new(&r.species).color(pal::TEXT));
                        ui.label(RichText::new(&r.body_name).color(pal::DIM));
                        let mut t = RichText::new(short_credits(r.expected_payout())).color(pal::GOLD);
                        if r.bonus_expected {
                            t = t.strong();
                        }
                        ui.label(t).on_hover_text(if r.bonus_expected { "Includes the first-logged bonus (5× total)" } else { "Base value" });
                        ui.end_row();
                    }
                });
            }
            ui.add_space(4.0);
            ui.add(egui::Label::new(RichText::new("Organic data is lost if you die. Sell it at Vista Genomics.").size(size::FOOTNOTE).color(pal::DIM)).wrap());
        });
}

pub fn sampling(ui: &mut Ui, cur: &SampleInProgress, pos: Option<(f64, f64)>, radius: Option<f64>, heading: Option<f64>) {
    ui.horizontal(|ui| {
        ui.add(egui::Label::new(RichText::new(&cur.species).font(display(size::TITLE))).wrap());
        if let Some(v) = cur.value {
            badge(ui, &short_credits(v), pal::GOLD);
        }
    });
    if let Some(var) = &cur.variant {
        ui.label(RichText::new(var).color(pal::DIM));
    }
    ui.label(RichText::new(format!("{} {}", icon::PLANET, cur.body_name)).color(pal::DIM));
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        for i in 1..=3u8 {
            let done = cur.stage >= i;
            let col = if done { pal::GOOD } else { pal::FAINT };
            ui.label(RichText::new(if done { icon::CHECK_CIRCLE } else { icon::CIRCLE }).size(22.0).color(col));
        }
        ui.label(RichText::new(format!("Sample {} of 3", cur.stage)).font(display(size::HEADLINE)));
    });
    let range = cur.colony_range_m.map(f64::from);
    if let Some(r) = range {
        ui.add(egui::Label::new(RichText::new(format!("Colony range {r:.0} m: samples must be at least this far apart.")).color(pal::DIM)).wrap());
    }
    let (Some(p), Some(rad)) = (pos, radius) else {
        ui.add(egui::Label::new(RichText::new("Position appears once you're on the surface.").size(size::FOOTNOTE).color(pal::DIM)).wrap());
        return;
    };
    if cur.points.is_empty() {
        ui.add(egui::Label::new(RichText::new("Sample positions are only recorded while Elite Nexus is running.").size(size::FOOTNOTE).color(pal::DIM)).wrap());
        return;
    }
    let nearest = cur.nearest_sample_m(p, rad).unwrap_or(0.0);
    let ok = range.is_none_or(|r| nearest >= r);
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        let (txt, col) = if ok { (format!("{} {:.0} m · OK to sample", icon::CHECK_CIRCLE, nearest), pal::GOOD) } else { (format!("{} {:.0} m · too close", icon::WARNING, nearest), pal::BAD) };
        ui.label(RichText::new(txt).font(display(size::TITLE)).color(col));
    });
    if let Some(r) = range
        && !ok {
            ui.label(RichText::new(format!("Move another {:.0} m away from the nearest sample", (r - nearest).max(0.0))).color(pal::WARN));
        }
    radar(ui, cur, p, rad, heading.unwrap_or(0.0), range.unwrap_or(100.0));
}

/// Top-down radar centred on the commander, "up" = current heading.
fn radar(ui: &mut Ui, cur: &SampleInProgress, me: (f64, f64), radius_m: f64, heading_deg: f64, range: f64) {
    let size = ui.available_width().min(320.0);
    ui.add_space(6.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let p = ui.painter_at(rect);
    let c = rect.center();
    let r_px = size / 2.0 - 8.0;
    p.circle_filled(c, r_px, pal::FIELD);
    p.circle_stroke(c, r_px, Stroke::new(1.0, pal::SEPARATOR));
    // Offsets of previous samples in metres (local tangent plane: x east, y north).
    let pts: Vec<(f64, f64)> = cur
        .points
        .iter()
        .map(|s| {
            let dlat = (s.lat - me.0).to_radians();
            let dlon = (s.lon - me.1).to_radians();
            let x = dlon * radius_m * me.0.to_radians().cos();
            let y = dlat * radius_m;
            (x, y)
        })
        .collect();
    let max_d = pts.iter().map(|(x, y)| (x * x + y * y).sqrt() + range).fold(range * 1.5, f64::max);
    let scale = r_px as f64 / max_d;
    // rings every colony range
    let mut ring = range;
    while ring < max_d {
        p.circle_stroke(c, (ring * scale) as f32, Stroke::new(1.0, pal::BORDER.gamma_multiply(0.6)));
        ring += range;
    }
    let h = heading_deg.to_radians();
    let to_screen = |(x, y): (f64, f64)| -> Pos2 {
        // rotate world so that heading points up
        let rx = x * h.cos() - y * h.sin();
        let ry = x * h.sin() + y * h.cos();
        Pos2::new(c.x + (rx * scale) as f32, c.y - (ry * scale) as f32)
    };
    for (i, pt) in pts.iter().enumerate() {
        let sp = to_screen(*pt);
        let d = (pt.0 * pt.0 + pt.1 * pt.1).sqrt();
        let inside = d < range;
        let col = if inside { pal::BAD } else { pal::GOOD };
        p.circle_filled(sp, (range * scale) as f32, col.gamma_multiply(0.12));
        p.circle_stroke(sp, (range * scale) as f32, Stroke::new(1.0, col.gamma_multiply(0.7)));
        p.circle_filled(sp, 5.0, col);
        p.text(sp + Vec2::new(8.0, -8.0), egui::Align2::LEFT_BOTTOM, format!("#{}", i + 1), display(size::CAPTION), pal::TEXT);
    }
    // me
    let tri = [c + Vec2::new(0.0, -9.0), c + Vec2::new(6.0, 7.0), c + Vec2::new(-6.0, 7.0)];
    p.add(egui::Shape::convex_polygon(tri.to_vec(), pal::ACCENT, Stroke::NONE));
    p.text(rect.left_top() + Vec2::new(6.0, 4.0), egui::Align2::LEFT_TOP, format!("{:.0} m", max_d), egui::FontId::proportional(size::CAPTION), pal::DIM);
    let _ = Color32::TRANSPARENT;
}

fn bio_bodies(ui: &mut Ui, gs: &GameState) {
    card_tinted(ui, icon::PLANET, "Biological signals in this system", pal::MINT, |ui| {
        let Some(sys) = gs.exploration.current_system() else {
            ui.label(RichText::new("No system data.").color(pal::DIM));
            return;
        };
        let mut bodies: Vec<_> = sys.bodies.values().filter(|b| b.bio_signals > 0 || !b.organics.is_empty()).collect();
        if bodies.is_empty() {
            empty(ui, icon::PLANT, "No biological signals detected", "FSS-scan the system; bodies with life appear here, DSS-map them to reveal the genera.");
            return;
        }
        bodies.sort_by(|a, b| a.distance_ls.total_cmp(&b.distance_ls));
        TableBuilder::new(ui)
            .striped(true)
            .resizable(false)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .column(Column::initial(110.0).clip(true))
            .column(Column::initial(70.0))
            .column(Column::initial(76.0))
            .column(Column::initial(70.0))
            .column(Column::initial(170.0).clip(true))
            .column(Column::remainder().clip(true))
            .header(30.0, |mut h| {
                for t in ["Body", "Signals", "Gravity", "Temp", "Atmosphere", "Genera / species"] {
                    h.col(|ui| th(ui, t));
                }
            })
            .body(|body| {
                body.rows(32.0, bodies.len(), |mut row| {
                    let b = bodies[row.index()];
                    row.col(|ui| {
                        ui.label(b.short_name(&sys.name));
                    });
                    row.col(|ui| {
                        ui.label(RichText::new(b.bio_signals.to_string()).color(pal::GOOD));
                    });
                    row.col(|ui| {
                        if let Some(g) = b.gravity_g() {
                            ui.label(format!("{g:.2} g"));
                        }
                    });
                    row.col(|ui| {
                        if let Some(t) = b.temperature_k {
                            ui.label(format!("{t:.0} K"));
                        }
                    });
                    row.col(|ui| {
                        ui.label(RichText::new(b.atmosphere.clone().unwrap_or_else(|| "—".into())).color(pal::DIM));
                    });
                    row.col(|ui| {
                        ui.horizontal(|ui| {
                            if b.genuses.is_empty() && b.organics.is_empty() {
                                let likely = b.likely_species();
                                if likely.is_empty() {
                                    ui.label(RichText::new("Map with the DSS to reveal the genera").color(pal::DIM));
                                }
                                for p in likely.iter().take(4) {
                                    badge(ui, &format!("{} {:.0}%", p.species, p.likelihood * 100.0), pal::FAINT).on_hover_text(format!("{} Cr · colony {} m", nexus_data::names::group_digits(p.value), p.colony_range_m));
                                }
                            }
                            for g in &b.genuses {
                                match b.organics.iter().find(|o| o.genus.eq_ignore_ascii_case(g)) {
                                    Some(o) if o.analysed => {
                                        badge(ui, &format!("{} {}", icon::CHECK, o.species), pal::GOOD);
                                    }
                                    Some(o) => {
                                        badge(ui, &format!("{} {}/3", o.species, o.samples), pal::WARN);
                                    }
                                    None => {
                                        // Not sampled yet: show the most likely species of this genus.
                                        let guess = b.predictions.iter().find(|p| p.genus.eq_ignore_ascii_case(g));
                                        let range = nexus_data::exobio::colony_range_m(g).map(|r| format!(" · {r} m")).unwrap_or_default();
                                        match guess {
                                            Some(p) => badge(ui, &format!("{}? {}{range}", p.species, short_credits(p.value)), pal::CYAN),
                                            None => badge(ui, &format!("{g}{range}"), pal::CYAN),
                                        };
                                    }
                                }
                            }
                        });
                    });
                });
            });
    });
    ui.add_space(GAP);
    card_tinted(ui, icon::CHART_BAR, "Totals", pal::BLUE, |ui| {
        Grid::new("biototals").max_col_width(ui.available_width()).num_columns(2).spacing([14.0, 6.0]).show(ui, |ui| {
            kv(ui, "Species analysed", gs.exobio.analysed_total.to_string());
            kv(ui, "Sold to Vista Genomics", credits(gs.exobio.sold_total));
            kv(ui, "Exobiologist rank", gs.cmdr.rank_name(nexus_data::ranks::RankKind::Exobiologist));
        });
        let _ = surface_distance;
    });
}
