//! Market: live EDDN-fed galaxy database — commodity search, trade routes, services, hotspots.

use crate::finder::{self, SellPlan};
use crate::jobs::Slot;
use crate::market::{ImportJob, ImportSource, MarketService, SPANSH_STATIONS_URL};
use crate::settings::PriceWatch;
use crate::theme::{display, pal, size};
use crate::widgets::{badge, bar, card, duration, empty, short_credits};
use egui::{Grid, RichText, Ui};
use egui_extras::{Column, TableBuilder};
use egui_phosphor::regular as icon;
use nexus_galaxy::{
    CommodityHit, CommodityQuery, Deal, DealQuery, GalaxyDb, HotspotHit, HotspotQuery, MaterialTrader, Services, Sort, StationHit, StationQuery,
    TechBroker, TradeMode, TradeQuery, TradeRoute,
};
use nexus_model::GameState;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    Commodity,
    Abnormal,
    Sell,
    Routes,
    Services,
    Hotspots,
    Alerts,
    Docked,
    Database,
}

impl Tab {
    /// For `--tab` (screenshots): `commodity`, `sell`, `routes`, `services`, `hotspots`, `alerts`, `docked`, `database`.
    pub fn from_name(name: &str) -> Option<Tab> {
        Some(match name.to_ascii_lowercase().as_str() {
            "commodity" => Tab::Commodity,
            "abnormal" => Tab::Abnormal,
            "sell" => Tab::Sell,
            "routes" => Tab::Routes,
            "services" => Tab::Services,
            "hotspots" => Tab::Hotspots,
            "alerts" => Tab::Alerts,
            "docked" => Tab::Docked,
            "database" => Tab::Database,
            _ => return None,
        })
    }
}

type Res<T> = Result<Vec<T>, String>;
/// Abnormal-price filters: mode, ratio ×100, radius, age, carriers, pad.
type AbnKey = (TradeMode, u64, Option<u64>, u64, bool, Option<char>);

pub struct MarketUi {
    pub tab: Tab,
    pub commodity: String,
    pub mode: TradeMode,
    pub sort: Sort,
    pub max_dist: f64,
    pub min_pad: Option<char>,
    pub max_age_h: f64,
    pub min_qty: i64,
    pub carriers: bool,
    pub planetary: bool,
    pub hits: Slot<Res<CommodityHit>>,
    pub radius: f64,
    pub hop: f64,
    pub routes: Slot<Res<TradeRoute>>,
    pub service: usize,
    pub stations: Slot<Res<StationHit>>,
    pub hotspot_signal: String,
    pub hotspot_min: i64,
    pub hotspots: Slot<Res<HotspotHit>>,
    pub hotspot_prices: Slot<Res<CommodityHit>>,
    /// Path typed into "import a downloaded dump".
    pub import_path: String,
    pub sell: Slot<Result<SellPlan, String>>,
    pub sell_radius: f64,
    /// (cargo snapshot time, tonnes, system): re-plan when any of them changes.
    sell_key: (i64, i64, u64),
    /// The rule being composed in "Price alerts".
    pub new_watch: PriceWatch,
    /// Find commodity: filters shown (hidden by default).
    pub show_options: bool,
    /// The pad filter follows the current ship until the user picks one.
    pub pad_auto: bool,
    /// Abnormal prices: stations paying far above (or selling far below) the galactic average.
    pub abn_mode: TradeMode,
    pub abn_ratio: f64,
    /// `None` = the whole galaxy.
    pub abn_radius: Option<f64>,
    pub abn_age_h: f64,
    pub abn_carriers: bool,
    pub abn: Slot<Res<Deal>>,
    /// The filters the shown results were found with (a change re-runs the scan).
    abn_key: Option<AbnKey>,
}

impl Default for MarketUi {
    fn default() -> Self {
        MarketUi {
            tab: Tab::Commodity,
            commodity: "Tritium".into(),
            mode: TradeMode::BuyFromStation,
            sort: Sort::BestPrice,
            max_dist: 60.0,
            min_pad: Some('L'),
            max_age_h: 72.0,
            min_qty: 100,
            carriers: true,
            planetary: true,
            hits: Slot::default(),
            radius: 40.0,
            hop: 25.0,
            routes: Slot::default(),
            service: 0,
            stations: Slot::default(),
            hotspot_signal: "Low Temperature Diamonds".into(),
            hotspot_min: 2,
            hotspots: Slot::default(),
            hotspot_prices: Slot::default(),
            import_path: String::new(),
            sell: Slot::default(),
            sell_radius: 60.0,
            sell_key: (0, 0, 0),
            new_watch: PriceWatch::default(),
            show_options: false,
            pad_auto: true,
            abn_mode: TradeMode::SellToStation,
            abn_ratio: 2.0,
            abn_radius: None,
            abn_age_h: 72.0,
            abn_carriers: true,
            abn: Slot::default(),
            abn_key: None,
        }
    }
}

/// Farthest a local-database search can reach, ly: the whole galaxy (it's about 100,000 ly
/// across, and the database itself has no limit).
const MAX_SEARCH_LY: f64 = 100_000.0;

const SERVICES: &[(&str, &str)] = &[
    ("Raw material trader", "raw"),
    ("Manufactured material trader", "manufactured"),
    ("Encoded material trader", "encoded"),
    ("Human technology broker", "human"),
    ("Guardian technology broker", "guardian"),
    ("Interstellar Factors", "factors"),
    ("Universal Cartographics", "cartographics"),
    ("Vista Genomics", "vista"),
    ("Shipyard", "shipyard"),
    ("Outfitting", "outfitting"),
    ("Black market", "blackmarket"),
];

const HOTSPOT_SIGNALS: &[&str] = &[
    "Low Temperature Diamonds",
    "Painite",
    "Void Opal",
    "Platinum",
    "Alexandrite",
    "Benitoite",
    "Grandidierite",
    "Monazite",
    "Musgravite",
    "Rhodplumsite",
    "Serendibite",
    "Tritium",
    "Bromellite",
];

fn pad_combo(ui: &mut Ui, id: &str, pad: &mut Option<char>) {
    egui::ComboBox::from_id_salt(id)
        .width(70.0)
        .selected_text(match pad {
            Some(c) => format!("{c} pad"),
            None => "Any pad".into(),
        })
        .show_ui(ui, |ui| {
            ui.selectable_value(pad, None, "Any pad");
            ui.selectable_value(pad, Some('S'), "S pad");
            ui.selectable_value(pad, Some('M'), "M pad");
            ui.selectable_value(pad, Some('L'), "L pad");
        });
}

fn age(updated_unix: i64) -> (String, egui::Color32) {
    let s = (nexus_journal::Timestamp::now().0 - updated_unix).max(0);
    let col = if s < 3600 { pal::GOOD } else if s < 6 * 3600 { pal::CYAN } else if s < 48 * 3600 { pal::WARN } else { pal::FAINT };
    let txt = if s < 60 {
        format!("{s}s")
    } else if s < 3600 {
        format!("{}m", s / 60)
    } else if s < 86_400 {
        format!("{}h", s / 3600)
    } else {
        format!("{}d", s / 86_400)
    };
    (txt, col)
}

