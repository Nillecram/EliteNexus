//! Navigation: the in-game route plus Spansh route planners (neutron, exact, carrier, riches,
//! exobiology). Whatever route is active drives the "next system → clipboard" automation.

use crate::jobs::Slot;
use crate::theme::{display, pal};
use crate::widgets::{badge, card, empty, short_credits};
use egui::{RichText, Ui};
use egui_extras::{Column, TableBuilder};
use egui_phosphor::regular as icon;
use nexus_data::exploration::{is_scoopable, star_description};
use nexus_model::location::distance;
use nexus_model::GameState;
use nexus_spansh::{
    CarrierRouteRequest, ExobiologyRouteRequest, GalaxyAlgorithm, GalaxyRouteRequest, NeutronRouteRequest, RichesRouteRequest, RouteWaypoints, ShipProfile, Spansh,
};

/// A route waypoint, whichever planner produced it.
#[derive(Debug, Clone)]
pub struct Waypoint {
    pub system: String,
    pub pos: Option<[f64; 3]>,
    pub star_class: Option<String>,
    pub jumps: u32,
    pub neutron: bool,
    pub refuel: bool,
    pub scoopable: Option<bool>,
    pub value: Option<u64>,
    pub note: String,
}

#[derive(Debug, Clone)]
pub struct PlannedRoute {
    pub kind: &'static str,
    pub summary: String,
    pub web_url: String,
    pub waypoints: Vec<Waypoint>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Planner {
    #[default]
    InGame,
    Neutron,
    Exact,
    Carrier,
    Riches,
    Exobiology,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RichesKind {
    #[default]
    All,
    EarthLike,
    Ammonia,
    RockyHmc,
}

pub struct NavUi {
    pub planner: Planner,
    pub from: String,
    pub to: String,
    pub range: f64,
    pub efficiency: u8,
    pub riches_kind: RichesKind,
    pub radius: u32,
    pub max_results: u32,
    pub min_value: u64,
    pub use_mapping: bool,
    /// Exact plotter options (as on spansh.co.uk/exact-plotter).
    pub exact: ExactOptions,
    /// The planner's advanced options are shown (hidden by default).
    pub show_options: bool,
    pub job: Slot<Result<PlannedRoute, String>>,
    pub planned: Option<PlannedRoute>,
}

#[derive(Debug, Clone, Copy)]
pub struct ExactOptions {
    pub algorithm: GalaxyAlgorithm,
    pub use_supercharge: bool,
    pub use_injections: bool,
    pub exclude_secondary: bool,
    pub refuel_every_scoopable: bool,
    pub already_supercharged: bool,
    /// Fuel the route must leave in the tank, t.
    pub reserve: f64,
}

impl Default for ExactOptions {
    fn default() -> Self {
        ExactOptions {
            algorithm: GalaxyAlgorithm::Optimistic,
            use_supercharge: true,
            use_injections: false,
            exclude_secondary: false,
            refuel_every_scoopable: true,
            already_supercharged: false,
            reserve: 0.0,
        }
    }
}

impl Default for NavUi {
    fn default() -> Self {
        NavUi {
            planner: Planner::InGame,
            from: String::new(),
            to: String::new(),
            range: 0.0,
            efficiency: 60,
            riches_kind: RichesKind::All,
            radius: 25,
            max_results: 25,
            min_value: 1_000_000,
            use_mapping: true,
            exact: ExactOptions::default(),
            show_options: false,
            job: Slot::default(),
            planned: None,
        }
    }
}

fn algorithm_label(a: GalaxyAlgorithm) -> &'static str {
    match a {
        GalaxyAlgorithm::Optimistic => "Optimistic (fastest)",
        GalaxyAlgorithm::Pessimistic => "Pessimistic",
        GalaxyAlgorithm::Guided => "Neutron-guided",
        GalaxyAlgorithm::Fuel => "Least fuel",
        GalaxyAlgorithm::FuelJumps => "Least fuel, fewer jumps",
    }
}

pub(crate) fn from_spansh(w: &nexus_spansh::Waypoint) -> Waypoint {
    let mut note = w.note.clone().unwrap_or_default();
    if !w.bodies.is_empty() {
        if !note.is_empty() {
            note.push_str(" · ");
        }
        note.push_str(&w.bodies.join(", "));
    }
    Waypoint {
        system: w.system.clone(),
        pos: w.coords.map(|c| [c.x, c.y, c.z]),
        star_class: None,
        jumps: w.jumps,
        neutron: w.neutron,
        refuel: w.refuel,
        scoopable: w.scoopable,
        value: w.value,
        note,
    }
}

/// Remaining waypoints of the active route (planner route if set, else the in-game route).
pub fn active_route(gs: &GameState, planned: Option<&PlannedRoute>) -> Vec<Waypoint> {
    if let Some(p) = planned {
        let here = gs.location.system.to_ascii_lowercase();
        let start = p.waypoints.iter().rposition(|w| w.system.to_ascii_lowercase() == here).map(|i| i + 1).unwrap_or(0);
        return p.waypoints[start.min(p.waypoints.len())..].to_vec();
    }
    let start = gs.nav_route.iter().position(|h| h.system_address == gs.location.system_address).map(|i| i + 1).unwrap_or(0);
    gs.nav_route[start.min(gs.nav_route.len())..]
        .iter()
        .map(|h| Waypoint {
            system: h.star_system.clone(),
            pos: Some(h.star_pos),
            star_class: Some(h.star_class.clone()),
            jumps: 1,
            neutron: h.star_class == "N",
            refuel: false,
            scoopable: Some(is_scoopable(&h.star_class)),
            value: None,
            note: String::new(),
        })
        .collect()
}

/// Returns true if a setting changed.
pub fn show(ui: &mut Ui, gs: &GameState, st: &mut NavUi, auto_copy: &mut bool, spansh: &Spansh, rt: &tokio::runtime::Handle) -> bool {
    let mut changed = false;
    if st.job.tick()
        && let Some(Ok(route)) = &st.job.last {
            st.planned = Some(route.clone());
        }
    if st.from.is_empty() {
        st.from = gs.location.system.clone();
    }
    if st.range <= 0.0 {
        st.range = gs.fleet.current().map(|s| s.max_jump_range).filter(|r| *r > 0.0).unwrap_or(30.0);
    }
    ui.horizontal(|ui| {
        changed |= ui.checkbox(auto_copy, "Copy the next system to the clipboard after every jump").changed();
        let route = active_route(gs, st.planned.as_ref());
        if let Some(next) = route.first()
            && ui.button(format!("{} Copy next: {}", icon::COPY, next.system)).clicked() {
                ui.ctx().copy_text(next.system.clone());
            }
        if st.planned.is_some() && ui.button(format!("{} Back to in-game route", icon::X)).clicked() {
            st.planned = None;
        }
    });
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        for (p, label) in [
            (Planner::InGame, "In-game route"),
            (Planner::Neutron, "Neutron plotter"),
            (Planner::Exact, "Exact plotter (your ship)"),
            (Planner::Carrier, "Fleet carrier"),
            (Planner::Riches, "Road to riches"),
            (Planner::Exobiology, "Exobiology"),
        ] {
            if ui.selectable_label(st.planner == p, label).clicked() {
                st.planner = p;
            }
        }
    });
    ui.add_space(6.0);
    if st.planner != Planner::InGame {
        planner_form(ui, gs, st, spansh, rt);
        ui.add_space(6.0);
    }
    route_view(ui, gs, st);
    changed
}

