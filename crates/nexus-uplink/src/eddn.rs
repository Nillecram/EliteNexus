//! EDDN message builder: live journal events and companion files in, schema-valid EDDN messages
//! out. Pure (no I/O, no clock), so it can be replayed over real journals and unit-tested.
//!
//! Rules implemented (see `docs/research/eddn.md`):
//! * envelope = `$schemaRef` + `header` + `message` only; header `uploaderID` (CMDR),
//!   `softwareName`, `softwareVersion`, and always `gameversion`/`gamebuild` byte-exact from
//!   `Fileheader`/`LoadGame` (never `gatewayTimestamp`);
//! * `horizons`/`odyssey` only when known from `LoadGame` (never `null`, never guessed);
//! * every `*_Localised` key removed at any depth, plus each schema's personal-data keys;
//! * every schema except `journal/1` is built from a whitelist of the schema's properties;
//! * `StarSystem`/`StarPos`/`SystemAddress` are added only from the last
//!   `Location`/`FSDJump`/`CarrierJump`, and only after the event's `SystemAddress` matched it;
//! * `/test` schemas for beta clients, test mode and pre-release software;
//! * `uniqueItems` arrays de-duplicated; unchanged station data not re-sent;
//! * `FSSSignalDiscovered` runs batched per system (Odyssey and Horizons orderings);
//! * nothing while crew in someone else's ship, in CQC, or inside an Operation.

use crate::json::{
    Obj, as_star_pos, fnv1a64, get_array, get_bool, get_nonempty, get_star_pos, get_str, get_u64, has_localised, has_null,
    is_date_time, is_int, parse_object, star_pos_value, strip_localised_obj, strip_symbol,
};
use crate::context::UploadContext;
use crate::session::{Location, Session};
use nexus_journal::{Companion, CompanionKind, Timestamp};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

/// The live EDDN gateway (HTTPS on 4430, trailing slash required).
pub const UPLOAD_URL: &str = "https://eddn.edcd.io:4430/upload/";

/// Every schema Elite Nexus sends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Schema {
    Journal,
    Commodity,
    Outfitting2,
    Outfitting3,
    Shipyard,
    NavRoute,
    FssDiscoveryScan,
    FssAllBodiesFound,
    FssBodySignals,
    FssSignalDiscovered,
    NavBeaconScan,
    CodexEntry,
    ScanBaryCentre,
    ApproachSettlement,
    DockingGranted,
    DockingDenied,
    FcMaterialsJournal,
    ScanOrganic,
}

impl Schema {
    pub const ALL: [Schema; 18] = [
        Schema::Journal,
        Schema::Commodity,
        Schema::Outfitting2,
        Schema::Outfitting3,
        Schema::Shipyard,
        Schema::NavRoute,
        Schema::FssDiscoveryScan,
        Schema::FssAllBodiesFound,
        Schema::FssBodySignals,
        Schema::FssSignalDiscovered,
        Schema::NavBeaconScan,
        Schema::CodexEntry,
        Schema::ScanBaryCentre,
        Schema::ApproachSettlement,
        Schema::DockingGranted,
        Schema::DockingDenied,
        Schema::FcMaterialsJournal,
        Schema::ScanOrganic,
    ];

    /// `journal/1`, `commodity/3`, …
    pub fn path(self) -> &'static str {
        match self {
            Schema::Journal => "journal/1",
            Schema::Commodity => "commodity/3",
            Schema::Outfitting2 => "outfitting/2",
            Schema::Outfitting3 => "outfitting/3",
            Schema::Shipyard => "shipyard/2",
            Schema::NavRoute => "navroute/1",
            Schema::FssDiscoveryScan => "fssdiscoveryscan/1",
            Schema::FssAllBodiesFound => "fssallbodiesfound/1",
            Schema::FssBodySignals => "fssbodysignals/1",
            Schema::FssSignalDiscovered => "fsssignaldiscovered/1",
            Schema::NavBeaconScan => "navbeaconscan/1",
            Schema::CodexEntry => "codexentry/1",
            Schema::ScanBaryCentre => "scanbarycentre/1",
            Schema::ApproachSettlement => "approachsettlement/1",
            Schema::DockingGranted => "dockinggranted/1",
            Schema::DockingDenied => "dockingdenied/1",
            Schema::FcMaterialsJournal => "fcmaterials_journal/1",
            Schema::ScanOrganic => "scanorganic/1",
        }
    }

    /// The `$schemaRef` value (`…/test` for test messages).
    pub fn schema_ref(self, test: bool) -> String {
        format!("https://eddn.edcd.io/schemas/{}{}", self.path(), if test { "/test" } else { "" })
    }

    /// Parses a `$schemaRef`, returning the schema and whether it is a `/test` ref.
    pub fn from_schema_ref(r: &str) -> Option<(Schema, bool)> {
        let rest = r.strip_prefix("https://eddn.edcd.io/schemas/")?;
        let (path, test) = match rest.strip_suffix("/test") {
            Some(p) => (p, true),
            None => (rest, false),
        };
        Schema::ALL.into_iter().find(|s| s.path() == path).map(|s| (s, test))
    }

    /// Station data (market/outfitting/shipyard/carrier bartender) goes stale quickly.
    pub fn is_station_data(self) -> bool {
        matches!(self, Schema::Commodity | Schema::Outfitting2 | Schema::Outfitting3 | Schema::Shipyard | Schema::FcMaterialsJournal)
    }

    /// File name of the schema in `docs/research/eddn-schemas/`.
    pub fn schema_file(self) -> &'static str {
        match self {
            Schema::Journal => "journal-v1.0.json",
            Schema::Commodity => "commodity-v3.0.json",
            Schema::Outfitting2 => "outfitting-v2.0.json",
            Schema::Outfitting3 => "outfitting-v3.0.json",
            Schema::Shipyard => "shipyard-v2.0.json",
            Schema::NavRoute => "navroute-v1.0.json",
            Schema::FssDiscoveryScan => "fssdiscoveryscan-v1.0.json",
            Schema::FssAllBodiesFound => "fssallbodiesfound-v1.0.json",
            Schema::FssBodySignals => "fssbodysignals-v1.0.json",
            Schema::FssSignalDiscovered => "fsssignaldiscovered-v1.0.json",
            Schema::NavBeaconScan => "navbeaconscan-v1.0.json",
            Schema::CodexEntry => "codexentry-v1.0.json",
            Schema::ScanBaryCentre => "scanbarycentre-v1.0.json",
            Schema::ApproachSettlement => "approachsettlement-v1.0.json",
            Schema::DockingGranted => "dockinggranted-v1.0.json",
            Schema::DockingDenied => "dockingdenied-v1.0.json",
            Schema::FcMaterialsJournal => "fcmaterials_journal-v1.0.json",
            Schema::ScanOrganic => "develop-branch_scanorganic-v1.0.json",
        }
    }
}

/// One complete EDDN message, ready to be serialised, gzipped and POSTed.
#[derive(Debug, Clone)]
pub struct EddnMessage {
    pub schema: Schema,
    /// Uses the `/test` schema.
    pub test: bool,
    /// `{"$schemaRef", "header", "message"}`.
    pub envelope: Value,
    /// Time of the journal event / companion file (for queue expiry).
    pub event_time: Timestamp,
}

impl EddnMessage {
    pub fn schema_ref(&self) -> &str {
        self.envelope.get("$schemaRef").and_then(Value::as_str).unwrap_or_default()
    }