/// Returns true when the price-alert rules changed (settings to save).
pub fn show(ui: &mut Ui, gs: &GameState, svc: &mut MarketService, st: &mut MarketUi, rt: &tokio::runtime::Handle, watches: &mut Vec<PriceWatch>) -> bool {
    st.hits.tick();
    st.routes.tick();
    st.stations.tick();
    st.hotspots.tick();
    st.hotspot_prices.tick();
    st.sell.tick();
    st.abn.tick();

    if let Some(t) = crate::widgets::tabs(
        ui,
        st.tab,
        &[
            (Tab::Commodity, "Find commodity"),
            (Tab::Abnormal, "Abnormal prices"),
            (Tab::Sell, "Sell my cargo"),
            (Tab::Routes, "Trade routes"),
            (Tab::Services, "Services"),
            (Tab::Hotspots, "Mining hotspots"),
            (Tab::Alerts, "Price alerts"),
            (Tab::Docked, "This station"),
            (Tab::Database, "Database & feed"),
        ],
    ) {
        st.tab = t;
    }
    ui.add_space(8.0);
    feed_header(ui, svc);
    ui.add_space(12.0);
    let Some(db) = svc.db.clone() else {
        empty(ui, icon::DATABASE, "Market database unavailable", svc.db_error.as_deref().unwrap_or("Starting…"));
        return false;
    };
    let origin = gs.location.star_pos;
    let ctx = ui.ctx().clone();
    let mut changed = false;
    match st.tab {
        Tab::Commodity => {
            // Smart default: the landing pad the current ship needs (until the user picks one).
            if st.pad_auto {
                st.min_pad = crate::finder::ship_pad(gs);
            }
            let mut search = false;
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(10.0, 10.0);
                let field = crate::widgets::search_field(ui, &mut st.commodity, "Commodity, e.g. Tritium, Gold, Painite", 320.0);
                search |= field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                crate::widgets::segmented(ui, &mut st.mode, &[(TradeMode::BuyFromStation, "Buy"), (TradeMode::SellToStation, "Sell")]);
                search |= crate::widgets::primary_button(ui, "Search").clicked();
                crate::widgets::disclosure(ui, &mut st.show_options, "Options");
            });
            if st.show_options {
                ui.horizontal_wrapped(|ui| {
                    ui.label("within");
                    ui.add(egui::DragValue::new(&mut st.max_dist).range(1.0..=MAX_SEARCH_LY).suffix(" ly")).on_hover_text("Searches every market Elite Nexus knows, up to the whole galaxy.");
                    let before = st.min_pad;
                    pad_combo(ui, "mpad", &mut st.min_pad);
                    if st.min_pad != before {
                        st.pad_auto = false;
                    }
                    ui.label("max age");
                    ui.add(egui::DragValue::new(&mut st.max_age_h).range(1.0..=720.0).suffix(" h"));
                    ui.label("min qty");
                    ui.add(egui::DragValue::new(&mut st.min_qty).range(1..=100_000));
                    crate::widgets::toggle(ui, &mut st.carriers, "Carriers");
                    crate::widgets::toggle(ui, &mut st.planetary, "Planetary");
                    egui::ComboBox::from_id_salt("msort")
                        .selected_text(match st.sort {
                            Sort::BestPrice => "Best price",
                            Sort::Nearest => "Nearest",
                            Sort::Freshest => "Freshest",
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut st.sort, Sort::BestPrice, "Best price");
                            ui.selectable_value(&mut st.sort, Sort::Nearest, "Nearest");
                            ui.selectable_value(&mut st.sort, Sort::Freshest, "Freshest");
                        });
                });
            }
            if search && !st.commodity.trim().is_empty() {
                let q = CommodityQuery {
                    commodity: st.commodity.trim().to_string(),
                    mode: st.mode,
                    origin,
                    max_distance_ly: origin.map(|_| st.max_dist),
                    min_pad: st.min_pad,
                    max_age_hours: Some(st.max_age_h),
                    min_quantity: st.min_qty,
                    include_carriers: st.carriers,
                    include_planetary: st.planetary,
                    limit: 100,
                    sort: st.sort,
                };
                let db = db.clone();
                st.hits.run(&ctx, move || db.search_commodity(&q).map_err(|e| e.to_string()));
            }
            results_commodity(ui, &st.hits, st.mode);
        }
        Tab::Routes => {
            let cargo = gs.fleet.current().map(|s| s.cargo_capacity as i64).unwrap_or(100);
            ui.horizontal_wrapped(|ui| {
                ui.label("Routes within");
                ui.add(egui::DragValue::new(&mut st.radius).range(5.0..=1000.0).suffix(" ly")).on_hover_text("Up to 1,000 ly. Every market in the area is loaded, so wide areas take a few seconds.");
                ui.label("of here, hop up to");
                ui.add(egui::DragValue::new(&mut st.hop).range(1.0..=500.0).suffix(" ly")).on_hover_text("Longest trip between buying and selling, up to 500 ly.");
                pad_combo(ui, "rpad", &mut st.min_pad);
                ui.label(format!("cargo {cargo} t, capital {}", short_credits(gs.cmdr.credits)));
                crate::widgets::toggle(ui, &mut st.carriers, "Carriers");
                if crate::widgets::primary_button(ui, "Find routes").clicked()
                    && let Some(o) = origin {
                        let q = TradeQuery {
                            origin: o,
                            radius_ly: st.radius,
                            max_hop_ly: st.hop,
                            min_pad: st.min_pad,
                            cargo_capacity: cargo.max(1),
                            capital: Some(gs.cmdr.credits.max(0)),
                            max_age_hours: Some(st.max_age_h),
                            include_carriers: st.carriers,
                            include_planetary: st.planetary,
                            ..Default::default()
                        };
                        let db = db.clone();
                        st.routes.run(&ctx, move || db.trade_routes(&q).map_err(|e| e.to_string()));
                    }
            });
            results_routes(ui, &st.routes);
        }
        Tab::Services => {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);
                for (i, (label, _)) in SERVICES.iter().enumerate() {
                    if crate::widgets::pill(ui, st.service == i, label).clicked() {
                        st.service = i;
                        if let Some(o) = origin {
                            let q = service_query(SERVICES[i].1, o, st.min_pad);
                            let db = db.clone();
                            st.stations.run(&ctx, move || db.stations_near(&q).map_err(|e| e.to_string()));
                        }
                    }
                }
            });
            results_stations(ui, &st.stations);
        }
        Tab::Hotspots => {
            ui.horizontal_wrapped(|ui| {
                egui::ComboBox::from_id_salt("hsig").selected_text(st.hotspot_signal.clone()).show_ui(ui, |ui| {
                    for s in HOTSPOT_SIGNALS {
                        ui.selectable_value(&mut st.hotspot_signal, (*s).to_string(), *s);
                    }
                });
                ui.label("at least");
                ui.add(egui::DragValue::new(&mut st.hotspot_min).range(1..=6));
                ui.label("hotspots within");
                ui.add(egui::DragValue::new(&mut st.max_dist).range(1.0..=MAX_SEARCH_LY).suffix(" ly")).on_hover_text("Searches every market Elite Nexus knows, up to the whole galaxy.");
                if crate::widgets::primary_button(ui, "Find hotspots").clicked()
                    && let Some(o) = origin {
                        let q = HotspotQuery { origin: o, signal: st.hotspot_signal.clone(), max_distance_ly: Some(st.max_dist), min_count: st.hotspot_min, limit: 50 };
                        let db2 = db.clone();
                        st.hotspots.run(&ctx, move || db2.hotspots_near(&q).map_err(|e| e.to_string()));
                        let pq = CommodityQuery {
                            commodity: st.hotspot_signal.clone(),
                            mode: TradeMode::SellToStation,
                            origin: Some(o),
                            max_distance_ly: Some(st.max_dist * 2.0),
                            max_age_hours: Some(72.0),
                            limit: 10,
                            ..Default::default()
                        };
                        let db3 = db.clone();
                        st.hotspot_prices.run(&ctx, move || db3.search_commodity(&pq).map_err(|e| e.to_string()));
                    }
            });
            ui.add(egui::Label::new(RichText::new("Hotspots come from DSS ring scans shared over EDDN, and your own. For galaxy-wide searches use Navigation › Spansh.").small().color(pal::DIM)).wrap());
            ui.add_space(8.0);
            crate::widgets::split(
                ui,
                0.5,
                360.0,
                |ui| results_hotspots(ui, &st.hotspots),
                |ui| {
                    card(ui, "Best places to sell nearby", |ui| results_commodity(ui, &st.hotspot_prices, TradeMode::SellToStation));
                },
            );
        }
        Tab::Abnormal => abnormal_tab(ui, gs, &db, st),
        // Card-style tabs scroll as a whole (the page itself does not scroll: tables scroll on their own).
        Tab::Sell => {
            egui::ScrollArea::vertical().id_salt("msell").auto_shrink([false, false]).show(ui, |ui| sell_tab(ui, gs, &db, st));
        }
        Tab::Alerts => {
            egui::ScrollArea::vertical().id_salt("malerts").auto_shrink([false, false]).show(ui, |ui| changed |= alerts_tab(ui, svc, st, watches));
        }
        Tab::Docked => docked_market(ui, gs),
        Tab::Database => {
            egui::ScrollArea::vertical().id_salt("mdb").auto_shrink([false, false]).show(ui, |ui| database_tab(ui, svc, st, rt));
        }
    }
    changed
}

