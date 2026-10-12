//! Raw JSON (EDDN envelopes, journal events, Market.json) -> [`Ingest`] items.
//!
//! Everything here treats its input as untrusted: missing or mistyped fields drop the field (or the
//! item), never panic. EDDN journal messages have had their `*_Localised` keys stripped, so every
//! name goes through [`crate::names`] to get the same display strings as Spansh.

use crate::commodities::normalize_symbol;
use crate::model::*;
use crate::names;
use crate::services::Services;
use crate::util::*;
use serde_json::Value;

/// Seconds a message may be ahead of the EDDN gateway timestamp before it is clamped.
const GATEWAY_FUTURE_TOLERANCE: i64 = 300;
/// Seconds a message may be ahead of the local clock before it is clamped.
const LOCAL_FUTURE_TOLERANCE: i64 = 600;

/// `https://eddn.edcd.io/schemas/commodity/3` -> `commodity/3`; `None` for `/test` schemas and
/// anything that doesn't look like an EDDN schema reference.
pub fn eddn_schema_name(schema_ref: &str) -> Option<&str> {
    let s = schema_ref.trim().trim_end_matches('/');
    if s.ends_with("/test") {
        return None;
    }
    let i = s.find("/schemas/")?;
    let rest = &s[i + "/schemas/".len()..];
    (!rest.is_empty()).then_some(rest)
}

impl Ingest {
    /// Converts one EDDN envelope. Handles `commodity/3`, `journal/1` (`FSDJump`, `Location`,
    /// `CarrierJump`, `Docked`, `SAASignalsFound`) and `approachsettlement/1`; returns nothing for
    /// `/test` schemas, other schemas and malformed messages.
    pub fn from_eddn(envelope: &Value) -> Vec<Ingest> {
        let Some(schema) = envelope.get("$schemaRef").and_then(Value::as_str) else {
            return Vec::new();
        };
        let Some(name) = eddn_schema_name(schema) else {
            return Vec::new();
        };
        let Some(msg) = envelope.get("message").filter(|m| m.is_object()) else {
            return Vec::new();
        };
        let gateway = envelope
            .get("header")
            .and_then(|h| get_ts(h, "gatewayTimestamp"));
        match name {
            "commodity/3" => commodity_snapshot(msg, gateway)
                .map(Ingest::Market)
                .into_iter()
                .collect(),
            "journal/1" => match get_str(msg, "event") {
                Some(event) => convert_journal(event, msg, Source::Eddn, gateway),
                None => Vec::new(),
            },
            "approachsettlement/1" => {
                convert_journal("ApproachSettlement", msg, Source::Eddn, gateway)
            }
            _ => Vec::new(),
        }
    }

    /// Converts one of our own journal events. `FSDJump` / `Location` / `CarrierJump` (system state,
    /// factions, powers; plus the station when docked), `Docked` (station details), `SAASignalsFound`
    /// on rings, `ApproachSettlement`, and `Market` **with** the `Items` of Market.json (pass the
    /// companion file's JSON as the event).
    pub fn from_journal_event(event: &str, json: &Value) -> Vec<Ingest> {
        convert_journal(event, json, Source::Journal, None)
    }

    /// Expands [`Ingest::Raw`] into concrete items; anything else is returned as is.
    pub fn expand(self) -> Vec<Ingest> {
        match self {
            Ingest::Raw(RawIngest::Eddn(v)) => Ingest::from_eddn(&v),
            Ingest::Raw(RawIngest::Journal { event, json }) => {
                Ingest::from_journal_event(&event, &json)
            }
            other => vec![other],
        }
    }
}

fn clamp_ts(ts: Option<i64>, gateway: Option<i64>) -> i64 {
    let now = now_unix();
    let mut t = ts.or(gateway).unwrap_or(now);
    if let Some(g) = gateway
        && t > g + GATEWAY_FUTURE_TOLERANCE
    {
        t = g;
    }
    if t > now + LOCAL_FUTURE_TOLERANCE {
        t = now;
    }
    t
}

