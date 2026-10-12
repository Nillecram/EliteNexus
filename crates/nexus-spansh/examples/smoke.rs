//! Live smoke test against spansh.co.uk: sequential requests, ≈ 15 including job polls (the
//! galaxy plotter alone runs for its whole 60 s `max_time`, polled at ¼-checkpoints). Takes ~75 s.
//!
//! ```text
//! cargo run -p nexus-spansh --example smoke
//! cargo run -p nexus-spansh --example smoke -- --only neutron,dump --record target/spansh-responses
//! ```
//!
//! Steps: neutron, carrier, galaxy, riches, exobio, tritium, ltd, dump, system.

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::Context as _;
use nexus_spansh::{
    CarrierRouteRequest, ExobiologyRouteRequest, GalaxyRouteRequest, HotspotQuery, JobOptions,
    MarketQuery, NeutronRouteRequest, RichesRouteRequest, RouteWaypoints, ShipProfile, Spansh,
    parse_timestamp,
};

const STEPS: [&str; 9] = [
    "neutron", "carrier", "galaxy", "riches", "exobio", "tritium", "ltd", "dump", "system",
];

/// An engineered Krait Phantom: 5A SCO drive (Mass Manager, optimal mass 2117.5 t) and a size-3
/// Guardian FSD booster, as the journal writes it.
const LOADOUT: &str = r#"{ "timestamp":"2026-10-01T12:00:00Z", "event":"Loadout", "Ship":"krait_light", "ShipID":7,
  "ShipName":"Wayfarer", "ShipIdent":"KP-01", "HullValue":40000000, "ModulesValue":60000000, "HullHealth":1.0,
  "UnladenMass":398.5, "CargoCapacity":16, "MaxJumpRange":68.2, "FuelCapacity":{ "Main":32.0, "Reserve":0.63 },
  "Rebuy":5000000, "Modules":[
    { "Slot":"FrameShiftDrive", "Item":"int_hyperdrive_overcharge_size5_class5", "On":true, "Priority":0, "Health":1.0,
      "Engineering":{ "Engineer":"Felicity Farseer", "EngineerID":300100, "BlueprintID":128673694,
        "BlueprintName":"FSD_LongRange", "Level":5, "Quality":1.0, "ExperimentalEffect":"special_fsd_heavy",
        "Modifiers":[ { "Label":"Mass", "Value":26.0, "OriginalValue":20.0, "LessIsGood":1 },
                      { "Label":"FSDOptimalMass", "Value":2117.5, "OriginalValue":1175.0, "LessIsGood":0 } ] } },
    { "Slot":"Slot03_Size3", "Item":"int_guardianfsdbooster_size3", "On":true, "Priority":0, "Health":1.0 },
    { "Slot":"FuelScoop", "Item":"int_fuelscoop_size5_class5", "On":true, "Priority":0, "Health":1.0 } ] }"#;

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let mut only: Vec<String> = Vec::new();
    let mut record: Option<String> = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--only" => {
                only = args
                    .next()
                    .context("--only needs a list")?
                    .split(',')
                    .map(str::to_owned)
                    .collect()
            }
            "--record" => record = Some(args.next().context("--record needs a directory")?),
            "-h" | "--help" => {
                println!("usage: smoke [--only {}] [--record DIR]", STEPS.join(","));
                return Ok(());
            }
            other => anyhow::bail!("unknown argument {other:?}"),
        }
    }

    let jobs = JobOptions::default()
        .poll_interval(Duration::from_millis(1500), Duration::from_secs(8))
        .on_progress(|p| {
            let id: String = p.job.chars().take(8).collect();
            println!(
                "   … job {id} {:?} (poll {}, {:.1}s)",
                p.state,
                p.polls,
                p.elapsed.as_secs_f64()
            );
        });
    let mut builder = Spansh::builder().job_options(jobs);
    if let Some(dir) = &record {
        builder = builder.record_responses(dir);
    }
    let spansh = builder.build()?;

    let total = Instant::now();
    let mut failed = Vec::new();
    for step in STEPS
        .into_iter()
        .filter(|s| only.is_empty() || only.iter().any(|o| o == s))
    {
        println!("\n== {step}");
        let started = Instant::now();
        let result = match step {
            "neutron" => neutron(&spansh).await,
            "carrier" => carrier(&spansh).await,
            "galaxy" => galaxy(&spansh).await,
            "riches" => riches(&spansh).await,
            "exobio" => exobio(&spansh).await,
            "tritium" => tritium(&spansh).await,
            "ltd" => ltd(&spansh).await,
            "dump" => dump(&spansh).await,
            _ => system(&spansh).await,
        };
        let secs = started.elapsed().as_secs_f64();
        match result {
            Ok(()) => println!("   [{step}: ok in {secs:.2}s]"),
            Err(e) => {
                println!("   [{step}: FAILED after {secs:.2}s] {e:#}");
                failed.push(step);
            }
        }
    }
    println!(
        "\ntotal {:.1}s, failed: {failed:?}",
        total.elapsed().as_secs_f64()
    );
    if failed.is_empty() {
        Ok(())
    } else {
        anyhow::bail!("{} step(s) failed", failed.len())
    }
}

