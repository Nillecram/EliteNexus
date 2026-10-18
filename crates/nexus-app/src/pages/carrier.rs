use crate::theme::{display, pal, size};
use crate::widgets::{badge, bar, card, duration, empty, kv, short_credits};
use egui::{Color32, Grid, RichText, Ui};
use egui_phosphor::regular as icon;
use nexus_journal::Timestamp;
use nexus_model::carrier::{timing, Carrier, JumpTimer};
use nexus_model::GameState;
use std::time::Duration;

/// Where the timings come from, for the timer's tooltip.
const TIMING_SOURCES: &str = "The departure time comes from the game's journal (CarrierJumpRequest) and already includes any wait for busy servers.\n\nThe 5-minute cooldown after a jump is from Frontier's patch notes when carriers launched (update 1.43, June 2020). The journal doesn't record it, and no later change was found.\n\nThe pre-flight lockdown (3 min 20 s before departure) and the cancel cutoff (10 minutes before) come from the community wiki, not Frontier, so take them as a guide.";

/// The jump countdown or cooldown: what's happening, time left, where, and a progress bar.
/// Repaints every second while a timer runs.
pub fn jump_timer(ui: &mut Ui, c: &Carrier) {
    let now = Timestamp::now();
    let Some(t) = c.jump_timer(now) else {
        badge(ui, "READY", pal::GOOD);
        ui.label(RichText::new("No jump scheduled").font(display(size::TITLE)).color(pal::TEXT));
        note(ui, "Schedule a jump in the carrier management screen: the countdown, and the cooldown after the jump, show here.");
        return;
    };
    ui.ctx().request_repaint_after(Duration::from_secs(1));
    let left = t.left(now);
    match &t {
        JumpTimer::Countdown { to, body, departs } => {
            let lockdown = left.is_some_and(|l| l <= timing::LOCKDOWN_SECS);
            let (phase, colr) = if lockdown { ("LOCKDOWN", pal::BAD) } else { ("JUMP COUNTDOWN", pal::WARN) };
            badge(ui, phase, colr);
            let big = left.map(duration).unwrap_or_else(|| "Scheduled".into());
            ui.label(RichText::new(big).font(display(size::TITLE)).color(colr)).on_hover_text(TIMING_SOURCES);
            destination(ui, icon::ARROW_FAT_RIGHT, to, body.as_deref());
            let detail = match (departs, left) {
                (Some(d), Some(l)) if l > timing::CANCEL_CUTOFF_SECS => {
                    format!("Departs {} · can be cancelled until about {}", local(*d), local(Timestamp(d.0 - timing::CANCEL_CUTOFF_SECS)))
                }
                (Some(d), Some(l)) if l > timing::LOCKDOWN_SECS => {
                    format!("Departs {} · too late to cancel · lockdown from about {}", local(*d), local(Timestamp(d.0 - timing::LOCKDOWN_SECS)))
                }
                (Some(d), _) => format!("Departs {} · pre-flight lockdown: pads closed, services stopped", local(*d)),
                (None, _) => "The journal didn't give a departure time.".into(),
            };
            note(ui, &detail);
            if let (Some(d), Some(p)) = (departs, &c.pending_jump) {
                let total = (d.0 - p.requested.0).max(1) as f32;
                bar(ui, (now.0 - p.requested.0) as f32 / total, colr, 6.0, None);
            }
        }
        JumpTimer::Cooldown { at, body, ready, estimated } => {
            badge(ui, "COOLDOWN", pal::CYAN);
            let big = left.map(duration).unwrap_or_default();
            ui.label(RichText::new(if *estimated { format!("≈ {big}") } else { big }).font(display(size::TITLE)).color(pal::CYAN)).on_hover_text(TIMING_SOURCES);
            destination(ui, icon::MAP_PIN, at, body.as_deref());
            note(ui, &format!("Can jump again from about {}", local(*ready)));
            if *estimated {
                note(ui, "Counted from the departure: the journal records the arrival only when you're aboard, so the cooldown ends a little later, by however long the jump took.");
            }
            bar(ui, 1.0 - left.unwrap_or(0) as f32 / timing::COOLDOWN_SECS as f32, pal::CYAN, 6.0, None);
        }
    }
}

