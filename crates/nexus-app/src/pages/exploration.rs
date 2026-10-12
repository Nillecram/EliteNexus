use crate::jobs::Slot;
use crate::pages::navigation::PlannedRoute;
use crate::theme::{display, pal, size};
use crate::tours::{self, Tour, limits};
use crate::widgets::{self, GAP, badge, bar, card, card_tinted, cards, empty, footnote, kv, short_credits, th};
use egui::{Grid, RichText, Sense, Ui};
use egui_extras::{Column, TableBuilder};
use egui_phosphor::regular as icon;
use nexus_model::exploration::{interesting, Body, BodyKind, DataStatus, Interest, SystemScan};
use nexus_model::GameState;
use nexus_spansh::RichesRouteRequest;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    Current,
    Find,
    Unsold,
    History,
    Codex,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HistoryFilter {
    #[default]
    All,
    Unsold,
    Sold,
    NotSellable,
    Lost,
}

/// Which bodies a Road to Riches tour looks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BodyFilter {
    #[default]
    All,
    EarthLike,
    Ammonia,
    RockyHmc,
}

/// "Find valuable bodies": Spansh's Road to Riches.
pub struct FindBodies {
    kind: BodyFilter,
    from: String,
    radius: u32,
    systems: u32,
    /// Least a body must be worth (all valuable bodies only), cr.
    min_value: u64,
    /// Furthest a body may be from the arrival star, ls.
    max_ls: u32,
    /// Rank by the DSS mapping value.
    mapping: bool,
    /// Jump range, ly (0 = the current ship's).
    range: f64,
    show_options: bool,
    job: Slot<Result<Tour, String>>,
    /// Search on the first frame (screenshot runs: `--tab find`).
    autorun: bool,
}

impl Default for FindBodies {
    fn default() -> Self {
        FindBodies {
            kind: BodyFilter::All,
            from: String::new(),
            radius: 50,
            systems: 25,
            min_value: 1_000_000,
            max_ls: 50_000,
            mapping: true,
            range: 0.0,
            show_options: false,
            job: Slot::default(),
            autorun: false,
        }
    }
}

#[derive(Default)]
pub struct ExplorationUi {
    pub tab: Tab,
    pub selected_body: Option<u32>,
    /// Show this system instead of the current one (from the unsold list).
    pub system: Option<u64>,
    pub history: HistoryFilter,
    pub find: FindBodies,
}

impl ExplorationUi {
    /// Opens on a tab by name (`find`, `history`, `unsold`); `autorun` starts the Find search on
    /// the first frame (screenshot runs).
    pub fn new(tab: Option<&str>, autorun: bool) -> Self {
        let tab = match tab {
            Some("find") => Tab::Find,
            Some("history") => Tab::History,
            Some("unsold") => Tab::Unsold,
            _ => Tab::Current,
        };
        let mut ui = ExplorationUi { tab, ..Default::default() };
        ui.find.autorun = autorun && tab == Tab::Find;
        ui
    }
}

