//! Simulates a live game session for testing: copies journal lines into a target folder with
//! (scaled) original timing, and rewrites Status.json-style companion files when asked.
//!
//!     cargo run -p nexus-journal --example replay -- <source journal file> <target dir> [speed=60] [start_line=0] [max_lines]
//!
//! The target file gets a fresh name (current time) so the watcher treats it as a new session.

use std::io::{BufRead, Write};
use std::time::Duration;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("usage: replay <source journal> <target dir> [speed] [start_line] [max_lines]");
        std::process::exit(2);
    }
    let src = std::path::PathBuf::from(&args[1]);
    let dir = std::path::PathBuf::from(&args[2]);
    let speed: f64 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(60.0);
    let start: usize = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(0);
    let max: usize = args.get(5).and_then(|s| s.parse().ok()).unwrap_or(usize::MAX);
    std::fs::create_dir_all(&dir).expect("target dir");
    let name = format!("Journal.{}.01.log", chrono::Utc::now().format("%Y-%m-%dT%H%M%S"));
    let target = dir.join(&name);
    let mut out = std::fs::OpenOptions::new().create(true).append(true).open(&target).expect("open target");
    let input = std::io::BufReader::new(std::fs::File::open(&src).expect("open source"));
    let mut prev_ts: Option<i64> = None;
    let mut written = 0usize;
    for (i, line) in input.lines().enumerate() {
        let Ok(line) = line else { continue };
        if i < start && !line.contains("\"event\":\"Fileheader\"") && !line.contains("\"event\":\"Commander\"") && !line.contains("\"event\":\"LoadGame\"") {
            continue;
        }
        if written >= max {
            break;
        }
        let ts = nexus_journal::raw::scan_line(&line).map(|(t, _)| t.0);
        if let (Some(p), Some(t)) = (prev_ts, ts) {
            let wait = ((t - p).max(0) as f64 / speed).min(3.0);
            std::thread::sleep(Duration::from_secs_f64(wait));
        }
        if ts.is_some() {
            prev_ts = ts;
        }
        writeln!(out, "{line}").expect("write");
        out.flush().ok();
        written += 1;
        if let Some((_, ev)) = nexus_journal::raw::scan_line(&line) {
            println!("{written:>5} {ev}");
        }
    }
    println!("replayed {written} lines into {}", target.display());
}