/// One line for compact places (the Dashboard): the timer's phase and time left.
pub fn jump_timer_line(c: &Carrier, now: Timestamp) -> Option<(String, Color32)> {
    let t = c.jump_timer(now)?;
    let left = t.left(now);
    Some(match &t {
        JumpTimer::Countdown { to, .. } => match left {
            Some(l) if l <= timing::LOCKDOWN_SECS => (format!("{} Jumping to {to} in {} (lockdown)", icon::ARROW_FAT_RIGHT, duration(l)), pal::BAD),
            Some(l) => (format!("{} Jumping to {to} in {}", icon::ARROW_FAT_RIGHT, duration(l)), pal::WARN),
            None => (format!("{} Jump to {to} scheduled", icon::ARROW_FAT_RIGHT), pal::WARN),
        },
        JumpTimer::Cooldown { at, estimated, .. } => {
            let approx = if *estimated { "≈ " } else { "" };
            (format!("{} Cooldown in {at}: can jump again in {approx}{}", icon::TIMER, duration(left.unwrap_or(0))), pal::CYAN)
        }
    })
}

fn destination(ui: &mut Ui, glyph: &str, system: &str, body: Option<&str>) {
    let text = match body.filter(|b| !b.is_empty() && !b.eq_ignore_ascii_case(system)) {
        Some(b) => format!("{glyph} {system} · {b}"),
        None => format!("{glyph} {system}"),
    };
    if ui.link(RichText::new(text).color(pal::TEXT)).on_hover_text("Click to copy the system name").clicked() {
        ui.ctx().copy_text(system.to_string());
    }
}

fn note(ui: &mut Ui, text: &str) {
    ui.add(egui::Label::new(RichText::new(text).size(size::FOOTNOTE).color(pal::DIM)).wrap());
}

fn local(t: Timestamp) -> String {
    t.to_local_string("%H:%M:%S")
}

