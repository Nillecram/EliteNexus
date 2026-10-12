//! The game state the app hands to the uplink with every live event.

use crate::json::{Obj, get_bool, get_nonempty, get_star_pos, get_str, get_u64, is_beta_version, parse_object};
use nexus_journal::RawEvent;
use serde::{Deserialize, Serialize};

/// What the app's model knows *after* applying a live event. The uplink uses it as the
/// fallback for anything it has not observed itself since it started (e.g. when the app starts
/// mid-session, the commander, game version and location come from the history-built model),
/// and for EDSM's transient `_system*`/`_station*`/`_shipId` fields.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UploadContext {
    /// In-game commander name.
    pub cmdr: String,
    /// Frontier ID (`F1234567`), empty if unknown.
    pub fid: String,
    /// `Fileheader.gameversion` (else `LoadGame.gameversion`), byte-exact.
    pub game_version: String,
    /// `Fileheader.build` (else `LoadGame.build`), byte-exact including the trailing space.
    pub game_build: String,
    /// `LoadGame.Horizons`, None if unknown.
    pub horizons: Option<bool>,
    /// `LoadGame.Odyssey`, None if unknown or absent.
    pub odyssey: Option<bool>,
    pub system_name: String,
    /// 0 if unknown.
    pub system_address: u64,
    pub star_pos: Option<[f64; 3]>,
    /// Docked station.
    pub market_id: Option<u64>,
    pub station_name: Option<String>,
    pub ship_id: Option<u64>,
    /// Beta/alpha game client.
    pub is_beta: bool,
}

/// Builds an [`UploadContext`] from journal events the way EDSM's reference `updateGameStatus`
/// tracker does. The app can use its own model instead; this exists for replays (the
/// `validate` example), tests, and as a reference implementation.
#[derive(Debug, Clone, Default)]
pub struct ContextTracker {
    ctx: UploadContext,
}

impl ContextTracker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn context(&self) -> &UploadContext {
        &self.ctx
    }

    /// Applies one journal event (unparseable lines are ignored).
    pub fn apply(&mut self, raw: &RawEvent) {
        if let Some(obj) = parse_object(&raw.line) {
            self.apply_json(&raw.name, &obj);
        }
    }

    pub fn apply_json(&mut self, name: &str, ev: &Obj) {
        let c = &mut self.ctx;
        match name {
            "Fileheader" => {
                let part = get_u64(ev, "part").unwrap_or(1);
                c.game_version = get_str(ev, "gameversion").unwrap_or_default().to_string();
                c.game_build = get_str(ev, "build").unwrap_or_default().to_string();
                c.is_beta = is_beta_version(&c.game_version);
                if part <= 1 {
                    // A new game session (not a continuation file).
                    c.cmdr.clear();
                    c.fid.clear();
                    c.horizons = None;
                    c.odyssey = None;
                    clear_location(c);
                    c.ship_id = None;
                }
            }
            "Commander" | "NewCommander" => {
                if let Some(n) = get_nonempty(ev, "Name") {
                    c.cmdr = n.to_string();
                }
                if let Some(f) = get_nonempty(ev, "FID") {
                    c.fid = f.to_string();
                }
            }
            "LoadGame" => {
                if let Some(n) = get_nonempty(ev, "Commander") {
                    c.cmdr = n.to_string();
                }
                if let Some(f) = get_nonempty(ev, "FID") {
                    c.fid = f.to_string();
                }
                c.horizons = get_bool(ev, "Horizons");
                c.odyssey = get_bool(ev, "Odyssey");
                if let Some(v) = get_nonempty(ev, "gameversion") {
                    c.game_version = v.to_string();
                    c.game_build = get_str(ev, "build").unwrap_or_default().to_string();
                    c.is_beta = is_beta_version(v);
                }
                c.ship_id = get_u64(ev, "ShipID");
                clear_location(c);
            }
            "Location" | "FSDJump" | "CarrierJump" | "Docked" => {
                if let Some(sys) = get_nonempty(ev, "StarSystem") {
                    if sys != c.system_name {
                        c.star_pos = None;
                    }
                    if sys == "ProvingGround" || sys == "CQC" {
                        clear_location(c);
                    } else {
                        c.system_name = sys.to_string();
                        if let Some(a) = get_u64(ev, "SystemAddress") {
                            c.system_address = a;
                        }
                        if let Some(p) = get_star_pos(ev, "StarPos") {
                            c.star_pos = Some(p);
                        }
                    }
                }
                let docked = name == "Docked" || get_bool(ev, "Docked") == Some(true);
                if docked {
                    c.market_id = get_u64(ev, "MarketID");
                    c.station_name = get_nonempty(ev, "StationName").map(str::to_string);
                } else if name != "CarrierJump" {
                    c.market_id = None;
                    c.station_name = None;
                }
            }
            "Undocked" => {
                c.market_id = None;
                c.station_name = None;
            }
            "JoinACrew" | "QuitACrew" => clear_location(c),
            "SetUserShipName" | "ShipyardSwap" | "Loadout" => {
                if let Some(id) = get_u64(ev, "ShipID") {
                    c.ship_id = Some(id);
                }
            }
            "ShipyardBuy" => c.ship_id = None,
            "ShipyardNew" => c.ship_id = get_u64(ev, "NewShipID"),
            _ => {}
        }
    }
}

fn clear_location(c: &mut UploadContext) {
    c.system_name.clear();
    c.system_address = 0;
    c.star_pos = None;
    c.market_id = None;
    c.station_name = None;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn apply(t: &mut ContextTracker, line: &str) {
        let raw = RawEvent::new(line, Default::default()).unwrap();
        t.apply(&raw);
    }

    #[test]
    fn tracks_a_session() {
        let mut t = ContextTracker::new();
        apply(&mut t, r#"{ "timestamp":"2026-09-05T00:01:46Z", "event":"Fileheader", "part":1, "language":"English/UK", "Odyssey":true, "gameversion":"4.4.1.1", "build":"r332841/r0 " }"#);
        apply(&mut t, r#"{ "timestamp":"2026-09-05T00:13:30Z", "event":"LoadGame", "FID":"F1", "Commander":"Jameson", "Horizons":true, "Odyssey":true, "Ship":"Empire_Courier", "ShipID":17, "GameMode":"Solo", "Credits":1, "Loan":0, "gameversion":"4.4.1.1", "build":"r332841/r0 " }"#);
        apply(&mut t, r#"{ "timestamp":"2026-09-05T00:14:00Z", "event":"Location", "Docked":true, "StationName":"Jameson Memorial", "MarketID":128666762, "StarSystem":"Shinrarta Dezhra", "SystemAddress":3932277478106, "StarPos":[55.71875,17.59375,27.15625] }"#);
        let c = t.context();
        assert_eq!(c.cmdr, "Jameson");
        assert_eq!(c.game_build, "r332841/r0 ");
        assert_eq!(c.odyssey, Some(true));
        assert_eq!(c.system_address, 3_932_277_478_106);
        assert_eq!(c.market_id, Some(128_666_762));
        assert_eq!(c.ship_id, Some(17));
        apply(&mut t, r#"{ "timestamp":"2026-09-05T00:15:00Z", "event":"Undocked", "StationName":"Jameson Memorial", "MarketID":128666762 }"#);
        assert_eq!(t.context().market_id, None);
        assert_eq!(t.context().system_name, "Shinrarta Dezhra");
    }
}
