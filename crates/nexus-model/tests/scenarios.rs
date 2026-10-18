//! Scenario tests: real-format journal lines through the full model.

use nexus_journal::{raw::scan_line, Event, EventCtx};
use nexus_model::{Alerts, Model};

/// Applies journal lines (live = true so alerts fire) and returns the collected alerts.
fn feed(model: &mut Model, lines: &[&str]) -> Vec<nexus_model::Alert> {
    let mut alerts = Alerts::live();
    for line in lines {
        let (ts, name) = scan_line(line).expect("valid journal line");
        let ev = Event::parse(name, line).expect("typed parse");
        let ctx = EventCtx { ts, name, line, live: true };
        model.apply(&ctx, &ev, &mut alerts);
    }
    alerts.items
}

const LOGIN: &[&str] = &[
    r#"{ "timestamp":"2026-10-01T10:00:00Z", "event":"Fileheader", "part":1, "language":"English/UK", "Odyssey":true, "gameversion":"4.4.1.1", "build":"r332841/r0 " }"#,
    r#"{ "timestamp":"2026-10-01T10:00:01Z", "event":"Commander", "FID":"F1", "Name":"Test" }"#,
    r#"{ "timestamp":"2026-10-01T10:00:02Z", "event":"LoadGame", "FID":"F1", "Commander":"Test", "Horizons":true, "Odyssey":true, "Ship":"explorer_nx", "ShipID":22, "ShipName":"", "ShipIdent":"", "FuelLevel":64.0, "FuelCapacity":64.0, "GameMode":"Solo", "Credits":1000000, "Loan":0, "language":"English/UK", "gameversion":"4.4.1.1", "build":"r332841/r0 " }"#,
    r#"{ "timestamp":"2026-10-01T10:00:03Z", "event":"Location", "Docked":false, "StarSystem":"Alpha", "SystemAddress":100, "StarPos":[0.0,0.0,0.0], "Population":0, "Body":"Alpha", "BodyID":0, "BodyType":"Star" }"#,
];

#[test]
fn login_sets_commander_and_location() {
    let mut m = Model::default();
    feed(&mut m, LOGIN);
    let gs = m.active().expect("commander");
    assert_eq!(gs.cmdr.name, "Test");
    assert_eq!(gs.cmdr.game_build, "r332841/r0 ", "game build must be kept byte-exact (EDDN)");
    assert_eq!(gs.location.system, "Alpha");
    assert_eq!(gs.fleet.current_ship_id, Some(22));
}

#[test]
fn unsold_data_and_sale_skipping() {
    let mut m = Model::default();
    feed(&mut m, LOGIN);
    let alerts = feed(
        &mut m,
        &[
            r#"{ "timestamp":"2026-10-01T10:01:00Z", "event":"Scan", "ScanType":"AutoScan", "BodyName":"Alpha", "BodyID":0, "StarSystem":"Alpha", "SystemAddress":100, "DistanceFromArrivalLS":0.0, "StarType":"K", "StellarMass":0.7, "WasDiscovered":true, "WasMapped":false }"#,
            r#"{ "timestamp":"2026-10-01T10:02:00Z", "event":"Scan", "ScanType":"Detailed", "BodyName":"Alpha 1", "BodyID":1, "StarSystem":"Alpha", "SystemAddress":100, "DistanceFromArrivalLS":400.0, "PlanetClass":"Earthlike body", "MassEM":1.0, "TerraformState":"", "Landable":false, "WasDiscovered":false, "WasMapped":false }"#,
            r#"{ "timestamp":"2026-10-01T10:05:00Z", "event":"FSDJump", "StarSystem":"Beta", "SystemAddress":200, "StarPos":[10.0,0.0,0.0], "Population":0, "JumpDist":10.0, "FuelUsed":1.0, "FuelLevel":63.0 }"#,
            r#"{ "timestamp":"2026-10-01T10:06:00Z", "event":"Scan", "ScanType":"AutoScan", "BodyName":"Beta", "BodyID":0, "StarSystem":"Beta", "SystemAddress":200, "DistanceFromArrivalLS":0.0, "StarType":"M", "StellarMass":0.3, "WasDiscovered":true }"#,
        ],
    );
    assert!(alerts.iter().any(|a| a.title.contains("Earth-like")), "ELW must raise an exploration alert");
    let gs = m.active().unwrap();
    assert_eq!(gs.exploration.unsold().count(), 2);
    assert!(gs.exploration.unsold_value() > 500_000, "first-discovered ELW is worth a lot");

    // Sell only Alpha: Beta was scanned before the sale but the game didn't buy it -> skipped.
    feed(
        &mut m,
        &[
            r#"{ "timestamp":"2026-10-01T11:00:00Z", "event":"FSDJump", "StarSystem":"Gamma", "SystemAddress":300, "StarPos":[20.0,0.0,0.0], "Population":5000, "JumpDist":10.0 }"#,
            r#"{ "timestamp":"2026-10-01T11:10:00Z", "event":"MultiSellExplorationData", "Discovered":[ { "SystemName":"Alpha", "NumBodies":2 } ], "BaseValue":300000, "Bonus":400000, "TotalEarnings":700000 }"#,
        ],
    );
    let gs = m.active().unwrap();
    assert_eq!(gs.exploration.unsold().count(), 0);
    assert_eq!(gs.exploration.skipped().count(), 1);
    assert_eq!(gs.exploration.sold_total, 700_000);

    // Sold data stays in the history, with what became of it.
    use nexus_model::exploration::DataStatus;
    let ex = &gs.exploration;
    assert_eq!(ex.history().count(), 2);
    let sale = ex.last_sale().expect("sale recorded");
    assert_eq!((sale.systems, sale.earnings), (1, 700_000));
    assert_eq!(ex.systems[&100].status(), DataStatus::Sold(sale.ts));
    assert_eq!(ex.systems[&200].status(), DataStatus::NotSellable);
}

