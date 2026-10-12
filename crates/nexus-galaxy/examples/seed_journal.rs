//! Seeds a database from the commander's own journals (what the app does at start-up) and runs a
//! few queries around the last known position.
//!
//!     cargo run --release -p nexus-galaxy --example seed_journal -- <db path> [journal dir]

use nexus_galaxy::*;
use std::time::Instant;

const RELEVANT: [&str; 7] = ["FSDJump", "Location", "CarrierJump", "Docked", "SAASignalsFound", "ApproachSettlement", "Market"];

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let db_path = args.next().expect("usage: seed_journal <db path> [journal dir]");
    let dir = args.next().map(Into::into).or_else(nexus_journal::default_journal_dir).expect("journal dir");
    let files = nexus_journal::list_journal_files(&dir).expect("list journals");
    let _ = std::fs::remove_file(&db_path);
    let db = GalaxyDb::open(&db_path)?;

    let t = Instant::now();
    let (mut lines, mut queued, mut last_pos, mut last_system) = (0usize, 0usize, None, String::new());
    for f in &files {
        let Ok(text) = std::fs::read_to_string(&f.path) else { continue };
        for line in text.lines() {
            lines += 1;
            let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else { continue };
            let Some(ev) = v.get("event").and_then(|e| e.as_str()) else { continue };
            if !RELEVANT.contains(&ev) {
                continue;
            }
            if let Some(p) = v.get("StarPos").and_then(|p| p.as_array()).filter(|p| p.len() == 3) {
                last_pos = Some([p[0].as_f64().unwrap_or(0.0), p[1].as_f64().unwrap_or(0.0), p[2].as_f64().unwrap_or(0.0)]);
                last_system = v.get("StarSystem").and_then(|s| s.as_str()).unwrap_or_default().to_string();
            }
            queued += db.ingest_journal(ev, &v);
        }
    }
    if let Ok(text) = std::fs::read_to_string(dir.join("Market.json"))
        && let Ok(v) = serde_json::from_str::<serde_json::Value>(text.trim_start_matches('\u{feff}'))
    {
        queued += db.ingest_journal("Market", &v);
    }
    db.flush()?;
    println!("{lines} journal lines -> {queued} items in {:.0} ms", t.elapsed().as_secs_f64() * 1e3);
    let s = db.stats()?;
    println!(
        "{} systems, {} stations ({} with market), {} market rows, {} ring signals, {} faction rows",
        s.systems, s.stations, s.stations_with_market, s.market_rows, s.ring_signals, s.faction_rows
    );

    let Some(origin) = last_pos else { return Ok(()) };
    println!("\nlast position: {last_system} {origin:?}");
    let q = Instant::now();
    let near = db.stations_near(&StationQuery { origin, limit: 8, include_carriers: true, ..Default::default() })?;
    println!("nearest known stations ({:.2} ms):", q.elapsed().as_secs_f64() * 1e3);
    for h in &near {
        println!(
            "  {:>7.1} ly  {:<34} {:<26} pad {:?}  trader {:?}  broker {:?}",
            h.distance_ly, h.station.name, h.station.system, h.station.max_pad, h.station.material_trader, h.station.tech_broker
        );
    }
    for signal in ["Platinum", "Painite", "Low Temperature Diamonds", "Tritium", "Monazite"] {
        let q = Instant::now();
        let hits = db.hotspots_near(&HotspotQuery { origin, signal: signal.into(), limit: 3, ..Default::default() })?;
        println!("{signal} hotspots ({:.2} ms): {}", q.elapsed().as_secs_f64() * 1e3, hits.len());
        for h in hits {
            println!("  {:>7.1} ly  {} ×{}  ({})", h.distance_ly, h.body_name, h.count, h.system);
        }
    }
    for st in db.recent_markets(3)? {
        if let Some(m) = db.station_market(st.market_id)? {
            let best = m.items.iter().filter(|i| i.sell > 0).max_by_key(|i| i.sell);
            println!(
                "market {} / {}: {} items, best sell {}",
                m.station.name,
                m.station.system,
                m.items.len(),
                best.map(|b| format!("{} {} Cr", b.name, b.sell)).unwrap_or_default()
            );
        }
    }
    Ok(())
}
