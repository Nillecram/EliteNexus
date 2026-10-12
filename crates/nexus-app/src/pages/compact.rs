//! Compact always-on-top HUD (second screen / beside the game).

use crate::theme::{display, pal};
use crate::widgets::{badge, bar, card, short_credits};
use egui::{RichText, Ui};
use egui_phosphor::regular as icon;
use nexus_data::exploration::{is_scoopable, star_description};
use nexus_model::{Alert, GameState};

pub fn show(ui: &mut Ui, gs: &GameState, alerts: &[Alert]) {
    let l = &gs.location;
    card(ui, "", |ui| {
        ui.label(RichText::new(&l.system).font(display(20.0)).color(pal::TEXT));
        ui.label(RichText::new(l.situation()).color(pal::ACCENT_SOFT));
        if let Some(s) = gs.fleet.current().filter(|s| s.fuel_main_capacity > 0.0) {
            let f = (gs.fleet.fuel_main / s.fuel_main_capacity) as f32;
            let col = if f < 0.25 { pal::BAD } else if f < 0.5 { pal::WARN } else { pal::CYAN };
            bar(ui, f, col, 12.0, Some(&format!("{} {:.1} / {:.0} t", icon::GAS_PUMP, gs.fleet.fuel_main, s.fuel_main_capacity)));
        }
        if let Some(t) = &l.fsd_target {
            let scoop = is_scoopable(&t.star_class);
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("{} {}", icon::CROSSHAIR, t.name)).color(pal::CYAN));
                badge(ui, &star_description(&t.star_class), if scoop { pal::GOOD } else { pal::WARN });
            });
            if let Some(j) = t.remaining_jumps.filter(|j| *j > 0) {
                ui.label(RichText::new(format!("{j} jumps remaining")).small().color(pal::DIM));
            }
        }
    });
    ui.add_space(6.0);
    if let Some(cur) = &gs.exobio.current {
        card(ui, "Sampling", |ui| {
            let pos = gs.status.as_ref().and_then(|s| s.lat_lon());
            let radius = gs.status.as_ref().and_then(|s| s.planet_radius);
            let heading = gs.status.as_ref().and_then(|s| s.heading);
            crate::pages::exobio::sampling(ui, cur, pos, radius, heading);
        });
        ui.add_space(6.0);
    }
    if let Some(sys) = gs.exploration.current_system() {
        card(ui, "System", |ui| {
            let total = sys.fss_body_count.unwrap_or(0);
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("{}/{} bodies", sys.scanned_bodies(), total)).font(display(16.0)));
                ui.label(RichText::new(short_credits(sys.value())).font(display(16.0)).color(pal::GOLD));
                let bio = sys.bio_signal_total();
                if bio > 0 {
                    badge(ui, &format!("{} {bio}", icon::PLANT), pal::GOOD);
                }
            });
            let mut top: Vec<_> = sys.bodies.values().filter(|b| b.has_scan).collect();
            top.sort_by_key(|b| std::cmp::Reverse(b.value.mapped));
            for b in top.iter().take(3).filter(|b| b.value.mapped >= 200_000) {
                ui.label(RichText::new(format!("{} — {}{}", b.short_name(&sys.name), short_credits(b.value.mapped), if b.mapped { " ✓" } else { "" })).color(pal::TEXT));
            }
        });
        ui.add_space(6.0);
    }
    card(ui, "Alerts", |ui| {
        if alerts.is_empty() {
            ui.label(RichText::new("Quiet so far.").color(pal::FAINT));
        }
        for a in alerts.iter().rev().take(6) {
            ui.label(RichText::new(format!("{} {}", a.ts.to_local_string("%H:%M"), a.title)).color(pal::TEXT));
            if !a.detail.is_empty() {
                ui.label(RichText::new(&a.detail).small().color(pal::DIM));
            }
        }
    });
}
