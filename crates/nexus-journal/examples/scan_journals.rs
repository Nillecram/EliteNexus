//! Loads every journal in the default (or given) folder and reports parse statistics.
//!
//!     cargo run --release -p nexus-journal --example scan_journals [-- <journal dir>]

use nexus_journal::{list_journal_files, load_history, Event};
use std::collections::BTreeMap;
use std::time::Instant;

fn main() {
    let dir = std::env::args().nth(1).map(Into::into).or_else(nexus_journal::default_journal_dir).expect("no journal dir");
    let files = list_journal_files(&dir).expect("cannot list journal dir");
    let bytes: u64 = files.iter().map(|f| f.size).sum();
    println!("{} files, {:.1} MB in {}", files.len(), bytes as f64 / 1e6, dir.display());

    let t = Instant::now();
    let mut total = 0usize;
    let mut typed = 0usize;
    let mut errors = 0u32;
    let mut failed: BTreeMap<String, (usize, String)> = BTreeMap::new();
    let mut by_name: BTreeMap<String, usize> = BTreeMap::new();
    load_history(&files, 32, |_f, fe| {
        errors += fe.parse_errors;
        for e in &fe.events {
            total += 1;
            let name = fe.name(e);
            *by_name.entry(name.to_string()).or_default() += 1;
            if Event::is_modelled(name) {
                if matches!(e.event, Event::Other) {
                    let entry = failed.entry(name.to_string()).or_insert((0, String::new()));
                    entry.0 += 1;
                    if entry.1.is_empty() {
                        let err = Event::parse(name, fe.line(e)).err().map(|e| e.to_string()).unwrap_or_default();
                        entry.1 = err;
                    }
                } else {
                    typed += 1;
                }
            }
        }
    });
    let el = t.elapsed();
    println!(
        "{total} events ({typed} typed) in {:.1} ms -> {:.0} MB/s, {:.0}k events/s; {errors} header/parse errors",
        el.as_secs_f64() * 1e3,
        bytes as f64 / 1e6 / el.as_secs_f64(),
        total as f64 / 1e3 / el.as_secs_f64()
    );
    if failed.is_empty() {
        println!("all modelled events parsed cleanly");
    } else {
        println!("modelled events that FAILED typed parsing:");
        for (k, (n, err)) in &failed {
            println!("  {k:<32} x{n:<5} {err}");
        }
    }
    let unmodelled: Vec<_> = by_name.iter().filter(|(k, _)| !Event::is_modelled(k)).collect();
    println!("unmodelled event names ({}):", unmodelled.len());
    for (k, n) in unmodelled {
        print!("{k}({n}) ");
    }
    println!();
}