    pub fn message(&self) -> Option<&Obj> {
        self.envelope.get("message").and_then(Value::as_object)
    }

    /// The exact body bytes sent (before gzip).
    pub fn to_json(&self) -> String {
        self.envelope.to_string()
    }
}

/// Builder settings.
#[derive(Debug, Clone, PartialEq)]
pub struct EddnOptions {
    pub software_name: String,
    pub software_version: String,
    /// Always use `/test` schemas (test mode, pre-release software).
    pub force_test: bool,
    /// Also send `outfitting/3` (merc-coin prices) next to `outfitting/2`. Off by default: EDDN's
    /// research recommends v2 for listener compatibility, v3 optionally as a second message.
    pub outfitting_v3: bool,
}

impl Default for EddnOptions {
    fn default() -> Self {
        EddnOptions {
            software_name: crate::config::DEFAULT_SOFTWARE_NAME.to_string(),
            software_version: env!("CARGO_PKG_VERSION").to_string(),
            force_test: false,
            outfitting_v3: false,
        }
    }
}

/// A journal event that announced a companion file which has not been read yet.
#[derive(Debug, Clone)]
struct Announcement {
    kind: CompanionKind,
    ts: Timestamp,
    market_id: Option<u64>,
}

/// Why a drop happened, for diagnostics.
type Reason = String;

/// Builds EDDN messages from a live event stream.
pub struct EddnBuilder {
    opts: EddnOptions,
    /// Pending contiguous run of `FSSSignalDiscovered` events.
    signals: Vec<Obj>,
    announced: Vec<Announcement>,
    /// `(schema, MarketID)` → (content hash, event time) of the last station message sent.
    station_sent: HashMap<(Schema, u64), (u64, i64)>,
    latest_ts: Timestamp,
    drops: BTreeMap<String, u64>,
}

/// How long identical station data is not re-sent (seconds of game time).
const STATION_DEDUPE_SECS: i64 = 3600;
/// How long an announcement waits for its companion file (seconds of game time).
const ANNOUNCEMENT_TTL_SECS: i64 = 60;

const JOURNAL_EVENTS: [&str; 6] = ["Docked", "FSDJump", "Scan", "Location", "SAASignalsFound", "CarrierJump"];
/// Top-level keys `journal/1` disallows (personal data).
const JOURNAL_DISALLOWED: [&str; 13] = [
    "ActiveFine",
    "CockpitBreach",
    "BoostUsed",
    "FuelLevel",
    "FuelUsed",
    "JumpDist",
    "Latitude",
    "Longitude",
    "Wanted",
    "IsNewEntry",
    "NewTraitsDiscovered",
    "Traits",
    "VoucherAmount",
];
/// `Factions[]` keys `journal/1` disallows (personal data).
const FACTION_DISALLOWED: [&str; 4] = ["HappiestSystem", "HomeSystem", "MyReputation", "SquadronFaction"];

impl EddnBuilder {
    pub fn new(opts: EddnOptions) -> Self {
        EddnBuilder {
            opts,
            signals: Vec::new(),
            announced: Vec::new(),
            station_sent: HashMap::new(),
            latest_ts: Timestamp::ZERO,
            drops: BTreeMap::new(),
        }
    }

    pub fn options(&self) -> &EddnOptions {
        &self.opts
    }

    pub fn set_options(&mut self, opts: EddnOptions) {
        self.opts = opts;
    }

    /// Counts of events that produced no message, by `"<what>: <reason>"`.
    pub fn drops(&self) -> &BTreeMap<String, u64> {
        &self.drops
    }

    fn note_drop(&mut self, what: &str, reason: &str) {
        *self.drops.entry(format!("{what}: {reason}")).or_default() += 1;
    }

    /// Feeds one journal event (live, in order). The event is applied to `session` after any
    /// pending `FSSSignalDiscovered` batch is closed and before this event's own messages are
    /// built. With `emit == false` only state is tracked (EDDN disabled, or not live).
    pub fn on_event(&mut self, session: &mut Session, ctx: &UploadContext, name: &str, ev: &Obj, emit: bool) -> Vec<EddnMessage> {
        let mut out = Vec::new();
        let ts = get_str(ev, "timestamp").and_then(Timestamp::parse).unwrap_or(self.latest_ts);
        self.latest_ts = self.latest_ts.max(ts);

        // 1. A run of FSSSignalDiscovered ends at the first other event.
        if name != "FSSSignalDiscovered" && !self.signals.is_empty() {
            let terminator = matches!(name, "Location" | "FSDJump" | "CarrierJump").then_some(ev);
            if let Some(m) = self.signal_batch(session, ctx, terminator) {
                out.push(m);
            }
        }

        // 2. Session state.
        session.apply(name, ev);

        if !emit {
            self.signals.clear();
            self.announced.clear();
            return out;
        }

        // 3. This event's own message.
        let what = name;
        let built: Option<Result<(Schema, Obj), Reason>> = match name {
            n if JOURNAL_EVENTS.contains(&n) => Some(journal(n, ev, session, ctx).map(|m| (Schema::Journal, m))),
            "FSSDiscoveryScan" => Some(fss_discovery_scan(ev, session, ctx).map(|m| (Schema::FssDiscoveryScan, m))),
            "FSSAllBodiesFound" => Some(fss_all_bodies_found(ev, session, ctx).map(|m| (Schema::FssAllBodiesFound, m))),
            "FSSBodySignals" => Some(fss_body_signals(ev, session, ctx).map(|m| (Schema::FssBodySignals, m))),
            "NavBeaconScan" => Some(nav_beacon_scan(ev, session, ctx).map(|m| (Schema::NavBeaconScan, m))),
            "CodexEntry" => Some(codex_entry(ev, session, ctx).map(|m| (Schema::CodexEntry, m))),
            "ScanBaryCentre" => Some(scan_bary_centre(ev, session, ctx).map(|m| (Schema::ScanBaryCentre, m))),
            "ApproachSettlement" => Some(approach_settlement(ev, session, ctx).map(|m| (Schema::ApproachSettlement, m))),
            "DockingGranted" => Some(docking_granted(ev).map(|m| (Schema::DockingGranted, m))),
            "DockingDenied" => Some(docking_denied(ev).map(|m| (Schema::DockingDenied, m))),
            "ScanOrganic" => Some(scan_organic(ev, session, ctx).map(|m| (Schema::ScanOrganic, m))),
            "FSSSignalDiscovered" => {
                if self.gate(session, ctx, Some(ev), what) {
                    self.signals.push(ev.clone());
                }
                None
            }
            "Market" | "Outfitting" | "Shipyard" | "NavRoute" | "FCMaterials" => {
                if self.gate(session, ctx, Some(ev), what) {
                    let kind = CompanionKind::from_event_name(name).unwrap_or(CompanionKind::Market);
                    self.announced.retain(|a| a.kind != kind);
                    self.announced.push(Announcement { kind, ts, market_id: get_u64(ev, "MarketID") });
                }
                None
            }
            _ => None,
        };
        let horizon = self.latest_ts.0 - ANNOUNCEMENT_TTL_SECS;
        self.announced.retain(|a| a.ts.0 >= horizon);

        if let Some(res) = built {
            match res {
                Ok((schema, msg)) => {
                    if self.gate(session, ctx, Some(ev), what)
                        && let Some(m) = self.envelope(schema, session, ctx, msg, what)
                    {
                        out.push(m);
                    }
                }
                Err(reason) => self.note_drop(what, &reason),
            }
        }
        out
    }

