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

/// The carrier jump timer through real journal lines: request, cancel, request again, arrive
/// while docked aboard, cool down, ready.
#[test]
fn carrier_jump_countdown_then_cooldown() {
    use nexus_journal::Timestamp;
    use nexus_model::carrier::JumpTimer;
    let t = |s: &str| Timestamp::parse(s).expect("time");
    let mut m = Model::default();
    feed(&mut m, LOGIN);
    feed(
        &mut m,
        &[
            r#"{ "timestamp":"2026-10-01T10:00:05Z", "event":"CarrierLocation", "CarrierType":"FleetCarrier", "CarrierID":3716323840, "StarSystem":"Alpha", "SystemAddress":100, "BodyID":0 }"#,
            r#"{ "timestamp":"2026-10-01T10:10:00Z", "event":"CarrierJumpRequest", "CarrierType":"FleetCarrier", "CarrierID":3716323840, "SystemName":"Beta", "Body":"Beta A 1", "SystemAddress":200, "BodyID":3, "DepartureTime":"2026-10-01T10:25:10Z" }"#,
        ],
    );
    let c = &m.active().unwrap().carrier;
    let timer = c.jump_timer(t("2026-10-01T10:15:10Z")).expect("countdown");
    assert!(matches!(&timer, JumpTimer::Countdown { to, .. } if to == "Beta"));
    assert_eq!(timer.left(t("2026-10-01T10:15:10Z")), Some(600));

    feed(&mut m, &[r#"{ "timestamp":"2026-10-01T10:12:00Z", "event":"CarrierJumpCancelled", "CarrierType":"FleetCarrier", "CarrierID":3716323840 }"#]);
    assert_eq!(m.active().unwrap().carrier.jump_timer(t("2026-10-01T10:12:01Z")), None, "cancelled");

    feed(
        &mut m,
        &[
            r#"{ "timestamp":"2026-10-01T10:20:00Z", "event":"CarrierJumpRequest", "CarrierType":"FleetCarrier", "CarrierID":3716323840, "SystemName":"Beta", "SystemAddress":200, "BodyID":0, "DepartureTime":"2026-10-01T10:35:00Z" }"#,
            r#"{ "timestamp":"2026-10-01T10:35:40Z", "event":"CarrierJump", "Docked":true, "StationName":"KFK-L3V", "StationType":"FleetCarrier", "MarketID":3716323840, "StarSystem":"Beta", "SystemAddress":200, "StarPos":[10.0,0.0,0.0], "Population":0, "Body":"Beta", "BodyID":0, "BodyType":"Star" }"#,
        ],
    );
    let gs = m.active().unwrap();
    assert_eq!(gs.carrier.system.as_deref(), Some("Beta"));
    let timer = gs.carrier.jump_timer(t("2026-10-01T10:36:40Z")).expect("cooldown");
    assert_eq!(timer, JumpTimer::Cooldown { at: "Beta".into(), body: Some("Beta".into()), ready: t("2026-10-01T10:40:40Z"), estimated: false });
    assert_eq!(gs.carrier.jump_timer(t("2026-10-01T10:40:40Z")), None, "ready to jump again");
}

const OWN_STATS: &str = r#"{ "timestamp":"2026-10-01T10:05:00Z", "event":"CarrierStats", "CarrierID":3716323840, "CarrierType":"FleetCarrier", "Callsign":"KFK-L3V", "Name":"TCG BARBAROS", "DockingAccess":"all", "AllowNotorious":false, "FuelLevel":500, "JumpRangeCurr":500.000000, "JumpRangeMax":500.000000, "PendingDecommission":false, "SpaceUsage":{ "TotalCapacity":25000, "Crew":6170, "Cargo":1200, "CargoSpaceReserved":0, "ShipPacks":0, "ModulePacks":0, "FreeSpace":17630 }, "Finance":{ "CarrierBalance":1000000000, "ReserveBalance":0, "AvailableBalance":1000000000 }, "Crew":[ { "CrewRole":"Captain", "Activated":true, "Enabled":true, "CrewName":"Raphael Mendez" } ], "ShipPacks":[ ], "ModulePacks":[ ] }"#;
const SQUADRON_STATS: &str = r#"{ "timestamp":"2026-10-01T10:06:00Z", "event":"CarrierStats", "CarrierID":3720000000, "CarrierType":"SquadronCarrier", "Callsign":"SQD-001", "Name":"VANGUARD HOME", "DockingAccess":"squadron", "AllowNotorious":false, "FuelLevel":1800, "JumpRangeCurr":500.000000, "JumpRangeMax":500.000000, "PendingDecommission":false, "SpaceUsage":{ "TotalCapacity":60000, "Crew":6170, "Cargo":20000, "CargoSpaceReserved":0, "ShipPacks":0, "ModulePacks":0, "FreeSpace":33830 }, "Finance":{ "CarrierBalance":5000000000, "ReserveBalance":0, "AvailableBalance":5000000000 }, "Crew":[ ], "ShipPacks":[ ], "ModulePacks":[ ] }"#;

/// A squadron carrier next to the commander's own: each keeps its own stats, place and jumps.
#[test]
fn squadron_carrier_alongside_a_personal_one() {
    use nexus_journal::Timestamp;
    use nexus_model::carrier::{CarrierKind, JumpTimer};
    let t = |s: &str| Timestamp::parse(s).expect("time");
    let mut m = Model::default();
    feed(&mut m, LOGIN);
    feed(
        &mut m,
        &[
            r#"{ "timestamp":"2026-10-01T10:00:04Z", "event":"SquadronStartup", "SquadronName":"VANGUARDS OF TEST", "SquadronID":42, "CurrentRank":3, "CurrentRankName":"Officer" }"#,
            r#"{ "timestamp":"2026-10-01T10:00:05Z", "event":"CarrierLocation", "CarrierType":"FleetCarrier", "CarrierID":3716323840, "StarSystem":"Alpha", "SystemAddress":100, "BodyID":0 }"#,
            r#"{ "timestamp":"2026-10-01T10:00:05Z", "event":"CarrierLocation", "CarrierType":"SquadronCarrier", "CarrierID":3720000000, "StarSystem":"Gamma", "SystemAddress":300, "BodyID":1 }"#,
            OWN_STATS,
            SQUADRON_STATS,
            r#"{ "timestamp":"2026-10-01T10:10:00Z", "event":"CarrierJumpRequest", "CarrierType":"SquadronCarrier", "CarrierID":3720000000, "SystemName":"Delta", "SystemAddress":400, "BodyID":0, "DepartureTime":"2026-10-01T10:25:00Z" }"#,
            r#"{ "timestamp":"2026-10-01T10:11:00Z", "event":"CarrierDepositFuel", "CarrierID":3720000000, "Amount":200, "Total":2000 }"#,
        ],
    );
    let gs = m.active().unwrap();
    let (own, sq) = (&gs.carrier, &gs.squadron_carrier);
    assert_eq!((own.callsign.as_str(), own.fuel, own.system.as_deref()), ("KFK-L3V", 500, Some("Alpha")));
    assert_eq!((sq.callsign.as_str(), sq.fuel, sq.system.as_deref()), ("SQD-001", 2000, Some("Gamma")), "an untyped event goes by the carrier's id");
    assert_eq!(sq.space.total_capacity, 60_000);
    assert!(own.pending_jump.is_none(), "the squadron carrier's jump isn't the personal one's");
    let now = t("2026-10-01T10:20:00Z");
    assert!(matches!(sq.jump_timer(now), Some(JumpTimer::Countdown { to, .. }) if to == "Delta"));
    let kinds: Vec<CarrierKind> = gs.carriers(now).into_iter().map(|(k, _)| k).collect();
    assert_eq!(kinds, [CarrierKind::Personal, CarrierKind::Squadron]);

    // Docked aboard the squadron carrier when it jumps: only it moves.
    feed(
        &mut m,
        &[r#"{ "timestamp":"2026-10-01T10:25:40Z", "event":"CarrierJump", "Docked":true, "StationName":"SQD-001", "StationType":"SquadronCarrier", "MarketID":3720000000, "StarSystem":"Delta", "SystemAddress":400, "StarPos":[30.0,0.0,0.0], "Population":0, "Body":"Delta", "BodyID":0, "BodyType":"Star" }"#],
    );
    let gs = m.active().unwrap();
    assert_eq!(gs.squadron_carrier.system.as_deref(), Some("Delta"));
    assert_eq!(gs.carrier.system.as_deref(), Some("Alpha"));
    assert!(matches!(gs.squadron_carrier.jump_timer(t("2026-10-01T10:26:00Z")), Some(JumpTimer::Cooldown { estimated: false, .. })));

    // Credits given to the squadron's carrier are spent; on the commander's own they stay theirs.
    feed(
        &mut m,
        &[
            r#"{ "timestamp":"2026-10-01T10:30:00Z", "event":"CarrierBankTransfer", "CarrierType":"SquadronCarrier", "CarrierID":3720000000, "Deposit":100000, "PlayerBalance":900000, "CarrierBalance":5000100000 }"#,
            r#"{ "timestamp":"2026-10-01T10:31:00Z", "event":"CarrierBankTransfer", "CarrierType":"FleetCarrier", "CarrierID":3716323840, "Deposit":200000, "PlayerBalance":700000, "CarrierBalance":1000200000 }"#,
        ],
    );
    let gs = m.active().unwrap();
    assert_eq!(gs.session.expenses, -100_000);
    assert_eq!((gs.squadron_carrier.finance.carrier_balance, gs.carrier.finance.carrier_balance), (5_000_100_000, 1_000_200_000));

    // Leaving the squadron leaves its carrier behind.
    feed(&mut m, &[r#"{ "timestamp":"2026-10-01T11:00:00Z", "event":"LeftSquadron", "SquadronName":"VANGUARDS OF TEST", "SquadronID":42 }"#]);
    let gs = m.active().unwrap();
    assert!(!gs.squadron_carrier.owned());
    assert!(gs.carrier.owned());
}

/// The Reddit report: a commander scrapped their own carrier and now uses their squadron's. The
/// page kept showing the old one as "decommissioning" and never the squadron carrier.
#[test]
fn decommissioned_carrier_gives_way_to_the_squadron_carrier() {
    use nexus_journal::Timestamp;
    use nexus_model::carrier::CarrierKind;
    let t = |s: &str| Timestamp::parse(s).expect("time");
    let mut m = Model::default();
    feed(&mut m, LOGIN);
    feed(
        &mut m,
        &[
            OWN_STATS,
            r#"{ "timestamp":"2026-10-01T10:20:00Z", "event":"CarrierDecommission", "CarrierID":3716323840, "ScrapRefund":4850000000, "ScrapTime":1791454800 }"#,
            &OWN_STATS.replace("10:05:00", "10:21:00").replace(r#""PendingDecommission":false"#, r#""PendingDecommission":true"#),
            SQUADRON_STATS,
        ],
    );
    let gs = m.active().unwrap();
    let before = t("2026-10-08T10:19:59Z");
    let after = t("2026-10-08T10:20:00Z");
    assert!(gs.carrier.pending_decommission);
    assert_eq!(gs.carrier.decommission.map(|d| (d.scrap_time, d.refund)), Some((after, 4_850_000_000)));
    assert_eq!(gs.carriers(before).len(), 2, "still there until the scrap time");
    assert!(gs.carrier.decommissioned(after));
    let left: Vec<CarrierKind> = gs.carriers(after).into_iter().map(|(k, _)| k).collect();
    assert_eq!(left, [CarrierKind::Squadron], "once scrapped, only the squadron carrier is left");
    assert_eq!(gs.squadron_carrier.name, "VANGUARD HOME", "a known personal carrier doesn't hide the squadron's");
    assert!(gs.feed.iter().any(|f| f.event == "CarrierDecommission"));
}

/// A decommission called off: the carrier stays.
#[test]
fn cancelled_decommission() {
    use nexus_journal::Timestamp;
    let mut m = Model::default();
    feed(&mut m, LOGIN);
    feed(
        &mut m,
        &[
            OWN_STATS,
            r#"{ "timestamp":"2026-10-01T10:20:00Z", "event":"CarrierDecommission", "CarrierType":"FleetCarrier", "CarrierID":3716323840, "ScrapRefund":4850000000, "ScrapTime":1791454800 }"#,
            r#"{ "timestamp":"2026-10-02T09:00:00Z", "event":"CarrierCancelDecommission", "CarrierType":"FleetCarrier", "CarrierID":3716323840 }"#,
        ],
    );
    let c = &m.active().unwrap().carrier;
    assert!(!c.pending_decommission && c.decommission.is_none());
    assert!(c.active(Timestamp::parse("2026-10-09T00:00:00Z").unwrap()));
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
