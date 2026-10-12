//! Imports a Spansh dump (file or stdin) into a galaxy database.
//!
//! ```text
//! cargo run -p nexus-galaxy --release --example import -- <db path> <dump.json.gz | -> [max MB]
//! curl -s https://downloads.spansh.co.uk/systems_1day.json.gz | target/release/examples/import galaxy.db -
//! ```
//!
//! `max MB` stops after that many input (compressed) megabytes, for smoke tests on large dumps.

use nexus_galaxy::*;
use std::io::Read;

fn main() {
    let _ = tracing_subscriber::fmt().with_env_filter("info").try_init();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (Some(db_path), Some(src)) = (args.first(), args.get(1)) else {
        eprintln!("usage: import <db path> <dump.json.gz | -> [max MB]");
        std::process::exit(2);
    };
    let max_bytes = args
        .get(2)
        .and_then(|s| s.parse::<f64>().ok())
        .map(|mb| (mb * 1e6) as u64);
    let db = GalaxyDb::open(db_path).expect("open database");
    let reader: Box<dyn Read> = if src == "-" {
        Box::new(std::io::stdin().lock())
    } else {
        Box::new(std::fs::File::open(src).expect("open dump"))
    };
    let summary = db
        .import_spansh_dump(reader, |p| {
            println!(
                "{:>8.1} MB read  {:>8} records  {:>8} systems  {:>6} stations  {:>6} markets  {:>9} market rows  {:>6} rings  \
                 {:>6} faction rows  {:>3} bad lines  {:>6.1} s",
                p.bytes_read as f64 / 1e6,
                p.records,
                p.systems,
                p.stations,
                p.markets,
                p.market_rows,
                p.ring_signals,
                p.faction_presences,
                p.parse_errors,
                p.elapsed.as_secs_f64()
            );
            max_bytes.is_none_or(|m| p.bytes_read < m)
        })
        .expect("import");
    let p = &summary.progress;
    println!(
        "\ndone: {} records in {:.1} s ({:.0} records/s, {:.1} MB/s input), cancelled={}, stale markets={}, \
         write errors={}, stream error={:?}",
        p.records,
        p.elapsed.as_secs_f64(),
        p.records as f64 / p.elapsed.as_secs_f64().max(1e-9),
        p.bytes_read as f64 / 1e6 / p.elapsed.as_secs_f64().max(1e-9),
        summary.cancelled,
        summary.stale_markets,
        summary.write_errors,
        summary.stream_error
    );
    let s = db.stats().expect("stats");
    println!(
        "db: {} systems, {} stations, {} markets, {} market rows, {} faction rows, {} ring signals, {:.1} MB",
        s.systems,
        s.stations,
        s.stations_with_market,
        s.market_rows,
        s.faction_rows,
        s.ring_signals,
        s.db_bytes as f64 / 1e6
    );
    for name in ["Sol", "Colonia", "Shinrarta Dezhra"] {
        if let Ok(Some(sys)) = db.system_by_name(name) {
            println!("  {name}: {:?}", sys.pos);
        }
    }
}
