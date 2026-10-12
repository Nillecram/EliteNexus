use crate::theme::{display, pal, size};
use crate::widgets::{self, GAP, badge, bar, card, card_tinted, cards, empty, kv, short_credits, th};
use egui::{Grid, RichText, Sense, Ui};
use egui_extras::{Column, TableBuilder};
use egui_phosphor::regular as icon;
use nexus_model::exploration::{interesting, Body, BodyKind, Interest, SystemScan};
use nexus_model::GameState;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    Current,
    Unsold,
    Skipped,
    Codex,
}

#[derive(Default)]
pub struct ExplorationUi {
    pub tab: Tab,
    pub selected_body: Option<u32>,
    /// Show this system instead of the current one (from the unsold list).
    pub system: Option<u64>,
}

pub fn show(ui: &mut Ui, gs: &GameState, st: &mut ExplorationUi, high_value: i64) {
    ui.horizontal(|ui| {
        if let Some(t) = widgets::tabs(ui, st.tab, &[(Tab::Current, "Current system"), (Tab::Unsold, "Unsold data"), (Tab::Skipped, "Not sellable"), (Tab::Codex, "Codex")]) {
            st.tab = t;
            if t == Tab::Current {
                st.system = None;
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(RichText::new(format!("{} unsold · {}", gs.exploration.unsold().count(), short_credits(gs.exploration.unsold_value()))).font(display(size::CALLOUT)).color(pal::GOLD));
        });
    });
    ui.add_space(GAP);
    match st.tab {
        Tab::Current => {
            let addr = st.system.or(gs.exploration.current);
            match addr.and_then(|a| gs.exploration.systems.get(&a)) {
                Some(sys) => system_view(ui, gs, sys, st, high_value),
                None => empty(ui, icon::PLANET, "No exploration data here yet", "Honk (FSS discovery scan) or scan bodies and they will appear instantly."),
            }
        }
        Tab::Unsold => systems_list(ui, gs.exploration.unsold().collect(), st, "No unsold cartographic data."),
        Tab::Skipped => {
            ui.add(egui::Label::new(RichText::new("Data scanned before a sale that Universal Cartographics did not buy. The game keeps refusing it, so it is excluded from unsold totals.").color(pal::DIM)).wrap());
            ui.add_space(8.0);
            systems_list(ui, gs.exploration.skipped().collect(), st, "Nothing here.");
        }
        Tab::Codex => codex(ui, gs),
    }
}

fn systems_list(ui: &mut Ui, mut list: Vec<&SystemScan>, st: &mut ExplorationUi, empty_text: &str) {
    if list.is_empty() {
        empty(ui, icon::CHECK_CIRCLE, empty_text, "");
        return;
    }
    list.sort_by_key(|s| std::cmp::Reverse(s.value()));
    let total: i64 = list.iter().map(|s| s.value()).sum();
    ui.label(RichText::new(format!("{} systems · {}", list.len(), short_credits(total))).font(display(size::HEADLINE)).color(pal::GOLD));
    ui.add_space(4.0);
    TableBuilder::new(ui)
        .striped(true)
        .resizable(false)
        .sense(Sense::click())
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .column(Column::initial(280.0).at_least(160.0).clip(true))
        .column(Column::initial(80.0))
        .column(Column::initial(110.0))
        .column(Column::initial(110.0))
        .column(Column::remainder())
        .header(30.0, |mut h| {
            for t in ["System", "Bodies", "Value", "If mapped", "Last visit"] {
                h.col(|ui| th(ui, t));
            }
        })
        .body(|body| {
            body.rows(30.0, list.len(), |mut row| {
                let s = list[row.index()];
                row.col(|ui| {
                    ui.label(&s.name);
                });
                row.col(|ui| {
                    ui.label(s.scanned_bodies().to_string());
                });
                row.col(|ui| {
                    ui.label(RichText::new(short_credits(s.value())).color(pal::GOLD));
                });
                row.col(|ui| {
                    ui.label(short_credits(s.potential_value()));
                });
                row.col(|ui| {
                    ui.label(RichText::new(s.last_visit.to_local_string("%d %b %H:%M")).color(pal::DIM));
                });
                if row.response().clicked() {
                    st.system = Some(s.address);
                    st.selected_body = None;
                    st.tab = Tab::Current;
                }
            });
        });
}

