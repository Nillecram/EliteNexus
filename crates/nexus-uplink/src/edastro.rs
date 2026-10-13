//! EDAstro journal upload (`GET /api/accepting`, then `POST /api/journal`), see the EDAstro
//! section of `docs/research/websites.md`.
//!
//! Only events EDAstro currently accepts *and* that have a reviewed privacy whitelist here are
//! sent; every other key is removed (EDMC's whitelist, plus a station-only whitelist for
//! `Docked`, which EDAstro started accepting in 2026).

use crate::json::{Obj, get_str, strip_localised};
use crate::transport::HttpResponse;
use nexus_journal::Timestamp;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const ACCEPTING_URL: &str = "https://edastro.com/api/accepting";
pub const JOURNAL_URL: &str = "https://edastro.com/api/journal";

/// What `/api/accepting` returned on 2026-10-12 (used until the live list has been fetched).
pub const DEFAULT_ACCEPTING: [&str; 4] = ["CarrierJumpRequest", "CarrierStats", "Docked", "ScanOrganic"];

/// Keys sent per event. Events without an entry are never sent.
pub fn whitelist(event: &str) -> Option<&'static [&'static str]> {
    Some(match event {
        // EDMC's whitelists (minus its missing-comma bug that dropped BodyID/DepartureTime).
        "CarrierStats" => &[
            "timestamp",
            "event",
            "Callsign",
            "Name",
            "CarrierID",
            "AllowNotorious",
            "PendingDecommission",
            "DockingAccess",
            "FuelLevel",
            "Crew",
        ],
        "CarrierJumpRequest" => &["timestamp", "event", "SystemName", "SystemAddress", "CarrierID", "Body", "BodyID", "DepartureTime"],
        "ScanOrganic" => &[
            "timestamp",
            "ScanType",
            "Genus",
            "Genus_Localised",
            "Species",
            "Species_Localised",
            "Variant",
            "Variant_Localised",
            "event",
            "Body",
            "SystemAddress",
        ],
        // Station facts only: no Wanted/ActiveFine/CockpitBreach/Taxi/Multicrew.
        "Docked" => &[
            "timestamp",
            "event",
            "StationName",
            "StationType",
            "StarSystem",
            "SystemAddress",
            "MarketID",
            "StationFaction",
            "StationGovernment",
            "StationAllegiance",
            "StationServices",
            "StationEconomy",
            "StationEconomies",
            "DistFromStarLS",
            "LandingPads",
            "StationState",
        ],
        _ => return None,
    })
}

/// The whitelisted copy of an event, or `None` if it must not be sent.
pub fn filter_event(name: &str, ev: &Obj) -> Option<Value> {
    let keys = whitelist(name)?;
    let mut out = Obj::new();
    for k in keys {
        if let Some(v) = ev.get(*k) {
            out.insert((*k).to_string(), v.clone());
        }
    }
    if name == "Docked" {
        let mut v = Value::Object(out);
        strip_localised(&mut v);
        return Some(v);
    }
    get_str(&out, "timestamp")?;
    Some(Value::Object(out))
}

/// One queued EDAstro event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdastroItem {
    pub cmdr: String,
    /// Current system for the header (batches never mix systems).
    pub system: String,
    pub odyssey: Option<bool>,
    pub event: Value,
    pub event_time: Timestamp,
    pub queued_at: Timestamp,
}

/// `{"status":200|401,...}` = accepted (401: nothing new, not an error). EDMC and the old EDAstro
/// plugin compare `status` as text, so `"200"` counts as well as `200`.
pub fn classify_response(resp: &HttpResponse) -> Result<(), String> {
    if !resp.is_success() {
        return Err(format!("HTTP {}", resp.status));
    }
    let reply = serde_json::from_str::<Value>(&resp.body).ok();
    let status = reply
        .as_ref()
        .and_then(|v| v.get("status"))
        .and_then(|s| s.as_i64().or_else(|| s.as_str().and_then(|t| t.trim().parse().ok())));
    match status {
        Some(200) | Some(401) => Ok(()),
        Some(other) => match reply.as_ref().and_then(|v| v.get("message")).and_then(Value::as_str) {
            Some(msg) => Err(format!("EDAstro status {other}: {}", excerpt(msg))),
            None => Err(format!("EDAstro status {other}")),
        },
        // Say what came back, so an unexpected answer can be recognised.
        None => Err(format!("unreadable response: {}", excerpt(&resp.body))),
    }
}

