//! Unit tests for the EDDN builders, using real journal lines. Every produced message is also
//! validated against the bundled EDDN JSON schemas.

use super::*;
use crate::json::parse_object;

pub(crate) fn schema_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/research/eddn-schemas")
}

/// Validates `envelope` against the schema file for `schema` (Draft 4, with formats).
pub(crate) fn validate(schema: Schema, envelope: &Value) -> Vec<String> {
    let path = schema_dir().join(schema.schema_file());
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let text = text.trim_start_matches('\u{feff}');
    let schema_json: Value = serde_json::from_str(text).unwrap();
    let validator = jsonschema::draft4::options().should_validate_formats(true).build(&schema_json).unwrap();
    validator.iter_errors(envelope).map(|e| format!("{} at {}", e, e.instance_path())).collect()
}

fn assert_valid(m: &EddnMessage) {
    let errors = validate(m.schema, &m.envelope);
    assert!(errors.is_empty(), "{} invalid: {errors:?}\n{}", m.schema.path(), m.envelope);
    assert!(preflight(m.schema, &m.envelope).is_ok());
}

const FILEHEADER: &str = r#"{ "timestamp":"2026-09-28T02:00:00Z", "event":"Fileheader", "part":1, "language":"English/UK", "Odyssey":true, "gameversion":"4.4.1.1", "build":"r332841/r0 " }"#;
const LOADGAME: &str = r#"{ "timestamp":"2026-09-28T02:00:10Z", "event":"LoadGame", "FID":"F1", "Commander":"Jameson", "Horizons":true, "Odyssey":true, "Ship":"Empire_Courier", "Ship_Localised":"Imperial Courier", "ShipID":17, "ShipName":"", "ShipIdent":"", "FuelLevel":8.000000, "FuelCapacity":8.000000, "GameMode":"Solo", "Credits":209273164, "Loan":0, "language":"English/UK", "gameversion":"4.4.1.1", "build":"r332841/r0 " }"#;
const FSDJUMP_SWOILZ: &str = r#"{ "timestamp":"2026-09-28T02:50:00Z", "event":"FSDJump", "Taxi":false, "Multicrew":false, "StarSystem":"Swoilz MH-W c17-3", "SystemAddress":908552967066, "StarPos":[27.78125,-491.25000,997.09375], "SystemAllegiance":"", "SystemEconomy":"$economy_None;", "SystemEconomy_Localised":"None", "SystemSecondEconomy":"$economy_None;", "SystemSecondEconomy_Localised":"None", "SystemGovernment":"$government_None;", "SystemGovernment_Localised":"None", "SystemSecurity":"$GAlAXY_MAP_INFO_state_anarchy;", "SystemSecurity_Localised":"Anarchy", "Population":0, "Body":"Swoilz MH-W c17-3 A", "BodyID":1, "BodyType":"Star", "JumpDist":21.123, "FuelUsed":1.234567, "FuelLevel":30.123456 }"#;

struct Rig {
    b: EddnBuilder,
    s: Session,
    ctx: UploadContext,
}

impl Rig {
    fn new() -> Self {
        let mut r = Rig { b: EddnBuilder::new(EddnOptions { outfitting_v3: true, ..Default::default() }), s: Session::new(), ctx: UploadContext::default() };
        r.feed(FILEHEADER);
        r.feed(LOADGAME);
        r
    }

    fn feed(&mut self, line: &str) -> Vec<EddnMessage> {
        let ev = parse_object(line).expect("test line is JSON");
        let name = get_str(&ev, "event").unwrap().to_string();
        let out = self.b.on_event(&mut self.s, &self.ctx, &name, &ev, true);
        for m in &out {
            assert_valid(m);
        }
        out
    }

    fn companion(&mut self, kind: CompanionKind, text: &str) -> Vec<EddnMessage> {
        let c = Companion::parse(kind, text).expect("companion parses");
        let out = self.b.on_companion(&mut self.s, &self.ctx, &c, true);
        for m in &out {
            assert_valid(m);
        }
        out
    }
}

fn msg(m: &EddnMessage) -> &Obj {
    m.message().unwrap()
}

#[test]
fn journal_fsdjump_strips_personal_data() {
    let mut r = Rig::new();
    let line = r#"{ "timestamp":"2026-09-05T01:06:02Z", "event":"FSDJump", "Taxi":false, "Multicrew":false, "StarSystem":"Synuefe BR-L c24-23", "SystemAddress":6406446944978, "StarPos":[220.96875,-124.81250,-12.59375], "SystemAllegiance":"Independent", "SystemEconomy":"$economy_Industrial;", "SystemEconomy_Localised":"Industrial", "Population":107996107, "Body":"Synuefe BR-L c24-23 A", "BodyID":1, "BodyType":"Star", "JumpDist":9.851, "FuelUsed":0.484949, "FuelLevel":7.515051, "BoostUsed":4, "Wanted":true, "Factions":[ { "Name":"Sacra Oculus", "FactionState":"None", "Government":"Theocracy", "Influence":0.162978, "Allegiance":"Independent", "Happiness":"$Faction_HappinessBand2;", "Happiness_Localised":"Happy", "MyReputation":0.000000, "SquadronFaction":true, "HappiestSystem":true, "HomeSystem":true, "RecoveringStates":[ { "State":"Expansion", "Trend":0 } ] } ] }"#;
    let out = r.feed(line);
    assert_eq!(out.len(), 1);
    let m = &out[0];
    assert_eq!(m.schema, Schema::Journal);
    assert_eq!(m.schema_ref(), "https://eddn.edcd.io/schemas/journal/1");
    let body = msg(m);
    for k in ["JumpDist", "FuelUsed", "FuelLevel", "BoostUsed", "Wanted", "SystemEconomy_Localised"] {
        assert!(!body.contains_key(k), "{k} must be removed");
    }
    let f = body["Factions"][0].as_object().unwrap();
    for k in ["MyReputation", "SquadronFaction", "HappiestSystem", "HomeSystem", "Happiness_Localised"] {
        assert!(!f.contains_key(k), "{k} must be removed");
    }
    assert_eq!(body["horizons"], json!(true));
    assert_eq!(body["odyssey"], json!(true));
    let header = m.envelope["header"].as_object().unwrap();
    assert_eq!(header["uploaderID"], json!("Jameson"));
    assert_eq!(header["softwareName"], json!("Elite Nexus"));
    assert_eq!(header["gameversion"], json!("4.4.1.1"));
    assert_eq!(header["gamebuild"], json!("r332841/r0 "), "gamebuild must keep its trailing space");
}