fn system_view(ui: &mut Ui, gs: &GameState, sys: &SystemScan, st: &mut ExplorationUi, high_value: i64) {
    let scanned = sys.scanned_bodies();
    let total = sys.fss_body_count.unwrap_or(0) as usize;
    let big = |ui: &mut Ui, text: String, color: egui::Color32| {
        ui.add(egui::Label::new(RichText::new(text).font(display(size::TITLE3)).color(color)).wrap());
    };
    let note = |ui: &mut Ui, text: String| {
        ui.add(egui::Label::new(RichText::new(text).size(size::FOOTNOTE).color(pal::DIM)).wrap());
    };
    let mut back = false;
    cards(ui, 4, 190.0, |c| {
        c.add(|ui| {
            card_tinted(ui, icon::PLANET, "System", pal::TEAL, |ui| {
                big(ui, sys.name.clone(), pal::TEXT);
                if st.system.is_some() && Some(sys.address) != gs.exploration.current && widgets::button(ui, "Back to the current system").clicked() {
                    back = true;
                }
            });
        });
        c.add(|ui| {
            card_tinted(ui, icon::BROADCAST, "FSS progress", pal::BLUE, |ui| {
                big(ui, if total > 0 { format!("{scanned} / {total} bodies") } else { format!("{scanned} bodies") }, pal::TEXT);
                if total > 0 {
                    bar(ui, scanned as f32 / total as f32, if sys.all_bodies_found { pal::GOOD } else { pal::BLUE }, 6.0, None);
                }
                if let Some(n) = sys.fss_non_body_count {
                    note(ui, format!("{n} non-body signals"));
                }
            });
        });
        c.add(|ui| {
            card_tinted(ui, icon::COINS, "Value", pal::GOLD, |ui| {
                big(ui, short_credits(sys.value()), pal::GOLD);
                note(ui, format!("{} if all mapped", short_credits(sys.potential_value())));
            });
        });
        c.add(|ui| {
            card_tinted(ui, icon::PLANT, "Signals", pal::GOOD, |ui| {
                let bio = sys.bio_signal_total();
                let geo: u32 = sys.bodies.values().map(|b| b.geo_signals).sum();
                big(ui, format!("{bio} bio · {geo} geo"), pal::TEXT);
                note(ui, format!("{} FSS signals logged", sys.signals.len()));
            });
        });
    });
    if back {
        st.system = None;
    }
    ui.add_space(GAP);

    let mut bodies: Vec<&Body> = sys.bodies.values().filter(|b| b.kind != BodyKind::BeltCluster).collect();
    // System-map order: walk the parent chain root-first so moons follow their planet.
    bodies.sort_by_cached_key(|b| tree_key(b));

    if bodies.is_empty() {
        let hint = match sys.fss_body_count {
            Some(n) if n > 0 => format!("{n} bodies detected by the discovery scan — open the FSS to resolve them; values, signals and first-discovery flags appear here as you scan."),
            _ => "Honk (discovery scan) and open the FSS: every body appears here as you resolve it.".to_string(),
        };
        empty(ui, icon::PLANET, "Nothing scanned in this system yet", &hint);
    } else {
        let selected = st.selected_body.and_then(|id| sys.bodies.get(&id));
        match selected {
            // Wide windows: the table with the body's details beside it; narrow: details first.
            Some(b) if ui.available_width() >= 1000.0 => {
                let dw = 360.0;
                ui.horizontal_top(|ui| {
                    let tw = ui.available_width() - dw - GAP;
                    ui.vertical(|ui| {
                        ui.set_width(tw);
                        body_table(ui, sys, &bodies, st, high_value);
                    });
                    ui.add_space(GAP - ui.spacing().item_spacing.x);
                    ui.vertical(|ui| {
                        ui.set_width(dw);
                        body_detail(ui, sys, b, st);
                    });
                });
            }
            Some(b) => {
                body_detail(ui, sys, b, st);
                ui.add_space(GAP);
                body_table(ui, sys, &bodies, st, high_value);
            }
            None => body_table(ui, sys, &bodies, st, high_value),
        }
    }

    if !sys.signals.is_empty() {
        ui.add_space(8.0);
        egui::CollapsingHeader::new(RichText::new(format!("{} FSS signals ({})", icon::BROADCAST, sys.signals.len())).font(display(size::CALLOUT)).color(pal::TEXT)).show(ui, |ui| {
            Grid::new("sigs").max_col_width(ui.available_width()).num_columns(3).striped(true).show(ui, |ui| {
                for s in &sys.signals {
                    ui.label(&s.name);
                    ui.label(RichText::new(s.uss_type.clone().or_else(|| s.kind.clone()).unwrap_or_default()).color(pal::DIM));
                    ui.label(RichText::new(if s.is_station { "station/carrier".to_string() } else { s.threat.map(|t| format!("threat {t}")).unwrap_or_default() }).color(pal::DIM));
                    ui.end_row();
                }
            });
        });
    }
}

