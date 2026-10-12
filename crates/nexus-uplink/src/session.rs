//! What the uplink itself observed about the game session (from live events only).
//!
//! Each field is `Unknown` until the uplink sees the event that defines it. While unknown, the
//! app-provided [`UploadContext`] is used instead (the app may have started mid-session and
//! built its model from history). Once the uplink has seen a session boundary (`Fileheader`,
//! `LoadGame`, …) its own observations are authoritative, so a stale model value is never used.

use crate::context::UploadContext;
use crate::geo::star_pos_matches_address;
use crate::json::{Obj, get_bool, get_nonempty, get_star_pos, get_str, get_u64, is_beta_version};
use nexus_journal::{Status, Timestamp};

/// A star system with its coordinates.
#[derive(Debug, Clone, PartialEq)]
pub struct Location {
    pub system_name: String,
    pub system_address: u64,
    pub star_pos: [f64; 3],
}

#[derive(Debug, Clone, Default, PartialEq)]
enum Tracked<T> {
    /// Not observed since the uplink started: fall back to the app's context.
    #[default]
    Unknown,
    /// Explicitly reset by the journal (new session, crew change, …): no value.
    Cleared,
    Known(T),
}

impl<T> Tracked<T> {
    fn known(&self) -> Option<&T> {
        match self {
            Tracked::Known(v) => Some(v),
            _ => None,
        }
    }
}

/// The parts of Status.json the uploaders use.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StatusSnapshot {
    pub timestamp: Timestamp,
    pub body_name: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub flags: u32,
    pub flags2: u32,
}

#[derive(Debug, Clone, Default)]
pub struct Session {
    cmdr: Tracked<String>,
    fid: Tracked<String>,
    /// `(gameversion, build)` of the journal currently being written.
    version: Tracked<(String, String)>,
    /// `(Horizons, Odyssey)` from LoadGame; each `None` if the key was absent.
    flags: Tracked<(Option<bool>, Option<bool>)>,
    location: Tracked<Location>,
    /// Joined someone else's crew (`JoinACrew` with another captain).
    joined_crew: bool,
    /// Last `Multicrew` flag seen on an event ("inside another player's ship").
    multicrew_flag: bool,
    game_mode: Option<String>,
    cqc: bool,
    /// `(Body, BodyID)` from ApproachBody / Location, cleared on LeaveBody / FSDJump.
    journal_body: Option<(String, u64)>,
    status: Option<StatusSnapshot>,
}

impl Session {
    pub fn new() -> Self {
        Self::default()
    }