fn short(job: &str) -> String {
    job.chars().take(8).collect()
}

async fn neutron(spansh: &Spansh) -> anyhow::Result<()> {
    let p = spansh
        .neutron_route(&NeutronRouteRequest::new("Sol", "Colonia", 60.0))
        .await?;
    let r = &p.route;
    println!(
        "   {} → {}: {:.0} ly, {} waypoints, ~{} jumps, {} neutron stars (job {} {:.1}s, {} polls)",
        r.source_system,
        r.destination_system,
        r.distance,
        r.system_jumps.len(),
        r.estimated_jumps(),
        r.neutron_count(),
        short(&p.job),
        p.elapsed.as_secs_f64(),
        p.polls
    );
    let n = r.system_jumps.len();
    for (i, w) in r
        .system_jumps
        .iter()
        .enumerate()
        .filter(|(i, _)| *i < 4 || *i + 2 > n)
    {
        if i == 4 {
            println!("     …");
        }
        println!(
            "     {:<34} {:>3} jumps {:>7.1} ly, {:>8.1} ly left{}",
            w.system,
            w.jumps,
            w.distance_jumped,
            w.distance_left,
            if w.neutron_star { "  [neutron]" } else { "" }
        );
    }
    println!("   {}", p.web_url);
    Ok(())
}

async fn carrier(spansh: &Spansh) -> anyhow::Result<()> {
    let req = CarrierRouteRequest::new("Sol", ["Achenar", "Shinrarta Dezhra"]).capacity_used(1_000);
    let p = spansh.carrier_route(&req).await?;
    let r = &p.route;
    println!(
        "   {} carrier jumps, {:.1} ly, {} t tritium, ~{} min (job {} {:.1}s, {} polls)",
        r.jump_count(),
        r.distance(),
        r.tritium_used(),
        r.estimated_duration().as_secs() / 60,
        short(&p.job),
        p.elapsed.as_secs_f64(),
        p.polls
    );
    for j in &r.jumps {
        println!(
            "     {:<20} {:>6.1} ly  tank {:>4} t  used {:>3} t  market {:>5} t{}{}{}",
            j.name,
            j.distance,
            j.fuel_in_tank,
            j.fuel_used,
            j.tritium_in_market,
            if j.must_restock {
                format!("  restock {} t", j.restock_amount)
            } else {
                String::new()
            },
            if j.has_icy_ring { "  icy ring" } else { "" },
            if j.is_desired_destination {
                "  [destination]"
            } else {
                ""
            }
        );
    }
    Ok(())
}

/// Road to Riches with the app's defaults (25 ly radius, 25 systems, ≥ 1 M cr, mapping value).
async fn riches(spansh: &Spansh) -> anyhow::Result<()> {
    let req = RichesRouteRequest::road_to_riches("Brestla", 60.0)
        .radius(25)
        .max_results(25)
        .min_value(1_000_000)
        .use_mapping_value(true);
    let p = spansh.riches_route(&req).await?;
    scan_route("riches", &p.route, &p.job, p.elapsed, p.polls, &p.web_url);
    Ok(())
}

