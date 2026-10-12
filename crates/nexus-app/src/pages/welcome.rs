//! First-run welcome: what Elite Nexus found, and the few choices that matter. A centred sheet, as
//! a Mac app shows on first launch.

use crate::settings::Settings;
use crate::theme::{bold, pal, size};
use crate::widgets::{self, group};
use egui::{Align2, CornerRadius, RichText, Sense, Ui, Vec2};
use egui_phosphor::regular as icon;
use nexus_model::GameState;

pub enum WelcomeResult {
    Open,
    Done,
    GoToAccounts,
}

pub fn show(ctx: &egui::Context, settings: &mut Settings, gs: Option<&GameState>, journal_files: usize) -> WelcomeResult {
    let mut result = WelcomeResult::Open;
    egui::Modal::new(egui::Id::new("welcome")).frame(egui::Frame::new().fill(pal::SIDEBAR).corner_radius(CornerRadius::same(20)).inner_margin(egui::Margin::same(28))).show(ctx, |ui: &mut Ui| {
        ui.set_width(540.0);
        ui.vertical_centered(|ui| {
            // The app icon, large.
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(72.0), Sense::hover());
            ui.painter().rect_filled(rect, CornerRadius::same(18), pal::ACCENT);
            ui.painter().text(rect.center(), Align2::CENTER_CENTER, icon::DIAMOND, egui::FontId::proportional(42.0), pal::ON_ACCENT);
            ui.add_space(12.0);
            ui.label(RichText::new("Welcome to Elite Nexus").font(bold(size::TITLE)).color(pal::TEXT));
            ui.add_space(4.0);
            let line = match gs {
                Some(gs) => format!(
                    "o7 CMDR {}. Found {} journal files: {} jumps, {} systems, {} ships{}.",
                    gs.cmdr.name,
                    journal_files,
                    gs.history.jumps.len(),
                    gs.history.visited.len(),
                    gs.fleet.ships.len(),
                    if gs.carrier.owned() { format!(" and your carrier {}", gs.carrier.name) } else { String::new() }
                ),
                None => "No commander found in the journal folder yet. Start the game and Elite Nexus picks everything up live.".to_string(),
            };
            ui.add(egui::Label::new(RichText::new(line).size(size::CALLOUT).color(pal::DIM)).wrap().halign(egui::Align::Center));
        });
        ui.add_space(18.0);
        group(ui, |g| {
            g.switch(&mut settings.tts_enabled, "Speak important alerts", "High-value bodies, sample spacing, fuel, interdictions…");
            g.switch(&mut settings.accounts.eddn_listen, "Receive the live market feed", "Fresh prices from every commander, in real time.");
            g.switch(&mut settings.accounts.eddn_upload, "Share my station and exploration data", "Like EDMC and EDDiscovery: game data only, no credits, cargo or personal info.");
            g.switch(&mut settings.auto_copy_next_system, "Copy the next system after each jump", "Paste it straight into the galaxy map.");
        });
        widgets::footnote(ui, "You can change any of this later in Settings and Accounts & sync.");
        ui.add_space(18.0);
        ui.horizontal(|ui| {
            if widgets::button(ui, &format!("{} Connect EDSM or Inara", icon::PLUGS_CONNECTED)).clicked() {
                result = WelcomeResult::GoToAccounts;
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if widgets::primary_button(ui, "Let's fly").clicked() {
                    result = WelcomeResult::Done;
                }
            });
        });
    });
    result
}