    /// Updates the session from one journal event (call for every live event, in order).
    pub fn apply(&mut self, name: &str, ev: &Obj) {
        if let Some(m) = get_bool(ev, "Multicrew") {
            self.multicrew_flag = m;
        }
        match name {
            "Fileheader" => {
                let gv = get_str(ev, "gameversion").unwrap_or_default().to_string();
                let build = get_str(ev, "build").unwrap_or_default().to_string();
                self.version = Tracked::Known((gv, build));
                if get_u64(ev, "part").unwrap_or(1) <= 1 {
                    // New game session (not a `Continued` part).
                    self.cmdr = Tracked::Cleared;
                    self.fid = Tracked::Cleared;
                    self.flags = Tracked::Cleared;
                    self.location = Tracked::Cleared;
                    self.joined_crew = false;
                    self.multicrew_flag = false;
                    self.game_mode = None;
                    self.cqc = false;
                    self.journal_body = None;
                }
            }
            "Commander" | "NewCommander" => {
                if let Some(n) = get_nonempty(ev, "Name") {
                    self.cmdr = Tracked::Known(n.to_string());
                }
                if let Some(f) = get_nonempty(ev, "FID") {
                    self.fid = Tracked::Known(f.to_string());
                }
            }
            "LoadGame" => {
                if let Some(n) = get_nonempty(ev, "Commander") {
                    self.cmdr = Tracked::Known(n.to_string());
                }
                if let Some(f) = get_nonempty(ev, "FID") {
                    self.fid = Tracked::Known(f.to_string());
                }
                self.flags = Tracked::Known((get_bool(ev, "Horizons"), get_bool(ev, "Odyssey")));
                if let Some(gv) = get_nonempty(ev, "gameversion") {
                    let build = get_str(ev, "build").unwrap_or_default().to_string();
                    self.version = Tracked::Known((gv.to_string(), build));
                }
                self.location = Tracked::Cleared;
                self.joined_crew = false;
                self.multicrew_flag = get_bool(ev, "Multicrew").unwrap_or(false);
                self.cqc = get_str(ev, "GameMode").is_some_and(|m| m.eq_ignore_ascii_case("CQC"));
                self.journal_body = None;
            }
            "Location" | "FSDJump" | "CarrierJump" => {
                let sys = get_nonempty(ev, "StarSystem");
                self.cqc = sys.is_some_and(|s| s == "ProvingGround" || s == "CQC");
                self.location = match (sys, get_u64(ev, "SystemAddress"), get_star_pos(ev, "StarPos")) {
                    (Some(s), Some(a), Some(p)) if !self.cqc => {
                        Tracked::Known(Location { system_name: s.to_string(), system_address: a, star_pos: p })
                    }
                    _ => Tracked::Cleared,
                };
                self.journal_body = if name == "Location" && get_str(ev, "BodyType") == Some("Planet") {
                    match (get_nonempty(ev, "Body"), get_u64(ev, "BodyID")) {
                        (Some(b), Some(id)) => Some((b.to_string(), id)),
                        _ => None,
                    }
                } else {
                    None
                };
            }
            "JoinACrew" => {
                self.location = Tracked::Cleared;
                let captain = get_str(ev, "Captain").unwrap_or_default();
                let me = self.cmdr.known().map(String::as_str).unwrap_or_default();
                self.joined_crew = !captain.eq_ignore_ascii_case(me);
            }
            "QuitACrew" => {
                self.location = Tracked::Cleared;
                self.joined_crew = false;
                self.multicrew_flag = false;
            }
            "GameModeChange" => {
                self.game_mode = get_nonempty(ev, "GameMode").map(str::to_string);
            }
            "ApproachBody" => {
                if let (Some(b), Some(id)) = (get_nonempty(ev, "Body"), get_u64(ev, "BodyID")) {
                    self.journal_body = Some((b.to_string(), id));
                }
            }
            "LeaveBody" => self.journal_body = None,
            "Music" => {
                if get_str(ev, "MusicTrack") == Some("MainMenu")
                    && let Some(s) = self.status.as_mut()
                {
                    s.body_name = None;
                }
            }
            _ => {}
        }
    }

    /// Updates the Status.json snapshot.
    pub fn apply_status(&mut self, st: &Status) {
        self.status = Some(StatusSnapshot {
            timestamp: st.timestamp,
            body_name: st.body_name.clone().filter(|b| !b.is_empty()),
            latitude: st.latitude.filter(|f| f.is_finite()),
            longitude: st.longitude.filter(|f| f.is_finite()),
            flags: st.flags,
            flags2: st.flags2,
        });
    }

    /// In-game commander name.
    pub fn cmdr<'a>(&'a self, ctx: &'a UploadContext) -> Option<&'a str> {
        match &self.cmdr {
            Tracked::Known(c) => Some(c.as_str()),
            Tracked::Cleared => None,
            Tracked::Unknown => Some(ctx.cmdr.as_str()).filter(|c| !c.trim().is_empty()),
        }
    }

    pub fn fid<'a>(&'a self, ctx: &'a UploadContext) -> Option<&'a str> {
        match &self.fid {
            Tracked::Known(c) => Some(c.as_str()),
            Tracked::Cleared => None,
            Tracked::Unknown => Some(ctx.fid.as_str()).filter(|c| !c.trim().is_empty()),
        }
    }