    /// Feeds a companion file the watcher read. `Status` updates the session; the station
    /// files become messages only when a live journal event announced them.
    pub fn on_companion(&mut self, session: &mut Session, ctx: &UploadContext, companion: &Companion, emit: bool) -> Vec<EddnMessage> {
        let (kind, raw) = match companion {
            Companion::Status(st) => {
                session.apply_status(st);
                return Vec::new();
            }
            Companion::Market(_, raw) => (CompanionKind::Market, raw),
            Companion::Outfitting(_, raw) => (CompanionKind::Outfitting, raw),
            Companion::Shipyard(_, raw) => (CompanionKind::Shipyard, raw),
            Companion::NavRoute(_, raw) => (CompanionKind::NavRoute, raw),
            Companion::FCMaterials(_, raw) => (CompanionKind::FCMaterials, raw),
            _ => return Vec::new(),
        };
        if !emit {
            return Vec::new();
        }
        let Some(file) = parse_object(raw) else {
            self.note_drop(kind.file_name(), "unparseable file");
            return Vec::new();
        };
        let file_ts = get_str(&file, "timestamp").and_then(Timestamp::parse).unwrap_or(Timestamp::ZERO);
        let market_id = get_u64(&file, "MarketID");
        // The file must belong to a live event: same timestamp (±5 s for NavRoute/FCMaterials,
        // as EDMC allows) and the same MarketID.
        let tolerance = if matches!(kind, CompanionKind::NavRoute | CompanionKind::FCMaterials) { 5 } else { 0 };
        let Some(pos) = self.announced.iter().position(|a| {
            a.kind == kind
                && (file_ts.0 - a.ts.0).abs() <= tolerance
                && (a.market_id.is_none() || a.market_id == market_id)
        }) else {
            return Vec::new();
        };
        self.announced.remove(pos);
        if !self.gate(session, ctx, None, kind.file_name()) {
            return Vec::new();
        }

        let what = kind.file_name();
        let mut built: Vec<(Schema, Result<Obj, Reason>)> = Vec::new();
        match kind {
            CompanionKind::Market => built.push((Schema::Commodity, commodity(&file))),
            CompanionKind::Outfitting => {
                built.push((Schema::Outfitting2, outfitting_v2(&file)));
                if self.opts.outfitting_v3 {
                    built.push((Schema::Outfitting3, outfitting_v3(&file)));
                }
            }
            CompanionKind::Shipyard => built.push((Schema::Shipyard, shipyard(&file))),
            CompanionKind::NavRoute => built.push((Schema::NavRoute, nav_route(&file))),
            CompanionKind::FCMaterials => built.push((Schema::FcMaterialsJournal, fc_materials(&file))),
            _ => {}
        }
        let mut out = Vec::new();
        for (schema, res) in built {
            match res {
                Ok(msg) => {
                    if schema.is_station_data() && !self.station_changed(schema, &msg) {
                        self.note_drop(schema.path(), "unchanged station data");
                        continue;
                    }
                    if let Some(m) = self.envelope(schema, session, ctx, msg, what) {
                        out.push(m);
                    }
                }
                Err(reason) => self.note_drop(schema.path(), &reason),
            }
        }
        out
    }

    /// Closes a pending `FSSSignalDiscovered` run using the tracked location (e.g. on shutdown).
    pub fn flush(&mut self, session: &Session, ctx: &UploadContext) -> Vec<EddnMessage> {
        if self.signals.is_empty() {
            return Vec::new();
        }
        self.signal_batch(session, ctx, None).into_iter().collect()
    }

    /// Whether anything may be sent for this event at all.
    fn gate(&mut self, session: &Session, ctx: &UploadContext, ev: Option<&Obj>, what: &str) -> bool {
        if let Some(reason) = session.blocked(ev) {
            self.note_drop(what, reason);
            return false;
        }
        if session.cmdr(ctx).is_none() {
            self.note_drop(what, "commander unknown");
            return false;
        }
        if session.game_version(ctx).0.trim().is_empty() {
            self.note_drop(what, "game version unknown");
            return false;
        }
        true
    }

    /// Wraps a message body in the EDDN envelope and runs the pre-flight checks.
    fn envelope(&mut self, schema: Schema, session: &Session, ctx: &UploadContext, mut msg: Obj, what: &str) -> Option<EddnMessage> {
        let (horizons, odyssey) = session.expansions(ctx);
        msg.remove("horizons");
        msg.remove("odyssey");
        if let Some(h) = horizons {
            msg.insert("horizons".into(), Value::Bool(h));
        }
        if let Some(o) = odyssey {
            msg.insert("odyssey".into(), Value::Bool(o));
        }
        let event_time = get_str(&msg, "timestamp").and_then(Timestamp::parse).unwrap_or(self.latest_ts);
        let cmdr = session.cmdr(ctx)?;
        let (gameversion, gamebuild) = session.game_version(ctx);
        let test = self.opts.force_test || session.is_beta(ctx);
        let envelope = json!({
            "$schemaRef": schema.schema_ref(test),
            "header": {
                "uploaderID": cmdr,
                "softwareName": self.opts.software_name,
                "softwareVersion": self.opts.software_version,
                "gameversion": gameversion,
                "gamebuild": gamebuild,
            },
            "message": Value::Object(msg),
        });
        if let Err(reason) = preflight(schema, &envelope) {
            self.note_drop(what, &format!("pre-flight: {reason}"));
            tracing::warn!(schema = schema.path(), reason, "EDDN message failed pre-flight checks, not sent");
            return None;
        }
        Some(EddnMessage { schema, test, envelope, event_time })
    }

    /// True if this station message differs from the last one sent for the same market.
    fn station_changed(&mut self, schema: Schema, msg: &Obj) -> bool {
        let Some(market) = get_u64(msg, "marketId").or_else(|| get_u64(msg, "MarketID")) else { return true };
        let mut content = msg.clone();
        content.remove("timestamp");
        let hash = fnv1a64(Value::Object(content).to_string().as_bytes());
        let t = get_str(msg, "timestamp").and_then(Timestamp::parse).unwrap_or(self.latest_ts).0;
        if let Some(&(h, at)) = self.station_sent.get(&(schema, market))
            && h == hash
            && (t - at).abs() < STATION_DEDUPE_SECS
        {
            return false;
        }
        if self.station_sent.len() > 512 {
            self.station_sent.clear();
        }
        self.station_sent.insert((schema, market), (hash, t));
        true
    }