fn bracket(v: Option<&Value>) -> Option<i8> {
    match v? {
        Value::String(s) if s.trim().is_empty() => Some(-1),
        other => as_i64(other).map(|n| n.clamp(-1, 3) as i8),
    }
}

fn price(v: &Value, key: &str) -> i64 {
    get_i64(v, key).unwrap_or(0).max(0)
}

fn commodity_snapshot(msg: &Value, gateway: Option<i64>) -> Option<MarketSnapshot> {
    let market_id = get_id(msg, "marketId")?;
    let system_name = get_text(msg, "systemName")?;
    let station_name = get_text(msg, "stationName")?;
    let list = msg.get("commodities")?.as_array()?;
    let mut items = Vec::with_capacity(list.len());
    for c in list {
        let Some(name) = get_str(c, "name") else {
            continue;
        };
        let symbol = normalize_symbol(name);
        if symbol.is_empty() {
            continue;
        }
        items.push(MarketItem {
            symbol,
            buy: price(c, "buyPrice"),
            sell: price(c, "sellPrice"),
            mean: price(c, "meanPrice"),
            stock: price(c, "stock"),
            stock_bracket: bracket(c.get("stockBracket")),
            demand: price(c, "demand"),
            demand_bracket: bracket(c.get("demandBracket")),
        });
    }
    let economies = msg.get("economies").and_then(Value::as_array).map(|a| {
        let mut e: Vec<(String, f64)> = a
            .iter()
            .filter_map(|x| {
                Some((
                    names::economy(get_str(x, "name")?)?,
                    get_f64(x, "proportion").unwrap_or(0.0),
                ))
            })
            .collect();
        sort_economies(&mut e);
        e
    });
    let prohibited =
        get_strings(msg, "prohibited").map(|v| v.iter().map(|s| normalize_symbol(s)).collect());
    Some(MarketSnapshot {
        market_id,
        system_name,
        station_name,
        station_type: get_text(msg, "stationType"),
        carrier_docking_access: get_text(msg, "carrierDockingAccess"),
        timestamp: clamp_ts(get_ts(msg, "timestamp"), gateway),
        items,
        prohibited,
        economies,
        source: Source::Eddn,
    })
}

fn sort_economies(e: &mut [(String, f64)]) {
    e.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
}

pub(crate) fn convert_journal(
    event: &str,
    v: &Value,
    source: Source,
    gateway: Option<i64>,
) -> Vec<Ingest> {
    if !v.is_object() {
        return Vec::new();
    }
    let ts = clamp_ts(get_ts(v, "timestamp"), gateway);
    match event {
        "FSDJump" | "Location" | "CarrierJump" => {
            // EDDN relays every commander's jumps (~250/min, mostly into empty systems). Keep what
            // the queries can use: populated systems, carrier jumps and arrivals docked at a
            // station; everything else would grow the database without bound.
            let docked = event != "FSDJump" && get_bool(v, "Docked") == Some(true);
            if source == Source::Eddn
                && event != "CarrierJump"
                && !docked
                && get_i64(v, "Population").unwrap_or(0) <= 0
            {
                return Vec::new();
            }
            let mut out = Vec::with_capacity(2);
            let Some(sys) = system_arrival(v, ts) else {
                return out;
            };
            let (id, name) = (sys.id64, sys.name.clone());
            out.push(Ingest::System(sys));
            if event != "FSDJump"
                && get_bool(v, "Docked") == Some(true)
                && let Some(st) = station_from_event(v, ts, Some(id), Some(name))
            {
                out.push(Ingest::Station(st));
            }
            out
        }
        "Docked" => {
            let mut out = Vec::with_capacity(2);
            let sys_id = get_id(v, "SystemAddress").filter(|a| *a > 1);
            let sys_name = get_text(v, "StarSystem");
            out.extend(position_only(v, ts, sys_id, sys_name.as_deref()));
            if let Some(st) = station_from_event(v, ts, sys_id, sys_name) {
                out.push(Ingest::Station(st));
            }
            out
        }
        "ApproachSettlement" => {
            let mut out = Vec::with_capacity(2);
            let sys_id = get_id(v, "SystemAddress").filter(|a| *a > 1);
            let sys_name = get_text(v, "StarSystem");
            out.extend(position_only(v, ts, sys_id, sys_name.as_deref()));
            if let Some(market_id) = get_id(v, "MarketID")
                && let Some(name) = get_text(v, "Name")
                && !name.starts_with('$')
            {
                let mut st = station_common(v, ts, market_id, name, sys_id, sys_name);
                st.planetary = Some(true);
                out.push(Ingest::Station(st));
            }
            out
        }
        "SAASignalsFound" => {
            let mut out = Vec::with_capacity(2);
            let sys_id = get_id(v, "SystemAddress").filter(|a| *a > 1);
            let sys_name = get_text(v, "StarSystem");
            out.extend(position_only(v, ts, sys_id, sys_name.as_deref()));
            if let Some(r) = ring_signals(v, ts, sys_id, sys_name) {
                out.push(Ingest::RingSignals(r));
            }
            out
        }
        "Market" => market_json(v, ts, source)
            .map(Ingest::Market)
            .into_iter()
            .collect(),
        _ => Vec::new(),
    }
}

