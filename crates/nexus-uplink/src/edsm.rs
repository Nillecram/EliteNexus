//! EDSM journal sync (`POST https://www.edsm.net/api-journal-v1`), see `docs/research/edsm.md`.
//!
//! * events are sent unmodified plus EDSM's six transient fields (`_systemAddress`,
//!   `_systemName`, `_systemCoordinates`, `_marketId`, `_stationName`, `_shipId`; `null` when
//!   unknown), in batches that never mix commander, credentials or game version/build;
//! * the discard list is fetched once (cached on disk, refreshed after 6 h, bundled fallback);
//! * stub events (`Cargo`, `ShipLocker`, `Backpack`, `FCMaterials`) are replaced by their
//!   companion file, as EDMC does, so EDSM gets the full inventory;
//! * only live, non-beta, Live-galaxy (4.x) data, never as crew in someone else's ship.

use crate::context::UploadContext;
use crate::json::{Obj, get_array, get_str};
use crate::transport::HttpResponse;
use nexus_journal::{CompanionKind, Timestamp};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub const JOURNAL_URL: &str = "https://www.edsm.net/api-journal-v1";
pub const DISCARD_URL: &str = "https://www.edsm.net/api-journal-v1/discard";

/// EDSM's live discard list as of 2026-10-12 (used until the live list has been fetched).
pub const DEFAULT_DISCARD: [&str; 141] = [
    "AfmuRepairs", "AppliedToSquadron", "ApproachBody", "AsteroidCracked", "BookDropship", "Bounty", "CancelDropship",
    "CapShipBond", "CargoTransfer", "CarrierBankTransfer", "CarrierBuy", "CarrierCrewServices", "CarrierDecommission",
    "CarrierDepositFuel", "CarrierDockingPermission", "CarrierFinance", "CarrierJumpCancelled", "CarrierJumpRequest",
    "CarrierModulePack", "CarrierNameChange", "CarrierStats", "CarrierTradeOrder", "ChangeCrewRole", "ClearSavedGame",
    "CockpitBreached", "CollectItems", "ColonisationConstructionDepot", "ColonisationContribution", "Commander",
    "Continued", "Coriolis", "CreateSuitLoadout", "CrewAssign", "CrewFire", "CrewLaunchFighter", "CrewMemberJoins",
    "CrewMemberQuits", "CrewMemberRoleChange", "CrimeVictim", "DatalinkScan", "DatalinkVoucher", "DataScanned",
    "DisbandedSquadron", "DiscoveryScan", "Disembark", "DockFighter", "DockingCancelled", "DockingDenied",
    "DockingGranted", "DockingRequested", "DockingTimeout", "DockSRV", "DropItems", "DropshipDeploy",
    "EDDCommodityPrices", "EDDItemSet", "EDShipyard", "Embark", "EndCrewSession", "EngineerApply",
    "EngineerLegacyConvert", "EscapeInterdiction", "FactionKillBond", "FighterDestroyed", "FighterRebuilt",
    "Fileheader", "FSSBodySignals", "FSSSignalDiscovered", "FuelScoop", "HeatDamage", "HeatWarning", "HullDamage",
    "InvitedToSquadron", "JetConeBoost", "JetConeDamage", "JoinedSquadron", "KickCrewMember", "LaunchDrone",
    "LaunchFighter", "LaunchSRV", "LeaveBody", "LeftSquadron", "Liftoff", "LoadoutEquipModule", "Market",
    "MassModuleStore", "MaterialDiscovered", "ModuleArrived", "ModuleInfo", "ModuleStore", "ModuleSwap", "Music",
    "NavBeaconScan", "NavRoute", "NavRouteClear", "NewCommander", "NpcCrewRank", "Outfitting", "Passengers",
    "PowerplayMerits", "PowerplayVote", "PowerplayVoucher", "ProspectedAsteroid", "PVPKill", "RebootRepair",
    "ReceiveText", "RepairDrone", "ReservoirReplenished", "SAASignalsFound", "ScanBaryCentre", "Scanned",
    "ScanOrganic", "Screenshot", "SendText", "SharedBookmarkToSquadron", "ShieldState", "ShipArrived",
    "ShipTargeted", "Shipyard", "ShipyardNew", "ShutDown", "Shutdown", "SquadronCreated", "SquadronStartup",
    "SRVDestroyed", "StartJump", "Status", "StoredModules", "SuitLoadout", "SupercruiseDestinationDrop",
    "SupercruiseEntry", "SupercruiseExit", "SwitchSuitLoadout", "SystemsShutdown", "Touchdown", "UnderAttack",
    "VehicleSwitch", "WingAdd", "WingInvite", "WingJoin", "WingLeave",
];

/// Events that make the pending batch go out right away (location changes, session edges).
pub const URGENT_EVENTS: [&str; 7] = ["FSDJump", "CarrierJump", "Location", "Docked", "LoadGame", "Shutdown", "Died"];

