//! Headless live monitor: loads history, then tails the journal and prints every live event,
//! alert and state change — exactly what the app's core thread does, without a window.
//!
//!     cargo run --release -p nexus-model --example live -- [minutes=30] [journal dir]

use nexus_journal::{list_journal_files, load_history, EventCtx, WatchMsg, Watcher, WatcherConfig};
use nexus_model::{Alerts, Model};
use std::io::Write;
use std::time::{Duration, Instant};

fn main() {
    let minutes: u64 = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(30);
    let dir = std::env::args().nth(2).map(Into::into).or_else(nexus_journal::default_journal_dir).expect("journal dir");
    let files = list_journal_files(&dir).expect("list");
    let mut model = Model::default();
    let mut quiet = Alerts::disabled();
    let mut resume = None;
    load_history(&files, 32, |f, fe| {
        for e in &fe.events {
            let ctx = EventCtx { ts: e.ts, name: fe.name(e), line: fe.line(e), live: false };
            model.apply(&ctx, &e.event, &mut quiet);
        }
        resume = Some((f.path.clone(), fe.consumed));
    });
    let out = std::io::stdout();
    let mut out = out.lock();
    let _ = writeln!(out, "history loaded: {} events; tailing {:?} for {minutes} min", model.events_applied, resume.as_ref().map(|r| &r.0));
    let (tx, rx) = crossbeam_channel::unbounded();
    let cfg = WatcherConfig { dir, poll: Duration::from_millis(100), resume, emit_initial_companions: false };
    let _w = Watcher::spawn(cfg, tx).expect("watcher");
    let end = Instant::now() + Duration::from_secs(minutes * 60);
    let mut unknown = std::collections::BTreeMap::<String, u32>::new();
    while Instant::now() < end {
        let Ok(msg) = rx.recv_timeout(Duration::from_millis(500)) else { continue };
        let mut alerts = Alerts::live();
        match msg {
            WatchMsg::Event(le) => {
                let lat_ms = (chrono::Utc::now().timestamp_millis() - le.raw.ts.0 * 1000).max(0);
                let ctx = EventCtx { ts: le.raw.ts, name: &le.raw.name, line: &le.raw.line, live: true };
                model.apply(&ctx, &le.event, &mut alerts);
                let typed = !matches!(le.event, nexus_journal::Event::Other);
                if !typed && nexus_journal::Event::is_modelled(&le.raw.name) {
                    let _ = writeln!(out, "!! PARSE FAILURE {}: {}", le.raw.name, &le.raw.line[..le.raw.line.len().min(300)]);
                }
                if !nexus_journal::Event::is_modelled(&le.raw.name) {
                    *unknown.entry(le.raw.name.to_string()).or_default() += 1;
                }
                let gs = model.active();
                let loc = gs.map(|g| format!("{} | {}", g.location.system, g.location.situation())).unwrap_or_default();
                let _ = writeln!(out, "{} +{:>5}ms {:<28} {}", le.raw.ts.to_local_string("%H:%M:%S"), lat_ms, le.raw.name, loc);
            }
            WatchMsg::Companion(c) => {
                model.apply_companion(&c, true, &mut alerts);
                if !matches!(c, nexus_journal::Companion::Status(_)) {
                    let _ = writeln!(out, "         companion {:?}", c.kind());
                }
            }
            WatchMsg::FileOpened(f) => {
                let _ = writeln!(out, "-- new journal file {}", f.file_name);
            }
            WatchMsg::Error(e) => {
                let _ = writeln!(out, "!! watcher error: {e}");
            }
        }
        for a in alerts.items {
            let _ = writeln!(out, "   >>> ALERT [{}/{:?}] {} — {}{}", a.kind.label(), a.priority, a.title, a.detail, a.speech.map(|s| format!("  (speak: {s})")).unwrap_or_default());
        }
        let _ = out.flush();
    }
    let _ = writeln!(out, "done. unmodelled live events: {unknown:?}");
    if let Some(gs) = model.active() {
        let _ = writeln!(
            out,
            "final: {} credits, in {} ({}), session jumps {} scans {} organics {}",
            gs.cmdr.credits,
            gs.location.system,
            gs.location.situation(),
            gs.session.jumps,
            gs.session.bodies_scanned,
            gs.session.organics_analysed
        );
    }
}