/// Returns a route to follow when the commander picks one.
pub fn show(ui: &mut Ui, gs: &GameState, st: &mut ExplorationUi, high_value: i64, spansh: &nexus_spansh::Spansh, rt: &tokio::runtime::Handle) -> Option<PlannedRoute> {
    let mut follow = None;
    ui.horizontal(|ui| {
        if let Some(t) = widgets::tabs(ui, st.tab, &[(Tab::Current, "Current system"), (Tab::Find, "Find valuable bodies"), (Tab::Unsold, "Unsold data"), (Tab::History, "History"), (Tab::Codex, "Codex")]) {
            st.tab = t;
            if t == Tab::Current {
                st.system = None;
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let ex = &gs.exploration;
            let unsold = ex.unsold().count();
            if unsold > 0 {
                ui.label(RichText::new(format!("{unsold} unsold · {}", short_credits(ex.unsold_value()))).font(display(size::CALLOUT)).color(pal::GOLD));
            } else if let Some(s) = ex.last_sale() {
                ui.label(RichText::new(format!("All sold · last sale {} · {}", s.ts.to_local_string("%d %b"), short_credits(s.earnings))).font(display(size::CALLOUT)).color(pal::DIM));
            }
        });
    });
    ui.add_space(GAP);
    match st.tab {
        Tab::Current => {
            let addr = st.system.or(gs.exploration.current);
            match addr.and_then(|a| gs.exploration.systems.get(&a)) {
                Some(sys) => system_view(ui, gs, sys, st, high_value),
                None => nothing_scanned(ui, gs, None, st),
            }
        }
        Tab::Find => {
            egui::ScrollArea::vertical().id_salt("find-bodies").auto_shrink([false, false]).show(ui, |ui| follow = find_bodies(ui, gs, &mut st.find, spansh, rt));
        }
        Tab::Unsold => systems_list(ui, gs.exploration.unsold().collect(), st, "No unsold cartographic data."),
        Tab::History => history(ui, gs, st),
        Tab::Codex => codex(ui, gs),
    }
    follow
}

/// The current system has nothing scanned: say why the numbers are empty and where to go instead.
fn nothing_scanned(ui: &mut Ui, gs: &GameState, sys: Option<&SystemScan>, st: &mut ExplorationUi) {
    let name = sys.map_or(gs.location.system.as_str(), |s| s.name.as_str());
    let here = sys.is_none_or(|s| s.address == gs.location.system_address);
    let populated = here && gs.location.info.population > 0;
    let (title, hint) = if populated {
        (
            format!("{name} is populated"),
            "Every body here was discovered long ago, so scanning pays very little. Explorers earn in unpopulated space: Find valuable bodies plans a tour there.".to_string(),
        )
    } else {
        (
            format!("Nothing scanned in {name} yet"),
            match sys.and_then(|s| s.fss_body_count) {
                Some(n) if n > 0 => format!("{n} bodies detected by the discovery scan. Open the FSS to resolve them: values, signals and first-discovery flags appear here as you scan."),
                _ => "Honk (discovery scan) and open the FSS: every body appears here as you resolve it.".to_string(),
            },
        )
    };
    empty(ui, icon::PLANET, &title, &hint);
    ui.vertical_centered(|ui| {
        if widgets::primary_button(ui, &format!("{} Find valuable bodies", icon::MAGNIFYING_GLASS)).clicked() {
            st.tab = Tab::Find;
        }
    });
}

fn status_badge(ui: &mut Ui, s: DataStatus) -> egui::Response {
    match s {
        DataStatus::Unsold => badge(ui, "Unsold", pal::GOLD),
        DataStatus::Sold(ts) => badge(ui, &format!("Sold {}", ts.to_local_string("%d %b")), pal::GOOD),
        DataStatus::NotSellable => badge(ui, "Not sellable", pal::WARN)
            .on_hover_text("Scanned before a sale that Universal Cartographics didn't buy. The game keeps refusing it, so it's left out of unsold totals."),
        DataStatus::Lost => badge(ui, "Lost", pal::BAD).on_hover_text("Unsold when the ship was destroyed."),
    }
}