/// Abnormal prices: anything a station pays far above the galactic average, or sells far below it,
/// across the whole database (or within a radius). The scan re-runs whenever a filter changes.
fn abnormal_tab(ui: &mut Ui, gs: &GameState, db: &GalaxyDb, st: &mut MarketUi) {
    let origin = gs.location.star_pos;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(10.0, 10.0);
        crate::widgets::segmented(ui, &mut st.abn_mode, &[(TradeMode::SellToStation, "Paying far above average"), (TradeMode::BuyFromStation, "Selling far below average")]);
        ui.label(RichText::new("at least").color(pal::DIM));
        let ratios: &[(u32, &str)] = &[(150, "1.5×"), (200, "2×"), (300, "3×"), (500, "5×"), (1000, "10×")];
        let mut r = (st.abn_ratio * 100.0).round() as u32;
        if crate::widgets::segmented(ui, &mut r, ratios) {
            st.abn_ratio = f64::from(r) / 100.0;
        }
        let scopes: &[(u32, &str)] = &[(0, "Whole galaxy"), (100, "Within 100 ly"), (500, "Within 500 ly")];
        let mut scope = st.abn_radius.map_or(0, |d| d as u32);
        if crate::widgets::segmented(ui, &mut scope, scopes) {
            st.abn_radius = (scope > 0).then_some(f64::from(scope));
        }
    });
    ui.add_space(4.0);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(14.0, 8.0);
        crate::widgets::toggle(ui, &mut st.abn_carriers, "Fleet carriers");
        ui.label(RichText::new("Prices from the last").color(pal::DIM));
        ui.add(egui::DragValue::new(&mut st.abn_age_h).range(1.0..=720.0).suffix(" h"));
        pad_combo(ui, "abnpad", &mut st.min_pad);
        if crate::widgets::button(ui, &format!("{} Refresh", icon::ARROW_CLOCKWISE)).clicked() {
            st.abn_key = None;
        }
    });
    let key = (st.abn_mode, (st.abn_ratio * 100.0) as u64, st.abn_radius.map(|r| r as u64), st.abn_age_h as u64, st.abn_carriers, st.min_pad);
    if st.abn_key != Some(key) && !st.abn.busy() && (st.abn_radius.is_none() || origin.is_some()) {
        st.abn_key = Some(key);
        let q = DealQuery {
            mode: st.abn_mode,
            origin,
            max_distance_ly: st.abn_radius,
            min_ratio: st.abn_ratio,
            min_pad: st.min_pad,
            include_carriers: st.abn_carriers,
            max_age_hours: Some(st.abn_age_h),
            cargo_capacity: gs.fleet.current().map(|s| i64::from(s.cargo_capacity.max(1))).unwrap_or(100),
            limit: 200,
        };
        let db = db.clone();
        st.abn.run(ui.ctx(), move || db.price_deals(&q).map_err(|e| e.to_string()));
    }
    ui.add_space(8.0);
    let explain = match st.abn_mode {
        TradeMode::SellToStation => "Stations and carriers buying a commodity for far more than its galactic average: sell there.",
        TradeMode::BuyFromStation => "Stations and carriers selling a commodity for far less than its galactic average: buy there.",
    };
    ui.add(egui::Label::new(RichText::new(explain).size(size::FOOTNOTE).color(pal::DIM)).wrap());
    if !status(ui, &st.abn) {
        return;
    }
    let Some(Ok(deals)) = &st.abn.last else { return };
    if deals.is_empty() {
        empty(ui, icon::MAGNIFYING_GLASS, "Nothing abnormal right now", "No price in range is that far from its galactic average. Try a lower multiple or older prices.");
        return;
    }
    let sell = st.abn_mode == TradeMode::SellToStation;
    // Narrow windows drop the average and quantity columns rather than spilling off the page.
    let wide = ui.available_width() >= 1040.0;
    let mut table = TableBuilder::new(ui)
        .id_salt("abnormal")
        .striped(true)
        .resizable(false)
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .column(Column::initial(180.0).at_least(110.0).clip(true))
        .column(Column::initial(118.0));
    if wide {
        table = table.column(Column::initial(100.0)).column(Column::initial(70.0)).column(Column::initial(84.0));
    } else {
        table = table.column(Column::initial(64.0));
    }
    let mut headers = vec!["Commodity", if sell { "They pay" } else { "They sell for" }];
    if wide {
        headers.extend(["Average", "×", if sell { "Demand" } else { "Stock" }]);
    } else {
        headers.push("×");
    }
    headers.extend(["Station", "Distance", "Updated"]);
    table
        .column(Column::remainder().at_least(140.0).clip(true))
        .column(Column::initial(76.0))
        .column(Column::initial(76.0))
        .header(30.0, |mut h| {
            for t in &headers {
                h.col(|ui| crate::widgets::th(ui, t));
            }
        })
        .body(|body| {
            body.rows(34.0, deals.len(), |mut row| {
                let d = &deals[row.index()];
                row.col(|ui| {
                    ui.label(RichText::new(&d.commodity_name).color(pal::TEXT));
                });
                row.col(|ui| {
                    ui.label(RichText::new(format!("{} Cr", nexus_data::names::group_digits(d.price))).font(display(size::CALLOUT)).color(if sell { pal::GOOD } else { pal::CYAN }));
                });
                if wide {
                    row.col(|ui| {
                        ui.label(RichText::new(format!("{} Cr", nexus_data::names::group_digits(d.average))).color(pal::DIM));
                    });
                }
                row.col(|ui| {
                    let col = if d.ratio >= 10.0 { pal::PINK } else if d.ratio >= 3.0 { pal::ACCENT } else { pal::GOLD };
                    ui.label(RichText::new(format!("{:.1}×", d.ratio)).font(display(size::CALLOUT)).color(col))
                        .on_hover_text(format!("Galactic average {} Cr · {} {}", nexus_data::names::group_digits(d.average), nexus_data::names::group_digits(d.quantity), if sell { "wanted" } else { "in stock" }));
                });
                if wide {
                    row.col(|ui| {
                        ui.label(RichText::new(nexus_data::names::group_digits(d.quantity)).color(pal::DIM));
                    });
                }
                row.col(|ui| {
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        let name = if d.is_carrier { format!("{} {}", icon::ANCHOR, d.station) } else { d.station.clone() };
                        ui.add(egui::Label::new(RichText::new(name).color(pal::TEXT)).truncate());
                        if ui.add(egui::Label::new(RichText::new(&d.system).size(size::CAPTION).color(pal::DIM)).truncate().sense(egui::Sense::click())).on_hover_text("Click to copy the system").clicked() {
                            ui.ctx().copy_text(d.system.clone());
                        }
                    });
                });
                row.col(|ui| {
                    ui.label(RichText::new(d.distance_ly.map(|x| format!("{x:.0} ly")).unwrap_or_default()).color(pal::DIM));
                });
                row.col(|ui| {
                    let (a, col) = age(d.updated_unix);
                    ui.label(RichText::new(format!("{a} ago")).color(col));
                });
            });
        });
}

