//! Manual smoke test against real bindings.
//!
//! ```text
//! cargo run -p nexus-binds --example inspect [-- [BINDINGS_DIR] [--game CONTROL_SCHEMES_DIR]
//!                                               [--out FILE.html] [--builtin] [--file X.binds]]
//! ```
//!
//! Lists the presets in the bindings folder (default `%LOCALAPPDATA%\…\Options\Bindings`),
//! analyses the active Ship preset and the effective per-category merge, prints a summary and
//! writes reference cards (`refcard.html`, `refcard-effective.html`) next to `--out` (default:
//! `$CARGO_TARGET_DIR`, else `target-agents/binds`). `--builtin` additionally analyses every
//! preset shipped with the game, which is how the conflict rules were calibrated. `--file`
//! analyses and renders just that one file.

use std::error::Error;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use nexus_binds::{
    Bindings, Importance, PresetCategory, Report, analyze, default_bindings_dir, find_presets,
    render_html,
};

fn main() -> Result<(), Box<dyn Error>> {
    let mut dir: Option<PathBuf> = None;
    let mut game: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut file: Option<PathBuf> = None;
    let mut builtin = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--game" => game = args.next().map(PathBuf::from),
            "--out" => out = args.next().map(PathBuf::from),
            "--file" => file = args.next().map(PathBuf::from),
            "--builtin" => builtin = true,
            _ => dir = Some(PathBuf::from(arg)),
        }
    }
    let out = out.unwrap_or_else(|| default_out_dir().join("refcard.html"));
    if let Some(file) = file {
        let bindings = Bindings::load(&file)?;
        let report = analyze(&bindings);
        println!("=== {} ===", file.display());
        print_summary(&bindings, &report);
        write_card(&out, &render_html(&bindings, &report))?;
        println!("\nWrote {}", out.display());
        return Ok(());
    }
    let dir = dir
        .or_else(default_bindings_dir)
        .ok_or("no bindings folder found")?;
    let game = game.or_else(probe_game_dir);

    println!("Bindings folder: {}", dir.display());
    let presets = find_presets(&dir)?;
    for p in &presets.presets {
        let version = p
            .version
            .map(|v| v.to_string())
            .unwrap_or_else(|| "-".into());
        let active: Vec<&str> = p.active_for.iter().map(|c| c.label()).collect();
        println!(
            "  {:<24} v{:<4} {}{}",
            p.name,
            version,
            if active.is_empty() {
                "inactive".to_owned()
            } else {
                format!("ACTIVE: {}", active.join(", "))
            },
            if p.older_versions.is_empty() {
                String::new()
            } else {
                format!("  (+{} older)", p.older_versions.len())
            }
        );
    }
    if let Some(start) = &presets.start_preset {
        let per: Vec<String> = PresetCategory::ALL
            .iter()
            .map(|&c| format!("{}={}", c.label(), start.get(c)))
            .collect();
        println!("  StartPreset: {}", per.join(", "));
    }

    let primary = presets.primary().ok_or("no .binds files found")?;
    let text = std::fs::read_to_string(&primary.path)?;
    let parse_time = time_parse(&text)?;
    let bindings = Bindings::parse(&text)?;
    let started = Instant::now();
    let report = analyze(&bindings);
    let analyze_time = started.elapsed();

    println!(
        "\n=== Active ship preset: {} ({} bytes) ===",
        primary.path.display(),
        text.len()
    );
    println!(
        "parse {:.3} ms (mean of 200), analyse {:.3} ms",
        ms(parse_time),
        ms(analyze_time)
    );
    print_summary(&bindings, &report);

    let started = Instant::now();
    let html = render_html(&bindings, &report);
    let render_time = started.elapsed();
    write_card(&out, &html)?;
    println!(
        "\nWrote {} ({} KB, rendered in {:.3} ms)",
        out.display(),
        html.len() / 1024,
        ms(render_time)
    );

    let builtin_dirs: Vec<&Path> = game.iter().map(PathBuf::as_path).collect();
    match presets.load_effective(&builtin_dirs) {
        Ok(effective) => {
            let report = analyze(&effective);
            println!(
                "\n=== Effective bindings (per-category presets): {} ===",
                effective.preset_name
            );
            print_summary(&effective, &report);
            let path = out.with_file_name("refcard-effective.html");
            write_card(&path, &render_html(&effective, &report))?;
            println!("\nWrote {}", path.display());
        }
        Err(err) => println!("\nEffective bindings unavailable: {err}"),
    }

    if builtin {
        let Some(game) = &game else {
            return Err("--builtin needs the game's ControlSchemes folder (--game)".into());
        };
        println!("\n=== Built-in presets in {} ===", game.display());
        let mut files: Vec<PathBuf> = std::fs::read_dir(game)?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| {
                p.extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("binds"))
            })
            .collect();
        files.sort();
        for path in files {
            let b = Bindings::load(&path)?;
            let r = analyze(&b);
            println!(
                "{:<34} bound {:>3}/{:<3} conflicts {:>2}  essentials missing {:>2}  modifier notes {}",
                b.preset_name,
                r.totals.bound_actions,
                r.totals.actions,
                r.conflicts.len(),
                r.missing_essentials().count(),
                r.modifier_notes.len()
            );
            for c in &r.conflicts {
                let names: Vec<&str> = c.actions.iter().map(|a| a.action.as_str()).collect();
                println!(
                    "    [{}] {}: {}",
                    c.scope.label(),
                    c.input,
                    names.join(", ")
                );
            }
        }
    }
    Ok(())
}