    /// Builds the `fsssignaldiscovered/1` message for the pending run.
    fn signal_batch(&mut self, session: &Session, ctx: &UploadContext, terminator: Option<&Obj>) -> Option<EddnMessage> {
        let signals = std::mem::take(&mut self.signals);
        let what = "FSSSignalDiscovered";
        if !self.gate(session, ctx, None, what) {
            return None;
        }
        // Odyssey writes the signals *before* the arrival event: use the arrival's location.
        let loc = match terminator {
            Some(ev) => match (get_nonempty(ev, "StarSystem"), get_u64(ev, "SystemAddress"), get_star_pos(ev, "StarPos")) {
                (Some(s), Some(a), Some(p)) => Some(Location { system_name: s.to_string(), system_address: a, star_pos: p }),
                _ => None,
            },
            None => session.location(ctx),
        };
        let Some(loc) = loc else {
            self.note_drop(what, "no tracked location");
            return None;
        };
        if signals.first().and_then(|s| get_u64(s, "SystemAddress")) != Some(loc.system_address) {
            self.note_drop(what, "SystemAddress does not match the tracked location");
            return None;
        }
        let mut seen = HashSet::new();
        let mut kept: Vec<Value> = Vec::new();
        for s in &signals {
            if get_u64(s, "SystemAddress") != Some(loc.system_address) {
                self.note_drop(what, "signal from another system");
                continue;
            }
            if get_str(s, "USSType") == Some("$USS_Type_MissionTarget;") {
                self.note_drop(what, "mission target signal");
                continue;
            }
            match signal_item(s) {
                Ok(item) => {
                    let v = Value::Object(item);
                    if seen.insert(v.to_string()) {
                        kept.push(v);
                    }
                }
                Err(reason) => self.note_drop(what, &reason),
            }
        }
        let first_ts = kept.first()?.get("timestamp")?.clone();
        let mut msg = Obj::new();
        msg.insert("event".into(), json!("FSSSignalDiscovered"));
        msg.insert("timestamp".into(), first_ts);
        msg.insert("SystemAddress".into(), json!(loc.system_address));
        msg.insert("StarSystem".into(), json!(loc.system_name));
        msg.insert("StarPos".into(), star_pos_value(loc.star_pos));
        msg.insert("signals".into(), Value::Array(kept));
        self.envelope(Schema::FssSignalDiscovered, session, ctx, msg, what)
    }
}

// ---------------------------------------------------------------------------------------------
// Whitelist helpers
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ty {
    /// Any string.
    Str,
    /// A non-empty string (`minLength: 1`).
    Text,
    /// A JSON integer.
    Int,
    /// A finite number.
    Num,
    Bool,
    /// Array of strings.
    Strs,
    /// Array of non-empty strings.
    Texts,
}