/// Every system with scans, and what became of the data.
fn history(ui: &mut Ui, gs: &GameState, st: &mut ExplorationUi) {
    let ex = &gs.exploration;
    let all: Vec<&SystemScan> = ex.history().collect();
    if all.is_empty() {
        empty(ui, icon::CLOCK_COUNTER_CLOCKWISE, "No scans yet", "Every system you scan is listed here, sold or not.");
        return;
    }
    let count = |f: HistoryFilter| all.iter().filter(|s| keep(f, s.status())).count();
    ui.horizontal_wrapped(|ui| {
        ui.label(
            RichText::new(format!("{} systems scanned · {} sales · {} earned", all.len(), ex.sales.len(), short_credits(ex.sold_total)))
                .font(display(size::HEADLINE))
                .color(pal::GOLD),
        );
    });
    ui.add_space(6.0);
    let mut options = vec![
        (HistoryFilter::All, format!("All {}", all.len())),
        (HistoryFilter::Unsold, format!("Unsold {}", count(HistoryFilter::Unsold))),
        (HistoryFilter::Sold, format!("Sold {}", count(HistoryFilter::Sold))),
        (HistoryFilter::NotSellable, format!("Not sellable {}", count(HistoryFilter::NotSellable))),
    ];
    let lost = count(HistoryFilter::Lost);
    if lost > 0 {
        options.push((HistoryFilter::Lost, format!("Lost {lost}")));
    }
    let opts: Vec<(HistoryFilter, &str)> = options.iter().map(|(f, l)| (*f, l.as_str())).collect();
    widgets::segmented(ui, &mut st.history, &opts);
    if st.history == HistoryFilter::NotSellable {
        footnote(ui, "Data scanned before a sale that Universal Cartographics didn't buy. The game keeps refusing it, so it's left out of unsold totals.");
    }
    ui.add_space(6.0);
    let mut list: Vec<&SystemScan> = all.into_iter().filter(|s| keep(st.history, s.status())).collect();
    list.sort_by_key(|s| std::cmp::Reverse(s.last_visit));
    if list.is_empty() {
        empty(ui, icon::CHECK_CIRCLE, "Nothing here", "");
        return;
    }
    TableBuilder::new(ui)
        .id_salt("history")
        .striped(true)
        .resizable(false)
        .sense(Sense::click())
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .column(Column::initial(280.0).at_least(160.0).clip(true))
        .column(Column::initial(70.0))
        .column(Column::initial(100.0))
        .column(Column::initial(130.0))
        .column(Column::remainder())
        .header(30.0, |mut h| {
            for t in ["System", "Bodies", "Value", "Data", "Last visit"] {
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
                    status_badge(ui, s.status());
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

fn keep(f: HistoryFilter, s: DataStatus) -> bool {
    match f {
        HistoryFilter::All => true,
        HistoryFilter::Unsold => s == DataStatus::Unsold,
        HistoryFilter::Sold => matches!(s, DataStatus::Sold(_)),
        HistoryFilter::NotSellable => s == DataStatus::NotSellable,
        HistoryFilter::Lost => s == DataStatus::Lost,
    }
}

/// Road to Riches: a tour of systems with valuable bodies to scan.
fn find_bodies(ui: &mut Ui, gs: &GameState, f: &mut FindBodies, spansh: &nexus_spansh::Spansh, rt: &tokio::runtime::Handle) -> Option<PlannedRoute> {
    f.job.tick();
    if f.range <= 0.0 {
        f.range = tours::jump_range(gs);
    }
    let mut follow = None;
    card_tinted(ui, icon::COINS, "Find valuable bodies", pal::GOLD, |ui| {
        ui.add(
            egui::Label::new(
                RichText::new("A tour of systems with bodies worth scanning: Earth-likes, water and ammonia worlds, terraformables (Spansh's Road to Riches). Spansh skips populated systems, so from the bubble the search widens until it finds some.")
                    .color(pal::DIM),
            )
            .wrap(),
        );
        ui.add_space(8.0);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(10.0, 10.0);
            ui.add(egui::TextEdit::singleline(&mut f.from).hint_text(format!("From {}", gs.location.system)).desired_width(200.0).margin(egui::Margin::symmetric(10, 6)));
            widgets::segmented(ui, &mut f.kind, &[(BodyFilter::All, "All valuable"), (BodyFilter::EarthLike, "Earth-likes"), (BodyFilter::Ammonia, "Ammonia worlds"), (BodyFilter::RockyHmc, "Rocky & metal-rich")]);
        });
        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(10.0, 10.0);
            ui.label(RichText::new("Within").color(pal::DIM));
            widgets::segmented(ui, &mut f.radius, &[(25, "25 ly"), (50, "50 ly"), (100, "100 ly"), (250, "250 ly"), (500, "500 ly"), (1000, "1,000 ly")]);
            ui.label(RichText::new("Stops").color(pal::DIM));
            widgets::segmented(ui, &mut f.systems, &[(10, "10"), (25, "25"), (50, "50"), (100, "100")]);
        });
        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(10.0, 10.0);
            widgets::toggle(ui, &mut f.mapping, "I'll map planets (DSS)").on_hover_text("Rank bodies by what they pay once mapped with the Detailed Surface Scanner. Off: by the FSS scan alone, quicker but much less.");
            let busy = f.job.busy();
            let label = if busy { format!("{} Searching…", icon::HOURGLASS) } else { format!("{} Search", icon::MAGNIFYING_GLASS) };
            let clicked = ui.add_enabled_ui(!busy, |ui| widgets::primary_button(ui, &label)).inner.clicked();
            if (clicked || std::mem::take(&mut f.autorun)) && !busy {
                let from = match f.from.trim() {
                    "" => gs.location.system.clone(),
                    s => s.to_string(),
                };
                let range = f.range.clamp(*limits::RANGE_LY.start(), *limits::RANGE_LY.end());
                let base = match f.kind {
                    BodyFilter::All => RichesRouteRequest::road_to_riches(from, range).min_value(f.min_value),
                    BodyFilter::EarthLike => RichesRouteRequest::earth_likes(from, range),
                    BodyFilter::Ammonia => RichesRouteRequest::ammonia_worlds(from, range),
                    BodyFilter::RockyHmc => RichesRouteRequest::rocky_hmc(from, range),
                };
                let req = base.radius(f.radius).max_results(f.systems).max_distance(f.max_ls).use_mapping_value(f.mapping);
                let sp = spansh.clone();
                f.job.run_async(ui.ctx(), rt, async move { tours::riches(&sp, req).await });
            }
            if busy {
                ui.spinner();
            }
        });
        ui.add_space(4.0);
        if widgets::disclosure(ui, &mut f.show_options, "Options") {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(10.0, 8.0);
                ui.label("Jump range");
                ui.add(egui::DragValue::new(&mut f.range).range(limits::RANGE_LY).speed(0.1).suffix(" ly")).on_hover_text("Spansh plans with 10 to 100 ly. Starts at your ship's range.");
                ui.label("Within");
                ui.add(egui::DragValue::new(&mut f.radius).range(limits::RADIUS_LY).suffix(" ly")).on_hover_text("Spansh searches up to 1,000 ly around the start.");
                ui.label("Stops");
                ui.add(egui::DragValue::new(&mut f.systems).range(limits::SYSTEMS)).on_hover_text("Spansh plans up to 2,000 systems.");
                if f.kind == BodyFilter::All {
                    ui.label("Bodies worth at least");
                    let mut m = f.min_value as f64 / 1e6;
                    if ui.add(egui::DragValue::new(&mut m).range(0.0..=50.0).speed(0.05).suffix(" M Cr")).changed() {
                        f.min_value = (m * 1e6).round() as u64;
                    }
                }
                ui.label("At most");
                ui.add(egui::DragValue::new(&mut f.max_ls).range(100..=1_000_000).speed(100.0).suffix(" ls from the star")).on_hover_text("Bodies further out take long to reach in supercruise.");
            });
        }
        ui.add_space(6.0);
        match &f.job.last {
            None => {}
            Some(Err(e)) => {
                ui.add(egui::Label::new(RichText::new(format!("{} {e}", icon::WARNING_CIRCLE)).color(pal::BAD)).wrap());
            }
            Some(Ok(t)) if t.is_empty() => {
                empty(
                    ui,
                    icon::PLANET,
                    &format!("Nothing worth scanning within {} ly", t.radius),
                    "Spansh skips populated systems and found no bodies that match, even with the search widened. Lower the minimum value under Options, or start from somewhere further out.",
                );
            }
            Some(Ok(t)) => follow = tour_results(ui, t, f.mapping),
        }
        ui.add_space(2.0);
        ui.label(RichText::new("Route data © Spansh (spansh.co.uk)").size(size::CAPTION).color(pal::FAINT));
    });
    follow
}