/// A position-only system update when the event carries `StarSystem` + `SystemAddress` + `StarPos`
/// (EDDN senders add `StarPos` to `Docked` / `SAASignalsFound`; our own journal doesn't have it).
fn position_only(v: &Value, ts: i64, id: Option<u64>, name: Option<&str>) -> Option<Ingest> {
    let pos = get_pos(v, "StarPos")?;
    Some(Ingest::System(SystemUpdate {
        id64: id?,
        name: name?.to_string(),
        pos: Some(pos),
        timestamp: ts,
        details: None,
    }))
}

fn system_arrival(v: &Value, ts: i64) -> Option<SystemUpdate> {
    let id64 = get_id(v, "SystemAddress").filter(|a| *a > 1)?;
    let name = get_text(v, "StarSystem")?;
    let (controlling_faction, controlling_faction_state) = match v.get("SystemFaction") {
        Some(Value::String(s)) if !s.trim().is_empty() => (Some(s.trim().to_string()), None),
        Some(f @ Value::Object(_)) => (
            get_text(f, "Name"),
            get_str(f, "FactionState").and_then(names::state),
        ),
        _ => (None, None),
    };
    let factions = v
        .get("Factions")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(faction).collect());
    let details = SystemDetails {
        population: get_i64(v, "Population")
            .filter(|p| *p >= 0)
            .map(|p| p as u64),
        allegiance: get_str(v, "SystemAllegiance").and_then(names::allegiance),
        government: get_str(v, "SystemGovernment").and_then(names::government),
        economy: get_str(v, "SystemEconomy").and_then(names::economy),
        second_economy: get_str(v, "SystemSecondEconomy").and_then(names::economy),
        security: get_str(v, "SystemSecurity").and_then(names::security),
        controlling_faction,
        controlling_faction_state,
        controlling_power: get_text(v, "ControllingPower"),
        powerplay_state: get_str(v, "PowerplayState").and_then(names::state),
        powers: get_strings(v, "Powers").unwrap_or_default(),
        factions,
    };
    Some(SystemUpdate {
        id64,
        name,
        pos: get_pos(v, "StarPos"),
        timestamp: ts,
        details: Some(details),
    })
}

fn states(f: &Value, key: &str) -> Vec<String> {
    f.get(key)
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|s| get_str(s, "State").and_then(names::state))
                .collect()
        })
        .unwrap_or_default()
}

