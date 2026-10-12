use crate::theme::{display, pal, size};
use crate::widgets::{GAP, badge, card_tinted, cards, credits, empty, kv, short_credits, th};
use egui::{Color32, Grid, Pos2, RichText, Sense, Stroke, Ui, Vec2};
use egui_extras::{Column, TableBuilder};
use egui_phosphor::regular as icon;
use nexus_model::exobio::SampleInProgress;
use nexus_model::location::surface_distance;
use nexus_model::GameState;

use crate::jobs::Slot;
use crate::untouched::{self, Candidate, Evidence, Stats};

/// State of the "Find untouched planets" search.
pub struct ExobioUi {
    target: usize,
    radius: u32,
    include_mapped: bool,
    job: Slot<Result<(Vec<Candidate>, Stats), String>>,
    /// The search the results belong to (species, radius).
    searched: Option<(&'static str, u32)>,
    /// Start a search on the first frame (screenshot runs: `--tab untouched`).
    autorun: bool,
}

impl Default for ExobioUi {
    fn default() -> Self {
        let target = untouched::targets().iter().position(|s| s.name == "Stratum Tectonicas").unwrap_or(0);
        ExobioUi { target, radius: 500, include_mapped: true, job: Slot::default(), searched: None, autorun: false }
    }
}

impl ExobioUi {
    /// `autorun`: start a search on the first frame (screenshot runs).
    pub fn new(autorun: bool) -> Self {
        ExobioUi { autorun, ..Default::default() }
    }
}

/// What the page asks the app to do.
pub enum ExoAction {
    /// Open Navigation with this system as the destination.
    PlotTo(String),
}

pub fn show(ui: &mut Ui, gs: &GameState, st: &mut ExobioUi, spansh: &nexus_spansh::Spansh, rt: &tokio::runtime::Handle, db: Option<&nexus_galaxy::GalaxyDb>) -> Option<ExoAction> {
    show_page(ui, gs);
    ui.add_space(GAP);
    let action = untouched_card(ui, gs, st, spansh, rt, db);
    ui.add_space(GAP);
    bio_bodies(ui, gs);
    action
}

/// "Find untouched planets": where a valuable species should grow and nobody sharing data has
/// mapped or sampled (see untouched.rs for what that can and cannot prove).
fn untouched_card(ui: &mut Ui, gs: &GameState, st: &mut ExobioUi, spansh: &nexus_spansh::Spansh, rt: &tokio::runtime::Handle, db: Option<&nexus_galaxy::GalaxyDb>) -> Option<ExoAction> {
    st.job.tick();
    let targets = untouched::targets();
    let mut action = None;
    card_tinted(ui, icon::SPARKLE, "Find untouched planets", pal::ACCENT, |ui| {
        ui.add(
            egui::Label::new(
                RichText::new("Planets where a valuable species should grow and that no commander sharing data (EDMC, EDDiscovery…) has mapped or sampled, so the first-logged bonus (×5) is likely still there.")
                    .color(pal::DIM),
            )
            .wrap(),
        );
        ui.add_space(8.0);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(10.0, 10.0);
            let cur = targets.get(st.target).or(targets.first()).copied();
            if let Some(cur) = cur {
                egui::ComboBox::from_id_salt("untouched-species")
                    .width(300.0)
                    .selected_text(format!("{} · {} first logged", cur.name, short_credits(nexus_data::bio_predict::payout(cur.value, true))))
                    .show_ui(ui, |ui| {
                        for (i, s) in targets.iter().enumerate() {
                            ui.selectable_value(&mut st.target, i, format!("{} · {} ({} first logged)", s.name, short_credits(s.value), short_credits(nexus_data::bio_predict::payout(s.value, true))));
                        }
                    });
            }
            crate::widgets::segmented(ui, &mut st.radius, &[(250, "250 ly"), (500, "500 ly"), (1000, "1,000 ly"), (2000, "2,000 ly")]);
            crate::widgets::toggle(ui, &mut st.include_mapped, "Include mapped, never sampled");
            let origin = gs.location.star_pos;
            let busy = st.job.busy();
            let label = if busy { "Searching…" } else { "Search" };
            let clicked = ui.add_enabled_ui(!busy && origin.is_some(), |ui| crate::widgets::primary_button(ui, label)).inner.clicked();
            if (clicked || std::mem::take(&mut st.autorun))
                && !busy
                && let (Some(origin), Some(species)) = (origin, cur)
            {
                let q = untouched::Query { species, origin, radius_ly: f64::from(st.radius), include_mapped: st.include_mapped, limit: 25 };
                st.searched = Some((species.name, st.radius));
                let sp = spansh.clone();
                let db = db.cloned();
                st.job.run_async(ui.ctx(), rt, async move {
                    let footfalled = move |name: &str| db.as_ref().and_then(|d| d.body_by_name(name).ok().flatten()).is_some_and(|b| b.was_footfalled.is_some_and(|s| s.value));
                    untouched::find(&sp, &q, footfalled).await
                });
            }
            if busy {
                ui.spinner();
            }
        });
        ui.add_space(6.0);
        match &st.job.last {
            None if !st.job.busy() => {
                ui.add(egui::Label::new(RichText::new("Pick a species and a range, then Search. Planets are checked on Spansh against everything commanders have shared.").size(size::FOOTNOTE).color(pal::DIM)).wrap());
            }
            None => {}
            Some(Err(e)) => {
                ui.add(egui::Label::new(RichText::new(format!("{} {e}", icon::WARNING_CIRCLE)).color(pal::BAD)).wrap());
            }
            Some(Ok((list, stats))) => {
                let (name, radius) = st.searched.unwrap_or(("", st.radius));
                ui.add(
                    egui::Label::new(
                        RichText::new(format!(
                            "{} planets with {} conditions and biology within {} ly. Of the {} nearest (out to {:.0} ly): {} already sampled, {} mapped without it, {} in populated systems, {} footfalled.",
                            if stats.matched >= 10_000 { "10,000+".to_string() } else { stats.matched.to_string() },
                            name,
                            radius,
                            stats.examined,
                            stats.reach_ly,
                            stats.sampled,
                            stats.mapped,
                            stats.populated,
                            stats.footfalled
                        ))
                        .size(size::FOOTNOTE)
                        .color(pal::DIM),
                    )
                    .wrap(),
                );
                ui.add_space(6.0);
                if list.is_empty() {
                    empty(ui, icon::PLANET, "Nothing untouched in range", "Every planet with these conditions nearby has been mapped or sampled by someone. Try a larger range: the further from the bubble, the more untouched planets.");
                    return;
                }
                Grid::new("untouched").max_col_width(ui.available_width()).num_columns(7).striped(true).spacing([14.0, 10.0]).show(ui, |ui| {
                    for t in ["Planet", "Distance", "Evidence", "Chance", "If first logged", "Last data", ""] {
                        th(ui, t);
                    }
                    ui.end_row();
                    for c in list {
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 0.0;
                            let short = c.body.strip_prefix(&c.system).map(str::trim).filter(|s| !s.is_empty()).unwrap_or(&c.body);
                            ui.label(RichText::new(short).font(display(size::CALLOUT)).color(pal::TEXT));
                            ui.label(RichText::new(&c.system).size(size::CAPTION).color(pal::DIM));
                        })
                        .response
                        .on_hover_text(format!(
                            "{} · {:.0} K · {:.2} g · {} biological signal{}{}",
                            c.atmosphere,
                            c.temperature_k.unwrap_or(0.0),
                            c.gravity_g.unwrap_or(0.0),
                            c.bio_signals,
                            if c.bio_signals == 1 { "" } else { "s" },
                            c.distance_ls.map(|d| format!(" · {d:.0} ls from arrival")).unwrap_or_default()
                        ));
                        ui.label(RichText::new(format!("{:.0} ly", c.distance_ly)).color(pal::TEXT));
                        let (txt, col, tip) = match c.evidence {
                            Evidence::NoTrace => ("No trace", pal::GOOD, "No commander who shares data has mapped or sampled this planet."),
                            Evidence::GenusConfirmed => ("Mapped, not sampled", pal::CYAN, "Someone DSS-mapped it, so the genus is confirmed, but nobody has logged samples."),
                        };
                        badge(ui, txt, col).on_hover_text(tip);
                        ui.label(RichText::new(format!("{:.0}%", c.chance * 100.0)).color(pal::TEXT));
                        ui.label(RichText::new(short_credits(c.value_first_logged)).font(display(size::CALLOUT)).color(pal::GOLD));
                        ui.label(RichText::new(c.last_data.map(|t| crate::widgets::ago(nexus_journal::Timestamp(t))).unwrap_or_else(|| "?".into())).color(pal::DIM));
                        ui.horizontal(|ui| {
                            if crate::widgets::icon_button(ui, icon::COPY, "Copy the system name").clicked() {
                                ui.ctx().copy_text(c.system.clone());
                            }
                            if crate::widgets::icon_button(ui, icon::PATH, "Plot a route there").clicked() {
                                action = Some(ExoAction::PlotTo(c.system.clone()));
                            }
                        });
                        ui.end_row();
                    }
                });
            }
        }
    });
    action
}

/// Sampling and unsold data.
fn show_page(ui: &mut Ui, gs: &GameState) {
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