/// Root-first chain of body IDs (parents reversed, then self) — sorts bodies into tree order.
fn tree_key(b: &Body) -> Vec<u32> {
    let mut key: Vec<u32> = b.parents.iter().rev().map(|(_, id)| *id).collect();
    key.push(b.id);
    key
}

/// Nesting depth for indentation: planets/stars in the parent chain (barycentres don't indent).
fn depth(b: &Body) -> usize {
    b.parents.iter().filter(|(k, _)| k != "Null").count().saturating_sub(1)
}

fn type_label(b: &Body) -> String {
    match b.kind {
        BodyKind::Star => nexus_data::exploration::star_description(b.star_type.as_deref().unwrap_or("?")),
        BodyKind::Planet => nexus_data::exploration::short_planet_class(b.planet_class.as_deref().unwrap_or("?")).to_string(),
        BodyKind::Ring => "Ring".into(),
        BodyKind::BeltCluster => "Belt cluster".into(),
        BodyKind::Barycentre => "Barycentre".into(),
        BodyKind::Unknown => "Unscanned".into(),
    }
}

fn interest_color(i: Interest) -> egui::Color32 {
    match i {
        Interest::EarthLike | Interest::WaterWorld | Interest::AmmoniaWorld | Interest::HighValue => pal::GOLD,
        Interest::Terraformable | Interest::GasGiantLife => pal::ACCENT,
        Interest::FirstDiscovery | Interest::FirstFootfallOpportunity => pal::PURPLE,
        Interest::BioSignals | Interest::GeoSignals => pal::GOOD,
        Interest::GuardianOrThargoid => pal::BAD,
        _ => pal::CYAN,
    }
}