/// One queued EDSM event. The API key itself is never written to disk, only a fingerprint of
/// the credentials it was queued for.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdsmItem {
    /// In-game commander (batches never mix commanders).
    pub cmdr: String,
    pub game_version: String,
    pub game_build: String,
    /// Fingerprint of the EDSM credentials at queue time.
    pub cred: u64,
    pub event: String,
    pub event_time: Timestamp,
    pub queued_at: Timestamp,
    pub urgent: bool,
    /// The event JSON with the transient fields.
    pub json: String,
}

impl EdsmItem {
    pub fn batch_key(&self) -> (&str, &str, &str, u64) {
        (&self.cmdr, &self.game_version, &self.game_build, self.cred)
    }
}

/// Is `name` on the discard list? (`Docked` is always kept, as EDMC does.)
pub fn is_discarded(list: &std::collections::HashSet<String>, name: &str) -> bool {
    name != "Docked" && list.contains(name)
}

/// Adds EDSM's transient state fields (`null` when unknown).
pub fn with_transient_fields(ev: &Obj, ctx: &UploadContext) -> Obj {
    let mut out = ev.clone();
    let opt_str = |s: &str| if s.is_empty() { Value::Null } else { json!(s) };
    out.insert("_systemAddress".into(), if ctx.system_address > 0 { json!(ctx.system_address) } else { Value::Null });
    out.insert("_systemName".into(), opt_str(&ctx.system_name));
    out.insert("_systemCoordinates".into(), ctx.star_pos.map(|p| json!(p)).unwrap_or(Value::Null));
    out.insert("_marketId".into(), ctx.market_id.map(|m| json!(m)).unwrap_or(Value::Null));
    out.insert("_stationName".into(), ctx.station_name.as_deref().map(opt_str).unwrap_or(Value::Null));
    out.insert("_shipId".into(), ctx.ship_id.map(|s| json!(s)).unwrap_or(Value::Null));
    out
}

/// A journal event whose content lives in a companion file (EDMC replaces these).
pub fn stub_companion(name: &str, ev: &Obj) -> Option<CompanionKind> {
    match name {
        "Cargo" if get_str(ev, "Vessel") == Some("Ship") && !ev.contains_key("Inventory") => Some(CompanionKind::Cargo),
        "ShipLocker" if !ev.contains_key("Items") => Some(CompanionKind::ShipLocker),
        "Backpack" if !ev.contains_key("Items") => Some(CompanionKind::Backpack),
        "FCMaterials" if !ev.contains_key("Items") => Some(CompanionKind::FCMaterials),
        _ => None,
    }
}

/// What an EDSM journal response means for the batch.
#[derive(Debug, Clone, PartialEq)]
pub enum EdsmOutcome {
    /// Processed: per-event `(msgnum, msg)`; the batch is done.
    Done(Vec<(i64, String)>),
    /// Credentials problem (201–203): stop until the configuration changes.
    BadCredentials(String),
    /// Software unknown or blacklisted (204/205): stop until the configuration changes.
    SoftwareRejected(String),
    /// The batch itself is unacceptable (206–208): drop it.
    BadBatch(String),
    /// Rate limited: retry after this many seconds.
    RateLimited(u64),
    /// Server or transport trouble: keep the batch and back off.
    Retry(String),
}

pub fn classify_response(resp: &HttpResponse, now: Timestamp) -> EdsmOutcome {
    if resp.status == 429 {
        return EdsmOutcome::RateLimited(rate_limit_wait(resp, now).unwrap_or(300));
    }
    if !resp.is_success() {
        return EdsmOutcome::Retry(format!("HTTP {}", resp.status));
    }
    let Ok(body) = serde_json::from_str::<Value>(&resp.body) else {
        return EdsmOutcome::Retry("unreadable response".into());
    };
    let msgnum = body.get("msgnum").and_then(Value::as_i64).unwrap_or(0);
    let msg = body.get("msg").and_then(Value::as_str).unwrap_or_default().to_string();
    let text = format!("EDSM {msgnum}: {msg}");
    match msgnum {
        100..=199 | 500..=599 => {
            // 5xx "Exception": EDSM keeps the message for later re-processing (EDMC: treat as sent).
            let per_event = get_array(body.as_object().unwrap_or(&Obj::new()), "events")
                .map(|evs| {
                    evs.iter()
                        .map(|e| {
                            (
                                e.get("msgnum").and_then(Value::as_i64).unwrap_or(msgnum),
                                e.get("msg").and_then(Value::as_str).unwrap_or_default().to_string(),
                            )
                        })
                        .collect()
                })
                .unwrap_or_default();
            EdsmOutcome::Done(per_event)
        }
        201..=203 => EdsmOutcome::BadCredentials(text),
        204 | 205 => EdsmOutcome::SoftwareRejected(text),
        206..=208 => EdsmOutcome::BadBatch(text),
        429 => EdsmOutcome::RateLimited(rate_limit_wait(resp, now).unwrap_or(300)),
        _ => EdsmOutcome::Retry(text),
    }
}