fn planner_form(ui: &mut Ui, gs: &GameState, st: &mut NavUi, spansh: &Spansh, rt: &tokio::runtime::Handle) {
    let ship_name = gs.fleet.current().map(|s| s.display_name());
    // Smart defaults: start here, with the current ship's jump range.
    if st.range <= 0.0 {
        st.range = gs.fleet.current().map(|s| (s.max_jump_range * 10.0).round() / 10.0).filter(|r| *r > 0.0).unwrap_or(20.0);
    }
    card(ui, "Spansh planner", |ui| {
        // The simple surface: where from, where to, go.
        ui.horizontal_wrapped(|ui| {
            ui.label("From");
            ui.add(egui::TextEdit::singleline(&mut st.from).hint_text(gs.location.system.as_str()).desired_width(220.0));
            if matches!(st.planner, Planner::Neutron | Planner::Exact | Planner::Carrier) {
                ui.label("To");
                ui.add(egui::TextEdit::singleline(&mut st.to).hint_text("destination system").desired_width(220.0));
            }
            let busy = st.job.busy();
            let label = if busy { format!("{} Plotting…", icon::HOURGLASS) } else { format!("{} Plot route", icon::PATH) };
            if ui.add_enabled(!busy, egui::Button::new(RichText::new(label).color(pal::ACCENT))).clicked() {
                match make_request(gs, st, spansh) {
                    Some(fut) => st.job.run_async(ui.ctx(), rt, fut),
                    None => st.job.last = Some(Err("This planner needs your ship's loadout (fly it once) and a destination.".into())),
                }
            }
            if busy {
                ui.spinner();
            }
            let options = format!("{} Options", if st.show_options { icon::CARET_DOWN } else { icon::CARET_RIGHT });
            if ui.add(egui::Button::new(RichText::new(options).small().color(pal::DIM)).frame(false)).clicked() {
                st.show_options = !st.show_options;
            }
        });
        let hint = match st.planner {
            Planner::Exact => ship_name.clone().map(|n| format!("Plans every jump for {n}: its real FSD, mass, fuel and cargo, scooping stops and neutron boosts.")),
            Planner::Neutron => Some(format!("Follows neutron stars with a {:.1} ly jump range.", st.range)),
            Planner::Carrier => Some(format!("Tritium in the tank {} t · cargo {} t.", gs.carrier.fuel, gs.carrier.space.cargo)),
            Planner::Riches => Some(format!("Valuable bodies to scan within {} ly.", st.radius)),
            Planner::Exobiology => Some(format!("Planets with valuable species within {} ly.", st.radius)),
            Planner::InGame => None,
        };
        if let Some(h) = hint {
            ui.label(RichText::new(h).small().color(pal::DIM));
        }
        // The hidden depth: everything the Spansh website offers, collapsed by default.
        if st.show_options {
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| match st.planner {
                Planner::Neutron => {
                    ui.label("Range");
                    ui.add(egui::DragValue::new(&mut st.range).range(5.0..=120.0).suffix(" ly"));
                    ui.label("Efficiency");
                    ui.add(egui::DragValue::new(&mut st.efficiency).range(1..=100).suffix(" %"))
                        .on_hover_text("Lower = more neutron detours for fewer jumps; higher = straighter route.");
                }
                Planner::Exact => {
                    let o = &mut st.exact;
                    egui::ComboBox::from_id_salt("exact-algo")
                        .selected_text(algorithm_label(o.algorithm))
                        .show_ui(ui, |ui| {
                            for a in [GalaxyAlgorithm::Optimistic, GalaxyAlgorithm::Pessimistic, GalaxyAlgorithm::Guided, GalaxyAlgorithm::Fuel, GalaxyAlgorithm::FuelJumps] {
                                ui.selectable_value(&mut o.algorithm, a, algorithm_label(a));
                            }
                        })
                        .response
                        .on_hover_text("Spansh routing strategy: Optimistic is usually fastest; Least fuel avoids scooping; Neutron-guided follows a neutron route.");
                    ui.checkbox(&mut o.use_supercharge, "Neutron boosts");
                    ui.checkbox(&mut o.use_injections, "FSD injections");
                    ui.checkbox(&mut o.exclude_secondary, "Skip secondary stars");
                    ui.checkbox(&mut o.refuel_every_scoopable, "Refuel at every scoopable");
                    ui.checkbox(&mut o.already_supercharged, "Already supercharged");
                    ui.label("Reserve");
                    ui.add(egui::DragValue::new(&mut o.reserve).range(0.0..=64.0).speed(0.1).suffix(" t"));
                }
                Planner::Riches | Planner::Exobiology => {
                    if st.planner == Planner::Riches {
                        egui::ComboBox::from_id_salt("rk")
                            .selected_text(match st.riches_kind {
                                RichesKind::All => "All valuable bodies",
                                RichesKind::EarthLike => "Earth-likes",
                                RichesKind::Ammonia => "Ammonia worlds",
                                RichesKind::RockyHmc => "Terraformable rocky/HMC",
                            })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut st.riches_kind, RichesKind::All, "All valuable bodies");
                                ui.selectable_value(&mut st.riches_kind, RichesKind::EarthLike, "Earth-likes");
                                ui.selectable_value(&mut st.riches_kind, RichesKind::Ammonia, "Ammonia worlds");
                                ui.selectable_value(&mut st.riches_kind, RichesKind::RockyHmc, "Terraformable rocky/HMC");
                            });
                        ui.checkbox(&mut st.use_mapping, "Count mapping value");
                    }
                    ui.label("Range");
                    ui.add(egui::DragValue::new(&mut st.range).range(5.0..=120.0).suffix(" ly"));
                    ui.label("Radius");
                    ui.add(egui::DragValue::new(&mut st.radius).range(5..=500).suffix(" ly"));
                    ui.label("Systems");
                    ui.add(egui::DragValue::new(&mut st.max_results).range(1..=100));
                    ui.label("Min value");
                    let mut mv = st.min_value as f64 / 1e6;
                    if ui.add(egui::DragValue::new(&mut mv).range(0.0..=100.0).speed(0.1).suffix(" M")).changed() {
                        st.min_value = (mv * 1e6) as u64;
                    }
                }
                Planner::Carrier | Planner::InGame => {
                    ui.label(RichText::new("No extra options for this planner.").small().color(pal::FAINT));
                }
            });
        }
        if let Some(Err(e)) = &st.job.last {
            ui.label(RichText::new(e).color(pal::BAD));
        }
        ui.label(RichText::new("Route data © Spansh (spansh.co.uk)").small().color(pal::FAINT));
    });
}