fn body_table(ui: &mut Ui, sys: &SystemScan, bodies: &[&Body], st: &mut ExplorationUi, high_value: i64) {
    TableBuilder::new(ui)
        .id_salt(("bodies", sys.address))
        .striped(true)
        .resizable(false)
        .sense(Sense::click())
        .min_scrolled_height(300.0)
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .column(Column::initial(130.0).at_least(90.0).clip(true))
        .column(Column::initial(150.0).at_least(90.0).clip(true))
        .column(Column::initial(84.0))
        .column(Column::initial(92.0))
        .column(Column::initial(92.0))
        .column(Column::initial(70.0))
        .column(Column::initial(70.0))
        .column(Column::remainder().at_least(160.0).clip(true))
        .header(30.0, |mut h| {
            for t in ["Body", "Type", "Distance", "Value", "Mapped", "Gravity", "Temp", "Notes"] {
                h.col(|ui| th(ui, t));
            }
        })
        .body(|body| {
            body.rows(32.0, bodies.len(), |mut row| {
                let b = bodies[row.index()];
                row.set_selected(st.selected_body == Some(b.id));
                let tags = interesting(b, sys);
                row.col(|ui| {
                    let indent = "    ".repeat(depth(b).min(4));
                    let prefix = if depth(b) > 0 { format!("{} ", icon::ARROW_ELBOW_DOWN_RIGHT) } else { String::new() };
                    let mut t = RichText::new(format!("{indent}{prefix}{}", b.short_name(&sys.name)));
                    if b.mapped {
                        t = t.color(pal::GOOD);
                    }
                    ui.label(t);
                });
                row.col(|ui| {
                    let mut t = RichText::new(type_label(b));
                    if b.is_terraformable() {
                        t = t.color(pal::ACCENT);
                    }
                    ui.label(t);
                });
                row.col(|ui| {
                    ui.label(RichText::new(format!("{:.0} ls", b.distance_ls)).color(pal::DIM));
                });
                row.col(|ui| {
                    ui.label(RichText::new(short_credits(b.value.current)).color(if b.value.current >= high_value { pal::GOLD } else { pal::TEXT }));
                });
                row.col(|ui| {
                    if b.kind == BodyKind::Planet {
                        let col = if b.value.mapped >= high_value { pal::GOLD } else { pal::DIM };
                        ui.label(RichText::new(short_credits(b.value.mapped)).color(col));
                    }
                });
                row.col(|ui| {
                    if let Some(g) = b.gravity_g() {
                        let col = if g > 2.5 { pal::BAD } else if g > 1.5 { pal::WARN } else { pal::DIM };
                        ui.label(RichText::new(format!("{g:.2} g")).color(col));
                    }
                });
                row.col(|ui| {
                    if let Some(t) = b.temperature_k {
                        ui.label(RichText::new(format!("{t:.0} K")).color(pal::DIM));
                    }
                });
                row.col(|ui| {
                    ui.horizontal(|ui| {
                        if b.landable {
                            ui.label(RichText::new(icon::FOOTPRINTS).color(if b.footfall { pal::GOOD } else { pal::DIM })).on_hover_text("Landable");
                        }
                        if b.bio_signals > 0 {
                            let v = b.predicted_value(nexus_data::bio_predict::first_logged_bonus_likely(b.was_footfalled, false, None));
                            let label = if v > 0 { format!("{} {} ≈{}", icon::PLANT, b.bio_signals, nexus_data::names::compact_credits(v)) } else { format!("{} {}", icon::PLANT, b.bio_signals) };
                            badge(ui, &label, pal::GOOD).on_hover_text(b.likely_species().iter().map(|p| format!("{} ({:.0}%)", p.species, p.likelihood * 100.0)).collect::<Vec<_>>().join("\n"));
                        }
                        if b.geo_signals > 0 {
                            badge(ui, &format!("{} {}", icon::MOUNTAINS, b.geo_signals), pal::WARN);
                        }
                        for t in tags.iter().filter(|t| !matches!(t, Interest::BioSignals | Interest::GeoSignals)).take(3) {
                            badge(ui, t.label(), interest_color(*t));
                        }
                    });
                });
                if row.response().clicked() {
                    st.selected_body = if st.selected_body == Some(b.id) { None } else { Some(b.id) };
                }
            });
        });
}