fn service_query(kind: &str, origin: [f64; 3], pad: Option<char>) -> StationQuery {
    let mut q = StationQuery { origin, min_pad: pad, limit: 25, ..Default::default() };
    match kind {
        "raw" => q.material_trader = Some(MaterialTrader::Raw),
        "manufactured" => q.material_trader = Some(MaterialTrader::Manufactured),
        "encoded" => q.material_trader = Some(MaterialTrader::Encoded),
        "human" => q.tech_broker = Some(TechBroker::Human),
        "guardian" => q.tech_broker = Some(TechBroker::Guardian),
        "factors" => q.services = Services::INTERSTELLAR_FACTORS,
        "cartographics" => q.services = Services::UNIVERSAL_CARTOGRAPHICS,
        "vista" => q.services = Services::VISTA_GENOMICS,
        "shipyard" => q.services = Services::SHIPYARD,
        "outfitting" => q.services = Services::OUTFITTING,
        "blackmarket" => q.services = Services::BLACK_MARKET,
        _ => {}
    }
    q
}

fn feed_header(ui: &mut Ui, svc: &mut MarketService) {
    ui.horizontal_wrapped(|ui| {
        match svc.feed_summary() {
            Some((connected, line)) => {
                badge(ui, if connected { "Live" } else { "Connecting" }, if connected { pal::GOOD } else { pal::WARN });
                ui.label(RichText::new(line).color(pal::DIM));
            }
            None => {
                badge(ui, "Feed off", pal::GRAY);
                ui.label(RichText::new("Turn on the live feed in Accounts & sync for real-time prices.").color(pal::DIM));
            }
        }
        if let Some(s) = svc.db_summary() {
            ui.label(RichText::new(s).small().color(pal::DIM));
        }
    });
    for l in svc.recent_headlines(4) {
        ui.label(RichText::new(format!("{} {l}", icon::BROADCAST)).small().color(pal::DIM));
    }
}

fn status<T>(ui: &mut Ui, slot: &Slot<Res<T>>) -> bool {
    if slot.busy() {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label(RichText::new("Searching…").color(pal::DIM));
        });
        return false;
    }
    match &slot.last {
        None => false,
        Some(Err(e)) => {
            ui.label(RichText::new(e).color(pal::BAD));
            false
        }
        Some(Ok(v)) => {
            if let Some(t) = slot.took {
                ui.label(RichText::new(format!("{} results in {:.1} ms", v.len(), t.as_secs_f64() * 1e3)).small().color(pal::DIM));
            }
            true
        }
    }
}

fn results_commodity(ui: &mut Ui, slot: &Slot<Res<CommodityHit>>, mode: TradeMode) {
    if slot.last.is_none() && !slot.busy() {
        empty(ui, icon::MAGNIFYING_GLASS, "Find the best price for anything", "Type a commodity and press Search: stations and carriers near you, best price first.");
        return;
    }
    if !status(ui, slot) {
        return;
    }
    let Some(Ok(hits)) = &slot.last else { return };
    if hits.is_empty() {
        ui.label(RichText::new("Nothing found — widen the distance/age or wait for the live feed to fill the database.").color(pal::DIM));
        return;
    }
    TableBuilder::new(ui)
        .id_salt(("chits", mode as u8))
        .striped(true)
        .resizable(false)
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .column(Column::initial(90.0))
        .column(Column::initial(80.0))
        .column(Column::initial(230.0))
        .column(Column::initial(200.0))
        .column(Column::initial(70.0))
        .column(Column::initial(50.0))
        .column(Column::initial(60.0))
        .column(Column::remainder())
        .header(30.0, |mut h| {
            for t in [if mode == TradeMode::BuyFromStation { "Buy at" } else { "Sell at" }, if mode == TradeMode::BuyFromStation { "Stock" } else { "Demand" }, "Station", "System", "Distance", "Pad", "Age", ""] {
                h.col(|ui| {
                    crate::widgets::th(ui, t);
                });
            }
        })
        .body(|body| {
            body.rows(32.0, hits.len(), |mut row| {
                let h = &hits[row.index()];
                row.col(|ui| {
                    ui.label(RichText::new(nexus_data::names::group_digits(h.price)).font(display(14.0)).color(pal::GOLD));
                });
                row.col(|ui| {
                    ui.label(RichText::new(nexus_data::names::group_digits(h.quantity)).color(pal::DIM));
                });
                row.col(|ui| {
                    ui.horizontal(|ui| {
                        if h.is_carrier {
                            ui.label(RichText::new(icon::ANCHOR).color(pal::CYAN)).on_hover_text("Fleet carrier");
                        } else if h.is_planetary {
                            ui.label(RichText::new(icon::PLANET).color(pal::DIM)).on_hover_text("Planetary");
                        }
                        ui.label(&h.station);
                        if let Some(ls) = h.dist_ls {
                            ui.label(RichText::new(format!("{ls:.0} ls")).small().color(pal::DIM));
                        }
                    });
                });
                row.col(|ui| {
                    if ui.link(&h.system).on_hover_text("Click to copy").clicked() {
                        ui.ctx().copy_text(h.system.clone());
                    }
                });
                row.col(|ui| {
                    ui.label(RichText::new(h.distance_ly.map(|d| format!("{d:.1} ly")).unwrap_or_default()).color(pal::DIM));
                });
                row.col(|ui| {
                    ui.label(RichText::new(h.max_pad.map(String::from).unwrap_or_else(|| "?".into())).color(pal::DIM));
                });
                row.col(|ui| {
                    let (t, c) = age(h.updated_unix);
                    ui.label(RichText::new(t).color(c));
                });
                row.col(|_| {});
            });
        });
}