type PlanFuture = std::pin::Pin<Box<dyn std::future::Future<Output = Result<PlannedRoute, String>> + Send>>;

fn make_request(gs: &GameState, st: &NavUi, spansh: &Spansh) -> Option<PlanFuture> {
    let sp = spansh.clone();
    let from = match st.from.trim() {
        "" => gs.location.system.clone(),
        f => f.to_string(),
    };
    let to = st.to.trim().to_string();
    if from.is_empty() {
        return None;
    }
    match st.planner {
        Planner::InGame => None,
        Planner::Neutron => {
            if to.is_empty() {
                return None;
            }
            let req = NeutronRouteRequest::new(from, to, st.range).efficiency(st.efficiency);
            Some(Box::pin(async move {
                let p = sp.neutron_route(&req).await.map_err(|e| e.to_string())?;
                let wps: Vec<Waypoint> = p.route.waypoints().iter().map(from_spansh).collect();
                Ok(PlannedRoute {
                    kind: "Neutron",
                    summary: format!("{} waypoints · {} jumps · {} neutron boosts", wps.len(), p.route.total_jumps(), p.route.neutron_count()),
                    web_url: p.web_url,
                    waypoints: wps,
                })
            }))
        }
        Planner::Exact => {
            if to.is_empty() {
                return None;
            }
            let json = gs.fleet.current().and_then(|s| s.loadout_json.clone())?;
            let cargo = gs.inventory.cargo.total().max(0) as u32;
            let o = st.exact;
            Some(Box::pin(async move {
                let profile = ShipProfile::from_loadout_json(&json).map_err(|e| format!("Ship profile: {e:?}"))?;
                let req = GalaxyRouteRequest::new(from.as_str(), to.as_str(), &profile)
                    .cargo(cargo)
                    .algorithm(o.algorithm)
                    .use_supercharge(o.use_supercharge)
                    .use_injections(o.use_injections, false)
                    .exclude_secondary(o.exclude_secondary)
                    .refuel_every_scoopable(o.refuel_every_scoopable)
                    .already_supercharged(o.already_supercharged)
                    .reserve(o.reserve);
                let p = sp.galaxy_route(&req).await.map_err(|e| e.to_string())?;
                let wps: Vec<Waypoint> = p.route.waypoints().iter().map(from_spansh).collect();
                Ok(PlannedRoute {
                    kind: "Exact",
                    summary: format!(
                        "{} jumps · {:.0} ly · {} refuels · {} neutron boosts · {:.1} t fuel",
                        p.route.jump_count(),
                        p.route.distance(),
                        p.route.refuel_stops(),
                        p.route.neutron_boosts(),
                        p.route.fuel_used()
                    ),
                    web_url: p.web_url,
                    waypoints: wps,
                })
            }))
        }
        Planner::Carrier => {
            if to.is_empty() {
                return None;
            }
            let c = &gs.carrier;
            let req = CarrierRouteRequest::new(from.as_str(), [to.as_str()]).capacity_used(c.space.cargo.max(0) as u32).fuel_loaded(c.fuel.max(0) as u32, 0);
            Some(Box::pin(async move {
                let p = sp.carrier_route(&req).await.map_err(|e| e.to_string())?;
                let wps: Vec<Waypoint> = p.route.waypoints().iter().map(from_spansh).collect();
                Ok(PlannedRoute {
                    kind: "Carrier",
                    summary: format!("{} carrier jumps · {:.0} ly · {} t tritium", p.route.jump_count(), p.route.distance(), p.route.tritium_used()),
                    web_url: p.web_url,
                    waypoints: wps,
                })
            }))
        }
        Planner::Riches => {
            let base = match st.riches_kind {
                RichesKind::All => RichesRouteRequest::road_to_riches(from, st.range),
                RichesKind::EarthLike => RichesRouteRequest::earth_likes(from, st.range),
                RichesKind::Ammonia => RichesRouteRequest::ammonia_worlds(from, st.range),
                RichesKind::RockyHmc => RichesRouteRequest::rocky_hmc(from, st.range),
            };
            let req = base.radius(st.radius).max_results(st.max_results).min_value(st.min_value).use_mapping_value(st.use_mapping);
            Some(Box::pin(async move {
                let p = sp.riches_route(&req).await.map_err(|e| e.to_string())?;
                let wps: Vec<Waypoint> = p.route.waypoints().iter().map(from_spansh).collect();
                let value: u64 = wps.iter().filter_map(|w| w.value).sum();
                Ok(PlannedRoute {
                    kind: "Road to riches",
                    summary: format!("{} systems · ~{} Cr", wps.len(), nexus_data::names::compact_credits(value as i64)),
                    web_url: p.web_url,
                    waypoints: wps,
                })
            }))
        }
        Planner::Exobiology => {
            let req = ExobiologyRouteRequest::new(from, st.range).radius(st.radius).max_results(st.max_results).min_value(st.min_value);
            Some(Box::pin(async move {
                let p = sp.exobiology_route(&req).await.map_err(|e| e.to_string())?;
                let wps: Vec<Waypoint> = p.route.waypoints().iter().map(from_spansh).collect();
                let value: u64 = wps.iter().filter_map(|w| w.value).sum();
                Ok(PlannedRoute {
                    kind: "Exobiology",
                    summary: format!("{} systems · ~{} Cr in species", wps.len(), nexus_data::names::compact_credits(value as i64)),
                    web_url: p.web_url,
                    waypoints: wps,
                })
            }))
        }
    }
}