fn body_detail(ui: &mut Ui, sys: &SystemScan, b: &Body, st: &mut ExplorationUi) {
    card(ui, "", |ui| {
        ui.horizontal(|ui| {
            ui.add(egui::Label::new(RichText::new(b.short_name(&sys.name)).font(display(size::TITLE3))).truncate());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if widgets::icon_button(ui, icon::X, "Close").clicked() {
                    st.selected_body = None;
                }
            });
        });
        ui.label(RichText::new(type_label(b)).color(pal::ACCENT_SOFT));
        ui.horizontal_wrapped(|ui| {
            for t in interesting(b, sys) {
                badge(ui, t.label(), interest_color(t));
            }
        });
        ui.add_space(4.0);
        Grid::new("bodydetail").max_col_width(ui.available_width()).num_columns(2).spacing([14.0, 6.0]).show(ui, |ui| {
            kv(ui, "Value now", RichText::new(short_credits(b.value.current)).color(pal::GOLD));
            if b.kind == BodyKind::Planet {
                kv(ui, "If mapped", short_credits(b.value.mapped));
            }
            kv(ui, "Distance", format!("{:.1} ls", b.distance_ls));
            if let Some(m) = b.mass_em {
                kv(ui, "Mass", format!("{m:.4} EM"));
            }
            if let Some(m) = b.stellar_mass {
                kv(ui, "Mass", format!("{m:.3} solar"));
            }
            if let Some(r) = b.radius_m {
                kv(ui, "Radius", format!("{:.0} km", r / 1000.0));
            }
            if let Some(g) = b.gravity_g() {
                kv(ui, "Gravity", format!("{g:.2} g"));
            }
            if let Some(t) = b.temperature_k {
                kv(ui, "Temperature", format!("{t:.0} K"));
            }
            if let Some(p) = b.pressure_atm().filter(|p| *p > 0.0) {
                kv(ui, "Pressure", format!("{p:.3} atm"));
            }
            if let Some(a) = &b.atmosphere {
                kv(ui, "Atmosphere", a.clone());
            }
            if let Some(v) = &b.volcanism {
                kv(ui, "Volcanism", v.clone());
            }
            if let Some(t) = &b.terraform_state {
                kv(ui, "Terraform", t.clone());
            }
            if let Some(p) = b.rotation_period_s {
                kv(ui, "Day length", format!("{:.1} h{}", p.abs() / 3600.0, if b.tidal_lock { " (locked)" } else { "" }));
            }
            if let Some(p) = b.orbital_period_s {
                kv(ui, "Orbit", format!("{:.1} days", p / 86_400.0));
            }
            if let Some(e) = b.eccentricity {
                kv(ui, "Eccentricity", format!("{e:.3}"));
            }
            let disc = match b.was_discovered {
                Some(false) => RichText::new("First discovery!").color(pal::PURPLE),
                Some(true) => RichText::new("Already discovered").color(pal::DIM),
                None => RichText::new("?").color(pal::DIM),
            };
            kv(ui, "Discovery", disc);
            if b.landable {
                let ff = match (b.was_footfalled, b.footfall) {
                    (_, true) => RichText::new("You have walked here").color(pal::GOOD),
                    (Some(false), _) => RichText::new("Nobody has set foot here").color(pal::PURPLE),
                    _ => RichText::new("Visited before").color(pal::DIM),
                };
                kv(ui, "Footfall", ff);
            }
            if b.mapped {
                let eff = match (b.probes_used, b.efficiency_target) {
                    (Some(p), Some(t)) => format!("{p} probes (target {t})"),
                    _ => "yes".into(),
                };
                kv(ui, "Mapped", eff);
            }
        });
        if !b.signals.is_empty() {
            ui.add_space(4.0);
            ui.label(RichText::new("Signals").font(display(size::FOOTNOTE)).color(pal::DIM));
            for (k, n) in &b.signals {
                ui.label(format!("{n} × {k}"));
            }
        }
        if !b.genuses.is_empty() || !b.organics.is_empty() {
            ui.add_space(4.0);
            ui.label(RichText::new("Biology").font(display(size::FOOTNOTE)).color(pal::DIM));
            for g in &b.genuses {
                let found = b.organics.iter().find(|o| o.genus.eq_ignore_ascii_case(g));
                match found {
                    Some(o) => ui.label(RichText::new(format!("{} — {} ({}/3)", g, o.species, o.samples)).color(if o.analysed { pal::GOOD } else { pal::TEXT })),
                    None => ui.label(RichText::new(g).color(pal::DIM)),
                };
            }
        }
        if !b.predictions.is_empty() {
            ui.add_space(4.0);
            ui.label(RichText::new("Predicted species").font(display(size::FOOTNOTE)).color(pal::DIM));
            let bonus = nexus_data::bio_predict::first_logged_bonus_likely(b.was_footfalled, false, None);
            for p in b.predictions.iter().filter(|p| p.likelihood >= 0.05).take(10) {
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new(format!("{:>3.0}%", p.likelihood * 100.0)).monospace().color(pal::DIM));
                    ui.label(RichText::new(&p.species).color(pal::TEXT));
                    ui.label(RichText::new(short_credits(nexus_data::bio_predict::payout(p.value, bonus))).color(pal::GOLD));
                    if let Some(v) = &p.variant {
                        ui.label(RichText::new(v).small().color(pal::DIM));
                    }
                    ui.label(RichText::new(format!("{} m", p.colony_range_m)).small().color(pal::DIM));
                });
            }
            if bonus {
                ui.label(RichText::new("Values include the likely first-logged bonus (5×).").small().color(pal::DIM));
            }
        }
        if !b.materials.is_empty() {
            ui.add_space(4.0);
            ui.label(RichText::new("Materials").font(display(size::FOOTNOTE)).color(pal::DIM));
            ui.horizontal_wrapped(|ui| {
                for (m, p) in &b.materials {
                    let jump = matches!(m.as_str(), "arsenic" | "germanium" | "niobium" | "polonium" | "vanadium" | "yttrium" | "cadmium");
                    let txt = RichText::new(format!("{} {:.1}%", nexus_data::names::split_camel(m), p));
                    ui.label(if jump { txt.color(pal::CYAN) } else { txt.color(pal::DIM) });
                }
            });
        }
        if !b.rings.is_empty() {
            ui.add_space(4.0);
            ui.label(RichText::new("Rings").font(display(size::FOOTNOTE)).color(pal::DIM));
            for r in &b.rings {
                let class = r.class.trim_start_matches("eRingClass_");
                ui.label(format!("{} — {}", r.name.strip_prefix(&sys.name).unwrap_or(&r.name).trim(), class));
                for (k, n) in &r.signals {
                    ui.label(RichText::new(format!("   {n} × {k} hotspot")).color(pal::GOLD));
                }
            }
        }
    });
}