/// The exobiology planner ("Expressway to Exomastery") with the app's defaults.
async fn exobio(spansh: &Spansh) -> anyhow::Result<()> {
    let req = ExobiologyRouteRequest::new("Brestla", 60.0)
        .radius(25)
        .max_results(25)
        .min_value(1_000_000);
    let p = spansh.exobiology_route(&req).await?;
    scan_route("exobio", &p.route, &p.job, p.elapsed, p.polls, &p.web_url);
    Ok(())
}

fn scan_route(
    what: &str,
    r: &nexus_spansh::ScanRoute,
    job: &str,
    elapsed: Duration,
    polls: u32,
    url: &str,
) {
    println!(
        "   {what}: {} systems, {} bodies, {} cr, ~{} jumps (job {} {:.1}s, {polls} polls)",
        r.systems.len(),
        r.body_count(),
        r.total_value(),
        r.total_jumps(),
        short(job),
        elapsed.as_secs_f64(),
    );
    for s in r.systems.iter().take(6) {
        println!("     {:<34} {:>2} jumps {:>12} cr", s.name, s.jumps, s.value());
        for b in s.bodies.iter().take(3) {
            println!(
                "       {:<30} {:<26} scan {:>9} map {:>9} landmarks {:>10} ({} species)",
                b.name,
                b.subtype.as_deref().unwrap_or("?"),
                b.estimated_scan_value,
                b.estimated_mapping_value,
                b.landmark_value,
                b.species().len()
            );
        }
    }
    for w in r.waypoints().iter().take(3) {
        println!("     waypoint {} value {:?} note {:?}", w.system, w.value, w.note);
    }
    println!("   {url}");
}

async fn galaxy(spansh: &Spansh) -> anyhow::Result<()> {
    let ship = ShipProfile::from_loadout_json(LOADOUT)?;
    println!(
        "   ship {}: FSD {}{}{} opt {:.1} t, max fuel {} t, ×{:.3}^{}, base {:.2} t, tank {} t, booster {:?} → max {:.1} ly, laden {:.1} ly",
        ship.ship,
        ship.fsd.size,
        ship.fsd.rating,
        if ship.fsd.sco { " SCO" } else { "" },
        ship.fsd.optimal_mass,
        ship.fsd.max_fuel_per_jump,
        ship.fsd.fuel_multiplier,
        ship.fsd.fuel_power,
        ship.base_mass(),
        ship.tank_size,
        ship.range_boost,
        ship.max_range(),
        ship.laden_range(0.0)
    );
    let p = spansh
        .galaxy_route(&GalaxyRouteRequest::new("Sol", "Maia", &ship))
        .await?;
    let r = &p.route;
    println!(
        "   {} jumps, {:.1} ly, {:.2} t fuel, {} refuel stops, {} neutron boosts, {} injections{} (job {} {:.1}s, {} polls)",
        r.jump_count(),
        r.distance(),
        r.fuel_used(),
        r.refuel_stops(),
        r.neutron_boosts(),
        r.injections(),
        if r.incomplete { ", INCOMPLETE" } else { "" },
        short(&p.job),
        p.elapsed.as_secs_f64(),
        p.polls
    );
    for j in &r.jumps {
        println!(
            "     {:<30} {:>5.1} ly  fuel {:>5.2} t (used {:>4.2}){}{}{}{}",
            j.name,
            j.distance,
            j.fuel_in_tank,
            j.fuel_used,
            if j.is_scoopable { "  scoopable" } else { "" },
            if j.must_refuel { "  REFUEL" } else { "" },
            if j.has_neutron { "  neutron" } else { "" },
            if j.must_inject { "  inject" } else { "" }
        );
    }
    println!(
        "   waypoint view: {} stops, {} jumps",
        r.waypoints().len(),
        r.total_jumps()
    );
    Ok(())
}

fn hours_old(timestamp: Option<&str>) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    match timestamp.and_then(parse_timestamp) {
        Some(t) => format!("{:.1} h old", (now - t) as f64 / 3600.0),
        None => "age ?".to_owned(),
    }
}

