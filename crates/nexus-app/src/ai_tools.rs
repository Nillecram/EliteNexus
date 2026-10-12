//! The assistant's tools: thin wrappers over the app's own data (game model, galaxy database,
//! journal, Spansh). Each returns compact JSON for the model.

use crate::finder;
use crate::pages::navigation::{PlannedRoute, from_spansh};
use nexus_ai::{ToolHost, ToolSpec};
use nexus_data::names::compact_credits;
use nexus_galaxy::{
    CommodityQuery, DealQuery, GalaxyDb, HotspotQuery, MaterialTrader, Services, Sort, StationQuery, TechBroker, TradeMode, TradeQuery,
};
use nexus_journal::JournalIndex;
use nexus_model::{GameState, Model};
use nexus_spansh::{ExobiologyRouteRequest, GalaxyRouteRequest, NeutronRouteRequest, RichesRouteRequest, RouteWaypoints, ShipProfile, Spansh};
use parking_lot::{Mutex, RwLock};
use serde_json::{Value, json};
use std::sync::Arc;
use tokio::runtime::Handle;

pub struct NexusTools {
    pub model: Arc<RwLock<Model>>,
    pub db: Option<GalaxyDb>,
    pub index: Arc<RwLock<JournalIndex>>,
    pub spansh: Spansh,
    pub rt: Handle,
    /// Routes the assistant planned, picked up by the Navigation page.
    pub routes: Arc<Mutex<Option<PlannedRoute>>>,
}

/// The commander facts the tools need, read under one short lock.
struct Me {
    pos: Option<[f64; 3]>,
    system: String,
    pad: Option<char>,
    cargo_capacity: i64,
    credits: i64,
    jump_range: f64,
    loadout: Option<String>,
}

fn arg_str(a: &Value, k: &str) -> Option<String> {
    a.get(k).and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty()).map(str::to_string)
}

fn arg_f64(a: &Value, k: &str) -> Option<f64> {
    a.get(k).and_then(|v| v.as_f64().or_else(|| v.as_str().and_then(|s| s.trim().parse().ok()))).filter(|v| v.is_finite())
}

fn arg_bool(a: &Value, k: &str) -> Option<bool> {
    a.get(k).and_then(Value::as_bool)
}

fn hours_ago(unix: i64) -> f64 {
    let h = (nexus_journal::Timestamp::now().0 - unix).max(0) as f64 / 3600.0;
    (h * 10.0).round() / 10.0
}

fn r1(x: f64) -> f64 {
    (x * 10.0).round() / 10.0
}

fn schema(props: Value, required: &[&str]) -> Value {
    json!({ "type": "object", "properties": props, "required": required })
}

const NEAR: &str = "System to search around; default: the commander's current system.";

impl NexusTools {
    fn db(&self) -> Result<&GalaxyDb, String> {
        self.db.as_ref().ok_or_else(|| "The market database is unavailable.".to_string())
    }

    fn me(&self) -> Result<Me, String> {
        let model = self.model.read();
        let gs = model.active().ok_or("No commander loaded yet.")?;
        let ship = gs.fleet.current();
        Ok(Me {
            pos: gs.location.star_pos,
            system: gs.location.system.clone(),
            pad: finder::ship_pad(gs),
            cargo_capacity: ship.map(|s| i64::from(s.cargo_capacity)).filter(|c| *c > 0).unwrap_or(0),
            credits: gs.cmdr.credits,
            jump_range: ship.map(|s| s.max_jump_range).unwrap_or(0.0),
            loadout: ship.and_then(|s| s.loadout_json.clone()),
        })
    }

    /// Position and name of `near_system` (local database, then Spansh), or the commander's.
    fn origin(&self, args: &Value, me: &Me) -> Result<([f64; 3], String), String> {
        if let Some(name) = arg_str(args, "near_system").filter(|n| !n.eq_ignore_ascii_case(&me.system)) {
            if let Some(db) = &self.db
                && let Ok(Some(s)) = db.system_by_name(&name)
                && let Some(p) = s.pos
            {
                return Ok((p, s.name));
            }
            let found = self.rt.block_on(self.spansh.system_by_name(&name)).map_err(|e| format!("Spansh: {e}"))?;
            return found.map(|s| ([s.x, s.y, s.z], s.name)).ok_or_else(|| format!("Unknown system \"{name}\"."));
        }
        me.pos.map(|p| (p, me.system.clone())).ok_or_else(|| "The commander's position is not known yet.".to_string())
    }