pub fn show(ui: &mut Ui, gs: &GameState) {
    let c = &gs.carrier;
    if !c.owned() {
        empty(ui, icon::ANCHOR, "No fleet carrier", "Carrier data appears when the game writes CarrierStats (open the carrier management screen).");
        return;
    }
    crate::widgets::cards(ui, 2, 300.0, |col| {
        col.add(|ui| {
            card(ui, "Carrier", |ui| {
            ui.label(RichText::new(&c.name).font(display(24.0)));
            ui.label(RichText::new(&c.callsign).font(display(16.0)).color(pal::ACCENT));
            ui.add_space(4.0);
            ui.label(RichText::new(format!("{} {}", icon::MAP_PIN, c.system.clone().unwrap_or_else(|| "Unknown".into()))).color(pal::TEXT));
            ui.horizontal_wrapped(|ui| {
                badge(ui, &format!("Docking: {}", c.docking_access), pal::CYAN);
                if c.allow_notorious {
                    badge(ui, "Notorious allowed", pal::WARN);
                }
                if c.pending_decommission {
                    badge(ui, "DECOMMISSIONING", pal::BAD);
                }
            });
        });
        });
        col.add(|ui| {
            card(ui, "Jump", |ui| jump_timer(ui, c));
        });
    });
    ui.add_space(8.0);
    crate::widgets::cards(ui, 2, 300.0, |col| {
        col.add(|ui| {
            card(ui, "Tritium", |ui| {
            ui.label(RichText::new(format!("{} t", c.fuel)).font(display(24.0)).color(pal::CYAN));
            bar(ui, c.fuel as f32 / 1000.0, pal::CYAN, 10.0, None);
            ui.label(RichText::new(format!("Jump range {:.0} / {:.0} ly", c.jump_range_curr, c.jump_range_max)).color(pal::DIM));
            ui.label(RichText::new("Use Navigation → Carrier route for exact tritium per jump (Spansh).").small().color(pal::DIM));
        });
        });
        col.add(|ui| {
            card(ui, "Finance", |ui| {
            ui.label(RichText::new(short_credits(c.finance.carrier_balance)).font(display(24.0)).color(pal::GOLD));
            Grid::new("cfin").max_col_width(ui.available_width()).num_columns(2).spacing([10.0, 3.0]).show(ui, |ui| {
                kv(ui, "Available", short_credits(c.finance.available_balance));
                kv(ui, "Reserved", short_credits(c.finance.reserve_balance));
                if let Some(p) = c.finance.reserve_percent {
                    kv(ui, "Reserve %", format!("{p:.0}%"));
                }
            });
        });
        });
    });
    ui.add_space(8.0);
    crate::widgets::cards(ui, 2, 360.0, |col| {
        col.add(|ui| {
            card(ui, "Capacity", |ui| {
            let s = &c.space;
            let total = s.total_capacity.max(1) as f32;
            Grid::new("cspace").num_columns(2).spacing([10.0, 4.0]).show(ui, |ui| {
                for (label, v, colr) in [
                    ("Cargo", s.cargo, pal::ACCENT),
                    ("Reserved for orders", s.cargo_space_reserved, pal::WARN),
                    ("Crew", s.crew, pal::CYAN),
                    ("Ship packs", s.ship_packs, pal::PURPLE),
                    ("Module packs", s.module_packs, pal::PURPLE),
                    ("Free", s.free_space, pal::GOOD),
                ] {
                    ui.label(RichText::new(label).color(pal::DIM));
                    bar(ui, v as f32 / total, colr, 12.0, Some(&format!("{v} t")));
                    ui.end_row();
                }
            });
        });
        });
        col.add(|ui| {
            card(ui, "Crew & services", |ui| {
            Grid::new("ccrew").max_col_width(ui.available_width()).num_columns(3).striped(true).spacing([10.0, 3.0]).show(ui, |ui| {
                for m in &c.crew {
                    ui.label(nexus_data::names::split_camel(&m.crew_role));
                    let (t, colr) = if !m.activated {
                        ("inactive", pal::FAINT)
                    } else if m.enabled == Some(false) {
                        ("suspended", pal::WARN)
                    } else {
                        ("active", pal::GOOD)
                    };
                    badge(ui, t, colr);
                    ui.label(RichText::new(m.crew_name.clone().unwrap_or_default()).color(pal::DIM));
                    ui.end_row();
                }
            });
        });
        });
    });
    ui.add_space(8.0);
    crate::widgets::cards(ui, 2, 360.0, |col| {
        col.add(|ui| {
            card(ui, "Market orders", |ui| {
            if c.trade_orders.is_empty() {
                ui.label(RichText::new("No orders recorded.").color(pal::DIM));
            }
            Grid::new("corders").num_columns(3).striped(true).show(ui, |ui| {
                for o in c.trade_orders.values() {
                    ui.label(&o.commodity);
                    if let Some(p) = o.purchase {
                        ui.label(RichText::new(format!("BUY {p}")).color(pal::GOOD));
                    } else if let Some(s) = o.sale {
                        ui.label(RichText::new(format!("SELL {s}")).color(pal::ACCENT));
                    } else {
                        ui.label("");
                    }
                    ui.label(RichText::new(o.price.map(|p| format!("{} Cr", nexus_data::names::group_digits(p))).unwrap_or_default()).color(pal::GOLD));
                    ui.end_row();
                }
            });
        });
        });
        col.add(|ui| {
            card(ui, "Jump history", |ui| {
            if c.jumps.is_empty() {
                ui.label(RichText::new("No jumps recorded yet.").color(pal::DIM));
            }
            Grid::new("cjumps").max_col_width(ui.available_width()).num_columns(2).striped(true).show(ui, |ui| {
                for j in c.jumps.iter().rev().take(30) {
                    ui.label(RichText::new(j.ts.to_local_string("%d %b %H:%M")).color(pal::FAINT));
                    ui.label(&j.system);
                    ui.end_row();
                }
            });
        });
        });
    });
}
