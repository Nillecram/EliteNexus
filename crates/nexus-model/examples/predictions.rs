//! Lists bio bodies from the journals with the model's predictions vs. what was actually sampled.
//!
//!     cargo run --release -p nexus-model --example predictions

use nexus_journal::{list_journal_files, load_history, EventCtx};
use nexus_model::{Alerts, Model};

fn main() {
    let dir = nexus_journal::default_journal_dir().expect("journal dir");
    let files = list_journal_files(&dir).expect("list");
    let mut model = Model::default();
    let mut quiet = Alerts::disabled();
    load_history(&files, 32, |_f, fe| {
        for e in &fe.events {
            let ctx = EventCtx { ts: e.ts, name: fe.name(e), line: fe.line(e), live: false };
            model.apply(&ctx, &e.event, &mut quiet);
        }
    });
    let gs = model.active().expect("cmdr");
    let mut hits = 0;
    let mut total = 0;
    for sys in gs.exploration.systems.values() {
        for b in sys.bodies.values().filter(|b| b.bio_signals > 0) {
            let likely = b.likely_species();
            println!(
                "{} — {} signals, genera {:?}, expected ≈{} Cr",
                b.name,
                b.bio_signals,
                b.genuses,
                nexus_data::names::compact_credits(b.predicted_value(true))
            );
            for p in likely.iter().take(6) {
                println!("    predicted {:<28} {:>5.1}%  {:>12} Cr  {}", p.species, p.likelihood * 100.0, nexus_data::names::group_digits(p.value), p.variant.clone().unwrap_or_default());
            }
            for o in &b.organics {
                total += 1;
                let top = likely.iter().find(|p| p.genus.eq_ignore_ascii_case(&o.genus)).map(|p| p.species.as_str());
                let ok = top.is_some_and(|t| t.eq_ignore_ascii_case(&o.species));
                if ok {
                    hits += 1;
                }
                println!("    actual    {:<28} {}", o.species, if ok { "✓ top prediction for its genus" } else { "✗" });
            }
        }
    }
    println!("\nsampled species predicted as #1 of their genus: {hits}/{total}");
}