#[test]
fn exobiology_sampling_and_bonus() {
    let mut m = Model::default();
    feed(&mut m, LOGIN);
    feed(
        &mut m,
        &[r#"{ "timestamp":"2026-10-01T10:01:00Z", "event":"Scan", "ScanType":"Detailed", "BodyName":"Alpha 2", "BodyID":2, "StarSystem":"Alpha", "SystemAddress":100, "DistanceFromArrivalLS":10.0, "PlanetClass":"Rocky body", "MassEM":0.1, "Landable":true, "WasDiscovered":false, "WasMapped":false, "WasFootfalled":false }"#],
    );
    let organic = |kind: &str, ts: &str| {
        format!(
            r#"{{ "timestamp":"{ts}", "event":"ScanOrganic", "ScanType":"{kind}", "Genus":"$Codex_Ent_Stratum_Genus_Name;", "Genus_Localised":"Stratum", "Species":"$Codex_Ent_Stratum_07_Name;", "Species_Localised":"Stratum Tectonicas", "Variant":"$Codex_Ent_Stratum_07_K_Name;", "Variant_Localised":"Stratum Tectonicas - Lime", "WasLogged":false, "SystemAddress":100, "Body":2 }}"#
        )
    };
    let l1 = organic("Log", "2026-10-01T10:10:00Z");
    let l2 = organic("Sample", "2026-10-01T10:12:00Z");
    let l3 = organic("Analyse", "2026-10-01T10:14:00Z");
    let alerts = feed(&mut m, &[&l1]);
    assert!(alerts.iter().any(|a| a.detail.contains("500 m")), "Stratum colony range is 500 m");
    feed(&mut m, &[&l2]);
    let alerts = feed(&mut m, &[&l3]);
    assert!(alerts.iter().any(|a| a.title.contains("Analysis complete")));
    let gs = m.active().unwrap();
    assert!(gs.exobio.current.is_none());
    assert_eq!(gs.exobio.unsold.len(), 1);
    let rec = &gs.exobio.unsold[0];
    assert_eq!(rec.value, 19_010_800);
    assert!(rec.bonus_expected, "unfootfalled body in an unpopulated system");
    assert_eq!(rec.expected_payout(), 19_010_800 * 5);

    // A partly sampled organism is abandoned when the commander jumps away.
    let l4 = organic("Log", "2026-10-01T10:20:00Z");
    feed(&mut m, &[&l4]);
    assert!(m.active().unwrap().exobio.current.is_some());
    feed(
        &mut m,
        &[r#"{ "timestamp":"2026-10-01T10:30:00Z", "event":"FSDJump", "StarSystem":"Beta", "SystemAddress":200, "StarPos":[10.0,0.0,0.0], "Population":0, "JumpDist":10.0, "FuelUsed":1.0, "FuelLevel":60.0 }"#],
    );
    assert!(m.active().unwrap().exobio.current.is_none(), "stale sample cleared after leaving the system");
}

#[test]
fn carrier_transfer_is_not_spending() {
    let mut m = Model::default();
    feed(&mut m, LOGIN);
    feed(
        &mut m,
        &[r#"{ "timestamp":"2026-10-01T10:30:00Z", "event":"CarrierBankTransfer", "CarrierID":3716323840, "Deposit":500000, "PlayerBalance":500000, "CarrierBalance":500000 }"#],
    );
    let gs = m.active().unwrap();
    assert_eq!(gs.session.expenses, 0, "a deposit to your own carrier is a transfer");
    assert_eq!(gs.cmdr.credits, 500_000);
}

#[test]
fn stored_ships_in_transit_show_destination() {
    let mut m = Model::default();
    feed(&mut m, LOGIN);
    feed(
        &mut m,
        &[r#"{ "timestamp":"2026-10-01T10:40:00Z", "event":"StoredShips", "StationName":"KFK-L3V", "MarketID":3716323840, "StarSystem":"Ross 775", "ShipsHere":[ ], "ShipsRemote":[ { "ShipID":5, "ShipType":"DiamondBackXL", "Name":"", "InTransit":true, "Value":1789130, "Hot":false } ] }"#],
    );
    let gs = m.active().unwrap();
    let s = &gs.fleet.ships[&5];
    assert!(s.location.in_transit);
    assert_eq!(s.location.station.as_deref(), Some("KFK-L3V"));
    assert_eq!(s.location.system.as_deref(), Some("Ross 775"));
    assert!(gs.fleet.ships.contains_key(&22), "current ship is kept");
}

#[test]
fn massacre_stack_end_to_end() {
    let mut m = Model::default();
    feed(&mut m, LOGIN);
    let accept = |id: u64, faction: &str, kills: u32, ts: &str| {
        format!(
            r#"{{ "timestamp":"{ts}", "event":"MissionAccepted", "Faction":"{faction}", "Name":"Mission_MassacreWing", "LocalisedName":"Kill pirates", "MissionID":{id}, "Reward":10000000, "TargetFaction":"Pirates", "KillCount":{kills}, "Expiry":"2026-10-03T10:00:00Z", "Wing":true }}"#
        )
    };
    let (a, b) = (accept(1, "A", 2, "2026-10-01T10:00:10Z"), accept(2, "B", 1, "2026-10-01T10:00:11Z"));
    feed(&mut m, &[&a, &b]);
    let bounty = r#"{ "timestamp":"2026-10-01T10:20:00Z", "event":"Bounty", "Rewards":[ { "Faction":"X", "Reward":1000 } ], "Target":"anaconda", "TotalReward":1000, "VictimFaction":"Pirates" }"#;
    let alerts = feed(&mut m, &[bounty]);
    assert_eq!(alerts.iter().filter(|a| a.title.contains("Massacre mission complete")).count(), 1, "B's single kill done");
    let stacks = m.active().unwrap().missions.massacre_stacks();
    assert_eq!(stacks[0].kills_remaining, 1);
    feed(&mut m, &[bounty]);
    assert_eq!(m.active().unwrap().missions.massacre_stacks()[0].kills_remaining, 0);
}

/// A construction depot snapshot, as the game writes it every few seconds while you're docked there.
fn depot_line(ts: &str, market_id: u64, progress: f64, provided_steel: i64) -> String {
    format!(
        r#"{{ "timestamp":"{ts}", "event":"ColonisationConstructionDepot", "MarketID":{market_id}, "ConstructionProgress":{progress}, "ConstructionComplete":false, "ConstructionFailed":false, "ResourcesRequired":[ {{ "Name":"$Aluminium_name;", "Name_Localised":"Aluminium", "RequiredAmount":510, "ProvidedAmount":0, "Payment":3239 }}, {{ "Name":"$Steel_name;", "Name_Localised":"Steel", "RequiredAmount":21325, "ProvidedAmount":{provided_steel}, "Payment":5057 }} ] }}"#
    )
}

const HINDS_PRIDE: u64 = 3_956_789_012;

/// Someone else's colonisation ship: the forum report of 0.5.0 (a phantom "$EXT_PANEL_ColonisationShip; Hind's Pride"
/// at 0 %, shown as an active construction forever).
#[test]
fn someone_elses_colonisation_ship_is_not_yours() {
    let mut m = Model::default();
    feed(&mut m, LOGIN);
    let snapshot = depot_line("2026-10-01T10:12:00Z", HINDS_PRIDE, 0.0, 0);
    feed(
        &mut m,
        &[
            r#"{ "timestamp":"2026-10-01T10:05:00Z", "event":"FSDJump", "StarSystem":"Hind", "SystemAddress":500, "StarPos":[5.0,0.0,0.0], "Population":0, "JumpDist":5.0, "FuelUsed":1.0, "FuelLevel":63.0 }"#,
            r#"{ "timestamp":"2026-10-01T10:11:00Z", "event":"Docked", "StationName":"$EXT_PANEL_ColonisationShip; Hind's Pride", "StationName_Localised":"System Colonisation Ship", "StationType":"SpaceConstructionDepot", "Taxi":false, "Multicrew":false, "StarSystem":"Hind", "SystemAddress":500, "MarketID":3956789012, "StationFaction":{ "Name":"Brewer Corporation" }, "StationGovernment":"$government_Corporate;", "StationServices":[ "dock", "autodock", "commodities", "contacts", "colonisationcontribution" ], "StationEconomy":"$economy_Colony;", "DistFromStarLS":12.3, "LandingPads":{ "Small":4, "Medium":4, "Large":8 } }"#,
            &snapshot,
        ],
    );
    let gs = m.active().unwrap();
    // The symbol is stripped wherever the name is shown: the situation line, the activity feed, the docking history.
    let here = gs.location.station.as_ref().expect("docked");
    assert_eq!(here.name, "Hind's Pride");
    assert!(here.is_colonisation_ship());
    assert_eq!(here.journal_name, "$EXT_PANEL_ColonisationShip; Hind's Pride", "uploads keep the journal's own name");
    assert!(gs.feed.iter().any(|f| f.text == "Docked at Hind's Pride (Hind)"));
    assert_eq!(gs.history.docks.last().map(|d| d.station.as_str()), Some("Hind's Pride"));

    let col = &gs.colonisation;
    let d = &col.depots[&HINDS_PRIDE];
    assert_eq!((d.station.as_deref(), d.system.as_str(), d.system_address), (Some("Hind's Pride"), "Hind", 500));
    assert!(d.colonisation_ship);
    assert_eq!(d.remaining_tons(), 21_835);
    use nexus_model::colonisation::Involvement;
    assert_eq!(col.involvement(d), Involvement::Visited, "no claim, no delivery: someone else's project");
    let soon = nexus_journal::Timestamp::parse("2026-10-01T12:00:00Z").unwrap();
    assert_eq!(col.active_depots(soon).count(), 0, "the dashboard doesn't show it as yours");

    // Leave and dock elsewhere. A snapshot of a depot you aren't docked at never gets this station's name or system;
    // one of a depot already known updates its figures and keeps where it is.
    let unknown = depot_line("2026-10-01T11:01:00Z", 3_956_000_001, 0.5, 100);
    let known = depot_line("2026-10-01T11:02:00Z", HINDS_PRIDE, 0.25, 5_000);
    feed(
        &mut m,
        &[
            r#"{ "timestamp":"2026-10-01T10:40:00Z", "event":"Undocked", "StationName":"$EXT_PANEL_ColonisationShip; Hind's Pride", "StationType":"SpaceConstructionDepot", "MarketID":3956789012, "Taxi":false, "Multicrew":false }"#,
            r#"{ "timestamp":"2026-10-01T10:50:00Z", "event":"FSDJump", "StarSystem":"Beta", "SystemAddress":200, "StarPos":[10.0,0.0,0.0], "Population":5000, "JumpDist":5.0, "FuelUsed":1.0, "FuelLevel":62.0 }"#,
            r#"{ "timestamp":"2026-10-01T11:00:00Z", "event":"Docked", "StationName":"Beta Hub", "StationType":"Coriolis", "StarSystem":"Beta", "SystemAddress":200, "MarketID":128000001, "StationServices":[ "dock", "commodities" ], "DistFromStarLS":300.0 }"#,
            &unknown,
            &known,
        ],
    );
    let gs = m.active().unwrap();
    assert!(gs.feed.iter().any(|f| f.text == "Undocked from Hind's Pride"));
    let col = &gs.colonisation;
    assert_eq!(col.depots.len(), 1, "an unplaced snapshot isn't pinned on the station you're docked at");
    let d = &col.depots[&HINDS_PRIDE];
    assert_eq!((d.station.as_deref(), d.system.as_str()), (Some("Hind's Pride"), "Hind"));
    assert_eq!(d.progress, 0.25);
    assert_eq!(d.remaining_tons(), 16_835);

    // Four weeks without a visit: out of date (a colonisation ship has four weeks to be finished).
    let later = nexus_journal::Timestamp::parse("2026-10-29T11:02:00Z").unwrap();
    assert!(d.is_stale(later));
    assert!(!d.is_stale(soon));
}

#[test]
fn claimed_and_delivered_constructions_are_yours() {
    let mut m = Model::default();
    feed(&mut m, LOGIN);
    let ship = depot_line("2026-10-01T10:21:00Z", 3_956_000_100, 0.1, 2_000);
    let site = depot_line("2026-10-01T11:01:00Z", 3_956_000_200, 0.6, 12_000);
    feed(
        &mut m,
        &[
            r#"{ "timestamp":"2026-10-01T10:01:00Z", "event":"ColonisationSystemClaim", "StarSystem":"Gamma", "SystemAddress":300 }"#,
            r#"{ "timestamp":"2026-10-01T10:10:00Z", "event":"FSDJump", "StarSystem":"Gamma", "SystemAddress":300, "StarPos":[20.0,0.0,0.0], "Population":0, "JumpDist":10.0 }"#,
            r#"{ "timestamp":"2026-10-01T10:20:00Z", "event":"Docked", "StationName":"$EXT_PANEL_ColonisationShip; Gamma Rising", "StationType":"SpaceConstructionDepot", "StarSystem":"Gamma", "SystemAddress":300, "MarketID":3956000100 }"#,
            &ship,
            r#"{ "timestamp":"2026-10-01T10:30:00Z", "event":"Undocked", "StationName":"$EXT_PANEL_ColonisationShip; Gamma Rising", "MarketID":3956000100 }"#,
            r#"{ "timestamp":"2026-10-01T10:50:00Z", "event":"FSDJump", "StarSystem":"Delta", "SystemAddress":400, "StarPos":[30.0,0.0,0.0], "Population":0, "JumpDist":10.0 }"#,
            r#"{ "timestamp":"2026-10-01T11:00:00Z", "event":"Docked", "StationName":"Orbital Construction Site: Delta Works", "StationType":"SpaceConstructionDepot", "StarSystem":"Delta", "SystemAddress":400, "MarketID":3956000200 }"#,
            &site,
            r#"{ "timestamp":"2026-10-01T11:02:00Z", "event":"ColonisationContribution", "MarketID":3956000200, "Contributions":[ { "Name":"$Steel_name;", "Name_Localised":"Steel", "Amount":200 } ] }"#,
        ],
    );
    use nexus_model::colonisation::Involvement;
    let col = &m.active().unwrap().colonisation;
    let ship = &col.depots[&3_956_000_100];
    assert_eq!(ship.station.as_deref(), Some("Gamma Rising"));
    assert_eq!(col.involvement(ship), Involvement::Claimed);
    let site = &col.depots[&3_956_000_200];
    assert!(!site.colonisation_ship);
    assert_eq!(site.delivered_tons(), 200);
    assert_eq!(col.involvement(site), Involvement::Contributed, "someone else's system, but you delivered");
    let soon = nexus_journal::Timestamp::parse("2026-10-02T00:00:00Z").unwrap();
    let active: Vec<u64> = col.active_depots(soon).map(|d| d.market_id).collect();
    assert_eq!(active, [3_956_000_200, 3_956_000_100], "most recently updated first");
    let later = nexus_journal::Timestamp::parse("2026-11-01T00:00:00Z").unwrap();
    assert_eq!(col.active_depots(later).count(), 0, "out of date after four weeks");

    // Releasing the claim makes the colonisation ship someone else's business again.
    feed(&mut m, &[r#"{ "timestamp":"2026-10-01T12:00:00Z", "event":"ColonisationSystemClaimRelease", "StarSystem":"Gamma", "SystemAddress":300 }"#]);
    let col = &m.active().unwrap().colonisation;
    assert_eq!(col.involvement(&col.depots[&3_956_000_100]), Involvement::Visited);
}