    fn commander(&self) -> Result<Value, String> {
        let model = self.model.read();
        let gs: &GameState = model.active().ok_or("No commander loaded yet.")?;
        let loc = &gs.location;
        let ship = gs.fleet.current();
        let cargo: Vec<Value> = gs.inventory.cargo.items.iter().map(|c| json!({ "name": c.name, "tonnes": c.count, "stolen": c.stolen, "mission": c.mission_id.is_some() })).collect();
        let carrier = gs.carrier.owned().then(|| {
            json!({
                "name": gs.carrier.name, "callsign": gs.carrier.callsign, "system": gs.carrier.system,
                "tritium_t": gs.carrier.fuel, "cargo_t": gs.carrier.space.cargo, "capacity_t": gs.carrier.space.total_capacity,
            })
        });
        Ok(json!({
            "commander": gs.cmdr.name,
            "credits": gs.cmdr.credits,
            "credits_text": compact_credits(gs.cmdr.credits),
            "location": {
                "system": loc.system, "coords": loc.star_pos, "body": loc.body,
                "station": loc.station.as_ref().filter(|_| loc.docked).map(|s| &s.name),
                "situation": loc.situation(),
                "population": loc.info.population, "economy": loc.info.economy, "security": loc.info.security,
                "controlling_faction": loc.info.controlling_faction,
            },
            "ship": ship.map(|s| json!({
                "type": s.type_name, "name": s.name, "landing_pad": finder::ship_pad(gs).map(String::from),
                "cargo_capacity_t": s.cargo_capacity, "cargo_used_t": gs.inventory.cargo.total(),
                "max_jump_range_ly": r1(s.max_jump_range), "fuel_t": r1(gs.fleet.fuel_main), "fuel_capacity_t": s.fuel_main_capacity,
                "rebuy": s.rebuy,
            })),
            "cargo": cargo,
            "fleet_carrier": carrier,
            "ranks": nexus_data::ranks::RankKind::ALL.iter().map(|k| json!({ "rank": format!("{k:?}"), "name": gs.cmdr.rank_name(*k) })).collect::<Vec<_>>(),
            "unsold_exploration_data_cr": gs.exploration.unsold_value(),
            "unsold_exobiology_data_cr": gs.exobio.unsold_value(),
        }))
    }

    fn find_commodity(&self, a: &Value) -> Result<Value, String> {
        let me = self.me()?;
        let (origin, near) = self.origin(a, &me)?;
        let commodity = arg_str(a, "commodity").ok_or("commodity is required")?;
        let sell = arg_str(a, "mode").is_some_and(|m| m.eq_ignore_ascii_case("sell"));
        let q = CommodityQuery {
            commodity: commodity.clone(),
            mode: if sell { TradeMode::SellToStation } else { TradeMode::BuyFromStation },
            origin: Some(origin),
            max_distance_ly: Some(arg_f64(a, "max_distance_ly").unwrap_or(100.0)),
            min_pad: me.pad,
            max_age_hours: Some(arg_f64(a, "max_age_hours").unwrap_or(72.0)),
            min_quantity: arg_f64(a, "min_quantity").map(|v| v as i64).unwrap_or(1),
            include_carriers: arg_bool(a, "include_carriers").unwrap_or(true),
            include_planetary: true,
            limit: arg_f64(a, "limit").map(|v| v as usize).unwrap_or(8).clamp(1, 25),
            sort: if arg_str(a, "sort").is_some_and(|s| s.eq_ignore_ascii_case("distance")) { Sort::Nearest } else { Sort::BestPrice },
        };
        let hits = self.db()?.search_commodity(&q).map_err(|e| e.to_string())?;
        Ok(json!({
            "commodity": commodity, "near": near, "mode": if sell { "stations buying (you sell)" } else { "stations selling (you buy)" },
            "results": hits.iter().map(|h| json!({
                "station": h.station, "system": h.system, "distance_ly": h.distance_ly.map(r1), "price": h.price,
                "quantity": h.quantity, "pad": h.max_pad.map(String::from), "fleet_carrier": h.is_carrier,
                "arrival_distance_ls": h.dist_ls.map(|d| d.round()), "age_hours": hours_ago(h.updated_unix),
            })).collect::<Vec<_>>(),
        }))
    }

    fn price_deals(&self, a: &Value) -> Result<Value, String> {
        let me = self.me()?;
        let (origin, near) = self.origin(a, &me)?;
        let buy = arg_str(a, "mode").is_some_and(|m| m.eq_ignore_ascii_case("buy"));
        let cargo = me.cargo_capacity.max(1);
        let q = DealQuery {
            mode: if buy { TradeMode::BuyFromStation } else { TradeMode::SellToStation },
            origin: Some(origin),
            max_distance_ly: Some(arg_f64(a, "max_distance_ly").unwrap_or(80.0)),
            min_ratio: arg_f64(a, "min_ratio").unwrap_or(1.3),
            min_pad: me.pad,
            include_carriers: arg_bool(a, "include_carriers").unwrap_or(true),
            max_age_hours: Some(48.0),
            cargo_capacity: cargo,
            limit: arg_f64(a, "limit").map(|v| v as usize).unwrap_or(10).clamp(1, 25),
        };
        let deals = self.db()?.price_deals(&q).map_err(|e| e.to_string())?;
        Ok(json!({
            "near": near, "cargo_capacity_t": cargo,
            "mode": if buy { "stations selling far below the galactic average" } else { "stations paying far above the galactic average" },
            "deals": deals.iter().map(|d| json!({
                "commodity": d.commodity_name, "station": d.station, "system": d.system, "distance_ly": d.distance_ly.map(r1),
                "price": d.price, "galactic_average": d.average, "percent_better": ((d.ratio - 1.0) * 100.0).round(),
                "quantity": d.quantity, "credits_better_than_average_per_hold": d.extra_value, "credits_for_one_hold": d.trade_value,
                "pad": d.max_pad.map(String::from), "fleet_carrier": d.is_carrier, "age_hours": hours_ago(d.updated_unix),
            })).collect::<Vec<_>>(),
        }))
    }