fn results_routes(ui: &mut Ui, slot: &Slot<Res<TradeRoute>>) {
    if !status(ui, slot) {
        return;
    }
    let Some(Ok(routes)) = &slot.last else { return };
    if routes.is_empty() {
        ui.label(RichText::new("No profitable routes in range with the current data.").color(pal::DIM));
        return;
    }
    TableBuilder::new(ui)
        .striped(true)
        .resizable(false)
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .column(Column::initial(170.0))
        .column(Column::initial(90.0))
        .column(Column::initial(100.0))
        .column(Column::initial(260.0))
        .column(Column::initial(260.0))
        .column(Column::remainder())
        .header(30.0, |mut h| {
            for t in ["Commodity", "Profit/t", "Trip profit", "Buy at", "Sell at", "Hop"] {
                h.col(|ui| {
                    crate::widgets::th(ui, t);
                });
            }
        })
        .body(|body| {
            body.rows(44.0, routes.len(), |mut row| {
                let r = &routes[row.index()];
                row.col(|ui| {
                    ui.label(&r.commodity_name);
                });
                row.col(|ui| {
                    ui.label(RichText::new(nexus_data::names::group_digits(r.profit_per_ton)).font(display(14.0)).color(pal::GOOD));
                });
                row.col(|ui| {
                    ui.label(RichText::new(short_credits(r.total_profit)).color(pal::GOLD));
                });
                for (end, price) in [(&r.from, r.buy_price), (&r.to, r.sell_price)] {
                    row.col(|ui| {
                        ui.vertical(|ui| {
                            ui.label(format!("{} — {} Cr", end.station, nexus_data::names::group_digits(price)));
                            let (a, c) = age(end.market_updated_unix);
                            ui.label(RichText::new(format!("{} · {:.1} ly · {}", end.system, end.distance_from_origin_ly, a)).small().color(c));
                        });
                    });
                }
                row.col(|ui| {
                    ui.label(RichText::new(format!("{:.1} ly", r.hop_ly)).color(pal::DIM));
                });
            });
        });
}

fn results_stations(ui: &mut Ui, slot: &Slot<Res<StationHit>>) {
    if !status(ui, slot) {
        return;
    }
    let Some(Ok(list)) = &slot.last else { return };
    TableBuilder::new(ui)
        .striped(true)
        .resizable(false)
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .column(Column::initial(260.0))
        .column(Column::initial(220.0))
        .column(Column::initial(80.0))
        .column(Column::initial(80.0))
        .column(Column::initial(50.0))
        .column(Column::remainder())
        .header(30.0, |mut h| {
            for t in ["Station", "System", "Distance", "From star", "Pad", "Economy"] {
                h.col(|ui| {
                    crate::widgets::th(ui, t);
                });
            }
        })
        .body(|body| {
            body.rows(32.0, list.len(), |mut row| {
                let s = &list[row.index()];
                row.col(|ui| {
                    ui.label(&s.station.name);
                });
                row.col(|ui| {
                    if ui.link(&s.station.system).on_hover_text("Click to copy").clicked() {
                        ui.ctx().copy_text(s.station.system.clone());
                    }
                });
                row.col(|ui| {
                    ui.label(RichText::new(format!("{:.1} ly", s.distance_ly)).color(pal::DIM));
                });
                row.col(|ui| {
                    ui.label(RichText::new(s.station.dist_ls.map(|d| format!("{d:.0} ls")).unwrap_or_default()).color(pal::DIM));
                });
                row.col(|ui| {
                    ui.label(RichText::new(s.station.max_pad.map(String::from).unwrap_or_else(|| "?".into())).color(pal::DIM));
                });
                row.col(|ui| {
                    ui.label(RichText::new(s.station.economy.clone().unwrap_or_default()).color(pal::DIM));
                });
            });
        });
}

fn results_hotspots(ui: &mut Ui, slot: &Slot<Res<HotspotHit>>) {
    card(ui, "Rings", |ui| {
        if !status(ui, slot) {
            return;
        }
        let Some(Ok(list)) = &slot.last else { return };
        if list.is_empty() {
            ui.label(RichText::new("No known hotspots in range yet.").color(pal::DIM));
        }
        for h in list {
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("{}×", h.count)).font(display(15.0)).color(pal::GOLD));
                ui.label(&h.body_name);
                ui.label(RichText::new(format!("{:.1} ly · {}", h.distance_ly, h.ring_type.clone().unwrap_or_default())).small().color(pal::DIM));
                if ui.small_button(icon::COPY).on_hover_text("Copy system").clicked() {
                    ui.ctx().copy_text(h.system.clone());
                }
            });
        }
    });
}