fn faction(f: &Value) -> Option<FactionUpdate> {
    Some(FactionUpdate {
        name: get_text(f, "Name")?,
        influence: get_f64(f, "Influence"),
        state: get_str(f, "FactionState").and_then(names::state),
        government: get_str(f, "Government").and_then(names::government),
        allegiance: get_str(f, "Allegiance").and_then(names::allegiance),
        happiness: get_str(f, "Happiness").and_then(names::happiness),
        active_states: states(f, "ActiveStates"),
        pending_states: states(f, "PendingStates"),
        recovering_states: states(f, "RecoveringStates"),
    })
}

/// Station fields shared by `Docked`, `Location`/`CarrierJump` (docked) and `ApproachSettlement`.
fn station_common(
    v: &Value,
    ts: i64,
    market_id: u64,
    name: String,
    system_id64: Option<u64>,
    system_name: Option<String>,
) -> StationUpdate {
    let economies = v
        .get("StationEconomies")
        .and_then(Value::as_array)
        .map(|a| {
            let mut e: Vec<(String, f64)> = a
                .iter()
                .filter_map(|x| {
                    Some((
                        names::economy(get_str(x, "Name")?)?,
                        get_f64(x, "Proportion").unwrap_or(0.0),
                    ))
                })
                .collect();
            sort_economies(&mut e);
            e
        });
    let primary_economy = get_str(v, "StationEconomy")
        .and_then(names::economy)
        .or_else(|| {
            economies
                .as_ref()
                .and_then(|e| e.first().map(|(n, _)| n.clone()))
        });
    let (faction, faction_state) = match v.get("StationFaction") {
        Some(Value::String(s)) if !s.trim().is_empty() => (Some(s.trim().to_string()), None),
        Some(f @ Value::Object(_)) => (
            get_text(f, "Name"),
            get_str(f, "FactionState").and_then(names::state),
        ),
        _ => (None, None),
    };
    StationUpdate {
        market_id,
        name,
        system_id64,
        system_name,
        station_type: None,
        dist_ls: None,
        landing_pads: None,
        services: get_strings(v, "StationServices").map(Services::from_names),
        primary_economy,
        economies,
        faction,
        faction_state,
        government: get_str(v, "StationGovernment").and_then(names::government),
        allegiance: get_str(v, "StationAllegiance").and_then(names::allegiance),
        carrier_docking_access: None,
        planetary: None,
        timestamp: ts,
    }
}

fn station_from_event(
    v: &Value,
    ts: i64,
    sys_id: Option<u64>,
    sys_name: Option<String>,
) -> Option<StationUpdate> {
    let market_id = get_id(v, "MarketID")?;
    let name = get_text(v, "StationName")?;
    let mut st = station_common(v, ts, market_id, name, sys_id, sys_name);
    st.station_type = get_text(v, "StationType");
    st.dist_ls = get_f64(v, "DistFromStarLS").filter(|d| *d >= 0.0);
    st.landing_pads = v
        .get("LandingPads")
        .filter(|p| p.is_object())
        .map(|p| LandingPads {
            small: get_i64(p, "Small").unwrap_or(0).clamp(0, 1000) as u32,
            medium: get_i64(p, "Medium").unwrap_or(0).clamp(0, 1000) as u32,
            large: get_i64(p, "Large").unwrap_or(0).clamp(0, 1000) as u32,
        });
    Some(st)
}

fn is_ring_name(body: &str) -> bool {
    let l = body.trim_end().to_ascii_lowercase();
    l.ends_with(" ring") || l.ends_with(" belt")
}

fn ring_signals(
    v: &Value,
    ts: i64,
    sys_id: Option<u64>,
    sys_name: Option<String>,
) -> Option<RingSignalsUpdate> {
    let body_name = get_text(v, "BodyName")?;
    let list = v.get("Signals")?.as_array()?;
    let mut signals = Vec::with_capacity(list.len());
    let mut any_commodity = false;
    for s in list {
        let Some(t) = get_str(s, "Type") else {
            continue;
        };
        if t.starts_with('$') {
            continue; // biological / geological / human ... signals of planets
        }
        any_commodity = true;
        let sym = normalize_symbol(t);
        if !sym.is_empty() {
            signals.push((sym, get_i64(s, "Count").unwrap_or(0).max(0)));
        }
    }
    if !is_ring_name(&body_name) && !any_commodity {
        return None;
    }
    Some(RingSignalsUpdate {
        system_id64: sys_id?,
        system_name: sys_name,
        body_name,
        ring_type: None,
        signals,
        timestamp: ts,
    })
}