#[test]
fn journal_scan_gets_starpos_only_after_cross_check() {
    let mut r = Rig::new();
    r.feed(FSDJUMP_SWOILZ);
    let scan = r#"{ "timestamp":"2026-09-28T02:58:00Z", "event":"Scan", "ScanType":"AutoScan", "BodyName":"Swoilz MH-W c17-3 B 1", "BodyID":10, "Parents":[ {"Star":2}, {"Null":0} ], "StarSystem":"Swoilz MH-W c17-3", "SystemAddress":908552967066, "DistanceFromArrivalLS":1234.5, "TidalLock":true, "TerraformState":"", "PlanetClass":"Icy body", "Atmosphere":"", "AtmosphereType":"None", "Volcanism":"", "MassEM":0.012, "Radius":2000000.0, "SurfaceGravity":1.2, "SurfaceTemperature":40.0, "SurfacePressure":0.0, "Landable":true, "Materials":[ { "Name":"sulphur", "Percent":26.1 }, { "Name":"carbon", "Name_Localised":"Carbon", "Percent":22.0 } ], "Composition":{ "Ice":0.7, "Rock":0.2, "Metal":0.1 }, "SemiMajorAxis":1.0e9, "Eccentricity":0.0001, "OrbitalInclination":0.1, "Periapsis":12.0, "OrbitalPeriod":1.0e6, "AscendingNode":1.0, "MeanAnomaly":2.0, "RotationPeriod":1.0e6, "AxialTilt":0.1, "WasDiscovered":true, "WasMapped":false, "WasFootfalled":false }"#;
    let out = r.feed(scan);
    assert_eq!(out.len(), 1);
    assert_eq!(msg(&out[0])["StarPos"], json!([27.78125, -491.25, 997.09375]));
    assert!(!has_localised(&out[0].envelope));

    // A Scan of the previous system arriving after a jump must be dropped.
    let jump = r#"{ "timestamp":"2026-09-28T03:00:00Z", "event":"FSDJump", "StarSystem":"Synuefe BR-L c24-23", "SystemAddress":6406446944978, "StarPos":[220.96875,-124.81250,-12.59375], "Population":0, "Body":"Synuefe BR-L c24-23 A", "BodyID":1, "BodyType":"Star", "JumpDist":9.851, "FuelUsed":0.484949, "FuelLevel":7.515051 }"#;
    assert_eq!(r.feed(jump).len(), 1);
    assert!(r.feed(&scan.replace("02:58:00", "03:00:05")).is_empty());
    assert!(r.b.drops().keys().any(|k| k.contains("does not match")));
}

#[test]
fn journal_docked_and_saasignals() {
    let mut r = Rig::new();
    let loc = r#"{ "timestamp":"2026-09-27T23:10:00Z", "event":"Location", "DistFromStarLS":12.3, "Docked":false, "Taxi":false, "Multicrew":false, "StarSystem":"Brestla", "SystemAddress":673101653409, "StarPos":[159.46875,-103.75000,-19.68750], "Body":"Brestla A 1", "BodyID":8, "BodyType":"Planet", "Latitude":15.1, "Longitude":78.0, "Population":0 }"#;
    let out = r.feed(loc);
    assert_eq!(out.len(), 1);
    assert!(!msg(&out[0]).contains_key("Latitude"), "Location Latitude is personal");
    let docked = r#"{ "timestamp":"2026-09-27T23:24:28Z", "event":"Docked", "StationName":"i Sola Prospect", "StationType":"CraterOutpost", "Taxi":false, "Multicrew":false, "StarSystem":"Brestla", "SystemAddress":673101653409, "MarketID":3516500480, "StationFaction":{ "Name":"Heilelang Patron's Principles", "FactionState":"Boom" }, "StationGovernment":"$government_Patronage;", "StationGovernment_Localised":"Patronage", "StationAllegiance":"Empire", "StationServices":[ "dock", "autodock", "commodities" ], "StationEconomy":"$economy_Colony;", "StationEconomy_Localised":"Colony", "StationEconomies":[ { "Name":"$economy_Colony;", "Name_Localised":"Colony", "Proportion":1.000000 } ], "DistFromStarLS":1048.35, "LandingPads":{ "Small":4, "Medium":4, "Large":2 }, "ActiveFine":true }"#;
    let out = r.feed(docked);
    assert_eq!(out.len(), 1);
    let body = msg(&out[0]);
    assert!(!body.contains_key("ActiveFine"));
    assert_eq!(body["StarPos"], json!([159.46875, -103.75, -19.6875]));
    let saa = r#"{ "timestamp":"2026-09-27T23:30:00Z", "event":"SAASignalsFound", "BodyName":"Brestla A 1", "SystemAddress":673101653409, "BodyID":8, "Signals":[ { "Type":"$SAA_SignalType_Biological;", "Type_Localised":"Biological", "Count":4 } ], "Genuses":[ { "Genus":"$Codex_Ent_Bacterial_Genus_Name;", "Genus_Localised":"Bacterium" } ] }"#;
    let out = r.feed(saa);
    assert_eq!(out.len(), 1);
    assert_eq!(msg(&out[0])["StarSystem"], json!("Brestla"));
    assert!(!has_localised(&out[0].envelope));
}