fn docked_market(ui: &mut Ui, gs: &GameState) {
    let Some(m) = &gs.market else {
        empty(ui, icon::STOREFRONT, "No market opened yet", "Open a station's commodity market — its full price list appears here.");
        return;
    };
    ui.label(RichText::new(format!("{} — {} · {}", m.station_name, m.star_system, m.timestamp.to_local_string("%d %b %H:%M"))).font(display(16.0)));
    let mut items: Vec<_> = m.items.iter().filter(|i| i.buy_price > 0 || i.sell_price > 0).collect();
    items.sort_by(|a, b| a.category_localised.cmp(&b.category_localised).then(a.name_localised.cmp(&b.name_localised)));
    TableBuilder::new(ui)
        .striped(true)
        .resizable(false)
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .column(Column::initial(240.0))
        .column(Column::initial(160.0))
        .column(Column::initial(90.0))
        .column(Column::initial(90.0))
        .column(Column::initial(90.0))
        .column(Column::remainder())
        .header(30.0, |mut h| {
            for t in ["Commodity", "Category", "Buy", "Sell", "Stock", "Demand"] {
                h.col(|ui| {
                    crate::widgets::th(ui, t);
                });
            }
        })
        .body(|body| {
            body.rows(32.0, items.len(), |mut row| {
                let i = items[row.index()];
                row.col(|ui| {
                    ui.label(i.name_localised.clone().unwrap_or_else(|| i.symbol()));
                });
                row.col(|ui| {
                    ui.label(RichText::new(i.category_localised.clone().unwrap_or_default()).color(pal::DIM));
                });
                row.col(|ui| {
                    if i.buy_price > 0 {
                        ui.label(RichText::new(nexus_data::names::group_digits(i.buy_price)).color(pal::ACCENT_SOFT));
                    }
                });
                row.col(|ui| {
                    if i.sell_price > 0 {
                        ui.label(RichText::new(nexus_data::names::group_digits(i.sell_price)).color(pal::GOLD));
                    }
                });
                row.col(|ui| {
                    ui.label(RichText::new(if i.stock > 0 { nexus_data::names::group_digits(i.stock) } else { String::new() }).color(pal::DIM));
                });
                row.col(|ui| {
                    ui.label(RichText::new(if i.demand > 0 { nexus_data::names::group_digits(i.demand) } else { String::new() }).color(pal::DIM));
                });
            });
        });
}

fn mb(bytes: u64) -> String {
    if bytes >= 1 << 30 { format!("{:.2} GB", bytes as f64 / (1u64 << 30) as f64) } else { format!("{:.1} MB", bytes as f64 / (1u64 << 20) as f64) }
}

fn database_tab(ui: &mut Ui, svc: &mut MarketService, st: &mut MarketUi, rt: &tokio::runtime::Handle) {
    let digits = |n: u64| nexus_data::names::group_digits(n as i64);
    crate::widgets::cards(ui, 2, 380.0, |c| {
      c.add(|ui| {
        card(ui, "Local galaxy database", |ui| match svc.db_stats().cloned() {
            Some(s) => {
                egui::Grid::new("dbstats").max_col_width(ui.available_width()).num_columns(2).spacing([16.0, 3.0]).show(ui, |ui| {
                    for (k, v) in [
                        ("Stations", digits(s.stations)),
                        ("Markets", digits(s.stations_with_market)),
                        ("Prices", digits(s.market_rows)),
                        ("Systems", digits(s.systems)),
                        ("Ring hotspots", digits(s.ring_signals)),
                        ("Faction entries", digits(s.faction_rows)),
                        ("Price history points", digits(s.price_history_rows)),
                        ("Size on disk", mb(s.db_bytes)),
                    ] {
                        ui.label(RichText::new(k).color(pal::DIM));
                        ui.label(RichText::new(v).font(display(14.0)));
                        ui.end_row();
                    }
                    ui.label(RichText::new("Newest market").color(pal::DIM));
                    match s.newest_market_update {
                        Some(t) => {
                            let (a, col) = age(t);
                            ui.label(RichText::new(format!("{a} ago")).color(col))
                        }
                        None => ui.label(RichText::new("none yet").color(pal::FAINT)),
                    };
                    ui.end_row();
                });
                ui.label(
                    RichText::new(format!("{} snapshots stored this session, {} older than what was already known", digits(s.market_snapshots), digits(s.stale_markets)))
                        .small().color(pal::DIM),
                );
            }
            None => {
                ui.label(RichText::new("No statistics yet.").color(pal::DIM));
            }
        });
        ui.add_space(8.0);
        card(ui, "Live feed (EDDN)", |ui| match svc.feed_snapshot() {
            Some(f) => {
                ui.horizontal(|ui| {
                    badge(ui, if f.connected { "Connected" } else { "Connecting" }, if f.connected { pal::GOOD } else { pal::WARN });
                    ui.label(RichText::new(format!("{} msgs/min", f.messages_per_minute)).font(display(15.0)).color(pal::ACCENT_SOFT));
                });
                ui.label(
                    RichText::new(format!(
                        "{} received · {} stored · {} duplicates · {} test messages skipped",
                        digits(f.total_messages),
                        digits(f.ingested),
                        digits(f.duplicates),
                        digits(f.test_dropped)
                    ))
                    .small()
                    .color(pal::DIM),
                );
                if f.decode_errors > 0 || f.dropped_backlog > 0 {
                    ui.label(RichText::new(format!("{} undecodable, {} dropped (backlog)", f.decode_errors, f.dropped_backlog)).small().color(pal::WARN));
                }
                if let Some(e) = &f.last_error {
                    ui.label(RichText::new(e).small().color(pal::BAD));
                }
                ui.add_space(4.0);
                for (schema, n) in f.per_schema.iter().take(8) {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(schema).small().color(pal::DIM));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(RichText::new(digits(*n)).small().color(pal::DIM));
                        });
                    });
                }
            }
            None => {
                ui.label(RichText::new("Off. Turn on \"Receive the live market feed\" in Accounts & sync.").color(pal::DIM));
            }
        });
      });
      c.add(|ui| {
        card(ui, "Baseline import (Spansh)", |ui| {
            ui.label(
                RichText::new(
                    "The live feed only sees markets that commanders visit while Elite Nexus runs. For a complete picture from day one, \
                     import Spansh's daily dump of every station: services, landing pads and the last known market of each.",
                )
                .color(pal::DIM),
            );
            ui.label(
                RichText::new(
                    "About 4.5 GB is streamed straight into the database (nothing else is saved; the database grows to roughly 1 GB). \
                     Takes from a few minutes to half an hour depending on your connection. The live feed keeps running meanwhile, \
                     and newer EDDN prices are never overwritten.",
                )
                .small().color(pal::DIM),
            );
            ui.add_space(6.0);
            let running = svc.import().is_some_and(|j| j.running());
            ui.add_enabled_ui(!running, |ui| {
                if crate::widgets::primary_button(ui, &format!("{} Download and import all stations", icon::DOWNLOAD_SIMPLE)).clicked() {
                    let ctx = ui.ctx().clone();
                    svc.start_import(ImportSource::Url(SPANSH_STATIONS_URL.into()), &ctx, rt);
                }
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(&mut st.import_path).hint_text(r"…or a dump you downloaded, e.g. D:\galaxy_stations.json.gz").desired_width(320.0));
                    if ui.button("Import file").clicked() && !st.import_path.trim().is_empty() {
                        let ctx = ui.ctx().clone();
                        let path = std::path::PathBuf::from(st.import_path.trim().trim_matches('"'));
                        svc.start_import(ImportSource::File(path), &ctx, rt);
                    }
                });
            });
            if let Some(job) = svc.import() {
                ui.add_space(6.0);
                import_progress(ui, job);
            }
        });
        ui.add_space(8.0);
        card(ui, "Where the data comes from", |ui| {
            for line in [
                "EDDN: every commander running EDMC, EDDiscovery, EDDI or Elite Nexus publishes station data the moment they dock. It arrives here in real time, usually before Inara or Spansh have processed it.",
                "Your own journal: every station you dock at and every market you open.",
                "Spansh dump: the daily baseline above.",
            ] {
                ui.add(egui::Label::new(RichText::new(format!("• {line}")).small().color(pal::DIM)).wrap());
            }
        });
      });
    });
}

