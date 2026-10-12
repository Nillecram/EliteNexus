//! Replays the local journals and companion files (read-only, no network) through the EDDN
//! message builder as if every event were live, and validates each produced message against
//! the EDDN JSON schemas in `docs/research/eddn-schemas` (Draft 4, with format checks).
//!
//! ```text
//! cargo run -p nexus-uplink --example validate
//! cargo run -p nexus-uplink --example validate -- --dir <journal dir> --show 20 --dump <out dir>
//! ```
//!
//! Options:
//! * `--dir <path>`       journal folder (default: `%USERPROFILE%\Saved Games\Frontier Developments\Elite Dangerous`)
//! * `--extra <path>`     another folder with companion files (Market.json, NavRoute.json, …); repeatable
//! * `--schemas <path>`   schema folder (default: `docs/research/eddn-schemas`)
//! * `--show <n>`         print up to n failures (default 20)
//! * `--dump <path>`      write every generated message to `<path>/<schema>.jsonl`
//!
//! Besides the JSON schemas, every message is checked for things the schemas cannot express:
//! no `_Localised` or personal-data keys at any depth, byte-exact header values, `horizons`/
//! `odyssey` exactly as in LoadGame, augmented `StarPos` equal to the coordinates the journal
//! gave for that `SystemAddress`, no duplicate messages, and a compressed size below 1 MiB.

use nexus_journal::{Companion, CompanionKind, Timestamp, list_journal_files};
use nexus_uplink::context::ContextTracker;
use nexus_uplink::inara::{InaraEvent, InaraMapper};
use nexus_uplink::{edastro, edsm};
use nexus_uplink::eddn::{EddnBuilder, EddnMessage, EddnOptions, Schema, preflight};
use nexus_uplink::geo::star_pos_matches_address;
use nexus_uplink::json::{Obj, get_bool, get_str, get_u64, gzip, has_localised, parse_object};
use nexus_uplink::session::Session;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

struct Args {
    dir: Option<PathBuf>,
    extra: Vec<PathBuf>,
    schemas: PathBuf,
    show: usize,
    dump: Option<PathBuf>,
}

fn parse_args() -> Result<Args, String> {
    let mut a = Args {
        dir: None,
        extra: Vec::new(),
        schemas: Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/research/eddn-schemas"),
        show: 20,
        dump: None,
    };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        let mut value = || it.next().ok_or_else(|| format!("{arg} needs a value"));
        match arg.as_str() {
            "--dir" => a.dir = Some(PathBuf::from(value()?)),
            "--extra" => a.extra.push(PathBuf::from(value()?)),
            "--schemas" => a.schemas = PathBuf::from(value()?),
            "--show" => a.show = value()?.parse().map_err(|e| format!("--show: {e}"))?,
            "--dump" => a.dump = Some(PathBuf::from(value()?)),
            "-h" | "--help" => return Err("usage: validate [--dir <journal dir>] [--extra <dir>]... [--schemas <dir>] [--show <n>] [--dump <dir>]".into()),
            other => return Err(format!("unknown argument {other}")),
        }
    }
    Ok(a)
}

/// Keys that must never appear anywhere in a message (personal or ephemeral data).
const PERSONAL: [&str; 17] = [
    "MyReputation",
    "HappiestSystem",
    "HomeSystem",
    "SquadronFaction",
    "Wanted",
    "ActiveFine",
    "CockpitBreach",
    "BoostUsed",
    "FuelLevel",
    "FuelUsed",
    "JumpDist",
    "IsNewEntry",
    "NewTraitsDiscovered",
    "Progress",
    "TimeRemaining",
    "WasLogged",
    "gatewayTimestamp",
];

fn find_key(v: &Value, keys: &[&str]) -> Option<String> {
    match v {
        Value::Object(m) => m.iter().find_map(|(k, x)| if keys.contains(&k.as_str()) { Some(k.clone()) } else { find_key(x, keys) }),
        Value::Array(a) => a.iter().find_map(|x| find_key(x, keys)),
        _ => None,
    }
}

#[derive(Default)]
struct Tally {
    total: u64,
    valid: u64,
    invalid: u64,
    test_refs: u64,
    max_gzip: usize,
}

