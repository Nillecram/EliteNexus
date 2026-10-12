use crate::theme::{display, pal};
use crate::widgets::{badge, card, duration, empty};
use egui::{Grid, RichText, Ui};
use egui_phosphor::regular as icon;
use nexus_model::mining::MiningSession;
use nexus_model::GameState;

pub fn show(ui: &mut Ui, gs: &GameState) {
    match &gs.mining.session {
        Some(s) => session(ui, s, true),
        None => empty(ui, icon::DIAMONDS_FOUR, "Not mining right now", "Fire a prospector limpet and a live session starts: tonnes/hour, prospect quality, motherlodes."),
    }
    if !gs.mining.history.is_empty() {
        ui.add_space(10.0);
        ui.label(RichText::new("PREVIOUS SESSIONS").font(display(14.0)).color(pal::ACCENT));
        for s in gs.mining.history.iter().rev().take(10) {
            session(ui, s, false);
            ui.add_space(6.0);
        }
    }
}

fn session(ui: &mut Ui, s: &MiningSession, live: bool) {
    let title = format!("{}{}", if live { "Live session — " } else { "" }, s.body.clone().unwrap_or_else(|| s.system.clone()));
    card(ui, &title, |ui| {
        ui.horizontal(|ui| {
            crate::widgets::stat(ui, "refined", format!("{} t", s.total_refined()), pal::ACCENT);
            ui.add_space(16.0);
            crate::widgets::stat(ui, "per hour", format!("{:.1} t", s.tons_per_hour()), pal::GOOD);
            ui.add_space(16.0);
            crate::widgets::stat(ui, "duration", duration(s.duration_secs()), pal::TEXT);
            ui.add_space(16.0);
            crate::widgets::stat(ui, "prospected", s.prospected.to_string(), pal::TEXT);
            ui.add_space(16.0);
            let (h, m, l) = s.content_counts();
            crate::widgets::stat(ui, "high / med / low", format!("{h} / {m} / {l}"), pal::CYAN);
        });
        ui.add_space(4.0);
        ui.columns(2, |c| {
            Grid::new(("refined", s.start.0)).num_columns(2).striped(true).show(&mut c[0], |ui| {
                for (name, n) in &s.refined {
                    ui.label(name);
                    ui.label(RichText::new(format!("{n} t")).color(pal::ACCENT_SOFT));
                    ui.end_row();
                }
            });
            let ui = &mut c[1];
            if !s.motherlodes.is_empty() {
                ui.horizontal_wrapped(|ui| {
                    for (m, n) in &s.motherlodes {
                        badge(ui, &format!("{n}× {m} core"), pal::GOLD);
                    }
                });
            }
            let mut best: Vec<_> = s.best_proportion.iter().collect();
            best.sort_by(|a, b| b.1.total_cmp(a.1));
            for (m, p) in best.iter().take(6) {
                ui.label(RichText::new(format!("Best {m}: {p:.1}%")).color(pal::DIM));
            }
        });
    });
}