fn print_summary(bindings: &Bindings, report: &Report) {
    let t = &report.totals;
    println!(
        "Preset {:?} version {} layout {}",
        bindings.preset_name,
        bindings
            .version
            .map(|v| v.to_string())
            .unwrap_or_else(|| "-".into()),
        bindings.keyboard_layout.as_deref().unwrap_or("-")
    );
    println!(
        "Actions: {} bound of {} ({} bindings); unknown to catalogue: {} {:?}",
        t.bound_actions,
        t.actions,
        t.bindings,
        t.unknown_actions.len(),
        t.unknown_actions
    );
    println!("Devices ({}):", report.devices.len());
    for d in &report.devices {
        println!(
            "  {:<28} {:<16} buttons {:>3}  axes {:>2}  modifier uses {:>2}  inputs {:>3}  actions {:>3}  conflicts {}",
            d.name,
            d.kind.label(),
            d.buttons,
            d.axes,
            d.modifier_uses,
            d.inputs,
            d.actions,
            d.conflicts
        );
    }
    println!("Conflicts: {}", report.conflicts.len());
    for c in &report.conflicts {
        let names: Vec<String> = c
            .actions
            .iter()
            .map(|a| format!("{} ({})", a.name, a.context.label()))
            .collect();
        println!(
            "  [{}] {} -> {}",
            c.scope.label(),
            c.input,
            names.join(", ")
        );
    }
    let essentials: Vec<_> = report.missing_essentials().collect();
    println!("Unbound essentials: {}", essentials.len());
    for m in &essentials {
        println!(
            "  [{}] {} -- bind {}",
            m.context.label(),
            m.label,
            m.options.join(" or ")
        );
    }
    let recommended: Vec<_> = report
        .missing
        .iter()
        .filter(|m| m.importance == Importance::Recommended)
        .collect();
    println!("Unbound recommended: {}", recommended.len());
    for m in &recommended {
        println!("  [{}] {}", m.context.label(), m.label);
    }
    println!("Modifier notes: {}", report.modifier_notes.len());
    for n in &report.modifier_notes {
        println!(
            "  [{}] {} is a modifier for {:?} and bound alone to {:?}",
            n.scope.label(),
            n.label,
            n.as_modifier,
            n.alone
        );
    }
}

fn time_parse(text: &str) -> Result<Duration, Box<dyn Error>> {
    const RUNS: u32 = 200;
    Bindings::parse(text)?; // warm-up (catalogue index, allocator)
    let started = Instant::now();
    for _ in 0..RUNS {
        std::hint::black_box(Bindings::parse(std::hint::black_box(text))?);
    }
    Ok(started.elapsed() / RUNS)
}

fn write_card(path: &Path, html: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, html)
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

fn default_out_dir() -> PathBuf {
    std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("..")
                .join("target-agents")
                .join("binds")
        })
}

/// The game's built-in presets live in `<install>\Products\<product>\ControlSchemes`.
fn probe_game_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("ED_CONTROL_SCHEMES") {
        return Some(PathBuf::from(dir));
    }
    let installs = [
        r"C:\Program Files (x86)\Steam\steamapps\common\Elite Dangerous",
        r"D:\SteamLibrary\steamapps\common\Elite Dangerous",
        r"E:\SteamLibrary\steamapps\common\Elite Dangerous",
        r"C:\Program Files\Epic Games\EliteDangerous",
        r"C:\Program Files (x86)\Frontier\EDLaunch",
    ];
    let products = ["elite-dangerous-odyssey-64", "elite-dangerous-64"];
    installs
        .iter()
        .flat_map(|i| {
            products
                .iter()
                .map(move |p| Path::new(i).join("Products").join(p).join("ControlSchemes"))
        })
        .find(|p| p.is_dir())
}