fn type_ok(v: &Value, ty: Ty) -> bool {
    match ty {
        Ty::Str => v.is_string(),
        Ty::Text => v.as_str().is_some_and(|s| !s.is_empty()),
        Ty::Int => is_int(v),
        Ty::Num => v.as_f64().is_some_and(f64::is_finite),
        Ty::Bool => v.is_boolean(),
        Ty::Strs => v.as_array().is_some_and(|a| a.iter().all(Value::is_string)),
        Ty::Texts => v.as_array().is_some_and(|a| a.iter().all(|x| x.as_str().is_some_and(|s| !s.is_empty()))),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Need {
    /// Missing or invalid → drop the message.
    Required,
    /// Missing or invalid → leave the key out.
    Optional,
    /// Missing → leave out; present but invalid → drop the message.
    Strict,
}

/// Copies `key` from `src` into `dst` if it has the expected type.
fn take(src: &Obj, dst: &mut Obj, key: &str, ty: Ty, need: Need) -> Result<(), Reason> {
    match src.get(key) {
        Some(v) if type_ok(v, ty) => {
            dst.insert(key.to_string(), v.clone());
            Ok(())
        }
        Some(_) if need != Need::Optional => Err(format!("invalid {key}")),
        None if need == Need::Required => Err(format!("missing {key}")),
        _ => Ok(()),
    }
}

/// `timestamp` (RFC 3339) and `event`.
fn base(ev: &Obj, event: &str) -> Result<Obj, Reason> {
    let mut m = Obj::new();
    let ts = get_str(ev, "timestamp").filter(|t| is_date_time(t)).ok_or("invalid timestamp")?;
    m.insert("timestamp".into(), json!(ts));
    m.insert("event".into(), json!(event));
    Ok(m)
}

/// The tracked location, if it is the system the event happened in.
fn located(session: &Session, ctx: &UploadContext, addr: u64, name: Option<&str>) -> Result<Location, Reason> {
    let loc = session.location(ctx).ok_or("no tracked location")?;
    if loc.system_address != addr {
        return Err("SystemAddress does not match the tracked location".into());
    }
    if let Some(n) = name
        && !n.eq_ignore_ascii_case(&loc.system_name)
    {
        return Err("system name does not match the tracked location".into());
    }
    Ok(loc)
}

fn add_system(m: &mut Obj, loc: &Location, with_name: bool) {
    if with_name {
        m.insert("StarSystem".into(), json!(loc.system_name));
    }
    m.insert("StarPos".into(), star_pos_value(loc.star_pos));
}

// ---------------------------------------------------------------------------------------------
// Journal-event schemas
// ---------------------------------------------------------------------------------------------

/// `journal/1`: the event minus personal data and `_Localised` keys (non-strict schema).
fn journal(name: &str, ev: &Obj, session: &Session, ctx: &UploadContext) -> Result<Obj, Reason> {
    let mut msg = ev.clone();
    for k in JOURNAL_DISALLOWED {
        msg.remove(k);
    }
    if let Some(Value::Array(factions)) = msg.get_mut("Factions") {
        for f in factions.iter_mut() {
            if let Some(fo) = f.as_object_mut() {
                for k in FACTION_DISALLOWED {
                    fo.remove(k);
                }
            }
        }
    }
    strip_localised_obj(&mut msg);
    msg.remove("horizons");
    msg.remove("odyssey");
    if !get_str(&msg, "timestamp").is_some_and(is_date_time) {
        return Err("invalid timestamp".into());
    }
    let addr = get_u64(&msg, "SystemAddress").ok_or("missing SystemAddress")?;
    if matches!(name, "FSDJump" | "Location" | "CarrierJump") {
        get_nonempty(&msg, "StarSystem").ok_or("missing StarSystem")?;
        get_star_pos(&msg, "StarPos").ok_or("invalid StarPos")?;
        return Ok(msg);
    }
    let has_system = get_nonempty(&msg, "StarSystem").is_some();
    let has_pos = get_star_pos(&msg, "StarPos").is_some();
    if !has_system || !has_pos {
        if msg.contains_key("StarPos") && !has_pos {
            return Err("invalid StarPos".into());
        }
        let loc = located(session, ctx, addr, get_nonempty(&msg, "StarSystem"))?;
        if !has_system {
            msg.insert("StarSystem".into(), json!(loc.system_name));
        }
        if !has_pos {
            msg.insert("StarPos".into(), star_pos_value(loc.star_pos));
        }
    }
    Ok(msg)
}

fn fss_discovery_scan(ev: &Obj, session: &Session, ctx: &UploadContext) -> Result<Obj, Reason> {
    let mut m = base(ev, "FSSDiscoveryScan")?;
    take(ev, &mut m, "SystemName", Ty::Text, Need::Required)?;
    take(ev, &mut m, "SystemAddress", Ty::Int, Need::Required)?;
    take(ev, &mut m, "BodyCount", Ty::Int, Need::Required)?;
    take(ev, &mut m, "NonBodyCount", Ty::Int, Need::Required)?;
    // `Progress` is personal (and disallowed).
    let addr = get_u64(&m, "SystemAddress").ok_or("invalid SystemAddress")?;
    let loc = located(session, ctx, addr, get_str(&m, "SystemName"))?;
    add_system(&mut m, &loc, false);
    Ok(m)
}

fn fss_all_bodies_found(ev: &Obj, session: &Session, ctx: &UploadContext) -> Result<Obj, Reason> {
    let mut m = base(ev, "FSSAllBodiesFound")?;
    take(ev, &mut m, "SystemName", Ty::Text, Need::Required)?;
    take(ev, &mut m, "SystemAddress", Ty::Int, Need::Required)?;
    take(ev, &mut m, "Count", Ty::Int, Need::Required)?;
    let addr = get_u64(&m, "SystemAddress").ok_or("invalid SystemAddress")?;
    let loc = located(session, ctx, addr, get_str(&m, "SystemName"))?;
    add_system(&mut m, &loc, false);
    Ok(m)
}

fn fss_body_signals(ev: &Obj, session: &Session, ctx: &UploadContext) -> Result<Obj, Reason> {
    let mut m = base(ev, "FSSBodySignals")?;
    take(ev, &mut m, "SystemAddress", Ty::Int, Need::Required)?;
    take(ev, &mut m, "BodyID", Ty::Int, Need::Required)?;
    take(ev, &mut m, "BodyName", Ty::Text, Need::Optional)?;
    let mut signals = Vec::new();
    for s in get_array(ev, "Signals").ok_or("missing Signals")? {
        let s = s.as_object().ok_or("invalid Signals")?;
        let mut item = Obj::new();
        take(s, &mut item, "Type", Ty::Str, Need::Required)?;
        take(s, &mut item, "Count", Ty::Int, Need::Required)?;
        signals.push(Value::Object(item));
    }
    if signals.is_empty() {
        return Err("no signals".into());
    }
    m.insert("Signals".into(), Value::Array(signals));
    let addr = get_u64(&m, "SystemAddress").ok_or("invalid SystemAddress")?;
    let loc = located(session, ctx, addr, None)?;
    add_system(&mut m, &loc, true);
    Ok(m)
}

/// One element of `fsssignaldiscovered/1` `signals` (strict items).
fn signal_item(s: &Obj) -> Result<Obj, Reason> {
    let mut item = Obj::new();
    let ts = get_str(s, "timestamp").filter(|t| is_date_time(t)).ok_or("invalid signal timestamp")?;
    item.insert("timestamp".into(), json!(ts));
    take(s, &mut item, "SignalName", Ty::Str, Need::Required)?;
    take(s, &mut item, "SignalType", Ty::Str, Need::Optional)?;
    take(s, &mut item, "IsStation", Ty::Bool, Need::Optional)?;
    take(s, &mut item, "USSType", Ty::Str, Need::Optional)?;
    take(s, &mut item, "SpawningState", Ty::Str, Need::Optional)?;
    take(s, &mut item, "SpawningFaction", Ty::Str, Need::Optional)?;
    take(s, &mut item, "SpawningPower", Ty::Str, Need::Optional)?;
    take(s, &mut item, "OpposingPower", Ty::Str, Need::Optional)?;
    take(s, &mut item, "ThreatLevel", Ty::Int, Need::Optional)?;
    // `TimeRemaining` (personal, ephemeral), `event`, `SystemAddress` and `*_Localised` are not copied.
    Ok(item)
}

fn nav_beacon_scan(ev: &Obj, session: &Session, ctx: &UploadContext) -> Result<Obj, Reason> {
    let mut m = base(ev, "NavBeaconScan")?;
    take(ev, &mut m, "SystemAddress", Ty::Int, Need::Required)?;
    take(ev, &mut m, "NumBodies", Ty::Int, Need::Required)?;
    let addr = get_u64(&m, "SystemAddress").ok_or("invalid SystemAddress")?;
    let loc = located(session, ctx, addr, None)?;
    add_system(&mut m, &loc, true);
    Ok(m)
}

fn codex_entry(ev: &Obj, session: &Session, ctx: &UploadContext) -> Result<Obj, Reason> {
    let mut m = base(ev, "CodexEntry")?;
    take(ev, &mut m, "System", Ty::Text, Need::Required)?;
    take(ev, &mut m, "SystemAddress", Ty::Int, Need::Required)?;
    take(ev, &mut m, "EntryID", Ty::Int, Need::Required)?;
    // EDMC drops the entry when one of these is empty rather than sending it without.
    take(ev, &mut m, "Name", Ty::Text, Need::Strict)?;
    take(ev, &mut m, "Region", Ty::Text, Need::Strict)?;
    take(ev, &mut m, "Category", Ty::Text, Need::Strict)?;
    take(ev, &mut m, "SubCategory", Ty::Text, Need::Strict)?;
    take(ev, &mut m, "Traits", Ty::Texts, Need::Strict)?;
    take(ev, &mut m, "Latitude", Ty::Num, Need::Optional)?;
    take(ev, &mut m, "Longitude", Ty::Num, Need::Optional)?;
    take(ev, &mut m, "NearestDestination", Ty::Str, Need::Optional)?;
    take(ev, &mut m, "VoucherAmount", Ty::Int, Need::Optional)?;
    take(ev, &mut m, "BodyID", Ty::Int, Need::Optional)?;
    take(ev, &mut m, "BodyName", Ty::Text, Need::Optional)?;
    // IsNewEntry / NewTraitsDiscovered are personal and not copied.
    let addr = get_u64(&m, "SystemAddress").ok_or("invalid SystemAddress")?;
    if addr <= 1 {
        return Err("bogus SystemAddress".into());
    }
    let loc = located(session, ctx, addr, get_str(&m, "System"))?;
    add_system(&mut m, &loc, false);
    add_body(&mut m, session, "BodyID");
    Ok(m)
}

/// Adds `BodyName`/`BodyID` per the codexentry README: `BodyName` from Status.json, `BodyID`
/// from the journal only when both name sources agree; never empty or `null` values.
fn add_body(m: &mut Obj, session: &Session, id_key: &str) {
    let status_body = session.status().and_then(|s| s.body_name.as_deref());
    let journal_body = session.journal_body();
    match (get_u64(m, id_key), m.contains_key("BodyName")) {
        (_, true) => {}
        (Some(id), false) => {
            // The event has the ID (Update 12+): add the name only if we know that body.
            if let Some((name, jid)) = journal_body
                && jid == id
                && status_body.is_none_or(|s| s == name)
            {
                m.insert("BodyName".into(), json!(name));
            }
        }
        (None, false) => {
            if let Some(sb) = status_body {
                m.insert("BodyName".into(), json!(sb));
                if let Some((name, jid)) = journal_body
                    && name == sb
                {
                    m.insert(id_key.into(), json!(jid));
                }
            }
        }
    }
}

fn scan_bary_centre(ev: &Obj, session: &Session, ctx: &UploadContext) -> Result<Obj, Reason> {
    let mut m = base(ev, "ScanBaryCentre")?;
    take(ev, &mut m, "StarSystem", Ty::Text, Need::Optional)?;
    take(ev, &mut m, "SystemAddress", Ty::Int, Need::Required)?;
    take(ev, &mut m, "BodyID", Ty::Int, Need::Required)?;
    for k in ["SemiMajorAxis", "Eccentricity", "OrbitalInclination", "Periapsis", "OrbitalPeriod", "AscendingNode", "MeanAnomaly"] {
        take(ev, &mut m, k, Ty::Num, Need::Optional)?;
    }
    let addr = get_u64(&m, "SystemAddress").ok_or("invalid SystemAddress")?;
    let loc = located(session, ctx, addr, get_str(&m, "StarSystem"))?;
    let has_name = m.contains_key("StarSystem");
    add_system(&mut m, &loc, !has_name);
    Ok(m)
}

fn approach_settlement(ev: &Obj, session: &Session, ctx: &UploadContext) -> Result<Obj, Reason> {
    let mut m = base(ev, "ApproachSettlement")?;
    take(ev, &mut m, "SystemAddress", Ty::Int, Need::Required)?;
    take(ev, &mut m, "Name", Ty::Str, Need::Required)?;
    take(ev, &mut m, "BodyID", Ty::Int, Need::Required)?;
    take(ev, &mut m, "BodyName", Ty::Str, Need::Required)?;
    // Missing on login at a planetary port (known game bug): drop, as EDMC does.
    take(ev, &mut m, "Latitude", Ty::Num, Need::Required)?;
    take(ev, &mut m, "Longitude", Ty::Num, Need::Required)?;
    take(ev, &mut m, "MarketID", Ty::Int, Need::Strict)?;
    take(ev, &mut m, "StationGovernment", Ty::Str, Need::Optional)?;
    take(ev, &mut m, "StationAllegiance", Ty::Str, Need::Optional)?;
    take(ev, &mut m, "StationEconomy", Ty::Str, Need::Optional)?;
    take(ev, &mut m, "StationServices", Ty::Strs, Need::Optional)?;
    if let Some(econ) = get_array(ev, "StationEconomies") {
        let mut items = Vec::new();
        for e in econ {
            let Some(e) = e.as_object() else { continue };
            let mut item = Obj::new();
            take(e, &mut item, "Name", Ty::Str, Need::Optional)?;
            take(e, &mut item, "Proportion", Ty::Num, Need::Optional)?;
            items.push(Value::Object(item));
        }
        m.insert("StationEconomies".into(), Value::Array(items));
    }
    if let Some(f) = ev.get("StationFaction").and_then(Value::as_object) {
        let mut item = Obj::new();
        take(f, &mut item, "Name", Ty::Str, Need::Optional)?;
        take(f, &mut item, "FactionState", Ty::Str, Need::Optional)?;
        m.insert("StationFaction".into(), Value::Object(item));
    }
    let addr = get_u64(&m, "SystemAddress").ok_or("invalid SystemAddress")?;
    let loc = located(session, ctx, addr, None)?;
    add_system(&mut m, &loc, true);
    Ok(m)
}

fn docking_granted(ev: &Obj) -> Result<Obj, Reason> {
    let mut m = base(ev, "DockingGranted")?;
    take(ev, &mut m, "MarketID", Ty::Int, Need::Required)?;
    take(ev, &mut m, "StationName", Ty::Str, Need::Required)?;
    take(ev, &mut m, "StationType", Ty::Str, Need::Optional)?;
    take(ev, &mut m, "LandingPad", Ty::Int, Need::Optional)?;
    // No StarSystem/StarPos/SystemAddress: the strict schema rejects them.
    Ok(m)
}

fn docking_denied(ev: &Obj) -> Result<Obj, Reason> {
    let mut m = base(ev, "DockingDenied")?;
    take(ev, &mut m, "MarketID", Ty::Int, Need::Required)?;
    take(ev, &mut m, "StationName", Ty::Str, Need::Required)?;
    take(ev, &mut m, "Reason", Ty::Str, Need::Required)?;
    take(ev, &mut m, "StationType", Ty::Str, Need::Optional)?;
    Ok(m)
}

fn scan_organic(ev: &Obj, session: &Session, ctx: &UploadContext) -> Result<Obj, Reason> {
    let scan_type = get_str(ev, "ScanType").ok_or("missing ScanType")?;
    if scan_type != "Log" && scan_type != "Sample" {
        // `Analyse` can fire later, even in another system: excluded by the schema.
        return Err(format!("ScanType {scan_type}"));
    }
    let mut m = base(ev, "ScanOrganic")?;
    m.insert("ScanType".into(), json!(scan_type));
    take(ev, &mut m, "Genus", Ty::Text, Need::Required)?;
    take(ev, &mut m, "Species", Ty::Text, Need::Required)?;
    take(ev, &mut m, "Variant", Ty::Text, Need::Optional)?;
    take(ev, &mut m, "SystemAddress", Ty::Int, Need::Required)?;
    // `Body` is renamed `BodyID`; `WasLogged` is not part of the schema.
    let body = ev.get("Body").filter(|v| is_int(v)).ok_or("missing Body")?.clone();
    m.insert("BodyID".into(), body);
    let addr = get_u64(&m, "SystemAddress").ok_or("invalid SystemAddress")?;
    let loc = located(session, ctx, addr, None)?;
    add_system(&mut m, &loc, true);
    // BodyName from journal tracking (cross-checked with Status.json when available).
    if let (Some(id), Some((name, jid))) = (get_u64(&m, "BodyID"), session.journal_body())
        && jid == id
    {
        let status_body = session.status().and_then(|s| s.body_name.as_deref());
        if status_body.is_none_or(|s| s == name) {
            m.insert("BodyName".into(), json!(name));
        }
    }
    // Latitude/Longitude from Status.json, only when it is fresh and about the same body (EDDI).
    if let (Some(st), Some(body_name), Some(t)) =
        (session.status(), get_str(&m, "BodyName"), get_str(ev, "timestamp").and_then(Timestamp::parse))
    {
        use nexus_journal::companion::flags2;
        let fresh = (t.0 - st.timestamp.0).abs() <= 60;
        let on_foot = st.flags2 & flags2::ON_FOOT_ON_PLANET != 0;
        if fresh
            && on_foot
            && st.body_name.as_deref() == Some(body_name)
            && let (Some(lat), Some(lon)) = (st.latitude, st.longitude)
        {
            m.insert("Latitude".into(), json!(lat));
            m.insert("Longitude".into(), json!(lon));
        }
    }
    Ok(m)
}

// ---------------------------------------------------------------------------------------------
// Companion-file schemas
// ---------------------------------------------------------------------------------------------

/// `systemName`/`stationName`/`marketId`/`timestamp` of a station file.
fn station_header(file: &Obj) -> Result<Obj, Reason> {
    let mut m = Obj::new();
    m.insert("systemName".into(), json!(get_nonempty(file, "StarSystem").ok_or("missing StarSystem")?));
    m.insert("stationName".into(), json!(get_nonempty(file, "StationName").ok_or("missing StationName")?));
    let market = file.get("MarketID").filter(|v| is_int(v)).ok_or("missing MarketID")?;
    m.insert("marketId".into(), market.clone());
    let ts = get_str(file, "timestamp").filter(|t| is_date_time(t)).ok_or("invalid timestamp")?;
    m.insert("timestamp".into(), json!(ts));
    Ok(m)
}

/// EDMC: `\$(.+)_name;` anchored at the start, else unchanged; then lower-cased (as for CAPI).
fn commodity_name(name: &str) -> String {
    let canonical = name.strip_prefix('$').and_then(|s| s.strip_suffix("_name;")).filter(|s| !s.is_empty()).unwrap_or(name);
    canonical.to_ascii_lowercase()
}

fn bracket_ok(v: &Value) -> bool {
    v.as_i64().is_some_and(|b| (0..=3).contains(&b)) || v.as_str() == Some("")
}

/// `Market.json` → `commodity/3`.
fn commodity(file: &Obj) -> Result<Obj, Reason> {
    let mut m = station_header(file)?;
    if let Some(t) = get_nonempty(file, "StationType") {
        m.insert("stationType".into(), json!(t));
    }
    if let Some(a) = get_nonempty(file, "CarrierDockingAccess") {
        m.insert("carrierDockingAccess".into(), json!(a));
    }
    let items = get_array(file, "Items").ok_or("missing Items")?;
    let mut by_name: BTreeMap<String, Value> = BTreeMap::new();
    for it in items {
        let Some(it) = it.as_object() else { continue };
        let category = get_str(it, "Category").map(strip_symbol).unwrap_or_default().to_ascii_lowercase();
        let raw_name = get_str(it, "Name").unwrap_or_default();
        let name = commodity_name(raw_name);
        // Limpets are not tradeable goods (EDDI/README: drop NonMarketable).
        if category.ends_with("nonmarketable") || name == "drones" || name.is_empty() {
            continue;
        }
        let mut c = Obj::new();
        c.insert("name".into(), json!(name));
        let fields = [
            ("MeanPrice", "meanPrice", false),
            ("BuyPrice", "buyPrice", false),
            ("Stock", "stock", false),
            ("StockBracket", "stockBracket", true),
            ("SellPrice", "sellPrice", false),
            ("Demand", "demand", false),
            ("DemandBracket", "demandBracket", true),
        ];
        let complete = fields.iter().all(|&(src, dst, bracket)| match it.get(src) {
            Some(v) if (bracket && bracket_ok(v)) || (!bracket && is_int(v)) => {
                c.insert(dst.to_string(), v.clone());
                true
            }
            _ => false,
        });
        if !complete {
            continue;
        }
        let flags: Vec<&str> = [("Producer", "Producer"), ("Consumer", "Consumer"), ("Rare", "Rare")]
            .into_iter()
            .filter(|(k, _)| get_bool(it, k) == Some(true))
            .map(|(_, f)| f)
            .collect();
        if !flags.is_empty() {
            c.insert("statusFlags".into(), json!(flags));
        }
        by_name.entry(name).or_insert(Value::Object(c));
    }
    // An empty list is valid and meaningful (a Fleet Carrier without orders).
    m.insert("commodities".into(), Value::Array(by_name.into_values().collect()));
    Ok(m)
}

/// EDMC's module regex `^Hpt_|^Int_|Armour_` (case-insensitive) with the matches capitalised.
fn module_name_v2(name: &str) -> Option<String> {
    let lower = name.to_ascii_lowercase();
    if lower == "int_planetapproachsuite" {
        return None;
    }
    if !(lower.starts_with("hpt_") || lower.starts_with("int_") || lower.contains("armour_")) {
        return None; // bobbleheads, decals, paint jobs, ship kits, …
    }
    let mut out = String::with_capacity(name.len());
    let mut i = 0;
    if lower.starts_with("hpt_") {
        out.push_str("Hpt_");
        i = 4;
    } else if lower.starts_with("int_") {
        out.push_str("Int_");
        i = 4;
    }
    while let Some(pos) = lower[i..].find("armour_") {
        out.push_str(&name[i..i + pos]);
        out.push_str("Armour_");
        i += pos + "armour_".len();
    }
    out.push_str(&name[i..]);
    schema_module_pattern(&out).then_some(out)
}

/// The schemas' `(^Hpt_|^hpt_|^Int_|^int_|_Armour_|_armour_)` pattern.
fn schema_module_pattern(s: &str) -> bool {
    s.starts_with("Hpt_")
        || s.starts_with("hpt_")
        || s.starts_with("Int_")
        || s.starts_with("int_")
        || s.contains("_Armour_")
        || s.contains("_armour_")
}

/// `Outfitting.json` → `outfitting/2` (names only, unique, sorted).
fn outfitting_v2(file: &Obj) -> Result<Obj, Reason> {
    let mut m = station_header(file)?;
    let modules: BTreeSet<String> = get_array(file, "Items")
        .ok_or("missing Items")?
        .iter()
        .filter_map(|it| it.as_object().and_then(|o| get_str(o, "Name")).and_then(module_name_v2))
        .collect();
    if modules.is_empty() {
        return Err("no modules".into());
    }
    m.insert("modules".into(), json!(modules));
    Ok(m)
}

/// `Outfitting.json` → `outfitting/3` (`{id, Name, BuyPrice, BuyMercCoinsPrice}`).
fn outfitting_v3(file: &Obj) -> Result<Obj, Reason> {
    let mut m = station_header(file)?;
    let mut modules: BTreeSet<(String, u64, i64, i64)> = BTreeSet::new();
    for it in get_array(file, "Items").ok_or("missing Items")? {
        let Some(it) = it.as_object() else { continue };
        let Some(name) = get_str(it, "Name") else { continue };
        // Same filter as v2, but the name is sent exactly as in the file.
        if module_name_v2(name).is_none() || !schema_module_pattern(name) {
            continue;
        }
        let (Some(id), Some(price)) = (get_u64(it, "id"), it.get("BuyPrice").filter(|v| is_int(v)).and_then(Value::as_i64)) else {
            continue;
        };
        let merc = match it.get("BuyMercCoinsPrice") {
            None => 0,
            Some(v) => match v.as_i64() {
                Some(p) if is_int(v) => p,
                _ => continue,
            },
        };
        modules.insert((name.to_string(), id, price, merc));
    }
    if modules.is_empty() {
        return Err("no modules".into());
    }
    let list: Vec<Value> = modules
        .into_iter()
        .map(|(name, id, price, merc)| json!({"id": id, "Name": name, "BuyPrice": price, "BuyMercCoinsPrice": merc}))
        .collect();
    m.insert("modules".into(), Value::Array(list));
    Ok(m)
}

/// `Shipyard.json` → `shipyard/2` (ship symbols, unique, sorted, lower-case).
fn shipyard(file: &Obj) -> Result<Obj, Reason> {
    let mut m = station_header(file)?;
    let ships: BTreeSet<String> = get_array(file, "PriceList")
        .ok_or("no PriceList")?
        .iter()
        .filter_map(|p| p.as_object().and_then(|o| get_nonempty(o, "ShipType")))
        .map(str::to_ascii_lowercase)
        .collect();
    if ships.is_empty() {
        return Err("no ships".into());
    }
    m.insert("ships".into(), json!(ships));
    if let Some(b) = get_bool(file, "AllowCobraMkIV") {
        m.insert("allowCobraMkIV".into(), json!(b));
    }
    Ok(m)
}

/// `NavRoute.json` → `navroute/1`.
fn nav_route(file: &Obj) -> Result<Obj, Reason> {
    if get_str(file, "event") != Some("NavRoute") {
        return Err("not a NavRoute (cleared route)".into());
    }
    let mut m = base(file, "NavRoute")?;
    let route = get_array(file, "Route").ok_or("missing Route")?;
    if route.is_empty() {
        return Err("empty Route".into());
    }
    let mut hops = Vec::with_capacity(route.len());
    for hop in route {
        let hop = hop.as_object().ok_or("invalid Route")?;
        let mut h = Obj::new();
        take(hop, &mut h, "StarSystem", Ty::Text, Need::Required)?;
        take(hop, &mut h, "SystemAddress", Ty::Int, Need::Required)?;
        let pos = hop.get("StarPos").and_then(as_star_pos).ok_or("invalid StarPos in Route")?;
        h.insert("StarPos".into(), star_pos_value(pos));
        take(hop, &mut h, "StarClass", Ty::Text, Need::Required)?;
        hops.push(Value::Object(h));
    }
    m.insert("Route".into(), Value::Array(hops));
    Ok(m)
}

/// `FCMaterials.json` → `fcmaterials_journal/1`.
fn fc_materials(file: &Obj) -> Result<Obj, Reason> {
    let mut m = base(file, "FCMaterials")?;
    take(file, &mut m, "MarketID", Ty::Int, Need::Required)?;
    take(file, &mut m, "CarrierName", Ty::Text, Need::Required)?;
    take(file, &mut m, "CarrierID", Ty::Text, Need::Required)?;
    let mut items = Vec::new();
    for it in get_array(file, "Items").ok_or("missing Items")? {
        let Some(it) = it.as_object() else { continue };
        let mut item = Obj::new();
        let ok = take(it, &mut item, "id", Ty::Int, Need::Required)
            .and_then(|_| take(it, &mut item, "Name", Ty::Text, Need::Required))
            .and_then(|_| take(it, &mut item, "Price", Ty::Int, Need::Required))
            .and_then(|_| take(it, &mut item, "Stock", Ty::Int, Need::Required))
            .and_then(|_| take(it, &mut item, "Demand", Ty::Int, Need::Required));
        if ok.is_ok() {
            items.push(Value::Object(item));
        }
    }
    m.insert("Items".into(), Value::Array(items));
    Ok(m)
}

// ---------------------------------------------------------------------------------------------
// Pre-flight checks (run on every message before it is queued)
// ---------------------------------------------------------------------------------------------

/// Schema-independent invariants every message must satisfy. Catches builder bugs before they
/// become public FAILED counts on the EDDN monitor.
pub fn preflight(schema: Schema, envelope: &Value) -> Result<(), &'static str> {
    let env = envelope.as_object().ok_or("envelope is not an object")?;
    if env.len() != 3 {
        return Err("envelope must have exactly $schemaRef, header and message");
    }
    let schema_ref = env.get("$schemaRef").and_then(Value::as_str).ok_or("missing $schemaRef")?;
    match Schema::from_schema_ref(schema_ref) {
        Some((s, _)) if s == schema => {}
        _ => return Err("$schemaRef does not match the schema"),
    }
    let header = env.get("header").and_then(Value::as_object).ok_or("missing header")?;
    if header.len() != 5 || header.contains_key("gatewayTimestamp") {
        return Err("header must have exactly the five sender keys");
    }
    for k in ["uploaderID", "softwareName", "softwareVersion"] {
        if !header.get(k).and_then(Value::as_str).is_some_and(|s| !s.trim().is_empty()) {
            return Err("header key missing or empty");
        }
    }
    for k in ["gameversion", "gamebuild"] {
        if !header.get(k).is_some_and(Value::is_string) {
            return Err("gameversion/gamebuild must be strings");
        }
    }
    let msg = env.get("message").and_then(Value::as_object).ok_or("missing message")?;
    if !get_str(msg, "timestamp").is_some_and(is_date_time) {
        return Err("message timestamp is not RFC 3339");
    }
    if has_null(envelope) {
        return Err("null value");
    }
    if has_localised(envelope) {
        return Err("_Localised key");
    }
    for k in ["horizons", "odyssey"] {
        if msg.get(k).is_some_and(|v| !v.is_boolean()) {
            return Err("horizons/odyssey must be booleans");
        }
    }
    if let Some(p) = msg.get("StarPos")
        && as_star_pos(p).is_none()
    {
        return Err("invalid StarPos");
    }
    let unique = |v: Option<&Value>| -> bool {
        match v.and_then(Value::as_array) {
            Some(a) => {
                let mut seen = HashSet::new();
                a.iter().all(|x| seen.insert(x.to_string()))
            }
            None => true,
        }
    };
    if !unique(msg.get("modules")) || !unique(msg.get("ships")) || !unique(msg.get("prohibited")) {
        return Err("duplicate items in a uniqueItems array");
    }
    if let Some(cs) = get_array(msg, "commodities")
        && !cs.iter().all(|c| unique(c.get("statusFlags")))
    {
        return Err("duplicate statusFlags");
    }
    if schema != Schema::Journal {
        // Only journal/1 is a pass-through; everything else must be whitelisted already.
        let allowed: &[&str] = allowed_message_keys(schema);
        if msg.keys().any(|k| !allowed.contains(&k.as_str())) {
            return Err("key not allowed by the schema");
        }
    }
    Ok(())
}

/// Top-level `message` keys each strict schema allows.
fn allowed_message_keys(schema: Schema) -> &'static [&'static str] {
    match schema {
        Schema::Journal => &[],
        Schema::Commodity => &[
            "systemName",
            "stationName",
            "stationType",
            "carrierDockingAccess",
            "marketId",
            "horizons",
            "odyssey",
            "timestamp",
            "commodities",
            "economies",
            "prohibited",
        ],
        Schema::Outfitting2 | Schema::Outfitting3 => {
            &["systemName", "stationName", "marketId", "horizons", "odyssey", "timestamp", "modules"]
        }
        Schema::Shipyard => &["systemName", "stationName", "marketId", "horizons", "odyssey", "allowCobraMkIV", "timestamp", "ships"],
        Schema::NavRoute => &["timestamp", "event", "horizons", "odyssey", "Route"],
        Schema::FssDiscoveryScan => {
            &["timestamp", "event", "horizons", "odyssey", "SystemName", "StarPos", "SystemAddress", "BodyCount", "NonBodyCount"]
        }
        Schema::FssAllBodiesFound => &["timestamp", "event", "horizons", "odyssey", "SystemName", "StarPos", "SystemAddress", "Count"],
        Schema::FssBodySignals => {
            &["timestamp", "event", "horizons", "odyssey", "StarSystem", "StarPos", "SystemAddress", "BodyID", "BodyName", "Signals"]
        }
        Schema::FssSignalDiscovered => &["event", "horizons", "odyssey", "timestamp", "SystemAddress", "signals", "StarSystem", "StarPos"],
        Schema::NavBeaconScan => &["timestamp", "event", "horizons", "odyssey", "StarSystem", "StarPos", "SystemAddress", "NumBodies"],
        Schema::CodexEntry => &[
            "timestamp",
            "event",
            "horizons",
            "odyssey",
            "System",
            "StarPos",
            "SystemAddress",
            "Name",
            "Region",
            "EntryID",
            "Category",
            "Latitude",
            "Longitude",
            "SubCategory",
            "NearestDestination",
            "VoucherAmount",
            "Traits",
            "BodyID",
            "BodyName",
        ],
        Schema::ScanBaryCentre => &[
            "timestamp",
            "event",
            "horizons",
            "odyssey",
            "StarSystem",
            "StarPos",
            "SystemAddress",
            "BodyID",
            "SemiMajorAxis",
            "Eccentricity",
            "OrbitalInclination",
            "Periapsis",
            "OrbitalPeriod",
            "AscendingNode",
            "MeanAnomaly",
        ],
        Schema::ApproachSettlement => &[
            "timestamp",
            "event",
            "horizons",
            "odyssey",
            "StarSystem",
            "StarPos",
            "StationGovernment",
            "StationAllegiance",
            "StationEconomies",
            "StationFaction",
            "StationServices",
            "StationEconomy",
            "SystemAddress",
            "Name",
            "MarketID",
            "BodyID",
            "BodyName",
            "Latitude",
            "Longitude",
        ],
        Schema::DockingGranted => &["timestamp", "event", "horizons", "odyssey", "MarketID", "StationName", "StationType", "LandingPad"],
        Schema::DockingDenied => &["timestamp", "event", "horizons", "odyssey", "MarketID", "StationName", "StationType", "Reason"],
        Schema::FcMaterialsJournal => &["timestamp", "event", "horizons", "odyssey", "MarketID", "CarrierName", "CarrierID", "Items"],
        Schema::ScanOrganic => &[
            "timestamp",
            "event",
            "horizons",
            "odyssey",
            "StarSystem",
            "StarPos",
            "ScanType",
            "Genus",
            "Species",
            "Variant",
            "SystemAddress",
            "BodyID",
            "BodyName",
            "Latitude",
            "Longitude",
        ],
    }
}

#[cfg(test)]
#[path = "eddn_tests.rs"]
pub(crate) mod tests;