fn route_view(ui: &mut Ui, gs: &GameState, st: &NavUi) {
    let route = active_route(gs, st.planned.as_ref());
    if let Some(p) = &st.planned {
        ui.horizontal(|ui| {
            badge(ui, p.kind, pal::CYAN);
            ui.label(RichText::new(&p.summary).font(display(15.0)));
            ui.hyperlink_to(format!("{} open on Spansh", icon::ARROW_SQUARE_OUT), &p.web_url);
        });
    }
    if route.is_empty() {
        empty(ui, icon::NAVIGATION_ARROW, "No route", "Plot a route in the galaxy map, or use a Spansh planner above.");
        return;
    }
    let mut prev = gs.location.star_pos;
    let mut total = 0.0;
    let mut legs = Vec::with_capacity(route.len());
    for w in &route {
        let d = match (prev, w.pos) {
            (Some(a), Some(b)) => distance(a, b),
            _ => 0.0,
        };
        total += d;
        legs.push((d, total));
        if w.pos.is_some() {
            prev = w.pos;
        }
    }
    let scoopable = route.iter().filter(|w| w.scoopable == Some(true)).count();
    let neutrons = route.iter().filter(|w| w.neutron).count();
    let mut run = 0;
    let mut worst = 0;
    for w in &route {
        if w.scoopable == Some(false) {
            run += 1;
            worst = worst.max(run);
        } else {
            run = 0;
        }
    }
    ui.columns(4, |c| {
        card(&mut c[0], "Destination", |ui| {
            ui.label(RichText::new(route.last().map(|w| w.system.as_str()).unwrap_or("?")).font(display(18.0)));
        });
        card(&mut c[1], "Remaining", |ui| {
            let jumps: u32 = route.iter().map(|w| w.jumps.max(1)).sum();
            ui.label(RichText::new(format!("{} waypoints · {} jumps · {:.0} ly", route.len(), jumps, total)).font(display(16.0)));
        });
        card(&mut c[2], "Fuel stars", |ui| {
            ui.label(RichText::new(format!("{scoopable} scoopable")).font(display(16.0)).color(pal::GOOD));
            if worst >= 3 {
                ui.label(RichText::new(format!("{} {worst} non-scoopable in a row", icon::WARNING)).color(pal::WARN));
            }
        });
        card(&mut c[3], "Neutron stars", |ui| {
            ui.label(RichText::new(format!("{neutrons} on route")).font(display(16.0)).color(if neutrons > 0 { pal::CYAN } else { pal::DIM }));
        });
    });
    ui.add_space(8.0);
    TableBuilder::new(ui)
        .striped(true)
        .resizable(true)
        .column(Column::initial(40.0))
        .column(Column::initial(270.0))
        .column(Column::initial(190.0))
        .column(Column::initial(80.0))
        .column(Column::initial(90.0))
        .column(Column::remainder())
        .header(22.0, |mut h| {
            for t in ["#", "System", "Star / flags", "Jumps", "Distance", "Notes"] {
                h.col(|ui| {
                    ui.label(RichText::new(t).color(pal::DIM).strong());
                });
            }
        })
        .body(|body| {
            body.rows(24.0, route.len(), |mut row| {
                let i = row.index();
                let w = &route[i];
                row.set_selected(i == 0);
                row.col(|ui| {
                    ui.label(RichText::new((i + 1).to_string()).color(pal::FAINT));
                });
                row.col(|ui| {
                    if ui.link(RichText::new(&w.system).color(if i == 0 { pal::ACCENT } else { pal::TEXT })).on_hover_text("Click to copy").clicked() {
                        ui.ctx().copy_text(w.system.clone());
                    }
                });
                row.col(|ui| {
                    ui.horizontal(|ui| {
                        if let Some(c) = &w.star_class {
                            let col = if is_scoopable(c) { pal::GOOD } else if matches!(c.as_str(), "N" | "H") || c.starts_with('D') { pal::WARN } else { pal::DIM };
                            ui.label(RichText::new(star_description(c)).color(col));
                        }
                        if w.neutron {
                            badge(ui, "neutron", pal::CYAN);
                        }
                        if w.refuel {
                            badge(ui, "refuel", pal::GOOD);
                        } else if w.scoopable == Some(true) && w.star_class.is_none() {
                            ui.label(RichText::new(icon::GAS_PUMP).color(pal::GOOD));
                        }
                    });
                });
                row.col(|ui| {
                    ui.label(RichText::new(w.jumps.max(1).to_string()).color(pal::DIM));
                });
                row.col(|ui| {
                    ui.label(RichText::new(format!("{:.1} ly", legs[i].0)).color(pal::DIM));
                });
                row.col(|ui| {
                    ui.horizontal(|ui| {
                        if let Some(v) = w.value.filter(|v| *v > 0) {
                            ui.label(RichText::new(short_credits(v as i64)).color(pal::GOLD));
                        }
                        if !w.note.is_empty() {
                            ui.label(RichText::new(&w.note).small().color(pal::DIM));
                        }
                    });
                });
            });
        });
}