/// What the journal itself said (for independent cross-checks of the builder's output).
#[derive(Default)]
struct Truth {
    gameversion: String,
    gamebuild: String,
    horizons: Option<bool>,
    odyssey: Option<bool>,
    cmdr: String,
    /// Every StarPos the journal reported for a SystemAddress.
    positions: HashMap<u64, [f64; 3]>,
}

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    let Some(dir) = args.dir.clone().or_else(nexus_journal::default_journal_dir) else {
        eprintln!("cannot locate the journal folder; pass --dir");
        return ExitCode::from(2);
    };

    // ---- schemas ---------------------------------------------------------------------------
    let mut validators = BTreeMap::new();
    for schema in Schema::ALL {
        let path = args.schemas.join(schema.schema_file());
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("cannot read {}: {e}", path.display());
                return ExitCode::from(2);
            }
        };
        let json: Value = match serde_json::from_str(text.trim_start_matches('\u{feff}')) {
            Ok(j) => j,
            Err(e) => {
                eprintln!("{}: {e}", path.display());
                return ExitCode::from(2);
            }
        };
        match jsonschema::draft4::options().should_validate_formats(true).build(&json) {
            Ok(v) => {
                validators.insert(schema, v);
            }
            Err(e) => {
                eprintln!("{}: {e}", path.display());
                return ExitCode::from(2);
            }
        }
    }

    // ---- inputs ------------------------------------------------------------------------------
    let files = match list_journal_files(&dir) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("cannot list {}: {e}", dir.display());
            return ExitCode::from(2);
        }
    };
    let mut companions: Vec<(CompanionKind, Timestamp, Companion, PathBuf)> = Vec::new();
    for d in std::iter::once(&dir).chain(args.extra.iter()) {
        for kind in [CompanionKind::Market, CompanionKind::Outfitting, CompanionKind::Shipyard, CompanionKind::NavRoute, CompanionKind::FCMaterials] {
            let path = d.join(kind.file_name());
            let Ok(text) = std::fs::read_to_string(&path) else { continue };
            match Companion::parse(kind, &text) {
                Some(c) => companions.push((kind, c.timestamp(), c, path)),
                None => println!("note: {} could not be parsed", path.display()),
            }
        }
    }

    println!("Journal folder : {}", dir.display());
    println!("Journal files  : {}", files.len());
    println!(
        "Companions     : {}",
        if companions.is_empty() {
            "none".to_string()
        } else {
            companions.iter().map(|(k, t, _, _)| format!("{} ({t})", k.file_name())).collect::<Vec<_>>().join(", ")
        }
    );

    let mut dump: HashMap<Schema, std::fs::File> = HashMap::new();
    if let Some(d) = &args.dump
        && let Err(e) = std::fs::create_dir_all(d)
    {
        eprintln!("cannot create {}: {e}", d.display());
        return ExitCode::from(2);
    }

    // ---- replay ------------------------------------------------------------------------------
    let opts = EddnOptions { outfitting_v3: true, ..EddnOptions::default() };
    let mut builder = EddnBuilder::new(opts);
    let mut session = Session::new();
    let mut tracker = ContextTracker::new();
    let mut truth = Truth::default();
    let mut tallies: BTreeMap<Schema, Tally> = BTreeMap::new();
    let mut failures: Vec<String> = Vec::new();
    let mut compliance: BTreeMap<&'static str, u64> = BTreeMap::new();
    let mut seen_messages: HashSet<String> = HashSet::new();
    let mut lines_total = 0u64;
    let mut lines_unparseable: BTreeMap<String, u64> = BTreeMap::new();
    let mut companions_used = 0usize;
    let mut boxel_checked = 0u64;
    let mut boxel_mismatch: Vec<String> = Vec::new();
    let mut inara = InaraMapper::new();
    let mut dry = DryRun::default();
    let discard: HashSet<String> = edsm::DEFAULT_DISCARD.iter().map(|s| s.to_string()).collect();
    let accepting: HashSet<String> = edastro::DEFAULT_ACCEPTING.iter().map(|s| s.to_string()).collect();

    for f in &files {
        let bytes = match std::fs::read(&f.path) {
            Ok(b) => b,
            Err(e) => {
                println!("note: cannot read {}: {e}", f.path.display());
                continue;
            }
        };
        let text = String::from_utf8_lossy(&bytes);
        for line in text.lines() {
            let line = line.trim_end_matches('\r');
            if line.trim().is_empty() {
                continue;
            }
            lines_total += 1;
            let Some(ev) = parse_object(line) else {
                let name = nexus_journal::raw::scan_line(line).map(|(_, n)| n.to_string()).unwrap_or_else(|| "?".into());
                *lines_unparseable.entry(name).or_default() += 1;
                continue;
            };
            let Some(name) = get_str(&ev, "event").map(str::to_string) else { continue };
            observe_truth(&mut truth, &name, &ev, &mut boxel_checked, &mut boxel_mismatch);
            tracker.apply_json(&name, &ev);
            let ctx = tracker.context().clone();

            let mut out = builder.on_event(&mut session, &ctx, &name, &ev, true);
            // EDSM / Inara / EDAstro dry run (same gates as the live uplink).
            let sync_ok = session.blocked(Some(&ev)).is_none() && !session.is_beta(&ctx) && session.is_live_galaxy(&ctx);
            let inara_events = inara.on_event(&name, &ev, &session, &ctx);
            if sync_ok {
                dry.inara(inara_events);
                if edsm::is_discarded(&discard, &name) {
                    dry.edsm_discarded += 1;
                } else {
                    dry.edsm(&edsm::with_transient_fields(&ev, &ctx));
                }
                if accepting.contains(&name) && edastro::filter_event(&name, &ev).is_some() {
                    *dry.edastro.entry(name.clone()).or_default() += 1;
                }
            }
            // Simulate the watcher: the companion file is read right after its journal event.
            if let Some(kind) = CompanionKind::from_event_name(&name) {
                let ts = get_str(&ev, "timestamp").and_then(Timestamp::parse).unwrap_or_default();
                for (k, t, c, _) in &companions {
                    if *k == kind && (t.0 - ts.0).abs() <= 5 {
                        let produced = builder.on_companion(&mut session, &ctx, c, true);
                        if !produced.is_empty() {
                            companions_used += 1;
                        }
                        out.extend(produced);
                    }
                }
            }
            for m in out {
                check_message(&m, &validators, &truth, &mut tallies, &mut failures, &mut compliance, &mut seen_messages);
                if let Some(d) = &args.dump {
                    if let std::collections::hash_map::Entry::Vacant(slot) = dump.entry(m.schema) {
                        let p = d.join(format!("{}.jsonl", m.schema.path().replace('/', "-")));
                        match std::fs::File::create(&p) {
                            Ok(f) => {
                                slot.insert(f);
                            }
                            Err(e) => println!("note: cannot create {}: {e}", p.display()),
                        }
                    }
                    if let Some(file) = dump.get_mut(&m.schema) {
                        let _ = writeln!(file, "{}", m.to_json());
                    }
                }
            }
        }
    }
    let ctx = tracker.context().clone();
    for m in builder.flush(&session, &ctx) {
        check_message(&m, &validators, &truth, &mut tallies, &mut failures, &mut compliance, &mut seen_messages);
    }

    // ---- report ------------------------------------------------------------------------------
    println!("Journal lines  : {lines_total} ({} not valid JSON, skipped: {:?})", lines_unparseable.values().sum::<u64>(), lines_unparseable);
    println!("Companion files turned into messages: {companions_used}");
    println!();
    println!("{:<24} {:>8} {:>8} {:>8} {:>10}", "schema", "messages", "valid", "invalid", "max gzip B");
    println!("{}", "-".repeat(62));
    let (mut total, mut valid, mut invalid) = (0, 0, 0);
    for schema in Schema::ALL {
        let t = tallies.get(&schema);
        let (n, v, i, g) = t.map_or((0, 0, 0, 0), |t| (t.total, t.valid, t.invalid, t.max_gzip));
        total += n;
        valid += v;
        invalid += i;
        println!("{:<24} {:>8} {:>8} {:>8} {:>10}", schema.path(), n, v, i, g);
    }
    println!("{}", "-".repeat(62));
    println!("{:<24} {:>8} {:>8} {:>8}", "TOTAL", total, valid, invalid);
    let test_refs: u64 = tallies.values().map(|t| t.test_refs).sum();
    println!();
    println!("Extra compliance checks (beyond the JSON schemas):");
    let checks = [
        "pre-flight",
        "no _Localised key at any depth",
        "no personal/ephemeral key at any depth",
        "no top-level Latitude/Longitude in journal/1",
        "header byte-exact (uploaderID, gameversion, gamebuild)",
        "horizons/odyssey exactly as LoadGame",
        "StarPos matches the journal's coordinates for the SystemAddress",
        "no duplicate message",
        "gzip size < 1 MiB",
    ];
    for c in checks {
        let bad = compliance.get(c).copied().unwrap_or(0);
        println!("  [{}] {c}{}", if bad == 0 { "ok" } else { "FAIL" }, if bad == 0 { String::new() } else { format!(" ({bad} messages)") });
    }
    println!("  [{}] live schemas only (no /test refs for a live client): {test_refs} /test", if test_refs == 0 { "ok" } else { "info" });
    println!(
        "  [{}] id64 boxel check on every Location/FSDJump/CarrierJump: {boxel_checked} checked, {} mismatches",
        if boxel_mismatch.is_empty() { "ok" } else { "FAIL" },
        boxel_mismatch.len()
    );
    for b in boxel_mismatch.iter().take(5) {
        println!("      {b}");
    }

    println!();
    println!("Events that produced no message (by reason):");
    for (reason, n) in builder.drops() {
        println!("  {n:>6}  {reason}");
    }

    if !failures.is_empty() {
        println!();
        println!("FAILURES (first {} of {}):", args.show.min(failures.len()), failures.len());
        for f in failures.iter().take(args.show) {
            println!("{f}");
        }
    }

    let dry_bad = dry.report();
    let compliance_bad: u64 = compliance.values().sum::<u64>() + dry_bad;
    if invalid == 0 && compliance_bad == 0 && boxel_mismatch.is_empty() {
        println!();
        println!("RESULT: all {total} generated messages are valid ({}%).", (valid * 100).checked_div(total).unwrap_or(100));
        ExitCode::SUCCESS
    } else {
        println!();
        println!("RESULT: {invalid} schema failures, {compliance_bad} compliance failures, {} boxel mismatches.", boxel_mismatch.len());
        ExitCode::FAILURE
    }
}