fn import_progress(ui: &mut Ui, job: &ImportJob) {
    let digits = |n: u64| nexus_data::names::group_digits(n as i64);
    let s = job.status.lock();
    ui.label(RichText::new(format!("{} {}", icon::DATABASE, s.source)).small().color(pal::DIM));
    let p = &s.progress;
    let frac = s.total_bytes.map(|t| (p.bytes_read as f64 / t.max(1) as f64).clamp(0.0, 1.0) as f32);
    let elapsed = s.started.map(|t| t.elapsed().as_secs_f64()).unwrap_or(0.0);
    let eta = frac.filter(|f| *f > 0.01 && s.finished.is_none()).map(|f| elapsed * (1.0 - f as f64) / f as f64);
    let text = match (frac, eta) {
        (Some(f), Some(e)) => format!("{:.1}% · {} of {} · about {} left", f * 100.0, mb(p.bytes_read), mb(s.total_bytes.unwrap_or(0)), duration(e as i64)),
        (Some(f), None) => format!("{:.1}% · {}", f * 100.0, mb(p.bytes_read)),
        _ => mb(p.bytes_read),
    };
    bar(ui, frac.unwrap_or(0.0), if s.finished.is_some() { pal::GOOD } else { pal::ACCENT }, 12.0, Some(&text));
    ui.label(
        RichText::new(format!(
            "{} systems · {} stations · {} markets · {} prices · {} hotspots",
            digits(p.systems),
            digits(p.stations),
            digits(p.markets),
            digits(p.market_rows),
            digits(p.ring_signals)
        ))
        .small()
        .color(pal::DIM),
    );
    let finished = s.finished.clone();
    drop(s);
    match &finished {
        None => {
            if ui.button(format!("{} Cancel", icon::X_CIRCLE)).on_hover_text("What was imported so far is kept").clicked() {
                job.cancel();
            }
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(500));
        }
        Some(Ok(sum)) => {
            let what = if sum.cancelled { "Cancelled" } else if sum.stream_error.is_some() { "Stopped early" } else { "Done" };
            ui.label(RichText::new(format!("{} {what} after {}", icon::CHECK_CIRCLE, duration(sum.progress.elapsed.as_secs() as i64))).color(pal::GOOD));
            if sum.stale_markets > 0 {
                ui.label(RichText::new(format!("{} markets kept from newer live data", digits(sum.stale_markets))).small().color(pal::DIM));
            }
            if let Some(e) = &sum.stream_error {
                ui.label(RichText::new(format!("Download interrupted ({e}). Everything before it was imported; run it again to finish.")).small().color(pal::WARN));
            }
        }
        Some(Err(e)) => {
            ui.label(RichText::new(format!("{} {e}", icon::WARNING)).color(pal::BAD));
        }
    }
}

/// Where to sell what is in the hold: best single stops (everything they buy, limited by demand)
/// and the best buyers of each commodity. Re-planned whenever the hold or the system changes.
fn sell_tab(ui: &mut Ui, gs: &GameState, db: &GalaxyDb, st: &mut MarketUi) {
    let hold = finder::sellable_hold(gs);
    if hold.is_empty() {
        empty(ui, icon::PACKAGE, "Nothing to sell", "No sellable cargo in your hold (mission cargo, stolen goods and limpets are left out).");
        return;
    }
    let Some(origin) = gs.location.star_pos else {
        empty(ui, icon::MAP_PIN, "Position unknown", "Jump or relog once so Elite Nexus knows where you are.");
        return;
    };
    let tons: i64 = hold.iter().map(|l| l.2).sum();
    let pad = finder::ship_pad(gs);
    let mut search = false;
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(format!("{} {} t of {} commodities", icon::PACKAGE, nexus_data::names::group_digits(tons), hold.len())).font(display(15.0)));
        ui.add_space(12.0);
        ui.label("buyers within");
        search |= ui.add(egui::DragValue::new(&mut st.sell_radius).range(5.0..=MAX_SEARCH_LY).suffix(" ly")).changed();
        ui.label(RichText::new(format!("· {} pad · prices up to {:.0} h old", pad.unwrap_or('L'), st.max_age_h)).color(pal::DIM));
        search |= ui.button(RichText::new(format!("{} Refresh", icon::ARROWS_CLOCKWISE)).color(pal::ACCENT)).clicked();
    });
    let key = (gs.inventory.cargo.updated.0, tons, gs.location.system_address);
    if (search || key != st.sell_key) && !st.sell.busy() {
        st.sell_key = key;
        let (db, radius, age_h) = (db.clone(), st.sell_radius, st.max_age_h);
        st.sell.run(ui.ctx(), move || finder::sell_plan(&db, origin, &hold, pad, radius, age_h));
    }
    if st.sell.busy() && st.sell.last.is_none() {
        ui.spinner();
        return;
    }
    let plan = match &st.sell.last {
        Some(Ok(p)) => p,
        Some(Err(e)) => {
            ui.label(RichText::new(e).color(pal::BAD));
            return;
        }
        None => return,
    };
    if let Some(t) = st.sell.took {
        ui.label(RichText::new(format!("planned in {:.1} ms", t.as_secs_f64() * 1e3)).small().color(pal::DIM));
    }
    if plan.stops.is_empty() {
        ui.label(RichText::new("No known buyers in range. Widen the radius, or fill the database (Market → Database & feed).").color(pal::DIM));
        return;
    }
    card(ui, "Best single stop", |ui| {
        TableBuilder::new(ui)
            .id_salt("sellstops")
            .striped(true)
            .resizable(false)
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .vscroll(false)
            .column(Column::initial(250.0))
            .column(Column::initial(190.0))
            .column(Column::initial(70.0))
            .column(Column::initial(110.0))
            .column(Column::initial(90.0))
            .column(Column::remainder())
            .header(30.0, |mut h| {
                for t in ["Station", "System", "Distance", "Pays", "Buys", "Age"] {
                    h.col(|ui| {
                        crate::widgets::th(ui, t);
                    });
                }
            })
            .body(|body| {
                body.rows(32.0, plan.stops.len(), |mut row| {
                    let s = &plan.stops[row.index()];
                    let h = &s.hit;
                    row.col(|ui| {
                        ui.horizontal(|ui| {
                            if h.is_carrier {
                                ui.label(RichText::new(icon::ANCHOR).color(pal::CYAN)).on_hover_text("Fleet carrier");
                            } else if h.is_planetary {
                                ui.label(RichText::new(icon::PLANET).color(pal::DIM)).on_hover_text("Planetary");
                            }
                            ui.label(&h.station);
                            if let Some(ls) = h.dist_ls {
                                ui.label(RichText::new(format!("{ls:.0} ls")).small().color(pal::DIM));
                            }
                        });
                    });
                    row.col(|ui| {
                        if ui.link(&h.system).on_hover_text("Click to copy").clicked() {
                            ui.ctx().copy_text(h.system.clone());
                        }
                    });
                    row.col(|ui| {
                        ui.label(RichText::new(h.distance_ly.map(|d| format!("{d:.1} ly")).unwrap_or_default()).color(pal::DIM));
                    });
                    row.col(|ui| {
                        ui.label(RichText::new(short_credits(s.total)).font(display(14.0)).color(pal::GOLD));
                    });
                    row.col(|ui| {
                        let took: i64 = s.lines.iter().map(|l| l.1).sum();
                        let detail = s.lines.iter().map(|(n, t, p)| format!("{n}: {t} t × {} Cr", nexus_data::names::group_digits(*p))).collect::<Vec<_>>().join("\n");
                        ui.label(RichText::new(format!("{}/{} items · {} t", s.lines.len(), plan.per_item.len(), took)).color(pal::DIM)).on_hover_text(detail);
                    });
                    row.col(|ui| {
                        let (t, c) = age(h.updated_unix);
                        ui.label(RichText::new(t).color(c));
                    });
                });
            });
    });
    ui.add_space(8.0);
    card(ui, "Best buyer per commodity", |ui| {
        Grid::new("sellitems").num_columns(4).striped(true).spacing([14.0, 4.0]).show(ui, |ui| {
            for t in ["Commodity", "Tonnes", "Best price", "Where"] {
                ui.label(RichText::new(t).color(pal::DIM));
            }
            ui.end_row();
            for (name, count, hits) in &plan.per_item {
                ui.label(name);
                ui.label(RichText::new(nexus_data::names::group_digits(*count)).color(pal::DIM));
                match hits.first() {
                    Some(h) => {
                        ui.label(RichText::new(format!("{} Cr", nexus_data::names::group_digits(h.price))).font(display(14.0)).color(pal::GOLD));
                        let others = hits.iter().skip(1).map(|o| format!("{} Cr — {} / {}", nexus_data::names::group_digits(o.price), o.station, o.system)).collect::<Vec<_>>().join("\n");
                        let r = ui.label(
                            RichText::new(format!("{} — {} · {:.1} ly · demand {}", h.station, h.system, h.distance_ly.unwrap_or(0.0), nexus_data::names::group_digits(h.quantity)))
                                .color(pal::DIM),
                        );
                        if !others.is_empty() {
                            r.on_hover_text(others);
                        }
                    }
                    None => {
                        ui.label(RichText::new("no buyer in range").color(pal::FAINT));
                        ui.label("");
                    }
                }
                ui.end_row();
            }
        });
    });
}
fn describe_watch(w: &PriceWatch) -> String {
    let digits = nexus_data::names::group_digits;
    let mut s = format!("{} {} {} Cr", w.commodity, if w.sell { "sells for at least" } else { "can be bought for at most" }, digits(w.price));
    s += &if w.max_ly > 0.0 { format!(" within {:.0} ly", w.max_ly) } else { " anywhere".to_string() };
    if let Some(p) = w.min_pad {
        s += &format!(", {p} pad");
    }
    s += &format!(", {} {} t+", if w.sell { "demand" } else { "stock" }, digits(w.min_qty.max(1)));
    if w.carriers {
        s += ", carriers too";
    }
    s
}