#[test]
fn fss_discovery_and_all_bodies() {
    let mut r = Rig::new();
    let jump = r#"{ "timestamp":"2026-09-27T20:27:30Z", "event":"FSDJump", "StarSystem":"Synuefe BR-L c24-23", "SystemAddress":6406446944978, "StarPos":[220.96875,-124.81250,-12.59375], "Population":0, "Body":"Synuefe BR-L c24-23 A", "BodyID":1, "BodyType":"Star", "JumpDist":9.851, "FuelUsed":0.484949, "FuelLevel":7.515051 }"#;
    r.feed(jump);
    let out = r.feed(r#"{ "timestamp":"2026-09-27T20:27:49Z", "event":"FSSDiscoveryScan", "Progress":0.465262, "BodyCount":22, "NonBodyCount":27, "SystemName":"Synuefe BR-L c24-23", "SystemAddress":6406446944978 }"#);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].schema, Schema::FssDiscoveryScan);
    assert!(!msg(&out[0]).contains_key("Progress"));
    let out = r.feed(r#"{ "timestamp":"2026-09-27T21:28:27Z", "event":"FSSAllBodiesFound", "SystemName":"Synuefe BR-L c24-23", "SystemAddress":6406446944978, "Count":22 }"#);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].schema, Schema::FssAllBodiesFound);
    // NaN progress: the line is not JSON at all and is never parsed (see json::parse_object).
    assert!(parse_object(r#"{ "timestamp":"2026-10-12T06:25:17Z", "event":"FSSDiscoveryScan", "Progress":-nan(ind), "BodyCount":0, "NonBodyCount":0, "SystemName":"Luyten's Star", "SystemAddress":7268024264097 }"#).is_none());
}

#[test]
fn fss_body_signals_and_nav_beacon() {
    let mut r = Rig::new();
    r.feed(FSDJUMP_SWOILZ);
    let out = r.feed(r#"{ "timestamp":"2026-09-28T02:58:26Z", "event":"FSSBodySignals", "BodyName":"Swoilz MH-W c17-3 B 1", "BodyID":10, "SystemAddress":908552967066, "Signals":[ { "Type":"$PlanetaryMiningLocation_Name;", "Type_Localised":"Planetary Mining Location", "Count":13 } ] }"#);
    assert_eq!(out.len(), 1);
    assert_eq!(msg(&out[0])["Signals"], json!([{"Type": "$PlanetaryMiningLocation_Name;", "Count": 13}]));
    let out = r.feed(r#"{ "timestamp":"2026-09-28T03:08:04Z", "event":"NavBeaconScan", "SystemAddress":908552967066, "NumBodies":46 }"#);
    assert_eq!(out.len(), 1);
    assert_eq!(msg(&out[0])["StarSystem"], json!("Swoilz MH-W c17-3"));
}

#[test]
fn fss_signals_are_batched_and_filtered() {
    let mut r = Rig::new();
    r.feed(FSDJUMP_SWOILZ);
    let sig = |t: &str, extra: &str| {
        format!(r#"{{ "timestamp":"{t}", "event":"FSSSignalDiscovered", "SystemAddress":908552967066, {extra} }}"#)
    };
    assert!(r.feed(&sig("2026-09-28T02:51:00Z", r#""SignalName":"$Warzone_PointRace_Low:#index=1;", "SignalName_Localised":"Conflict Zone [Low Intensity]", "SignalType":"Combat""#)).is_empty());
    assert!(r.feed(&sig("2026-09-28T02:51:00Z", r#""SignalName":"$Warzone_PointRace_Low:#index=1;", "SignalName_Localised":"Conflict Zone [Low Intensity]", "SignalType":"Combat""#)).is_empty());
    assert!(r.feed(&sig("2026-09-28T02:51:01Z", r#""SignalName":"$USS_HighGradeEmissions;", "SignalName_Localised":"Unidentified signal source", "SignalType":"USS", "USSType":"$USS_Type_ValuableSalvage;", "USSType_Localised":"Encoded emissions", "SpawningState":"", "SpawningFaction":"The Dukes of Mikunn", "ThreatLevel":0, "TimeRemaining":1626.583130"#)).is_empty());
    assert!(r.feed(&sig("2026-09-28T02:51:02Z", r#""SignalName":"$USS_MissionTarget;", "SignalType":"USS", "USSType":"$USS_Type_MissionTarget;", "TimeRemaining":100.0"#)).is_empty());
    assert!(r.feed(&sig("2026-09-28T02:51:03Z", r#""SignalName":"KFK-L3V", "SignalType":"FleetCarrier", "IsStation":true"#)).is_empty());
    let out = r.feed(r#"{ "timestamp":"2026-09-28T02:51:10Z", "event":"Music", "MusicTrack":"Supercruise" }"#);
    assert_eq!(out.len(), 1);
    let m = msg(&out[0]);
    assert_eq!(out[0].schema, Schema::FssSignalDiscovered);
    assert_eq!(m["timestamp"], json!("2026-09-28T02:51:00Z"));
    assert_eq!(m["StarSystem"], json!("Swoilz MH-W c17-3"));
    let signals = m["signals"].as_array().unwrap();
    assert_eq!(signals.len(), 3, "duplicate and mission-target signals removed: {signals:?}");
    assert!(signals.iter().all(|s| s.get("TimeRemaining").is_none() && s.get("SystemAddress").is_none() && s.get("event").is_none()));
}

#[test]
fn fss_signals_odyssey_order_uses_the_arrival() {
    let mut r = Rig::new();
    r.feed(FSDJUMP_SWOILZ);
    // Signals of the *next* system written before its FSDJump (Odyssey ordering).
    r.feed(r#"{ "timestamp":"2026-09-28T03:00:00Z", "event":"FSSSignalDiscovered", "SystemAddress":6406446944978, "SignalName":"SadiCarnot Reach", "SignalType":"StationONeilOrbis", "IsStation":true }"#);
    let jump = r#"{ "timestamp":"2026-09-28T03:00:01Z", "event":"FSDJump", "StarSystem":"Synuefe BR-L c24-23", "SystemAddress":6406446944978, "StarPos":[220.96875,-124.81250,-12.59375], "Population":1, "Body":"Synuefe BR-L c24-23 A", "BodyID":1, "BodyType":"Star", "JumpDist":9.851, "FuelUsed":0.484949, "FuelLevel":7.515051 }"#;
    let out = r.feed(jump);
    assert_eq!(out.len(), 2);
    assert_eq!(out[0].schema, Schema::FssSignalDiscovered);
    assert_eq!(msg(&out[0])["StarSystem"], json!("Synuefe BR-L c24-23"));
    assert_eq!(out[1].schema, Schema::Journal);
    // Horizons ordering with a mismatching first signal: the whole batch is dropped.
    r.feed(r#"{ "timestamp":"2026-09-28T03:01:00Z", "event":"FSSSignalDiscovered", "SystemAddress":908552967066, "SignalName":"X", "SignalType":"USS" }"#);
    assert!(r.feed(r#"{ "timestamp":"2026-09-28T03:01:05Z", "event":"Music", "MusicTrack":"Supercruise" }"#).is_empty());
}

#[test]
fn codex_entry_whitelisted_with_body() {
    let mut r = Rig::new();
    r.feed(FSDJUMP_SWOILZ);
    r.feed(r#"{ "timestamp":"2026-09-28T03:05:00Z", "event":"ApproachBody", "StarSystem":"Swoilz MH-W c17-3", "SystemAddress":908552967066, "Body":"Swoilz MH-W c17-3 A 4", "BodyID":6 }"#);
    let out = r.feed(r#"{ "timestamp":"2026-09-28T03:13:55Z", "event":"CodexEntry", "EntryID":2420702, "Name":"$Codex_Ent_Stratum_07_K_Name;", "Name_Localised":"Stratum Tectonicas - Lime", "SubCategory":"$Codex_SubCategory_Organic_Structures;", "SubCategory_Localised":"Organic structures", "Category":"$Codex_Category_Biology;", "Category_Localised":"Biological and Geological", "Region":"$Codex_RegionName_18;", "Region_Localised":"Inner Orion Spur", "System":"Swoilz MH-W c17-3", "SystemAddress":908552967066, "BodyID":6, "NearestDestination":"", "Latitude":-39.788132, "Longitude":115.866631, "IsNewEntry":true }"#);
    assert_eq!(out.len(), 1);
    let m = msg(&out[0]);
    assert!(!m.contains_key("IsNewEntry"));
    assert_eq!(m["BodyName"], json!("Swoilz MH-W c17-3 A 4"));
    assert_eq!(m["BodyID"], json!(6));
    assert_eq!(m["StarPos"], json!([27.78125, -491.25, 997.09375]));
    // An empty Name is dropped rather than sent without it.
    let bad = r#"{ "timestamp":"2026-09-28T03:14:00Z", "event":"CodexEntry", "EntryID":1, "Name":"", "SubCategory":"$a;", "Category":"$b;", "Region":"$c;", "System":"Swoilz MH-W c17-3", "SystemAddress":908552967066 }"#;
    assert!(r.feed(bad).is_empty());
}

#[test]
fn codex_entry_body_from_status_only_when_names_agree() {
    let mut r = Rig::new();
    r.feed(FSDJUMP_SWOILZ);
    r.feed(r#"{ "timestamp":"2026-09-28T03:05:00Z", "event":"ApproachBody", "StarSystem":"Swoilz MH-W c17-3", "SystemAddress":908552967066, "Body":"Swoilz MH-W c17-3 7 a", "BodyID":20 }"#);
    // Status.json says we are now near the binary companion "7 b".
    let st = r#"{ "timestamp":"2026-09-28T03:06:00Z", "event":"Status", "Flags":2097152, "Flags2":0, "BodyName":"Swoilz MH-W c17-3 7 b", "Latitude":1.0, "Longitude":2.0 }"#;
    r.companion(CompanionKind::Status, st);
    let ev = r#"{ "timestamp":"2026-09-28T03:07:00Z", "event":"CodexEntry", "EntryID":1400160, "Name":"$Codex_Ent_IceFumarole_WaterGeysers_Name;", "SubCategory":"$Codex_SubCategory_Geology_and_Anomalies;", "Category":"$Codex_Category_Biology;", "Region":"$Codex_RegionName_18;", "System":"Swoilz MH-W c17-3", "SystemAddress":908552967066 }"#;
    let out = r.feed(ev);
    let m = msg(&out[0]);
    assert_eq!(m["BodyName"], json!("Swoilz MH-W c17-3 7 b"));
    assert!(!m.contains_key("BodyID"), "BodyID only when the status and journal names agree");
}

#[test]
fn scan_organic_rules() {
    let mut r = Rig::new();
    r.feed(FSDJUMP_SWOILZ);
    r.feed(r#"{ "timestamp":"2026-09-28T03:05:00Z", "event":"ApproachBody", "StarSystem":"Swoilz MH-W c17-3", "SystemAddress":908552967066, "Body":"Swoilz MH-W c17-3 A 4", "BodyID":6 }"#);
    let st = r#"{ "timestamp":"2026-09-28T03:13:50Z", "event":"Status", "Flags":2097152, "Flags2":16, "Oxygen":1.0, "Health":1.0, "Temperature":293.0, "SelectedWeapon":"$humanoid_sampletool_name;", "Gravity":0.1, "LegalState":"Clean", "Latitude":-39.788132, "Longitude":115.866631, "Heading":10, "BodyName":"Swoilz MH-W c17-3 A 4" }"#;
    r.companion(CompanionKind::Status, st);
    let line = r#"{ "timestamp":"2026-09-28T03:13:55Z", "event":"ScanOrganic", "ScanType":"Log", "Genus":"$Codex_Ent_Stratum_Genus_Name;", "Genus_Localised":"Stratum", "Species":"$Codex_Ent_Stratum_07_Name;", "Species_Localised":"Stratum Tectonicas", "Variant":"$Codex_Ent_Stratum_07_K_Name;", "Variant_Localised":"Stratum Tectonicas - Lime", "WasLogged":false, "SystemAddress":908552967066, "Body":6 }"#;
    let out = r.feed(line);
    assert_eq!(out.len(), 1);
    let m = msg(&out[0]);
    assert_eq!(m["BodyID"], json!(6));
    assert!(!m.contains_key("Body") && !m.contains_key("WasLogged"));
    assert_eq!(m["BodyName"], json!("Swoilz MH-W c17-3 A 4"));
    assert_eq!(m["Latitude"], json!(-39.788132));
    let analyse = line.replace("\"Log\"", "\"Analyse\"");
    assert!(r.feed(&analyse).is_empty());
}

#[test]
fn scan_bary_centre_and_approach_settlement() {
    let mut r = Rig::new();
    let jump = r#"{ "timestamp":"2026-09-05T01:06:02Z", "event":"FSDJump", "StarSystem":"Synuefe BR-L c24-23", "SystemAddress":6406446944978, "StarPos":[220.96875,-124.81250,-12.59375], "Population":0, "Body":"Synuefe BR-L c24-23 A", "BodyID":1, "BodyType":"Star", "JumpDist":9.851, "FuelUsed":0.484949, "FuelLevel":7.515051 }"#;
    r.feed(jump);
    let out = r.feed(r#"{ "timestamp":"2026-09-05T01:06:08Z", "event":"ScanBaryCentre", "StarSystem":"Synuefe BR-L c24-23", "SystemAddress":6406446944978, "BodyID":5, "SemiMajorAxis":3173774600.028992, "Eccentricity":0.000005, "OrbitalInclination":-0.000842, "Periapsis":59.954093, "OrbitalPeriod":141736.418009, "AscendingNode":-44.134295, "MeanAnomaly":23.785090 }"#);
    assert_eq!(out.len(), 1);
    let loc = r#"{ "timestamp":"2026-09-27T23:10:00Z", "event":"Location", "Docked":false, "StarSystem":"Brestla", "SystemAddress":673101653409, "StarPos":[159.46875,-103.75000,-19.68750], "Body":"Brestla A 1", "BodyID":8, "BodyType":"Planet", "Population":0 }"#;
    r.feed(loc);
    let settlement = r#"{ "timestamp":"2026-09-27T23:20:43Z", "event":"ApproachSettlement", "Name":"i Sola Prospect", "MarketID":3516500480, "StationFaction":{ "Name":"Heilelang Patron's Principles", "Name_Localised":"Heilelang", "FactionState":"Boom" }, "StationGovernment":"$government_Patronage;", "StationGovernment_Localised":"Patronage", "StationAllegiance":"Empire", "StationServices":[ "dock", "autodock", "commodities" ], "StationEconomy":"$economy_Colony;", "StationEconomy_Localised":"Colony", "StationEconomies":[ { "Name":"$economy_Colony;", "Name_Localised":"Colony", "Proportion":1.000000 } ], "SystemAddress":673101653409, "BodyID":8, "BodyName":"Brestla A 1", "Latitude":15.128005, "Longitude":78.013084 }"#;
    let out = r.feed(settlement);
    assert_eq!(out.len(), 1);
    let m = msg(&out[0]);
    assert_eq!(m["MarketID"], json!(3_516_500_480u64));
    assert_eq!(m["StationFaction"], json!({"Name": "Heilelang Patron's Principles", "FactionState": "Boom"}));
    // Without Latitude/Longitude (login bug) the message is dropped.
    let no_latlon = settlement.replace(r#", "Latitude":15.128005, "Longitude":78.013084"#, "");
    assert!(r.feed(&no_latlon).is_empty());
}

#[test]
fn docking_events() {
    let mut r = Rig::new();
    let out = r.feed(r#"{ "timestamp":"2026-09-05T01:08:28Z", "event":"DockingGranted", "LandingPad":36, "MarketID":4220345091, "StationName":"SadiCarnot Reach", "StationName_Localised":"Sadi Carnot", "StationType":"Orbis" }"#);
    assert_eq!(out.len(), 1);
    let m = msg(&out[0]);
    assert!(!m.contains_key("StarSystem") && !m.contains_key("StarPos") && !m.contains_key("SystemAddress"));
    let out = r.feed(r#"{ "timestamp":"2026-09-05T01:08:08Z", "event":"DockingDenied", "Reason":"Distance", "MarketID":4220345091, "StationName":"SadiCarnot Reach", "StationType":"Orbis" }"#);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].schema, Schema::DockingDenied);
}

const MARKET_JSON: &str = r#"{ "timestamp":"2026-10-12T06:38:32Z", "event":"Market", "MarketID":3704011520, "StationName":"B4Z-LQY", "StationType":"FleetCarrier", "CarrierDockingAccess":"all", "StarSystem":"Luyten's Star", "Items":[
{ "id":128049200, "Name":"$superconductors_name;", "Name_Localised":"Superconductors", "Category":"$MARKET_category_industrial_materials;", "Category_Localised":"Industrial materials", "BuyPrice":328, "SellPrice":0, "MeanPrice":0, "StockBracket":0, "DemandBracket":0, "Stock":0, "Demand":0, "Consumer":false, "Producer":false, "Rare":false },
{ "id":128668547, "Name":"$unknownartifact_name;", "Name_Localised":"Thargoid Sensor", "Category":"$MARKET_category_salvage;", "Category_Localised":"Salvage", "BuyPrice":0, "SellPrice":30418000, "MeanPrice":0, "StockBracket":0, "DemandBracket":2, "Stock":0, "Demand":15, "Consumer":true, "Producer":false, "Rare":false },
{ "id":128066403, "Name":"$drones_name;", "Name_Localised":"Limpet", "Category":"$MARKET_category_nonmarketable;", "Category_Localised":"Limpet", "BuyPrice":101, "SellPrice":0, "MeanPrice":0, "StockBracket":3, "DemandBracket":0, "Stock":9999, "Demand":0, "Consumer":false, "Producer":true, "Rare":false }
 ] }"#;

#[test]
fn commodity_from_market_json() {
    let mut r = Rig::new();
    // Without the announcing journal event, the file is ignored (it may be stale).
    assert!(r.companion(CompanionKind::Market, MARKET_JSON).is_empty());
    r.feed(r#"{ "timestamp":"2026-10-12T06:38:32Z", "event":"Market", "MarketID":3704011520, "StationName":"B4Z-LQY", "StationType":"FleetCarrier", "CarrierDockingAccess":"all", "StarSystem":"Luyten's Star" }"#);
    let out = r.companion(CompanionKind::Market, MARKET_JSON);
    assert_eq!(out.len(), 1);
    let m = msg(&out[0]);
    assert_eq!(m["systemName"], json!("Luyten's Star"));
    assert_eq!(m["stationType"], json!("FleetCarrier"));
    let cs = m["commodities"].as_array().unwrap();
    assert_eq!(cs.len(), 2, "limpets removed");
    assert_eq!(cs[0]["name"], json!("superconductors"));
    assert_eq!(cs[1]["statusFlags"], json!(["Consumer"]));
    assert!(cs.iter().all(|c| c.get("id").is_none() && c.get("Rare").is_none() && c.get("Producer").is_none()));
    // Unchanged data for the same market is not re-sent.
    r.feed(r#"{ "timestamp":"2026-10-12T06:40:00Z", "event":"Market", "MarketID":3704011520, "StationName":"B4Z-LQY", "StationType":"FleetCarrier", "StarSystem":"Luyten's Star" }"#);
    let again = MARKET_JSON.replace("06:38:32", "06:40:00");
    assert!(r.companion(CompanionKind::Market, &again).is_empty());
}

#[test]
fn outfitting_v2_v3_deduplicated() {
    let mut r = Rig::new();
    r.feed(r#"{ "timestamp":"2026-10-11T17:29:49Z", "event":"Outfitting", "MarketID":3228867072, "StationName":"Dobrovolskiy Enterprise", "StarSystem":"WISE 1506+7027" }"#);
    let file = r#"{ "timestamp":"2026-10-11T17:29:49Z", "event":"Outfitting", "MarketID":3228867072, "StationName":"Dobrovolskiy Enterprise", "StarSystem":"WISE 1506+7027", "Horizons":true, "Items":[
{ "id":128049409, "Name":"hpt_pulselaserburst_turret_large", "BuyPrice":800400 },
{ "id":128891602, "Name":"hpt_basicmissilerack_fixed_large", "BuyPrice":0, "BuyMercCoinsPrice":850 },
{ "id":128891602, "Name":"hpt_basicmissilerack_fixed_large", "BuyPrice":0, "BuyMercCoinsPrice":850 },
{ "id":128891603, "Name":"hpt_basicmissilerack_fixed_large", "BuyPrice":1021500 },
{ "id":128049250, "Name":"sidewinder_armour_grade1", "BuyPrice":0 },
{ "id":128672313, "Name":"int_planetapproachsuite", "BuyPrice":500 },
{ "id":128667728, "Name":"paintjob_sidewinder_default_01", "BuyPrice":0 },
{ "id":128064338, "Name":"int_engine_size3_class5", "BuyPrice":507912 }
 ] }"#;
    let out = r.companion(CompanionKind::Outfitting, file);
    assert_eq!(out.len(), 2);
    let v2 = msg(&out[0]);
    assert_eq!(out[0].schema, Schema::Outfitting2);
    assert_eq!(
        v2["modules"],
        json!(["Hpt_basicmissilerack_fixed_large", "Hpt_pulselaserburst_turret_large", "Int_engine_size3_class5", "sidewinder_Armour_grade1"])
    );
    let v3 = msg(&out[1]);
    assert_eq!(out[1].schema, Schema::Outfitting3);
    let mods = v3["modules"].as_array().unwrap();
    assert_eq!(mods.len(), 5, "exact duplicates dropped, same name with other id/price kept: {mods:?}");
    assert!(mods.iter().all(|m| m.get("BuyMercCoinsPrice").is_some()));
}

#[test]
fn shipyard_and_navroute() {
    let mut r = Rig::new();
    r.feed(r#"{ "timestamp":"2026-10-12T08:16:48Z", "event":"Shipyard", "MarketID":3716323840, "StationName":"KFK-L3V", "StarSystem":"Ross 775" }"#);
    let file = r#"{ "timestamp":"2026-10-12T08:16:48Z", "event":"Shipyard", "MarketID":3716323840, "StationName":"KFK-L3V", "StarSystem":"Ross 775", "Horizons":true, "AllowCobraMkIV":false, "PriceList":[
{ "id":0, "ShipType":"explorer_nx", "ShipType_Localised":"Caspian Explorer", "ShipPrice":293975947 },
{ "id":1, "ShipType":"Explorer_NX", "ShipPrice":293975947 } ] }"#;
    let out = r.companion(CompanionKind::Shipyard, file);
    assert_eq!(out.len(), 1);
    assert_eq!(msg(&out[0])["ships"], json!(["explorer_nx"]));
    assert_eq!(msg(&out[0])["allowCobraMkIV"], json!(false));

    r.feed(r#"{ "timestamp":"2026-10-12T08:20:32Z", "event":"NavRoute" }"#);
    let route = r#"{ "timestamp":"2026-10-12T08:20:32Z", "event":"NavRoute", "Route":[
{ "StarSystem":"Synuefe BR-L c24-24", "SystemAddress":6681324851922, "StarPos":[228.37500,-130.09375,-8.81250], "StarClass":"K" },
{ "StarSystem":"Synuefe BR-L c24-23", "SystemAddress":6406446944978, "StarPos":[220.96875,-124.81250,-12.59375], "StarClass":"K" },
{ "StarSystem":"Brestla", "SystemAddress":673101653409, "StarPos":[159.46875,-103.75000,-19.68750], "StarClass":"M" }
 ] }"#;
    let out = r.companion(CompanionKind::NavRoute, route);
    assert_eq!(out.len(), 1);
    assert_eq!(msg(&out[0])["Route"].as_array().unwrap().len(), 3);
    // A cleared route is never sent.
    r.feed(r#"{ "timestamp":"2026-10-12T08:21:00Z", "event":"NavRoute" }"#);
    assert!(r.companion(CompanionKind::NavRoute, r#"{ "timestamp":"2026-10-12T08:21:00Z", "event":"NavRouteClear", "Route":[  ] }"#).is_empty());
}

#[test]
fn fc_materials() {
    let mut r = Rig::new();
    r.feed(r#"{ "timestamp":"2026-10-12T08:30:00Z", "event":"FCMaterials", "MarketID":3716323840, "CarrierName":"TCG BARBAROS", "CarrierID":"KFK-L3V" }"#);
    let file = r#"{ "timestamp":"2026-10-12T08:30:01Z", "event":"FCMaterials", "MarketID":3716323840, "CarrierName":"TCG BARBAROS", "CarrierID":"KFK-L3V", "Items":[
{ "id":128961524, "Name":"$aerogel_name;", "Name_Localised":"Aerogel", "Price":500, "Stock":0, "Demand":10 },
{ "id":128972290, "Name":"$chemicalcatalyst_name;", "Name_Localised":"Chemical Catalyst", "Price":400, "Stock":5, "Demand":0 } ] }"#;
    let out = r.companion(CompanionKind::FCMaterials, file);
    assert_eq!(out.len(), 1);
    let items = msg(&out[0])["Items"].as_array().unwrap();
    assert!(items.iter().all(|i| i.get("Name_Localised").is_none()));
}

#[test]
fn no_upload_without_session_or_in_crew() {
    let mut b = EddnBuilder::new(EddnOptions::default());
    let mut s = Session::new();
    let ctx = UploadContext::default();
    let ev = parse_object(FSDJUMP_SWOILZ).unwrap();
    assert!(b.on_event(&mut s, &ctx, "FSDJump", &ev, true).is_empty(), "no commander/game version yet");

    let mut r = Rig::new();
    r.feed(r#"{ "timestamp":"2026-09-28T02:10:00Z", "event":"JoinACrew", "Captain":"Someone Else" }"#);
    assert!(r.feed(FSDJUMP_SWOILZ).is_empty());
    r.feed(r#"{ "timestamp":"2026-09-28T02:55:00Z", "event":"QuitACrew", "Captain":"Someone Else" }"#);
    assert_eq!(r.feed(FSDJUMP_SWOILZ).len(), 1);
}

#[test]
fn test_schemas_for_beta_and_test_mode() {
    let mut r = Rig::new();
    r.feed(r#"{ "timestamp":"2026-09-28T02:30:00Z", "event":"Fileheader", "part":1, "language":"English/UK", "Odyssey":true, "gameversion":"4.5.0.100 Beta", "build":"r340000/r0 " }"#);
    r.feed(LOADGAME.replace("\"gameversion\":\"4.4.1.1\"", "\"gameversion\":\"4.5.0.100 Beta\"").as_str());
    let out = r.feed(FSDJUMP_SWOILZ);
    assert_eq!(out[0].schema_ref(), "https://eddn.edcd.io/schemas/journal/1/test");

    let mut b = EddnBuilder::new(EddnOptions { force_test: true, ..Default::default() });
    let mut s = Session::new();
    let ctx = UploadContext::default();
    for l in [FILEHEADER, LOADGAME] {
        let ev = parse_object(l).unwrap();
        b.on_event(&mut s, &ctx, get_str(&ev, "event").unwrap(), &ev, true);
    }
    let ev = parse_object(FSDJUMP_SWOILZ).unwrap();
    let out = b.on_event(&mut s, &ctx, "FSDJump", &ev, true);
    assert!(out[0].test);
    assert!(out[0].schema_ref().ends_with("/test"));
}

#[test]
fn horizons_odyssey_omitted_when_unknown() {
    let mut b = EddnBuilder::new(EddnOptions::default());
    let mut s = Session::new();
    let ctx = UploadContext::default();
    let lines = [
        r#"{ "timestamp":"2026-09-28T02:00:00Z", "event":"Fileheader", "part":1, "language":"English/UK", "Odyssey":false, "gameversion":"3.8.0.407", "build":"r269000/r0 " }"#,
        r#"{ "timestamp":"2026-09-28T02:00:10Z", "event":"LoadGame", "FID":"F1", "Commander":"Jameson", "Horizons":true, "Credits":1, "Loan":0 }"#,
    ];
    for l in lines {
        let ev = parse_object(l).unwrap();
        b.on_event(&mut s, &ctx, get_str(&ev, "event").unwrap(), &ev, true);
    }
    let ev = parse_object(FSDJUMP_SWOILZ).unwrap();
    let out = b.on_event(&mut s, &ctx, "FSDJump", &ev, true);
    let m = msg(&out[0]);
    assert_eq!(m["horizons"], json!(true));
    assert!(!m.contains_key("odyssey"), "no Odyssey key in LoadGame: omit, never false/null");
    assert_valid(&out[0]);
}

#[test]
fn context_fallback_when_app_started_mid_session() {
    // No Fileheader/LoadGame seen: everything comes from the app's model.
    let mut b = EddnBuilder::new(EddnOptions::default());
    let mut s = Session::new();
    let ctx = UploadContext {
        cmdr: "Jameson".into(),
        game_version: "4.4.1.1".into(),
        game_build: "r332841/r0 ".into(),
        horizons: Some(true),
        odyssey: Some(true),
        system_name: "Swoilz MH-W c17-3".into(),
        system_address: 908_552_967_066,
        star_pos: Some([27.78125, -491.25, 997.09375]),
        ..Default::default()
    };
    let ev = parse_object(r#"{ "timestamp":"2026-09-28T02:58:26Z", "event":"FSSBodySignals", "BodyName":"Swoilz MH-W c17-3 B 1", "BodyID":10, "SystemAddress":908552967066, "Signals":[ { "Type":"$SAA_SignalType_Geological;", "Type_Localised":"Geological", "Count":2 } ] }"#).unwrap();
    let out = b.on_event(&mut s, &ctx, "FSSBodySignals", &ev, true);
    assert_eq!(out.len(), 1);
    assert_valid(&out[0]);
    assert_eq!(out[0].envelope["header"]["uploaderID"], json!("Jameson"));
}

#[test]
fn preflight_catches_bad_messages() {
    let good = json!({"$schemaRef": "https://eddn.edcd.io/schemas/dockingdenied/1", "header": {"uploaderID": "a", "softwareName": "b", "softwareVersion": "c", "gameversion": "", "gamebuild": ""}, "message": {"timestamp": "2026-01-01T00:00:00Z", "event": "DockingDenied", "MarketID": 1, "StationName": "x", "Reason": "Distance"}});
    assert!(preflight(Schema::DockingDenied, &good).is_ok());
    let mut extra = good.clone();
    extra["message"]["StarSystem"] = json!("Sol");
    assert!(preflight(Schema::DockingDenied, &extra).is_err());
    let mut null = good.clone();
    null["message"]["StationType"] = Value::Null;
    assert!(preflight(Schema::DockingDenied, &null).is_err());
    let mut loc = good.clone();
    loc["message"]["StationName_Localised"] = json!("x");
    assert!(preflight(Schema::DockingDenied, &loc).is_err());
    assert!(preflight(Schema::Journal, &good).is_err(), "schema ref mismatch");
}

#[test]
fn schema_refs_round_trip() {
    for s in Schema::ALL {
        assert_eq!(Schema::from_schema_ref(&s.schema_ref(false)), Some((s, false)));
        assert_eq!(Schema::from_schema_ref(&s.schema_ref(true)), Some((s, true)));
        assert!(schema_dir().join(s.schema_file()).exists(), "{}", s.schema_file());
    }
    assert_eq!(Schema::from_schema_ref("https://eddn.edcd.io/schemas/blackmarket/1"), None);
}

#[test]
fn validator_is_strict() {
    // Guards the meaning of "100 % valid": formats, additionalProperties and Draft-4 integers
    // are all enforced by the validator used in tests and in the `validate` example.
    let good = json!({"$schemaRef": "https://eddn.edcd.io/schemas/dockingdenied/1", "header": {"uploaderID": "a", "softwareName": "b", "softwareVersion": "c", "gameversion": "", "gamebuild": ""}, "message": {"timestamp": "2026-01-01T00:00:00Z", "event": "DockingDenied", "MarketID": 1, "StationName": "x", "Reason": "Distance"}});
    assert!(validate(Schema::DockingDenied, &good).is_empty());
    let mut bad_time = good.clone();
    bad_time["message"]["timestamp"] = json!("2026-01-01 00:00:00");
    assert!(!validate(Schema::DockingDenied, &bad_time).is_empty(), "date-time format is checked");
    let mut extra = good.clone();
    extra["message"]["StarSystem"] = json!("Sol");
    assert!(!validate(Schema::DockingDenied, &extra).is_empty(), "additionalProperties is enforced");
    let mut float_id = good.clone();
    float_id["message"]["MarketID"] = json!(1.5);
    assert!(!validate(Schema::DockingDenied, &float_id).is_empty(), "integers are integers");
    let mut dup = json!({"$schemaRef": "https://eddn.edcd.io/schemas/shipyard/2", "header": good["header"].clone(), "message": {"systemName": "a", "stationName": "b", "marketId": 1, "timestamp": "2026-01-01T00:00:00Z", "ships": ["x", "x"]}});
    assert!(!validate(Schema::Shipyard, &dup).is_empty(), "uniqueItems is enforced");
    dup["message"]["ships"] = json!(["x"]);
    assert!(validate(Schema::Shipyard, &dup).is_empty());
    let mut loc = json!({"$schemaRef": "https://eddn.edcd.io/schemas/journal/1", "header": good["header"].clone(), "message": {"timestamp": "2026-01-01T00:00:00Z", "event": "Docked", "StarSystem": "Sol", "StarPos": [0.0, 0.0, 0.0], "SystemAddress": 10477373803u64, "StationName_Localised": "x"}});
    assert!(!validate(Schema::Journal, &loc).is_empty(), "_Localised is disallowed in journal/1");
    loc["message"].as_object_mut().unwrap().remove("StationName_Localised");
    loc["message"]["Wanted"] = json!(false);
    assert!(!validate(Schema::Journal, &loc).is_empty(), "personal keys are disallowed in journal/1");
}

#[test]
fn carrier_jump_and_horizons_market_edge_cases() {
    let mut r = Rig::new();
    // CarrierJump while docked on the carrier (journal/1, like FSDJump).
    let jump = r#"{ "timestamp":"2026-10-12T09:00:00Z", "event":"CarrierJump", "Docked":true, "StationName":"KFK-L3V", "StationType":"FleetCarrier", "MarketID":3716323840, "StationFaction":{ "Name":"FleetCarrier" }, "StationGovernment":"$government_Carrier;", "StationGovernment_Localised":"Private Ownership ", "StationServices":[ "dock", "autodock", "commodities" ], "StationEconomy":"$economy_Carrier;", "StationEconomy_Localised":"Private Enterprise", "StationEconomies":[ { "Name":"$economy_Carrier;", "Name_Localised":"Private Enterprise", "Proportion":1.000000 } ], "Taxi":false, "Multicrew":false, "StarSystem":"Synuefe BR-L c24-23", "SystemAddress":6406446944978, "StarPos":[220.96875,-124.81250,-12.59375], "SystemAllegiance":"Independent", "SystemEconomy":"$economy_Industrial;", "Population":0, "Body":"Synuefe BR-L c24-23 A", "BodyID":1, "BodyType":"Star", "Factions":[ { "Name":"Sacra Oculus", "FactionState":"None", "Government":"Theocracy", "Influence":0.162978, "Allegiance":"Independent", "Happiness":"$Faction_HappinessBand2;", "MyReputation":0.0 } ] }"#;
    let out = r.feed(jump);
    assert_eq!(out.len(), 1);
    assert!(!msg(&out[0])["Factions"][0].as_object().unwrap().contains_key("MyReputation"));
    // A market with a temporarily available commodity ("" bracket) and an empty fleet carrier market.
    r.feed(r#"{ "timestamp":"2026-10-12T09:01:00Z", "event":"Market", "MarketID":3716323840, "StationName":"KFK-L3V", "StationType":"FleetCarrier", "StarSystem":"Synuefe BR-L c24-23" }"#);
    let market = r#"{ "timestamp":"2026-10-12T09:01:00Z", "event":"Market", "MarketID":3716323840, "StationName":"KFK-L3V", "StationType":"FleetCarrier", "CarrierDockingAccess":"all", "StarSystem":"Synuefe BR-L c24-23", "Items":[ { "id":128049204, "Name":"$tritium_name;", "Name_Localised":"Tritium", "Category":"$MARKET_category_chemicals;", "BuyPrice":50000, "SellPrice":0, "MeanPrice":51707, "StockBracket":"", "DemandBracket":0, "Stock":1000, "Demand":0, "Consumer":false, "Producer":false, "Rare":false } ] }"#;
    // nexus_journal::Companion::parse rejects "" brackets (MarketItem brackets are i64), so build the
    // companion by hand: the EDDN builder only uses the raw text.
    let c = Companion::Market(std::sync::Arc::new(nexus_journal::companion::MarketFile::default()), std::sync::Arc::from(market));
    let out = r.b.on_companion(&mut r.s, &r.ctx, &c, true);
    assert_valid(&out[0]);
    assert_eq!(msg(&out[0])["commodities"][0]["stockBracket"], json!(""));
    r.feed(r#"{ "timestamp":"2026-10-12T09:02:00Z", "event":"Market", "MarketID":3716323841, "StationName":"X9X-X9X", "StationType":"FleetCarrier", "StarSystem":"Synuefe BR-L c24-23" }"#);
    let empty = r#"{ "timestamp":"2026-10-12T09:02:00Z", "event":"Market", "MarketID":3716323841, "StationName":"X9X-X9X", "StationType":"FleetCarrier", "StarSystem":"Synuefe BR-L c24-23", "Items":[ ] }"#;
    let out = r.companion(CompanionKind::Market, empty);
    assert_eq!(msg(&out[0])["commodities"], json!([]), "an empty carrier market is meaningful and valid");
}