/// `X-Rate-Limit-Remaining`/`X-Rate-Limit-Reset`: seconds to wait when the budget is exhausted.
/// The reset value is treated as an epoch if it is large, else as a delay (clients disagree);
/// capped at one hour.
pub fn rate_limit_wait(resp: &HttpResponse, now: Timestamp) -> Option<u64> {
    let remaining: i64 = resp.header("X-Rate-Limit-Remaining")?.trim().parse().ok()?;
    if remaining > 1 && resp.status != 429 {
        return None;
    }
    let reset: i64 = resp.header("X-Rate-Limit-Reset").and_then(|v| v.trim().parse().ok()).unwrap_or(60);
    let wait = if reset > 1_000_000_000 { reset - now.0 } else { reset };
    Some(wait.clamp(1, 3600) as u64)
}

/// On-disk cache of the discard list.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscardCache {
    pub fetched: Timestamp,
    pub events: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json::parse_object;

    #[test]
    fn transient_fields_are_added_with_nulls() {
        let ev = parse_object(r#"{ "timestamp":"2026-10-12T08:32:40Z", "event":"Loadout", "Ship":"explorer_nx", "ShipID":22 }"#).unwrap();
        let ctx = UploadContext { system_name: "Ross 775".into(), system_address: 1, ship_id: Some(22), ..Default::default() };
        let out = with_transient_fields(&ev, &ctx);
        assert_eq!(out["_systemName"], json!("Ross 775"));
        assert_eq!(out["_shipId"], json!(22));
        assert_eq!(out["_marketId"], Value::Null);
        assert_eq!(out["_systemCoordinates"], Value::Null);
        assert_eq!(out["Ship"], json!("explorer_nx"), "the event itself is unmodified");
    }

    #[test]
    fn discard_list_keeps_docked() {
        let set: std::collections::HashSet<String> = ["Docked", "Music"].iter().map(|s| s.to_string()).collect();
        assert!(!is_discarded(&set, "Docked"));
        assert!(is_discarded(&set, "Music"));
        assert!(DEFAULT_DISCARD.contains(&"FSSSignalDiscovered"));
        assert!(!DEFAULT_DISCARD.contains(&"FSDJump"));
    }

    #[test]
    fn stubs_are_detected() {
        let stub = parse_object(r#"{ "timestamp":"2026-10-12T06:29:46Z", "event":"Cargo", "Vessel":"Ship", "Count":185 }"#).unwrap();
        assert_eq!(stub_companion("Cargo", &stub), Some(CompanionKind::Cargo));
        let full = parse_object(r#"{ "timestamp":"2026-10-12T06:29:46Z", "event":"Cargo", "Vessel":"Ship", "Count":0, "Inventory":[] }"#).unwrap();
        assert_eq!(stub_companion("Cargo", &full), None);
        let srv = parse_object(r#"{ "timestamp":"2026-10-12T06:29:46Z", "event":"Cargo", "Vessel":"SRV", "Count":0 }"#).unwrap();
        assert_eq!(stub_companion("Cargo", &srv), None);
    }

    #[test]
    fn responses() {
        let now = Timestamp(1_800_000_000);
        let ok = HttpResponse::new(200, r#"{"msgnum":100,"msg":"OK","events":[{"msgnum":100,"msg":"OK"},{"msgnum":304,"msg":"Discarded event"}]}"#);
        assert_eq!(classify_response(&ok, now), EdsmOutcome::Done(vec![(100, "OK".into()), (304, "Discarded event".into())]));
        let bad = HttpResponse::new(200, r#"{"msgnum":203,"msg":"Commander name/API Key not found","events":[]}"#);
        assert!(matches!(classify_response(&bad, now), EdsmOutcome::BadCredentials(_)));
        let black = HttpResponse::new(200, r#"{"msgnum":205,"msg":"Blacklisted software"}"#);
        assert!(matches!(classify_response(&black, now), EdsmOutcome::SoftwareRejected(_)));
        let old = HttpResponse::new(200, r#"{"msgnum":208,"msg":"Game/Build version too old"}"#);
        assert!(matches!(classify_response(&old, now), EdsmOutcome::BadBatch(_)));
        assert!(matches!(classify_response(&HttpResponse::new(502, ""), now), EdsmOutcome::Retry(_)));
        assert!(matches!(classify_response(&HttpResponse::new(200, "<html>"), now), EdsmOutcome::Retry(_)));
        let mut limited = HttpResponse::new(200, r#"{"msgnum":100,"msg":"OK","events":[]}"#);
        limited.headers = vec![("x-rate-limit-remaining".into(), "0".into()), ("X-Rate-Limit-Reset".into(), "1800000120".into())];
        assert_eq!(rate_limit_wait(&limited, now), Some(120));
        limited.headers[1].1 = "45".into();
        assert_eq!(rate_limit_wait(&limited, now), Some(45));
    }
}