/// Rules checked against every live market update. Returns true when the rules changed.
fn alerts_tab(ui: &mut Ui, svc: &MarketService, st: &mut MarketUi, watches: &mut Vec<PriceWatch>) -> bool {
    let mut changed = false;
    ui.label(
        RichText::new(
            "Every market update from the live feed (and every market you open) is checked against these rules the moment it arrives, \
             usually seconds after a commander docks. A rule fires at most once per station per hour, as a toast and, with voice on, spoken.",
        )
        .color(pal::DIM),
    );
    ui.add_space(6.0);
    card(ui, "New alert", |ui| {
        let w = &mut st.new_watch;
        ui.horizontal_wrapped(|ui| {
            ui.add(egui::TextEdit::singleline(&mut w.commodity).hint_text("Commodity").desired_width(190.0));
            crate::widgets::segmented(ui, &mut w.sell, &[(true, "sells for at least"), (false, "can be bought for at most")]);
            ui.add(egui::DragValue::new(&mut w.price).range(1..=100_000_000).speed(100.0).suffix(" Cr"));
            ui.label("within");
            ui.add(egui::DragValue::new(&mut w.max_ly).range(0.0..=20_000.0).suffix(" ly")).on_hover_text("0 = anywhere");
            pad_combo(ui, "wpad", &mut w.min_pad);
            ui.label(if w.sell { "min demand" } else { "min stock" });
            ui.add(egui::DragValue::new(&mut w.min_qty).range(1..=1_000_000).suffix(" t"));
            crate::widgets::toggle(ui, &mut w.carriers, "Fleet carriers");
        });
        let known = !w.commodity.trim().is_empty() && svc.resolves(w.commodity.trim());
        ui.horizontal(|ui| {
            if ui.add_enabled(known, egui::Button::new(RichText::new(format!("{} Add alert", icon::BELL_RINGING)).color(pal::ACCENT))).clicked() {
                let mut rule = w.clone();
                rule.commodity = rule.commodity.trim().to_string();
                rule.enabled = true;
                watches.push(rule);
                changed = true;
            }
            if !known && !w.commodity.trim().is_empty() {
                ui.label(RichText::new("Unknown commodity").color(pal::BAD));
            }
        });
    });
    ui.add_space(8.0);
    card(ui, "Your alerts", |ui| {
        if watches.is_empty() {
            ui.label(RichText::new("No alerts yet. Example: Tritium sells for at least 60,000 Cr within 50 ly.").color(pal::FAINT));
            return;
        }
        let mut remove = None;
        for (i, w) in watches.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                changed |= crate::widgets::switch(ui, &mut w.enabled).on_hover_text("Enabled").changed();
                let col = if w.enabled { pal::TEXT } else { pal::FAINT };
                ui.label(RichText::new(describe_watch(w)).color(col));
                if !svc.resolves(&w.commodity) {
                    ui.label(RichText::new("unknown commodity").small().color(pal::BAD));
                }
                if ui.small_button(icon::TRASH).on_hover_text("Delete").clicked() {
                    remove = Some(i);
                }
            });
        }
        if let Some(i) = remove {
            watches.remove(i);
            changed = true;
        }
    });
    if svc.feed_state().is_none() {
        ui.add_space(6.0);
        ui.label(RichText::new(format!("{} The live feed is off, so only markets you open yourself are checked. Turn on \"Receive the live market feed\" in Accounts & sync.", icon::WARNING)).color(pal::WARN));
    }
    changed
}