    fn sell_my_cargo(&self, a: &Value) -> Result<Value, String> {
        let me = self.me()?;
        let (origin, near) = self.origin(a, &me)?;
        let hold = {
            let model = self.model.read();
            finder::sellable_hold(model.active().ok_or("No commander loaded yet.")?)
        };
        if hold.is_empty() {
            return Ok(json!({ "message": "The hold has no sellable cargo (mission cargo, stolen goods and limpets are left out)." }));
        }
        let plan = finder::sell_plan(self.db()?, origin, &hold, me.pad, arg_f64(a, "max_distance_ly").unwrap_or(60.0), 72.0)?;
        Ok(json!({
            "near": near,
            "hold": hold.iter().map(|(_, n, t)| json!({ "name": n, "tonnes": t })).collect::<Vec<_>>(),
            "best_single_stops": plan.stops.iter().take(5).map(|s| json!({
                "station": s.hit.station, "system": s.hit.system, "distance_ly": s.hit.distance_ly.map(r1), "total_credits": s.total,
                "buys": s.lines.iter().map(|(n, t, p)| json!({ "name": n, "tonnes": t, "price": p })).collect::<Vec<_>>(),
                "fleet_carrier": s.hit.is_carrier, "age_hours": hours_ago(s.hit.updated_unix),
            })).collect::<Vec<_>>(),
            "best_buyer_per_item": plan.per_item.iter().map(|(n, t, hits)| json!({
                "name": n, "tonnes": t,
                "best": hits.first().map(|h| json!({ "station": h.station, "system": h.system, "price": h.price, "demand": h.quantity, "distance_ly": h.distance_ly.map(r1) })),
            })).collect::<Vec<_>>(),
        }))
    }

    fn trade_routes(&self, a: &Value) -> Result<Value, String> {
        let me = self.me()?;
        let (origin, near) = self.origin(a, &me)?;
        let q = TradeQuery {
            origin,
            radius_ly: arg_f64(a, "radius_ly").unwrap_or(40.0),
            max_hop_ly: arg_f64(a, "max_hop_ly").unwrap_or(25.0),
            min_pad: me.pad,
            cargo_capacity: me.cargo_capacity.max(1),
            capital: Some(me.credits.max(0)),
            max_age_hours: Some(48.0),
            include_carriers: false,
            include_planetary: true,
            limit: arg_f64(a, "limit").map(|v| v as usize).unwrap_or(6).clamp(1, 20),
            ..Default::default()
        };
        let routes = self.db()?.trade_routes(&q).map_err(|e| e.to_string())?;
        Ok(json!({
            "near": near, "cargo_capacity_t": q.cargo_capacity,
            "routes": routes.iter().map(|r| json!({
                "commodity": r.commodity_name, "profit_per_ton": r.profit_per_ton, "tonnes": r.units, "profit_per_trip": r.total_profit,
                "buy_at": { "station": r.from.station, "system": r.from.system, "price": r.buy_price, "distance_ly": r1(r.from.distance_from_origin_ly), "age_hours": hours_ago(r.from.market_updated_unix) },
                "sell_at": { "station": r.to.station, "system": r.to.system, "price": r.sell_price, "distance_ly": r1(r.to.distance_from_origin_ly), "age_hours": hours_ago(r.to.market_updated_unix) },
                "hop_ly": r1(r.hop_ly),
            })).collect::<Vec<_>>(),
        }))
    }

    fn nearest_service(&self, a: &Value) -> Result<Value, String> {
        let me = self.me()?;
        let (origin, near) = self.origin(a, &me)?;
        let service = arg_str(a, "service").ok_or("service is required")?;
        let mut q = StationQuery { origin, min_pad: me.pad, limit: arg_f64(a, "limit").map(|v| v as usize).unwrap_or(5).clamp(1, 15), ..Default::default() };
        match service.as_str() {
            "raw_material_trader" => q.material_trader = Some(MaterialTrader::Raw),
            "manufactured_material_trader" => q.material_trader = Some(MaterialTrader::Manufactured),
            "encoded_material_trader" => q.material_trader = Some(MaterialTrader::Encoded),
            "human_tech_broker" => q.tech_broker = Some(TechBroker::Human),
            "guardian_tech_broker" => q.tech_broker = Some(TechBroker::Guardian),
            "interstellar_factors" => q.services = Services::INTERSTELLAR_FACTORS,
            "universal_cartographics" => q.services = Services::UNIVERSAL_CARTOGRAPHICS,
            "vista_genomics" => q.services = Services::VISTA_GENOMICS,
            "shipyard" => q.services = Services::SHIPYARD,
            "outfitting" => q.services = Services::OUTFITTING,
            "black_market" => q.services = Services::BLACK_MARKET,
            "refuel" => q.services = Services::REFUEL,
            "repair" => q.services = Services::REPAIR,
            other => return Err(format!("Unknown service \"{other}\".")),
        }
        let hits = self.db()?.stations_near(&q).map_err(|e| e.to_string())?;
        Ok(json!({
            "service": service, "near": near,
            "stations": hits.iter().map(|h| json!({
                "station": h.station.name, "system": h.station.system, "distance_ly": r1(h.distance_ly),
                "arrival_distance_ls": h.station.dist_ls.map(|d| d.round()), "pad": h.station.max_pad.map(String::from),
                "type": h.station.station_type,
            })).collect::<Vec<_>>(),
            "note": "Material trader and technology broker types are inferred from the station's economy.",
        }))
    }