/// The start of a server reply on one line, for status messages.
fn excerpt(text: &str) -> String {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.is_empty() {
        return "empty".into();
    }
    match flat.char_indices().nth(80) {
        Some((i, _)) => format!("\"{}…\"", &flat[..i]),
        None => format!("\"{flat}\""),
    }
}

/// On-disk cache of the accepted-events list.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcceptingCache {
    pub fetched: Timestamp,
    pub events: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json::parse_object;

    #[test]
    fn whitelists_remove_private_data() {
        let stats = parse_object(r#"{ "timestamp":"2026-10-12T08:28:37Z", "event":"CarrierStats", "CarrierID":3716323840, "CarrierType":"FleetCarrier", "Callsign":"KFK-L3V", "Name":"TCG BARBAROS", "DockingAccess":"all", "AllowNotorious":false, "FuelLevel":500, "JumpRangeCurr":500.000000, "JumpRangeMax":500.000000, "PendingDecommission":false, "SpaceUsage":{ "TotalCapacity":25000 }, "Finance":{ "CarrierBalance":1000000000 }, "Crew":[ { "CrewRole":"Captain", "Activated":true, "Enabled":true, "CrewName":"Raphael Mendez" } ] }"#).unwrap();
        let out = filter_event("CarrierStats", &stats).unwrap();
        assert!(out.get("Finance").is_none() && out.get("SpaceUsage").is_none());
        assert_eq!(out["Callsign"], "KFK-L3V");
        let docked = parse_object(r#"{ "timestamp":"2026-09-27T23:24:28Z", "event":"Docked", "StationName":"i Sola Prospect", "StationType":"CraterOutpost", "Taxi":false, "Multicrew":false, "StarSystem":"Brestla", "SystemAddress":673101653409, "MarketID":3516500480, "StationEconomies":[ { "Name":"$economy_Colony;", "Name_Localised":"Colony", "Proportion":1.0 } ], "Wanted":true, "ActiveFine":true }"#).unwrap();
        let out = filter_event("Docked", &docked).unwrap();
        assert!(out.get("Wanted").is_none() && out.get("ActiveFine").is_none() && out.get("Taxi").is_none());
        assert!(out["StationEconomies"][0].get("Name_Localised").is_none());
        assert!(filter_event("FSDJump", &docked).is_none());
    }

    #[test]
    fn responses() {
        assert!(classify_response(&HttpResponse::new(200, r#"{"status":200,"message":"OK"}"#)).is_ok());
        assert!(classify_response(&HttpResponse::new(200, r#"{"status":401,"message":"nothing"}"#)).is_ok());
        assert!(classify_response(&HttpResponse::new(200, r#"{"status":500,"message":"x"}"#)).is_err());
        assert!(classify_response(&HttpResponse::new(502, "")).is_err());
        // The status as text, as EDMC accepts it.
        assert!(classify_response(&HttpResponse::new(200, r#"{"status":"200","message":"OK"}"#)).is_ok());
        assert!(classify_response(&HttpResponse::new(200, r#"{"status":"401","message":"No valid events"}"#)).is_ok());
        assert_eq!(classify_response(&HttpResponse::new(200, r#"{"status":"500","message":"db down"}"#)), Err("EDAstro status 500: \"db down\"".into()));
        // Anything else says what came back.
        assert_eq!(classify_response(&HttpResponse::new(200, "<html>\n  <b>Error</b>")), Err("unreadable response: \"<html> <b>Error</b>\"".into()));
        assert_eq!(classify_response(&HttpResponse::new(200, "")), Err("unreadable response: empty".into()));
        let long = classify_response(&HttpResponse::new(200, "x".repeat(300))).unwrap_err();
        assert!(long.ends_with("…\"") && long.len() < 120, "{long}");
    }
}