fn observe_truth(truth: &mut Truth, name: &str, ev: &Obj, boxel_checked: &mut u64, boxel_mismatch: &mut Vec<String>) {
    match name {
        "Fileheader" => {
            truth.gameversion = get_str(ev, "gameversion").unwrap_or_default().to_string();
            truth.gamebuild = get_str(ev, "build").unwrap_or_default().to_string();
            if get_u64(ev, "part").unwrap_or(1) <= 1 {
                truth.horizons = None;
                truth.odyssey = None;
            }
        }
        "Commander" => truth.cmdr = get_str(ev, "Name").unwrap_or_default().to_string(),
        "LoadGame" => {
            truth.cmdr = get_str(ev, "Commander").unwrap_or_default().to_string();
            truth.horizons = get_bool(ev, "Horizons");
            truth.odyssey = get_bool(ev, "Odyssey");
            if let Some(v) = get_str(ev, "gameversion").filter(|v| !v.is_empty()) {
                truth.gameversion = v.to_string();
                truth.gamebuild = get_str(ev, "build").unwrap_or_default().to_string();
            }
        }
        "Location" | "FSDJump" | "CarrierJump" => {
            if let (Some(a), Some(p)) = (get_u64(ev, "SystemAddress"), nexus_uplink::json::get_star_pos(ev, "StarPos")) {
                truth.positions.insert(a, p);
                *boxel_checked += 1;
                if !star_pos_matches_address(a, p) {
                    boxel_mismatch.push(format!("{} {a} {p:?}", get_str(ev, "StarSystem").unwrap_or_default()));
                }
            }
        }
        _ => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn check_message(
    m: &EddnMessage,
    validators: &BTreeMap<Schema, jsonschema::Validator>,
    truth: &Truth,
    tallies: &mut BTreeMap<Schema, Tally>,
    failures: &mut Vec<String>,
    compliance: &mut BTreeMap<&'static str, u64>,
    seen: &mut HashSet<String>,
) {
    let t = tallies.entry(m.schema).or_default();
    t.total += 1;
    if m.test {
        t.test_refs += 1;
    }
    let Some(validator) = validators.get(&m.schema) else { return };
    let errors: Vec<String> = validator.iter_errors(&m.envelope).map(|e| format!("{e} at {}", e.instance_path())).collect();
    if errors.is_empty() {
        t.valid += 1;
    } else {
        t.invalid += 1;
        failures.push(format!("{}: {}\n    {}", m.schema.path(), errors.join("; "), m.to_json()));
    }
    let json = m.to_json();
    let gz = gzip(json.as_bytes()).map(|b| b.len()).unwrap_or(usize::MAX);
    t.max_gzip = t.max_gzip.max(gz);

    let mut fail = |check: &'static str, detail: String| {
        *compliance.entry(check).or_default() += 1;
        failures.push(format!("{} [{check}] {detail}\n    {json}", m.schema.path()));
    };
    if let Err(e) = preflight(m.schema, &m.envelope) {
        fail("pre-flight", e.to_string());
    }
    if has_localised(&m.envelope) {
        fail("no _Localised key at any depth", String::new());
    }
    if let Some(k) = find_key(&m.envelope["message"], &PERSONAL) {
        fail("no personal/ephemeral key at any depth", k);
    }
    let msg = m.message().cloned().unwrap_or_default();
    if m.schema == Schema::Journal && (msg.contains_key("Latitude") || msg.contains_key("Longitude")) {
        fail("no top-level Latitude/Longitude in journal/1", String::new());
    }
    let h = &m.envelope["header"];
    if h["uploaderID"].as_str() != Some(truth.cmdr.as_str())
        || h["gameversion"].as_str() != Some(truth.gameversion.as_str())
        || h["gamebuild"].as_str() != Some(truth.gamebuild.as_str())
    {
        fail("header byte-exact (uploaderID, gameversion, gamebuild)", h.to_string());
    }
    if msg.get("horizons").and_then(Value::as_bool) != truth.horizons || msg.get("odyssey").and_then(Value::as_bool) != truth.odyssey {
        fail("horizons/odyssey exactly as LoadGame", format!("{:?}/{:?}", truth.horizons, truth.odyssey));
    }
    if let (Some(addr), Some(pos)) = (get_u64(&msg, "SystemAddress"), nexus_uplink::json::get_star_pos(&msg, "StarPos")) {
        match truth.positions.get(&addr) {
            Some(p) if *p == pos => {}
            other => fail("StarPos matches the journal's coordinates for the SystemAddress", format!("{addr}: {pos:?} vs journal {other:?}")),
        }
    }
    if !seen.insert(json.clone()) {
        fail("no duplicate message", String::new());
    }
    if gz >= 1 << 20 {
        fail("gzip size < 1 MiB", gz.to_string());
    }
}

/// What EDSM, Inara and EDAstro would have received (nothing is sent).
#[derive(Default)]
struct DryRun {
    edsm_events: u64,
    edsm_discarded: u64,
    edsm_missing_transient: u64,
    inara: BTreeMap<String, u64>,
    inara_failures: Vec<String>,
    edastro: BTreeMap<String, u64>,
}

/// Required `eventData` properties per Inara event (docs/research/inara.md §5).
fn inara_required(name: &str) -> Option<&'static [&'static str]> {
    Some(match name {
        "addCommanderTravelDock" => &["starsystemName", "stationName"],
        "addCommanderTravelFSDJump" | "addCommanderTravelCarrierJump" | "setCommanderTravelLocation" => &["starsystemName"],
        "addCommanderTravelLand" => &["starsystemName", "starsystemBodyName"],
        "setCommanderShip" | "setCommanderShipLoadout" | "addCommanderShip" | "delCommanderShip" => &["shipType", "shipGameID"],
        "setCommanderShipTransfer" => &["shipType", "shipGameID", "starsystemName", "stationName"],
        "setCommanderRankPilot" => &["rankName"],
        "setCommanderRankEngineer" => &["engineerName"],
        "setCommanderRankPower" => &["powerName", "rankValue"],
        "setCommanderReputationMajorFaction" => &["majorfactionName", "majorfactionReputation"],
        "setCommanderReputationMinorFaction" => &["minorfactionName", "minorfactionReputation"],
        "setCommanderInventoryCargo" | "setCommanderInventoryMaterials" => &["itemName", "itemCount"],
        "setCommanderInventory" => &["itemName", "itemCount", "itemType"],
        "resetCommanderInventory" => &["itemType"],
        "setCommanderStorageModules" => &["itemName"],
        "addCommanderMission" => &["missionName", "missionGameID", "starsystemNameOrigin"],
        "setCommanderMissionAbandoned" | "setCommanderMissionCompleted" | "setCommanderMissionFailed" => &["missionGameID"],
        "addCommanderPermit" => &["starsystemName"],
        "addCommanderCombatDeath" => &["starsystemName"],
        "addCommanderCombatInterdicted" | "addCommanderCombatInterdiction" | "addCommanderCombatInterdictionEscape" => {
            &["starsystemName", "opponentName", "isPlayer"]
        }
        "addCommanderCombatKill" => &["starsystemName", "opponentName"],
        "setCommanderCredits" => &["commanderCredits"],
        "setCommanderGameStatistics" => &[],
        "setCommanderSuitLoadout" | "updateCommanderSuitLoadout" | "delCommanderSuitLoadout" => &["loadoutGameID"],
        "setCommunityGoal" => &["communitygoalGameID", "communitygoalName", "starsystemName", "stationName", "goalExpiry"],
        "setCommanderCommunityGoalProgress" => &["communitygoalGameID", "contribution", "percentileBand", "percentileBandReward"],
        "addCommanderFriend" | "delCommanderFriend" => &["commanderName", "gamePlatform"],
        _ => return None,
    })
}