fn codex(ui: &mut Ui, gs: &GameState) {
    let list = &gs.exploration.codex;
    if list.is_empty() {
        empty(ui, icon::BOOK_BOOKMARK, "No codex entries yet", "Scan anything new and it shows up here.");
        return;
    }
    TableBuilder::new(ui)
        .striped(true)
        .resizable(false)
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .column(Column::initial(124.0))
        .column(Column::initial(260.0).clip(true))
        .column(Column::initial(160.0).clip(true))
        .column(Column::initial(200.0).clip(true))
        .column(Column::remainder().clip(true))
        .header(30.0, |mut h| {
            for t in ["When", "Entry", "Category", "System", "Region"] {
                h.col(|ui| th(ui, t));
            }
        })
        .body(|body| {
            body.rows(30.0, list.len(), |mut row| {
                let c = &list[list.len() - 1 - row.index()];
                row.col(|ui| {
                    ui.label(RichText::new(c.ts.to_local_string("%d %b %H:%M")).color(pal::DIM));
                });
                row.col(|ui| {
                    ui.label(RichText::new(&c.name).color(if c.new_entry { pal::PURPLE } else { pal::TEXT }));
                });
                row.col(|ui| {
                    ui.label(&c.sub_category);
                });
                row.col(|ui| {
                    ui.label(&c.system);
                });
                row.col(|ui| {
                    ui.label(RichText::new(&c.region).color(pal::DIM));
                });
            });
        });
}