    /// `(gameversion, gamebuild)`, byte-exact; empty strings if unknown.
    pub fn game_version<'a>(&'a self, ctx: &'a UploadContext) -> (&'a str, &'a str) {
        match &self.version {
            Tracked::Known((v, b)) => (v.as_str(), b.as_str()),
            Tracked::Cleared => ("", ""),
            Tracked::Unknown => (ctx.game_version.as_str(), ctx.game_build.as_str()),
        }
    }

    /// `(horizons, odyssey)` from LoadGame, `None` when not known.
    pub fn expansions(&self, ctx: &UploadContext) -> (Option<bool>, Option<bool>) {
        match &self.flags {
            Tracked::Known(f) => *f,
            Tracked::Cleared => (None, None),
            Tracked::Unknown => (ctx.horizons, ctx.odyssey),
        }
    }

    /// The current system. A location from the app's context is used only if its
    /// `StarPos` is consistent with its `SystemAddress`.
    pub fn location(&self, ctx: &UploadContext) -> Option<Location> {
        match &self.location {
            Tracked::Known(l) => Some(l.clone()),
            Tracked::Cleared => None,
            Tracked::Unknown => {
                let pos = ctx.star_pos?;
                (ctx.system_address > 1
                    && !ctx.system_name.trim().is_empty()
                    && star_pos_matches_address(ctx.system_address, pos))
                .then(|| Location { system_name: ctx.system_name.clone(), system_address: ctx.system_address, star_pos: pos })
            }
        }
    }

    /// Beta/alpha game client.
    pub fn is_beta(&self, ctx: &UploadContext) -> bool {
        ctx.is_beta || is_beta_version(self.game_version(ctx).0)
    }

    /// Live galaxy (game version ≥ 4.0). Unknown versions count as not live.
    pub fn is_live_galaxy(&self, ctx: &UploadContext) -> bool {
        crate::json::game_major_version(self.game_version(ctx).0).is_some_and(|m| m >= 4)
    }

    /// Why nothing from this event may be uploaded (crew in someone else's ship, CQC, an
    /// Operation), or `None` if uploads are allowed.
    pub fn blocked(&self, ev: Option<&Obj>) -> Option<&'static str> {
        use nexus_journal::companion::flags2;
        if self.joined_crew || self.multicrew_flag || ev.and_then(|e| get_bool(e, "Multicrew")) == Some(true) {
            return Some("multicrew");
        }
        if self.status.as_ref().is_some_and(|s| s.flags2 & flags2::IN_MULTICREW != 0) {
            return Some("multicrew");
        }
        if self.cqc {
            return Some("CQC");
        }
        if let Some(m) = &self.game_mode
            && m != "MainGame"
        {
            return Some("not in the main game");
        }
        None
    }

    /// `(Body, BodyID)` the commander approached (journal tracking).
    pub fn journal_body(&self) -> Option<(&str, u64)> {
        self.journal_body.as_ref().map(|(b, id)| (b.as_str(), *id))
    }

    pub fn status(&self) -> Option<&StatusSnapshot> {
        self.status.as_ref()
    }

    /// True once the uplink has seen the session's own `Fileheader`/`LoadGame`.
    pub fn has_own_session(&self) -> bool {
        !matches!(self.cmdr, Tracked::Unknown)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json::parse_object;

    fn apply(s: &mut Session, line: &str) {
        let o = parse_object(line).unwrap();
        let name = get_str(&o, "event").unwrap().to_string();
        s.apply(&name, &o);
    }

    #[test]
    fn falls_back_to_context_until_session_seen() {
        let mut s = Session::new();
        let ctx = UploadContext {
            cmdr: "Jameson".into(),
            game_version: "4.4.1.1".into(),
            game_build: "r332841/r0 ".into(),
            horizons: Some(true),
            odyssey: Some(true),
            system_name: "Sol".into(),
            system_address: 10_477_373_803,
            star_pos: Some([0.0, 0.0, 0.0]),
            ..Default::default()
        };
        assert_eq!(s.cmdr(&ctx), Some("Jameson"));
        assert_eq!(s.game_version(&ctx), ("4.4.1.1", "r332841/r0 "));
        assert_eq!(s.location(&ctx).unwrap().system_name, "Sol");
        // an inconsistent model location is not trusted
        let bad = UploadContext { star_pos: Some([220.96875, -124.8125, -12.59375]), ..ctx.clone() };
        assert!(s.location(&bad).is_none());

        apply(&mut s, r#"{ "timestamp":"2026-09-05T00:01:46Z", "event":"Fileheader", "part":1, "language":"English/UK", "Odyssey":true, "gameversion":"4.4.1.2", "build":"r333000/r0 " }"#);
        assert_eq!(s.cmdr(&ctx), None);
        assert!(s.location(&ctx).is_none());
        assert_eq!(s.game_version(&ctx), ("4.4.1.2", "r333000/r0 "));
        assert_eq!(s.expansions(&ctx), (None, None));
        apply(&mut s, r#"{ "timestamp":"2026-09-05T00:13:30Z", "event":"LoadGame", "FID":"F1", "Commander":"Jameson", "Horizons":true, "Credits":1, "Loan":0 }"#);
        assert_eq!(s.expansions(&ctx), (Some(true), None));
        apply(&mut s, r#"{ "timestamp":"2026-09-05T01:06:02Z", "event":"FSDJump", "Taxi":false, "Multicrew":false, "StarSystem":"Synuefe BR-L c24-23", "SystemAddress":6406446944978, "StarPos":[220.96875,-124.81250,-12.59375] }"#);
        assert_eq!(s.location(&ctx).unwrap().system_address, 6_406_446_944_978);
        assert!(s.blocked(None).is_none());
    }

    #[test]
    fn crew_and_modes_block() {
        let mut s = Session::new();
        apply(&mut s, r#"{ "timestamp":"2026-09-05T00:13:30Z", "event":"LoadGame", "FID":"F1", "Commander":"Jameson", "Horizons":true, "Odyssey":true }"#);
        apply(&mut s, r#"{ "timestamp":"2026-09-05T00:14:30Z", "event":"JoinACrew", "Captain":"Someone" }"#);
        assert_eq!(s.blocked(None), Some("multicrew"));
        apply(&mut s, r#"{ "timestamp":"2026-09-05T00:15:30Z", "event":"QuitACrew", "Captain":"Someone" }"#);
        assert!(s.blocked(None).is_none());
        apply(&mut s, r#"{ "timestamp":"2026-09-05T00:16:30Z", "event":"GameModeChange", "GameMode":"Operation" }"#);
        assert!(s.blocked(None).is_some());
        apply(&mut s, r#"{ "timestamp":"2026-09-05T00:17:30Z", "event":"GameModeChange", "GameMode":"MainGame" }"#);
        assert!(s.blocked(None).is_none());
        let ev = parse_object(r#"{"event":"Docked","Multicrew":true}"#).unwrap();
        assert!(s.blocked(Some(&ev)).is_some());
    }

    #[test]
    fn continued_journal_keeps_the_session() {
        let mut s = Session::new();
        let ctx = UploadContext::default();
        apply(&mut s, r#"{ "timestamp":"2026-09-05T00:13:30Z", "event":"LoadGame", "FID":"F1", "Commander":"Jameson", "Horizons":true, "Odyssey":true, "gameversion":"4.4.1.1", "build":"r332841/r0 " }"#);
        apply(&mut s, r#"{ "timestamp":"2026-09-05T01:06:02Z", "event":"FSDJump", "StarSystem":"Sol", "SystemAddress":10477373803, "StarPos":[0,0,0] }"#);
        apply(&mut s, r#"{ "timestamp":"2026-09-05T02:00:00Z", "event":"Fileheader", "part":2, "language":"English/UK", "Odyssey":true, "gameversion":"4.4.1.1", "build":"r332841/r0 " }"#);
        assert_eq!(s.cmdr(&ctx), Some("Jameson"));
        assert!(s.location(&ctx).is_some());
    }
}
