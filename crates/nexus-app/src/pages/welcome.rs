//! First-run welcome: what Elite Nexus found, and the few choices that matter.

use crate::settings::Settings;
use crate::theme::{display, pal};
use egui::{RichText, Ui};
use egui_phosphor::regular as icon;
use nexus_model::GameState;

pub enum WelcomeResult {
    Open,
    Done,
    GoToAccounts,
}

pub fn show(ctx: &egui::Context, settings: &mut Settings, gs: Option<&GameState>, journal_files: usize) -> WelcomeResult {
    let mut result = WelcomeResult::Open;
    egui::Modal::new(egui::Id::new("welcome")).show(ctx, |ui: &mut Ui| {
        ui.set_width(560.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new(icon::DIAMOND).size(30.0).color(pal::ACCENT));
            ui.label(RichText::new("Welcome to Elite Nexus").font(display(26.0)));
        });
        ui.add_space(6.0);
        match gs {
            Some(gs) => {
                ui.label(RichText::new(format!("o7 CMDR {}.", gs.cmdr.name)).font(display(18.0)).color(pal::ACCENT_SOFT));
                ui.label(RichText::new(format!(
                    "I read {} journal files: {} jumps, {} systems, {} ships{}.",
                    journal_files,
                    gs.history.jumps.len(),
                    gs.history.visited.len(),
                    gs.fleet.ships.len(),
                    if gs.carrier.owned() { format!(" and your carrier {}", gs.carrier.name) } else { String::new() }
                ))
                .color(pal::TEXT));
            }
            None => {
                ui.label(RichText::new("No commander found in the journal folder yet — start the game and Elite Nexus picks everything up live.").color(pal::DIM));
            }
        }
        ui.add_space(10.0);
        ui.label(RichText::new("A FEW CHOICES").font(display(13.0)).color(pal::ACCENT));
        ui.checkbox(&mut settings.tts_enabled, "Speak important alerts (high-value bodies, sample spacing, fuel, interdictions…)");
        ui.checkbox(&mut settings.accounts.eddn_listen, "Receive the live EDDN market feed (fresh prices from every commander)");
        ui.checkbox(&mut settings.accounts.eddn_upload, "Contribute my station & exploration data to EDDN (like EDMC/EDD do)");
        ui.label(RichText::new("Only game data is shared (prices, services, scans) — no credits, cargo or personal info. You can change this any time in Accounts & sync.").small().color(pal::FAINT));
        ui.checkbox(&mut settings.auto_copy_next_system, "Copy the next route system to the clipboard after each jump");
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            if ui.button(RichText::new(format!("{} Connect EDSM / Inara…", icon::PLUGS_CONNECTED)).color(pal::CYAN)).clicked() {
                result = WelcomeResult::GoToAccounts;
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(RichText::new("Let's fly").font(display(16.0)).color(pal::ACCENT)).clicked() {
                    result = WelcomeResult::Done;
                }
            });
        });
    });
    result
}