async fn tritium(spansh: &Spansh) -> anyhow::Result<()> {
    let query = MarketQuery::buy("Tritium", "Sol")
        .min_quantity(1_000)
        .max_age(Duration::from_secs(7 * 86_400))
        .size(5);
    let resp = spansh.market_search(&query).await?;
    println!(
        "   {} stations sell ≥ 1000 t Tritium (market updated within 7 days); nearest:",
        resp.count
    );
    for s in &resp.results {
        let row = s.commodity("Tritium");
        println!(
            "     {:<24} {:<20} {:>6.2} ly {:>8.0} ls  supply {:>7}  buy {:>6} cr  {}  [{}]",
            s.name,
            s.system_name,
            s.distance.unwrap_or_default(),
            s.distance_to_arrival.unwrap_or_default(),
            row.map(|r| r.supply).unwrap_or_default(),
            row.map(|r| r.buy_price).unwrap_or_default(),
            hours_old(s.market_updated_at.as_deref()),
            s.station_type.as_deref().unwrap_or("?")
        );
    }
    Ok(())
}

async fn ltd(spansh: &Spansh) -> anyhow::Result<()> {
    let commodity = "Low Temperature Diamonds";
    let resp = spansh
        .hotspot_search(&HotspotQuery::new(commodity, 2, "Sol").size(5))
        .await?;
    println!(
        "   {} bodies with ≥ 2 LTD hotspots in one ring; nearest:",
        resp.count
    );
    for b in &resp.results {
        let ring = b.best_ring(commodity);
        println!(
            "     {:<28} {:>6.1} ly  {:<30} ×{}  {:<10} reserve {}",
            b.name,
            b.distance.unwrap_or_default(),
            ring.map(|r| r.name.as_str()).unwrap_or("?"),
            b.hotspots(commodity),
            ring.and_then(|r| r.ring_type.as_deref()).unwrap_or("?"),
            b.reserve_level.as_deref().unwrap_or("?")
        );
    }
    Ok(())
}

async fn dump(spansh: &Spansh) -> anyhow::Result<()> {
    let sol = spansh.dump(10_477_373_803).await?;
    let stations: Vec<_> = sol.all_stations().collect();
    let markets = stations.iter().filter(|s| s.market.is_some()).count();
    let carriers = stations.iter().filter(|s| s.is_fleet_carrier()).count();
    let ringed = sol.bodies.iter().filter(|b| !b.rings.is_empty()).count();
    println!(
        "   {} ({}): {} bodies ({} ringed), {} stations ({} with market, {} carriers), {} factions, controlled by {:?} ({:?})",
        sol.name,
        sol.id64,
        sol.bodies.len(),
        ringed,
        stations.len(),
        markets,
        carriers,
        sol.factions.len(),
        sol.controlling_power,
        sol.power_state
    );
    if let Some(earth) = sol.body("Earth") {
        println!(
            "     Earth: {:?}, {:.3} g, {:.0} K, {:.1} km, {} station(s), updated {:?}",
            earth.sub_type,
            earth.gravity.unwrap_or_default(),
            earth.surface_temperature.unwrap_or_default(),
            earth.radius.unwrap_or_default(),
            earth.stations.len(),
            earth.update_time
        );
    }
    Ok(())
}

async fn system(spansh: &Spansh) -> anyhow::Result<()> {
    let rec = spansh
        .system_by_name("Shinrarta Dezhra")
        .await?
        .context("Spansh does not know Shinrarta Dezhra")?;
    println!(
        "   {} (id64 {}) at ({}, {}, {}): pop {:?}, {} bodies, {} stations, permit {:?}, faction {:?}, power {:?} {:?}",
        rec.name,
        rec.id64,
        rec.x,
        rec.y,
        rec.z,
        rec.population,
        rec.bodies.len(),
        rec.stations.len(),
        rec.needs_permit,
        rec.controlling_minor_faction,
        rec.controlling_power,
        rec.power_state
    );
    Ok(())
}