impl DryRun {
    fn edsm(&mut self, ev: &Obj) {
        self.edsm_events += 1;
        let keys = ["_systemAddress", "_systemName", "_systemCoordinates", "_marketId", "_stationName", "_shipId"];
        if !keys.iter().all(|k| ev.contains_key(*k)) {
            self.edsm_missing_transient += 1;
        }
    }

    fn inara(&mut self, events: Vec<InaraEvent>) {
        for e in events {
            *self.inara.entry(e.name.clone()).or_default() += 1;
            let mut problem = None;
            if !nexus_uplink::json::is_date_time(&e.timestamp) {
                problem = Some("eventTimestamp is not ISO 8601".to_string());
            } else if nexus_uplink::json::has_null(&e.data) {
                problem = Some("null value".to_string());
            } else {
                match inara_required(&e.name) {
                    None => problem = Some("unknown Inara event".to_string()),
                    Some(req) => {
                        let items: Vec<&Value> = match &e.data {
                            Value::Array(a) => a.iter().collect(),
                            other => vec![other],
                        };
                        if items.is_empty() && !e.name.starts_with("setCommanderInventory") && e.name != "setCommanderStorageModules" {
                            problem = Some("empty eventData array".into());
                        }
                        for it in items {
                            if let Some(missing) = req.iter().find(|k| it.get(**k).is_none_or(|v| v.as_str() == Some(""))) {
                                problem = Some(format!("missing {missing}"));
                            }
                        }
                    }
                }
            }
            if let Some(p) = problem {
                self.inara_failures.push(format!("{}: {p}: {}", e.name, e.data));
            }
        }
    }

    fn report(&self) -> u64 {
        println!();
        println!("Other services (dry run with the same gates; nothing is sent):");
        println!(
            "  EDSM    : {} events would be sent ({} filtered by the bundled discard list); {} without all six transient fields",
            self.edsm_events, self.edsm_discarded, self.edsm_missing_transient
        );
        println!("  EDAstro : {}", if self.edastro.is_empty() { "nothing accepted".to_string() } else { format!("{:?}", self.edastro) });
        println!("  Inara   : {} events, {} with missing required properties", self.inara.values().sum::<u64>(), self.inara_failures.len());
        for (name, n) in &self.inara {
            println!("      {n:>6}  {name}");
        }
        for f in self.inara_failures.iter().take(10) {
            println!("      FAIL {f}");
        }
        self.edsm_missing_transient + self.inara_failures.len() as u64
    }
}
