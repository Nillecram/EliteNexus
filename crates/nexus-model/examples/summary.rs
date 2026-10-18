//! Folds every journal into the model and prints what Elite Nexus knows about the commander.
//!
//!     cargo run --release -p nexus-model --example summary [-- <journal dir>]

use nexus_data::names::{compact_credits, group_digits};
use nexus_data::ranks::RankKind;
use nexus_journal::{list_journal_files, load_history, Companion, CompanionKind, EventCtx};
use nexus_model::{Alerts, Model};
use std::time::Instant;

fn main() {
    let dir = std::env::args().nth(1).map(Into::into).or_else(nexus_journal::default_journal_dir).expect("journal dir");
    let files = list_journal_files(&dir).expect("list");
    let t = Instant::now();
    let mut model = Model::default();
    let mut alerts = Alerts::disabled();
    load_history(&files, 32, |_f, fe| {
        for e in &fe.events {
            let ctx = EventCtx { ts: e.ts, name: fe.name(e), line: fe.line(e), live: false };
            model.apply(&ctx, &e.event, &mut alerts);
        }
    });
    for kind in CompanionKind::ALL {
        if let Ok(text) = std::fs::read_to_string(dir.join(kind.file_name()))
            && let Some(c) = Companion::parse(kind, &text) {
                model.apply_companion(&c, false, &mut alerts);
            }
    }
    let load_ms = t.elapsed().as_secs_f64() * 1e3;

    println!("== {} events from {} files in {:.1} ms; {} commander(s)", model.events_applied, files.len(), load_ms, model.commanders.len());
    let Some(gs) = model.active() else {
        println!("no commander found");
        return;
    };
    let c = &gs.cmdr;
    println!("CMDR {} ({}), {} Cr, mode {}, game {} build {}, odyssey={}", c.name, c.fid, group_digits(c.credits), c.game_mode, c.game_version, c.game_build.trim(), c.odyssey);
    for k in RankKind::ALL {
        println!("  {:<13} {:<20} {:>3}%", k.label(), c.rank_name(k), c.progress.get(k));
    }
    println!("  engineers unlocked: {}", c.unlocked_engineers().map(|e| format!("{} (G{})", e.name, e.rank.unwrap_or(0))).collect::<Vec<_>>().join(", "));
    if let Some(sq) = &c.squadron {
        println!("  squadron: {}", sq.name);
    }
    if !c.squadron_applications.is_empty() {
        println!("  squadron applications: {}", c.squadron_applications.join(", "));
    }
    let l = &gs.location;
    println!("LOCATION {} ({}) pos {:?} — {}; body {:?}", l.system, l.system_address, l.star_pos, l.situation(), l.body);
    if let Some(st) = &l.station {
        println!("  station {} [{}] market {} services {}", st.name, st.station_type, st.market_id, st.services.len());
    }
    println!("FLEET ({} ships, {} Cr total)", gs.fleet.ships.len(), compact_credits(gs.fleet.total_value()));
    for s in gs.fleet.ships.values() {
        let cur = if Some(s.id) == gs.fleet.current_ship_id { "*" } else { " " };
        println!(
            " {cur}#{:<3} {:<34} jump {:>5.1} ly  value {:>8}  engineered {:>2}  @ {}",
            s.id,
            s.display_name(),
            s.max_jump_range,
            compact_credits(s.total_value()),
            s.engineered_modules(),
            match (&s.location.system, s.location.in_transit) {
                (Some(sys), true) => format!("in transit → {} ({})", s.location.station.clone().unwrap_or_default(), sys),
                (Some(sys), false) => sys.clone(),
                (None, _) => if cur == "*" { "(current)".into() } else { "?".into() },
            }
        );
    }
    let m = &gs.inventory.materials;
    println!("MATERIALS raw {} / manufactured {} / encoded {}", m.raw.len(), m.manufactured.len(), m.encoded.len());
    println!("CARGO {} t: {}", gs.inventory.cargo.total(), gs.inventory.cargo.items.iter().map(|i| format!("{} {}", i.count, i.name)).collect::<Vec<_>>().join(", "));
    println!("SHIP LOCKER {} items", gs.inventory.ship_locker.total());
    println!("MISSIONS active {}: {}", gs.missions.active.len(), gs.missions.active.values().map(|m| m.title.clone()).collect::<Vec<_>>().join(" | "));
    for s in gs.missions.massacre_stacks() {
        println!("  massacre vs {}: {} missions, {}/{} kills left, {} Cr", s.target_faction, s.missions, s.kills_remaining, s.kills_total, compact_credits(s.total_reward));
    }
    let ex = &gs.exploration;
    let unsold: Vec<_> = ex.unsold().collect();
    println!(
        "EXPLORATION {} systems known, {} unsold ({} Cr est.), {} skipped by the game in sales, sold total {} Cr, codex entries {}",
        ex.systems.len(),
        unsold.len(),
        compact_credits(ex.unsold_value()),
        ex.skipped().count(),
        compact_credits(ex.sold_total),
        ex.codex.len()
    );
    if let Some(cs) = ex.current_system() {
        println!("  current system {}: {} bodies scanned of {:?}, value {} Cr, bio signals {}", cs.name, cs.scanned_bodies(), cs.fss_body_count, compact_credits(cs.value()), cs.bio_signal_total());
    }
    let bio = &gs.exobio;
    println!("EXOBIO analysed {}, unsold {} (~{} Cr), sold {} Cr", bio.analysed_total, bio.unsold.len(), compact_credits(bio.unsold_value()), compact_credits(bio.sold_total));
    for r in bio.unsold.iter().take(8) {
        println!("  {} on {} — {} Cr{}", r.species, r.body_name, group_digits(r.value), if r.bonus_expected { " (+bonus)" } else { "" });
    }
    for (label, cr) in [("CARRIER", &gs.carrier), ("SQUADRON CARRIER", &gs.squadron_carrier)] {
        if cr.owned() {
            let scrap = cr.decommission.map(|d| format!(", decommission: scrapped {}", d.scrap_time.to_iso())).unwrap_or_default();
            println!("{label} {} {} id {:?} fuel {} t, at {:?}, balance {} Cr, crew roles {}{scrap}", cr.callsign, cr.name, cr.id, cr.fuel, cr.system, compact_credits(cr.finance.carrier_balance), cr.crew.len());
        }
    }
    for d in gs.colonisation.depots.values() {
        println!("COLONISATION depot {} in {}: {:.0}% ({} t remaining)", d.station.clone().unwrap_or_default(), d.system, d.progress * 100.0, d.remaining_tons());
    }
    let h = &gs.history;
    println!("HISTORY jumps {}, {:.0} ly, systems visited {}, docks {}, deaths {}, ledger entries {}", h.jumps.len(), h.total_distance_ly, h.visited.len(), h.docks.len(), h.deaths.len(), h.ledger.len());
    for (cat, inc, exp) in h.totals_since(nexus_journal::Timestamp::ZERO) {
        println!("  {:<14} +{:>14} {:>15}", cat.label(), group_digits(inc), group_digits(exp));
    }
    println!("SESSION since {}: {} jumps, {:.1} ly, {} scans, income {} Cr", gs.session.start.to_iso(), gs.session.jumps, gs.session.distance_ly, gs.session.bodies_scanned, compact_credits(gs.session.income));
    println!("FEED (last 12):");
    for f in gs.feed.iter().rev().take(12) {
        println!("  {} {:<26} {}", f.ts.to_local_string("%m-%d %H:%M"), f.event, f.text);
    }
}