/// The systems of a Road to Riches tour and the bodies to scan in each.
fn tour_results(ui: &mut Ui, t: &Tour, mapping: bool) -> Option<PlannedRoute> {
    let mut follow = None;
    let stops: Vec<_> = t.route.systems.iter().filter(|s| !s.bodies.is_empty()).collect();
    ui.horizontal_wrapped(|ui| {
        ui.label(
            RichText::new(format!(
                "{} systems · {} bodies · ~{} {} · about {} jumps",
                stops.len(),
                t.route.body_count(),
                short_credits(t.route.total_value() as i64),
                if mapping { "if you map them" } else { "from FSS scans" },
                t.route.total_jumps()
            ))
            .font(display(size::HEADLINE))
            .color(pal::GOLD),
        );
        if widgets::primary_button(ui, &format!("{} Follow this route", icon::NAVIGATION_ARROW)).on_hover_text("Shows it on the Navigation page and copies the next system after every jump").clicked() {
            follow = Some(t.planned("Road to riches"));
        }
        ui.hyperlink_to(format!("{} Open on Spansh", icon::ARROW_SQUARE_OUT), &t.web_url);
    });
    if let Some(asked) = t.widened_from {
        ui.add(egui::Label::new(RichText::new(format!("{} Nothing within {asked} ly (Spansh skips populated systems), so the search was widened to {} ly.", icon::INFO, t.radius)).color(pal::WARN)).wrap());
    }
    ui.add_space(6.0);
    // The wrapping column goes last: a grid gives the last column the width that's left.
    Grid::new("riches").max_col_width(ui.available_width()).num_columns(5).striped(true).spacing([14.0, 8.0]).show(ui, |ui| {
        for h in ["#", "System", "Jumps", "Value", "Bodies to scan"] {
            th(ui, h);
        }
        ui.end_row();
        for (i, s) in stops.iter().enumerate() {
            ui.label(RichText::new((i + 1).to_string()).color(pal::FAINT));
            if ui.link(RichText::new(&s.name).color(pal::TEXT)).on_hover_text("Click to copy").clicked() {
                ui.ctx().copy_text(s.name.clone());
            }
            ui.label(RichText::new(s.jumps.max(1).to_string()).color(pal::DIM));
            ui.label(RichText::new(short_credits(s.value() as i64)).color(pal::GOLD));
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                for b in &s.bodies {
                    let short = b.name.strip_prefix(&s.name).map(str::trim).filter(|n| !n.is_empty()).unwrap_or(&b.name);
                    let kind = b.subtype.as_deref().unwrap_or("Body");
                    let terra = if b.is_terraformable { " (terraformable)" } else { "" };
                    ui.add(egui::Label::new(RichText::new(format!("{short} · {kind}{terra} · {:.0} ls · {}", b.distance_to_arrival, short_credits(b.exploration_value() as i64))).color(pal::DIM)).wrap());
                }
            });
            ui.end_row();
        }
    });
    follow
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
    if sys.bodies.values().all(|b| b.kind == BodyKind::BeltCluster) {
        // Nothing scanned: four cards of zeros would only look broken.
        if st.system.is_some() && Some(sys.address) != gs.exploration.current && widgets::button(ui, &format!("{} Back to the current system", icon::ARROW_U_UP_LEFT)).clicked() {
            st.system = None;
        }
        nothing_scanned(ui, gs, Some(sys), st);
        signals_list(ui, sys);
        return;
    }
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
    signals_list(ui, sys);
}

/// The FSS signals of a system (stations, carriers, USSs…), collapsed.
fn signals_list(ui: &mut Ui, sys: &SystemScan) {
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