    fn mining_hotspots(&self, a: &Value) -> Result<Value, String> {
        let me = self.me()?;
        let (origin, near) = self.origin(a, &me)?;
        let commodity = arg_str(a, "commodity").ok_or("commodity is required")?;
        let radius = arg_f64(a, "max_distance_ly").unwrap_or(100.0);
        let db = self.db()?;
        let spots = db
            .hotspots_near(&HotspotQuery { origin, signal: commodity.clone(), max_distance_ly: Some(radius), min_count: 1, limit: arg_f64(a, "limit").map(|v| v as usize).unwrap_or(6).clamp(1, 15) })
            .map_err(|e| e.to_string())?;
        let sells = db
            .search_commodity(&CommodityQuery {
                commodity: commodity.clone(),
                mode: TradeMode::SellToStation,
                origin: Some(origin),
                max_distance_ly: Some(radius * 2.0),
                min_pad: me.pad,
                max_age_hours: Some(72.0),
                limit: 3,
                ..Default::default()
            })
            .map_err(|e| e.to_string())?;
        Ok(json!({
            "commodity": commodity, "near": near,
            "hotspots": spots.iter().map(|h| json!({ "ring": h.body_name, "system": h.system, "hotspots": h.count, "distance_ly": r1(h.distance_ly), "ring_type": h.ring_type })).collect::<Vec<_>>(),
            "best_sell_prices": sells.iter().map(|h| json!({ "station": h.station, "system": h.system, "price": h.price, "demand": h.quantity, "distance_ly": h.distance_ly.map(r1), "age_hours": hours_ago(h.updated_unix) })).collect::<Vec<_>>(),
            "note": if spots.is_empty() { "No hotspots known nearby yet: they come from ring scans shared over EDDN and the Spansh import." } else { "" },
        }))
    }