/// Market.json (or a `Market` event that carries `Items`).
fn market_json(v: &Value, ts: i64, source: Source) -> Option<MarketSnapshot> {
    let list = v.get("Items")?.as_array()?;
    let market_id = get_id(v, "MarketID")?;
    let station_name = get_text(v, "StationName")?;
    let system_name = get_text(v, "StarSystem")?;
    let mut items = Vec::with_capacity(list.len());
    for it in list {
        if get_str(it, "Category").is_some_and(|c| c.to_ascii_lowercase().contains("nonmarketable"))
        {
            continue; // limpets; EDDN drops them too
        }
        let Some(name) = get_str(it, "Name") else {
            continue;
        };
        let symbol = normalize_symbol(name);
        if symbol.is_empty() {
            continue;
        }
        items.push(MarketItem {
            symbol,
            buy: price(it, "BuyPrice"),
            sell: price(it, "SellPrice"),
            mean: price(it, "MeanPrice"),
            stock: price(it, "Stock"),
            stock_bracket: bracket(it.get("StockBracket")),
            demand: price(it, "Demand"),
            demand_bracket: bracket(it.get("DemandBracket")),
        });
    }
    Some(MarketSnapshot {
        market_id,
        system_name,
        station_name,
        station_type: get_text(v, "StationType"),
        carrier_docking_access: get_text(v, "CarrierDockingAccess"),
        timestamp: ts,
        items,
        prohibited: None,
        economies: None,
        source,
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use serde_json::json;

    pub(crate) fn eddn_commodity() -> Value {
        json!({
            "$schemaRef": "https://eddn.edcd.io/schemas/commodity/3",
            "header": {"uploaderID": "abc", "softwareName": "E:D Market Connector [Windows]", "softwareVersion": "6.1.2",
                       "gatewayTimestamp": "2026-10-12T09:23:12.361367Z"},
            "message": {
                "systemName": "Sol", "stationName": "Daedalus", "marketId": 128016384, "stationType": "Orbis",
                "timestamp": "2026-10-12T09:23:10Z",
                "commodities": [
                    {"name": "gold", "meanPrice": 47000, "buyPrice": 45000, "stock": 1200, "stockBracket": 2,
                     "sellPrice": 44000, "demand": 0, "demandBracket": ""},
                    {"name": "Tritium", "meanPrice": 50000, "buyPrice": 0, "stock": 0, "stockBracket": 0,
                     "sellPrice": 52000, "demand": 9000, "demandBracket": 3, "statusFlags": ["Consumer"]},
                    {"name": "", "buyPrice": 1}
                ],
                "economies": [{"name": "Refinery", "proportion": 1.0}],
                "prohibited": ["Narcotics", "BattleWeapons"]
            }
        })
    }

    pub(crate) fn eddn_fsdjump() -> Value {
        json!({
            "$schemaRef": "https://eddn.edcd.io/schemas/journal/1",
            "header": {"uploaderID": "x", "softwareName": "EDDI", "softwareVersion": "5.0.6", "gatewayTimestamp": "2026-10-12T09:00:01Z"},
            "message": {
                "timestamp": "2026-10-12T09:00:00Z", "event": "FSDJump", "StarSystem": "Sol", "SystemAddress": 10477373803u64,
                "StarPos": [0.0, 0.0, 0.0], "SystemAllegiance": "Federation", "SystemEconomy": "$economy_Refinery;",
                "SystemSecondEconomy": "$economy_Service;", "SystemGovernment": "$government_Democracy;",
                "SystemSecurity": "$SYSTEM_SECURITY_high;", "Population": 18320926115u64,
                "ControllingPower": "Jerome Archer", "Powers": ["Jerome Archer", "Edmund Mahon"], "PowerplayState": "Stronghold",
                "SystemFaction": {"Name": "Mother Gaia", "FactionState": "None"},
                "Factions": [
                    {"Name": "Mother Gaia", "FactionState": "None", "Government": "Democracy", "Influence": 0.315,
                     "Allegiance": "Federation", "Happiness": "$Faction_HappinessBand2;",
                     "PendingStates": [{"State": "Expansion", "Trend": 0}], "RecoveringStates": [{"State": "PublicHoliday", "Trend": 0}]},
                    {"Name": "Federal Congress", "FactionState": "CivilLiberty", "Government": "Democracy", "Influence": 0.168,
                     "Allegiance": "Federation", "ActiveStates": [{"State": "CivilLiberty"}]}
                ],
                "horizons": true, "odyssey": true
            }
        })
    }

    #[test]
    fn schema_names() {
        assert_eq!(
            eddn_schema_name("https://eddn.edcd.io/schemas/commodity/3"),
            Some("commodity/3")
        );
        assert_eq!(
            eddn_schema_name("https://eddn.edcd.io/schemas/commodity/3/test"),
            None
        );
        assert_eq!(eddn_schema_name("garbage"), None);
    }

    #[test]
    fn eddn_commodity_message() {
        let out = Ingest::from_eddn(&eddn_commodity());
        assert_eq!(out.len(), 1);
        let Ingest::Market(m) = &out[0] else {
            panic!("not a market: {out:?}")
        };
        assert_eq!(m.market_id, 128016384);
        assert_eq!(m.system_name, "Sol");
        assert_eq!(m.station_type.as_deref(), Some("Orbis"));
        assert_eq!(m.timestamp, 1_791_796_990);
        assert_eq!(m.items.len(), 2);
        assert_eq!(m.items[0].symbol, "gold");
        assert_eq!(m.items[0].demand_bracket, Some(-1));
        assert_eq!(m.items[1].symbol, "tritium");
        assert_eq!(m.items[1].sell, 52000);
        assert_eq!(
            m.prohibited.as_deref(),
            Some(&["narcotics".to_string(), "battleweapons".to_string()][..])
        );
        assert_eq!(
            m.economies.as_ref().map(|e| e[0].0.as_str()),
            Some("Refinery")
        );
        assert_eq!(m.source, Source::Eddn);
    }

    #[test]
    fn test_schemas_and_garbage_are_ignored() {
        let mut v = eddn_commodity();
        v["$schemaRef"] = json!("https://eddn.edcd.io/schemas/commodity/3/test");
        assert!(Ingest::from_eddn(&v).is_empty());
        assert!(Ingest::from_eddn(&json!({"foo": 1})).is_empty());
        assert!(Ingest::from_eddn(&json!([1, 2, 3])).is_empty());
        let mut v = eddn_commodity();
        v["message"]["marketId"] = json!("not a number");
        assert!(Ingest::from_eddn(&v).is_empty());
        assert!(Ingest::from_journal_event("FSDJump", &json!("string")).is_empty());
    }

    #[test]
    fn eddn_fsdjump_message() {
        let out = Ingest::from_eddn(&eddn_fsdjump());
        assert_eq!(out.len(), 1);
        let Ingest::System(s) = &out[0] else { panic!() };
        assert_eq!(s.id64, 10477373803);
        assert_eq!(s.pos, Some([0.0, 0.0, 0.0]));
        let d = s.details.as_ref().expect("details");
        assert_eq!(d.economy.as_deref(), Some("Refinery"));
        assert_eq!(d.second_economy.as_deref(), Some("Service"));
        assert_eq!(d.government.as_deref(), Some("Democracy"));
        assert_eq!(d.security.as_deref(), Some("High"));
        assert_eq!(d.controlling_faction.as_deref(), Some("Mother Gaia"));
        assert_eq!(d.powerplay_state.as_deref(), Some("Stronghold"));
        assert_eq!(d.powers.len(), 2);
        let f = d.factions.as_ref().expect("factions");
        assert_eq!(f.len(), 2);
        assert_eq!(f[0].happiness.as_deref(), Some("Happy"));
        assert_eq!(f[0].recovering_states, vec!["Public Holiday".to_string()]);
        assert_eq!(f[1].state.as_deref(), Some("Civil Liberty"));
    }

    #[test]
    fn eddn_jumps_into_empty_systems_are_skipped() {
        let mut v = eddn_fsdjump();
        v["message"]["Population"] = json!(0);
        assert!(Ingest::from_eddn(&v).is_empty(), "unpopulated EDDN jump");
        v["message"].as_object_mut().unwrap().remove("Population");
        assert!(Ingest::from_eddn(&v).is_empty(), "no population at all");
        // ...but our own journal keeps every system we visit.
        assert_eq!(Ingest::from_journal_event("FSDJump", &v["message"]).len(), 1);
        // A carrier jump from EDDN is kept (carrier positions matter).
        v["message"]["event"] = json!("CarrierJump");
        assert_eq!(Ingest::from_eddn(&v).len(), 1);
    }

    #[test]
    fn eddn_docked_with_starpos() {
        let v = json!({
            "$schemaRef": "https://eddn.edcd.io/schemas/journal/1",
            "header": {"uploaderID": "x", "softwareName": "EDMC", "softwareVersion": "6"},
            "message": {"timestamp": "2026-10-12T09:00:00Z", "event": "Docked", "StationName": "Ehrlich City",
                "StationType": "CraterPort", "StarSystem": "Sol", "SystemAddress": 10477373803u64, "MarketID": 3534390016u64,
                "StationFaction": {"Name": "Sol Workers' Party"}, "StationGovernment": "$government_Democracy;",
                "StationServices": ["dock", "autodock", "commodities", "materialtrader", "facilitator", "refuel"],
                "StationEconomy": "$economy_Industrial;",
                "StationEconomies": [{"Name": "$economy_Industrial;", "Proportion": 0.8}, {"Name": "$economy_HighTech;", "Proportion": 0.2}],
                "DistFromStarLS": 230.8, "LandingPads": {"Small": 8, "Medium": 8, "Large": 8}, "StarPos": [0.0, 0.0, 0.0]}
        });
        let out = Ingest::from_eddn(&v);
        assert_eq!(out.len(), 2, "{out:?}");
        assert!(matches!(&out[0], Ingest::System(s) if s.details.is_none() && s.pos.is_some()));
        let Ingest::Station(st) = &out[1] else {
            panic!()
        };
        assert_eq!(st.market_id, 3534390016);
        assert_eq!(st.system_id64, Some(10477373803));
        assert_eq!(st.primary_economy.as_deref(), Some("Industrial"));
        assert_eq!(
            st.economies.as_ref().map(|e| e[1].0.as_str()),
            Some("High Tech")
        );
        assert_eq!(
            st.landing_pads,
            Some(LandingPads {
                small: 8,
                medium: 8,
                large: 8
            })
        );
        let svc = st.services.expect("services");
        assert!(svc.contains(
            Services::MATERIAL_TRADER | Services::INTERSTELLAR_FACTORS | Services::MARKET
        ));
        assert_eq!(st.faction.as_deref(), Some("Sol Workers' Party"));
    }

    #[test]
    fn ring_signals_from_saa() {
        let v = json!({"timestamp": "2026-10-12T09:00:00Z", "event": "SAASignalsFound", "BodyName": "HIP 21991 A 1 A Ring",
            "SystemAddress": 3107509474002u64, "BodyID": 7,
            "Signals": [{"Type": "Painite", "Count": 2}, {"Type": "LowTemperatureDiamond", "Type_Localised": "Low Temperature Diamonds", "Count": 1}]});
        let out = Ingest::from_journal_event("SAASignalsFound", &v);
        assert_eq!(out.len(), 1);
        let Ingest::RingSignals(r) = &out[0] else {
            panic!()
        };
        assert_eq!(
            r.signals,
            vec![
                ("painite".to_string(), 2),
                ("lowtemperaturediamond".to_string(), 1)
            ]
        );
        // planet signals are not ring hotspots
        let planet = json!({"timestamp": "2026-10-12T09:00:00Z", "event": "SAASignalsFound", "BodyName": "Sol 3",
            "SystemAddress": 10477373803u64, "Signals": [{"Type": "$SAA_SignalType_Biological;", "Count": 3}]});
        assert!(Ingest::from_journal_event("SAASignalsFound", &planet).is_empty());
    }

    #[test]
    fn own_market_json() {
        let v = json!({"timestamp": "2026-10-12T09:00:00Z", "event": "Market", "MarketID": 128016384u64, "StationName": "Daedalus",
        "StationType": "Orbis", "StarSystem": "Sol", "Items": [
            {"id": 128049154, "Name": "$gold_name;", "Name_Localised": "Gold", "Category": "$MARKET_category_metals;",
             "BuyPrice": 45000, "SellPrice": 44000, "MeanPrice": 47000, "StockBracket": 2, "DemandBracket": 0,
             "Stock": 1200, "Demand": 1, "Consumer": false, "Producer": true, "Rare": false},
            {"id": 128066403, "Name": "$drones_name;", "Category": "$MARKET_category_nonmarketable;", "BuyPrice": 101, "SellPrice": 101}
        ]});
        let out = Ingest::from_journal_event("Market", &v);
        let [Ingest::Market(m)] = &out[..] else {
            panic!("{out:?}")
        };
        assert_eq!(m.items.len(), 1);
        assert_eq!(m.items[0].symbol, "gold");
        assert_eq!(m.source, Source::Journal);
        // the bare journal event (no Items) carries no market data
        let bare = json!({"timestamp": "2026-10-12T09:00:00Z", "event": "Market", "MarketID": 1, "StationName": "x", "StarSystem": "y"});
        assert!(Ingest::from_journal_event("Market", &bare).is_empty());
    }

    #[test]
    fn location_docked_gives_system_and_station() {
        let v = json!({"timestamp": "2026-10-12T09:00:00Z", "event": "CarrierJump", "Docked": true, "StationName": "T2X-02X",
            "StationType": "FleetCarrier", "MarketID": 3708716800u64, "StarSystem": "Phua Fraae AA-A h0", "SystemAddress": 819519,
            "StarPos": [-34608.78125, 513.34375, 26058.59375], "Population": 0, "SystemAllegiance": "",
            "StationServices": ["dock", "commodities"]});
        let out = Ingest::from_journal_event("CarrierJump", &v);
        assert_eq!(out.len(), 2);
        let Ingest::Station(st) = &out[1] else {
            panic!()
        };
        assert_eq!(st.system_id64, Some(819519));
        assert_eq!(st.station_type.as_deref(), Some("FleetCarrier"));
        let Ingest::System(s) = &out[0] else { panic!() };
        assert_eq!(s.details.as_ref().and_then(|d| d.allegiance.clone()), None);
        assert!(s.details.as_ref().is_some_and(|d| d.factions.is_none()));
    }

    #[test]
    fn future_timestamps_are_clamped() {
        let mut v = eddn_commodity();
        v["message"]["timestamp"] = json!("2099-01-01T00:00:00Z");
        let out = Ingest::from_eddn(&v);
        let [Ingest::Market(m)] = &out[..] else {
            panic!()
        };
        assert_eq!(
            m.timestamp, 1_791_796_992,
            "clamped to the gateway timestamp"
        );
    }

    #[test]
    fn raw_expands() {
        let raw = Ingest::Raw(RawIngest::Eddn(eddn_commodity()));
        assert_eq!(raw.expand().len(), 1);
    }
}