    fn system_info(&self, a: &Value) -> Result<Value, String> {
        let name = arg_str(a, "system").ok_or("system is required")?;
        let db = self.db()?;
        let Some(s) = db.system_by_name(&name).map_err(|e| e.to_string())? else {
            return Err(format!("\"{name}\" is not in the local database yet."));
        };
        let stations = db.stations_in_system(s.id64).map_err(|e| e.to_string())?;
        let factions = db.system_factions(s.id64).map_err(|e| e.to_string())?;
        let me = self.me().ok();
        let dist = me.and_then(|m| m.pos).zip(s.pos).map(|(a, b)| r1(((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()));
        Ok(json!({
            "system": s.name, "distance_ly": dist, "population": s.population, "allegiance": s.allegiance, "government": s.government,
            "economy": s.economy, "second_economy": s.second_economy, "security": s.security,
            "controlling_faction": s.controlling_faction, "faction_state": s.controlling_faction_state,
            "power": s.controlling_power, "powerplay_state": s.powerplay_state,
            "stations": stations.iter().take(25).map(|st| json!({
                "name": st.name, "type": st.station_type, "pad": st.max_pad.map(String::from), "arrival_distance_ls": st.dist_ls.map(|d| d.round()),
                "economy": st.economy, "faction": st.faction, "fleet_carrier": st.is_carrier,
            })).collect::<Vec<_>>(),
            "factions": factions.iter().map(|f| json!({ "name": f.name, "influence": f.influence.map(|i| (i * 1000.0).round() / 10.0), "state": f.state })).collect::<Vec<_>>(),
        }))
    }

    fn plan_route(&self, a: &Value) -> Result<Value, String> {
        let me = self.me()?;
        let to = arg_str(a, "to").ok_or("to is required")?;
        let from = arg_str(a, "from").unwrap_or_else(|| me.system.clone());
        let neutron = arg_str(a, "kind").is_some_and(|k| k.eq_ignore_ascii_case("neutron"));
        let planned = if neutron || me.loadout.is_none() {
            if me.jump_range <= 0.0 {
                return Err("The ship's jump range is not known yet (fly it once).".into());
            }
            let req = NeutronRouteRequest::new(from.clone(), to.clone(), me.jump_range).efficiency(60);
            let p = self.rt.block_on(self.spansh.neutron_route(&req)).map_err(|e| format!("Spansh: {e}"))?;
            let w: Vec<_> = p.route.waypoints().iter().map(from_spansh).collect();
            PlannedRoute { kind: "Neutron", summary: format!("{} waypoints · {} jumps · {} neutron boosts", w.len(), p.route.total_jumps(), p.route.neutron_count()), web_url: p.web_url, waypoints: w }
        } else {
            let profile = ShipProfile::from_loadout_json(me.loadout.as_deref().unwrap_or_default()).map_err(|e| format!("Ship profile: {e:?}"))?;
            let cargo = {
                let model = self.model.read();
                model.active().map(|g| g.inventory.cargo.total().max(0) as u32).unwrap_or(0)
            };
            let req = GalaxyRouteRequest::new(from.as_str(), to.as_str(), &profile).cargo(cargo);
            let p = self.rt.block_on(self.spansh.galaxy_route(&req)).map_err(|e| format!("Spansh: {e}"))?;
            let w: Vec<_> = p.route.waypoints().iter().map(from_spansh).collect();
            PlannedRoute {
                kind: "Exact",
                summary: format!(
                    "{} jumps · {:.0} ly · {} refuels · {} neutron boosts",
                    p.route.jump_count(),
                    p.route.distance(),
                    p.route.refuel_stops(),
                    p.route.neutron_boosts()
                ),
                web_url: p.web_url,
                waypoints: w,
            }
        };
        let out = json!({
            "from": from, "to": to, "planner": planned.kind, "summary": planned.summary, "spansh_url": planned.web_url,
            "first_waypoints": planned.waypoints.iter().take(12).map(|w| json!({ "system": w.system, "note": w.note })).collect::<Vec<_>>(),
            "waypoint_count": planned.waypoints.len(),
            "note": "The route is now shown on the Navigation page and the next system is copied to the clipboard after each jump.",
        });
        *self.routes.lock() = Some(planned);
        Ok(out)
    }

    fn money_routes(&self, a: &Value) -> Result<Value, String> {
        let me = self.me()?;
        let from = arg_str(a, "near_system").unwrap_or_else(|| me.system.clone());
        let range = if me.jump_range > 0.0 { me.jump_range } else { 20.0 };
        let radius = arg_f64(a, "radius_ly").unwrap_or(30.0).clamp(5.0, 500.0) as u32;
        let count = arg_f64(a, "max_systems").unwrap_or(15.0).clamp(1.0, 50.0) as u32;
        let kind = arg_str(a, "kind").unwrap_or_else(|| "exobiology".into()).to_lowercase();
        let (label, plotted) = if kind.contains("exo") || kind.contains("bio") {
            let req = ExobiologyRouteRequest::new(from.clone(), range).radius(radius).max_results(count).min_value(1_000_000);
            ("Exobiology", self.rt.block_on(self.spansh.exobiology_route(&req)))
        } else {
            let base = match kind.as_str() {
                "earth_likes" | "earth-likes" | "elw" => RichesRouteRequest::earth_likes(from.clone(), range),
                "ammonia_worlds" | "ammonia" => RichesRouteRequest::ammonia_worlds(from.clone(), range),
                _ => RichesRouteRequest::road_to_riches(from.clone(), range),
            };
            ("Road to riches", self.rt.block_on(self.spansh.riches_route(&base.radius(radius).max_results(count).min_value(300_000).use_mapping_value(true))))
        };
        let p = plotted.map_err(|e| format!("Spansh: {e}"))?;
        let w: Vec<_> = p.route.waypoints().iter().map(from_spansh).collect();
        let total: u64 = w.iter().filter_map(|x| x.value).sum();
        let out = json!({
            "kind": label, "from": from, "systems": w.len(), "total_value_cr": total, "total_value_text": compact_credits(total as i64), "spansh_url": p.web_url,
            "stops": w.iter().take(20).map(|x| json!({ "system": x.system, "value_cr": x.value, "bodies": x.note })).collect::<Vec<_>>(),
            "note": "Values assume the first discovery/mapping or first-logged bonuses where Spansh expects them. The route is now on the Navigation page.",
        });
        *self.routes.lock() = Some(PlannedRoute { kind: label, summary: format!("{} systems · ~{} Cr", w.len(), compact_credits(total as i64)), web_url: p.web_url, waypoints: w });
        Ok(out)
    }

    fn exobiology_here(&self) -> Result<Value, String> {
        let model = self.model.read();
        let gs = model.active().ok_or("No commander loaded yet.")?;
        let Some(sys) = gs.exploration.systems.get(&gs.location.system_address) else {
            return Ok(json!({ "message": "No scans of the current system yet: FSS-scan it first." }));
        };
        let bodies: Vec<Value> = sys
            .bodies
            .values()
            .filter(|b| b.bio_signals > 0)
            .map(|b| {
                json!({
                    "body": b.name, "biological_signals": b.bio_signals, "distance_ls": b.distance_ls.round(),
                    "genera_from_dss": b.genuses,
                    "sampled": b.organics.iter().map(|o| json!({ "species": o.species, "samples": o.samples, "analysed": o.analysed })).collect::<Vec<_>>(),
                    "likely_species": b.likely_species().iter().take(8).map(|p| json!({ "species": p.species, "value_cr": p.value, "likelihood_pct": (p.likelihood * 100.0).round(), "colony_range_m": p.colony_range_m })).collect::<Vec<_>>(),
                    "expected_value_cr": b.predicted_value(b.was_footfalled == Some(false)),
                })
            })
            .collect();
        Ok(json!({ "system": sys.name, "bio_bodies": bodies, "unsold_exobiology_data_cr": gs.exobio.unsold_value() }))
    }

    fn journal_search(&self, a: &Value) -> Result<Value, String> {
        let needle = arg_str(a, "query").ok_or("query is required")?.to_lowercase();
        let limit = arg_f64(a, "limit").map(|v| v as usize).unwrap_or(15).clamp(1, 40);
        let files = self.index.read().files.clone();
        let mut hits = Vec::new();
        'files: for path in files.iter().rev() {
            let Ok(text) = std::fs::read_to_string(path) else { continue };
            for line in text.lines().rev() {
                if line.to_lowercase().contains(&needle) {
                    let v: Value = serde_json::from_str(line).unwrap_or(Value::Null);
                    let mut snippet: String = line.chars().take(400).collect();
                    if line.len() > snippet.len() {
                        snippet.push('…');
                    }
                    hits.push(json!({ "time": v["timestamp"], "event": v["event"], "line": snippet }));
                    if hits.len() >= limit {
                        break 'files;
                    }
                }
            }
        }
        Ok(json!({ "query": needle, "matches_newest_first": hits }))
    }
}

impl ToolHost for NexusTools {
    fn tools(&self) -> Vec<ToolSpec> {
        specs()
    }

    fn call(&self, name: &str, args: &Value) -> Result<Value, String> {
        match name {
            "commander" => self.commander(),
            "find_commodity" => self.find_commodity(args),
            "price_deals" => self.price_deals(args),
            "sell_my_cargo" => self.sell_my_cargo(args),
            "trade_routes" => self.trade_routes(args),
            "nearest_service" => self.nearest_service(args),
            "mining_hotspots" => self.mining_hotspots(args),
            "system_info" => self.system_info(args),
            "plan_route" => self.plan_route(args),
            "money_routes" => self.money_routes(args),
            "exobiology_here" => self.exobiology_here(),
            "journal_search" => self.journal_search(args),
            "knowledge" => {
                let q = arg_str(args, "query").ok_or("query is required")?;
                let hits = nexus_ai::knowledge::search(&q, 4);
                Ok(json!({ "sections": hits.into_iter().map(|(t, x)| json!({ "title": t, "text": x })).collect::<Vec<_>>() }))
            }
            other => Err(format!("Unknown tool \"{other}\".")),
        }
    }
}

/// The tool list sent to the model. Kept stable (providers cache it).
pub fn specs() -> Vec<ToolSpec> {
    vec![
        ToolSpec {
            name: "commander",
            description: "The commander's current state: location, docked station, ship (type, landing pad, cargo capacity, jump range, fuel, rebuy), cargo hold, credits, fleet carrier, ranks, unsold exploration and exobiology data.",
            parameters: schema(json!({}), &[]),
        },
        ToolSpec {
            name: "find_commodity",
            description: "Where to buy or sell one commodity near a system, from the live market database (EDDN, refreshed in real time). Results respect the ship's landing pad size.",
            parameters: schema(
                json!({
                    "commodity": { "type": "string", "description": "Commodity name, e.g. Tritium, Gold, Low Temperature Diamonds." },
                    "mode": { "type": "string", "enum": ["buy", "sell"], "description": "buy = stations selling it (you buy); sell = stations buying it (you sell)." },
                    "near_system": { "type": "string", "description": NEAR },
                    "max_distance_ly": { "type": "number", "description": "Search radius, default 100." },
                    "min_quantity": { "type": "integer", "description": "Minimum stock (buy) or demand (sell)." },
                    "sort": { "type": "string", "enum": ["price", "distance"] },
                    "include_carriers": { "type": "boolean" },
                    "limit": { "type": "integer" }
                }),
                &["commodity"],
            ),
        },
        ToolSpec {
            name: "price_deals",
            description: "Best deals right now: stations paying far above the galactic average for any commodity (mode sell) or selling far below it (mode buy), ranked by what one full hold of the commander's ship earns. The fastest way to find unusually profitable trades, including fleet-carrier buy orders.",
            parameters: schema(
                json!({
                    "mode": { "type": "string", "enum": ["sell", "buy"] },
                    "near_system": { "type": "string", "description": NEAR },
                    "max_distance_ly": { "type": "number", "description": "Default 80." },
                    "min_ratio": { "type": "number", "description": "How much better than average, e.g. 1.3 = 30 % (default)." },
                    "include_carriers": { "type": "boolean" },
                    "limit": { "type": "integer" }
                }),
                &[],
            ),
        },
        ToolSpec {
            name: "sell_my_cargo",
            description: "Where to sell everything currently in the hold: the best single stations (limited by their demand) and the best buyer for each item.",
            parameters: schema(json!({ "max_distance_ly": { "type": "number", "description": "Default 60." }, "near_system": { "type": "string", "description": NEAR } }), &[]),
        },
        ToolSpec {
            name: "trade_routes",
            description: "Best buy-here-sell-there trade routes near a system, sized to the ship's cargo capacity and the commander's credits.",
            parameters: schema(
                json!({
                    "near_system": { "type": "string", "description": NEAR },
                    "radius_ly": { "type": "number", "description": "Both stations within this distance, default 40." },
                    "max_hop_ly": { "type": "number", "description": "Maximum distance between the two stations, default 25." },
                    "limit": { "type": "integer" }
                }),
                &[],
            ),
        },
        ToolSpec {
            name: "nearest_service",
            description: "Nearest stations with a service, for the commander's landing pad size.",
            parameters: schema(
                json!({
                    "service": { "type": "string", "enum": ["raw_material_trader", "manufactured_material_trader", "encoded_material_trader", "human_tech_broker", "guardian_tech_broker", "interstellar_factors", "universal_cartographics", "vista_genomics", "shipyard", "outfitting", "black_market", "refuel", "repair"] },
                    "near_system": { "type": "string", "description": NEAR },
                    "limit": { "type": "integer" }
                }),
                &["service"],
            ),
        },
        ToolSpec {
            name: "mining_hotspots",
            description: "Known planetary-ring hotspots for a mineable commodity near a system, plus the best nearby places to sell it.",
            parameters: schema(
                json!({
                    "commodity": { "type": "string", "description": "e.g. Platinum, Painite, Low Temperature Diamonds, Void Opal, Tritium." },
                    "near_system": { "type": "string", "description": NEAR },
                    "max_distance_ly": { "type": "number", "description": "Default 100." },
                    "limit": { "type": "integer" }
                }),
                &["commodity"],
            ),
        },
        ToolSpec {
            name: "system_info",
            description: "Facts about a star system from the local database: distance from the commander, population, economy, security, factions and their states, powerplay, stations.",
            parameters: schema(json!({ "system": { "type": "string" } }), &["system"]),
        },
        ToolSpec {
            name: "plan_route",
            description: "Plans a route with Spansh: the exact plotter uses the commander's real ship (fuel, scooping, neutron boosts); the neutron plotter only needs the jump range. The route is put on the Navigation page and the next system is copied to the clipboard after every jump.",
            parameters: schema(
                json!({
                    "to": { "type": "string", "description": "Destination system." },
                    "from": { "type": "string", "description": "Start system; default: current." },
                    "kind": { "type": "string", "enum": ["exact", "neutron"] }
                }),
                &["to"],
            ),
        },
        ToolSpec {
            name: "money_routes",
            description: "Plans a money-making exploration trip with Spansh near a system: valuable exobiology targets, or road-to-riches bodies (earth-likes, water/ammonia worlds, terraformables). Returns stops with values and the total; the route goes to the Navigation page.",
            parameters: schema(
                json!({
                    "kind": { "type": "string", "enum": ["exobiology", "road_to_riches", "earth_likes", "ammonia_worlds"] },
                    "near_system": { "type": "string", "description": NEAR },
                    "radius_ly": { "type": "number", "description": "Default 30." },
                    "max_systems": { "type": "integer", "description": "Default 15." }
                }),
                &[],
            ),
        },
        ToolSpec {
            name: "exobiology_here",
            description: "Bodies with biological signals in the current system: genera found, species already sampled, likely species with their values (predictions), and unsold exobiology data.",
            parameters: schema(json!({}), &[]),
        },
        ToolSpec {
            name: "journal_search",
            description: "Searches the commander's own game journal history (every event since they started playing), newest first. Use for questions about past trips, sales, missions, ships, places visited.",
            parameters: schema(json!({ "query": { "type": "string", "description": "Text to find, e.g. a system, station, ship, commodity or event name." }, "limit": { "type": "integer" } }), &["query"]),
        },
        ToolSpec {
            name: "knowledge",
            description: "Searches the built-in knowledge base: game mechanics, money-making strategies, exobiology, mining, engineering, carriers, colonisation, useful websites, and how to use Elite Nexus.",
            parameters: schema(json!({ "query": { "type": "string" } }), &["query"]),
        },
    ]
}

/// One line about the commander for each question (location, ship, credits, date).
pub fn context_line(gs: &GameState) -> String {
    let loc = &gs.location;
    let place = match loc.station.as_ref().filter(|_| loc.docked) {
        Some(s) => format!("docked at {} in {}", s.name, loc.system),
        None => format!("in {} ({})", loc.system, loc.situation()),
    };
    let ship = gs
        .fleet
        .current()
        .map(|s| {
            format!(
                "flying a {} ({} pad, {} t cargo, {:.1} ly jump)",
                s.type_name,
                finder::ship_pad(gs).unwrap_or('?'),
                s.cargo_capacity,
                s.max_jump_range
            )
        })
        .unwrap_or_default();
    format!(
        "CMDR {} {place}, {ship}, {} Cr; today is {}",
        gs.cmdr.name,
        compact_credits(gs.cmdr.credits),
        nexus_journal::Timestamp::now().to_local_string("%Y-%m-%d")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{TempDb, good, station};
    use nexus_journal::{Event, EventCtx, raw::scan_line};
    use nexus_model::Alerts;

    /// A commander in Alpha flying an Anaconda with 100 t of gold aboard.
    fn commander() -> Model {
        let now = nexus_journal::Timestamp::now().to_iso();
        let lines = [
            format!(r#"{{ "timestamp":"{now}", "event":"Fileheader", "part":1, "language":"English/UK", "Odyssey":true, "gameversion":"4.4.1.1", "build":"r1 " }}"#),
            format!(r#"{{ "timestamp":"{now}", "event":"Commander", "FID":"F1", "Name":"Tester" }}"#),
            format!(r#"{{ "timestamp":"{now}", "event":"LoadGame", "FID":"F1", "Commander":"Tester", "Horizons":true, "Odyssey":true, "Ship":"Anaconda", "ShipID":1, "ShipName":"", "ShipIdent":"", "FuelLevel":32.0, "FuelCapacity":32.0, "GameMode":"Solo", "Credits":250000000, "Loan":0 }}"#),
            format!(r#"{{ "timestamp":"{now}", "event":"Location", "Docked":false, "StarSystem":"Alpha", "SystemAddress":1001, "StarPos":[0.0,0.0,0.0], "Population":1000, "Body":"Alpha", "BodyID":0, "BodyType":"Star" }}"#),
            format!(r#"{{ "timestamp":"{now}", "event":"Loadout", "Ship":"anaconda", "ShipID":1, "ShipName":"", "ShipIdent":"", "HullValue":100, "ModulesValue":100, "HullHealth":1.0, "UnladenMass":400.0, "CargoCapacity":400, "MaxJumpRange":30.5, "FuelCapacity":{{ "Main":32.0, "Reserve":1.07 }}, "Rebuy":5000000, "Modules":[] }}"#),
            format!(r#"{{ "timestamp":"{now}", "event":"Cargo", "Vessel":"Ship", "Count":100, "Inventory":[ {{ "Name":"gold", "Name_Localised":"Gold", "Count":100, "Stolen":0 }} ] }}"#),
        ];
        let mut m = Model::default();
        let mut quiet = Alerts::disabled();
        for line in &lines {
            let (ts, name) = scan_line(line).expect("line");
            let ev = Event::parse(name, line).expect("event");
            m.apply(&EventCtx { ts, name, line, live: false }, &ev, &mut quiet);
        }
        m
    }

    #[test]
    fn tools_answer_from_the_commanders_data() {
        let t = TempDb::new("ai-tools");
        let db = t.db();
        station(db, ("Alpha", 1001, 0.0), "Alpha Port", 5001, serde_json::json!([good("gold", 0, 0, 60_000, 80)]));
        station(db, ("Beta", 1002, 10.0), "Beta Hub", 5002, serde_json::json!([good("gold", 0, 0, 55_000, 1000), good("tritium", 40_000, 500, 0, 0)]));
        db.flush().unwrap();
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        let tools = NexusTools {
            model: Arc::new(RwLock::new(commander())),
            db: Some(db.clone()),
            index: Arc::new(RwLock::new(JournalIndex::default())),
            spansh: Spansh::new(),
            rt: rt.handle().clone(),
            routes: Arc::new(Mutex::new(None)),
        };

        let me = tools.call("commander", &json!({})).unwrap();
        assert_eq!(me["ship"]["landing_pad"], "L");
        assert_eq!(me["ship"]["cargo_capacity_t"], 400);
        assert_eq!(me["cargo"][0]["tonnes"], 100);

        let buy = tools.call("find_commodity", &json!({ "commodity": "Tritium", "mode": "buy" })).unwrap();
        assert_eq!(buy["results"][0]["station"], "Beta Hub");
        assert_eq!(buy["results"][0]["distance_ly"], 10.0);

        let sell = tools.call("sell_my_cargo", &json!({})).unwrap();
        assert_eq!(sell["best_single_stops"][0]["station"], "Beta Hub", "takes all 100 t: {sell}");
        assert_eq!(sell["best_single_stops"][0]["total_credits"], 100 * 55_000);

        let k = tools.call("knowledge", &json!({ "query": "neutron star jump range boost" })).unwrap();
        assert!(k["sections"][0]["title"].as_str().unwrap().contains("Travel"));

        let s = tools.call("system_info", &json!({ "system": "beta" })).unwrap();
        assert_eq!(s["stations"][0]["name"], "Beta Hub");
        assert_eq!(s["distance_ly"], 10.0);

        assert!(tools.call("journal_search", &json!({ "query": "gold" })).unwrap()["matches_newest_first"].as_array().unwrap().is_empty());
        assert!(tools.call("nope", &json!({})).is_err());
        assert!(tools.call("find_commodity", &json!({})).is_err(), "missing required argument is an error, not a panic");
        assert_eq!(specs().len(), 13);

        // The full loop with a scripted model: one tool call, then an answer.
        struct Script(parking_lot::Mutex<Vec<nexus_ai::Reply>>);
        impl nexus_ai::Backend for Script {
            fn complete(&self, _s: &str, tools: &[ToolSpec], turns: &[nexus_ai::Turn]) -> Result<nexus_ai::Reply, nexus_ai::AiError> {
                assert_eq!(tools.len(), 13);
                if turns.len() > 1 {
                    assert!(matches!(&turns[2], nexus_ai::Turn::ToolResult { content, is_error: false, .. } if content.contains("Beta Hub")));
                }
                Ok(self.0.lock().remove(0))
            }
        }
        let reply = |text: &str, calls: Vec<nexus_ai::ToolCall>| nexus_ai::Reply { text: text.into(), calls, raw: json!({ "role": "assistant" }), usage: Default::default(), truncated: false };
        let script = Script(parking_lot::Mutex::new(vec![
            reply("", vec![nexus_ai::ToolCall { id: "1".into(), name: "sell_my_cargo".into(), args: json!({}) }]),
            reply("Sell your 100 t of gold at **Beta Hub** for 5.5M Cr.", vec![]),
        ]));
        let mut conv = Vec::new();
        let ans = nexus_ai::ask(&script, nexus_ai::SYSTEM_PROMPT, &tools, &mut conv, "Where should I sell my cargo?".into(), |_| {}).unwrap();
        assert!(ans.text.contains("Beta Hub"));
        assert_eq!(ans.tools_used, vec!["sell_my_cargo"]);
    }
}

#[cfg(test)]
mod real_data {
    use super::*;
    use nexus_journal::{EventCtx, list_journal_files, load_history};
    use nexus_model::Alerts;

    /// Runs the local-data tools over this PC's real journals (prints the results).
    /// `cargo test -p nexus-app -- --ignored real_journals --nocapture`
    #[test]
    #[ignore]
    fn real_journals() {
        let Some(dir) = nexus_journal::default_journal_dir() else { return };
        let files = list_journal_files(&dir).unwrap_or_default();
        let mut model = Model::default();
        let mut index = JournalIndex::default();
        let mut quiet = Alerts::disabled();
        load_history(&files, 32, |f, fe| {
            index.add_file(f.path.clone());
            for e in &fe.events {
                model.apply(&EventCtx { ts: e.ts, name: fe.name(e), line: fe.line(e), live: false }, &e.event, &mut quiet);
            }
        });
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        let t = crate::testutil::TempDb::new("ai-real");
        let context = context_line(model.active().unwrap());
        let tools = NexusTools {
            model: Arc::new(RwLock::new(model)),
            db: Some(t.db().clone()),
            index: Arc::new(RwLock::new(index)),
            spansh: Spansh::new(),
            rt: rt.handle().clone(),
            routes: Arc::new(Mutex::new(None)),
        };
        println!("context: {context}");
        for (name, args) in [
            ("commander", json!({})),
            ("exobiology_here", json!({})),
            ("journal_search", json!({ "query": "Stratum", "limit": 3 })),
            ("sell_my_cargo", json!({})),
            ("price_deals", json!({})),
            ("knowledge", json!({ "query": "first logged bonus" })),
        ] {
            let started = std::time::Instant::now();
            let out = tools.call(name, &args);
            let text = match &out {
                Ok(v) => v.to_string(),
                Err(e) => format!("ERROR {e}"),
            };
            println!("\n== {name} ({:.1} ms, {} chars)\n{}", started.elapsed().as_secs_f64() * 1e3, text.len(), text.chars().take(700).collect::<String>());
            assert!(out.is_ok(), "{name} failed: {text}");
        }
    }
}
