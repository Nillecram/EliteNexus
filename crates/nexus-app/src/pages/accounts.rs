//! Website/service connections (EDDN, EDSM, Inara, EDAstro, Frontier CAPI). All opt-in.

use crate::secrets;
use crate::settings::Accounts;
use crate::theme::{display, pal};
use crate::widgets::{ago, badge, card};
use egui::{RichText, Ui};
use egui_phosphor::regular as icon;
use nexus_uplink::{ServiceStatus, UplinkStatus};

/// What the services report, refreshed every frame while the page is open.
#[derive(Debug, Clone, Default)]
pub struct StatusView {
    /// One line about the incoming EDDN feed (market service).
    pub eddn_feed: Option<String>,
    pub uplink: UplinkStatus,
}

#[derive(Default)]
pub struct AccountsUi {
    edsm_key_input: String,
    inara_key_input: String,
}

fn status_line(ui: &mut Ui, s: &ServiceStatus) {
    if !s.enabled && s.queued == 0 && s.sent == 0 && s.failed == 0 {
        return;
    }
    ui.horizontal_wrapped(|ui| {
        badge(ui, &format!("{} sent", s.sent), pal::GOOD);
        if s.queued > 0 {
            badge(ui, &format!("{} queued", s.queued), pal::CYAN);
        }
        if s.failed > 0 {
            badge(ui, &format!("{} failed", s.failed), pal::BAD);
        }
        if let Some(t) = s.last_success {
            ui.label(RichText::new(format!("last accepted {}", ago(t))).small().color(pal::DIM));
        }
    });
    if let Some(p) = &s.paused {
        ui.label(RichText::new(format!("{} Paused: {p}", icon::PAUSE_CIRCLE)).small().color(pal::WARN));
    }
    if let Some(e) = &s.last_error {
        ui.label(RichText::new(e).small().color(pal::BAD));
    }
}

/// Masked secret editor. Returns true when the stored (sealed) value changed.
fn secret_field(ui: &mut Ui, stored: &mut String, input: &mut String, hint: &str) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        if !stored.is_empty() && input.is_empty() {
            ui.label(RichText::new(format!("{} key saved (encrypted)", icon::LOCK_KEY)).color(pal::GOOD));
            if ui.small_button("Remove").clicked() {
                stored.clear();
                changed = true;
            }
        }
        let resp = ui.add(egui::TextEdit::singleline(input).password(true).hint_text(hint).desired_width(300.0));
        if (resp.lost_focus() || ui.button("Save").clicked()) && !input.trim().is_empty() {
            *stored = secrets::seal(input.trim());
            input.clear();
            changed = true;
        }
    });
    changed
}

/// Returns true if settings changed.
pub fn show(ui: &mut Ui, acc: &mut Accounts, st: &mut AccountsUi, cmdr_name: &str, status: &StatusView) -> bool {
    let mut changed = false;
    ui.label(RichText::new("Connect your commander to the community services. Everything is opt-in — nothing leaves this PC until you switch it on. Keys are encrypted with Windows DPAPI.").color(pal::DIM));
    ui.add_space(8.0);
    ui.columns(2, |c| {
        let ui = &mut c[0];
        card(ui, "EDDN — Elite Dangerous Data Network", |ui| {
            changed |= ui.checkbox(&mut acc.eddn_listen, "Receive the live market feed (all commanders' station data, in real time)").changed();
            if let Some(f) = &status.eddn_feed {
                ui.label(RichText::new(f).small().color(pal::CYAN));
            }
            ui.add_space(4.0);
            changed |= ui.checkbox(&mut acc.eddn_upload, "Contribute my market, outfitting, shipyard and exploration data").changed();
            ui.label(RichText::new("What you send is what EDMC/EDD send: station prices and services, system & body scans, routes. No credits, cargo or personal data. EDDN republishes it without your name. It keeps Inara, Spansh, EDSM and everyone else (including this app) up to date.").small().color(pal::FAINT));
            if acc.eddn_upload {
                changed |= ui
                    .checkbox(&mut acc.eddn_paused, "Paused: send nothing until I resume (also a button in the header)")
                    .on_hover_text("Market prices are only sent when you undock, so pausing while docked keeps a market you just found to yourself.")
                    .changed();
            }
            changed |= ui.checkbox(&mut acc.eddn_test_mode, "Test mode (send to EDDN's /test schemas only)").changed();
            status_line(ui, &status.uplink.eddn);
        });
        ui.add_space(8.0);
        card(ui, "EDAstro", |ui| {
            changed |= ui.checkbox(&mut acc.edastro_enabled, "Send the events EDAstro asks for (carrier jumps/stats, docking, organic scans)").changed();
            ui.label(RichText::new("Feeds EDAstro's galaxy maps and statistics. No key needed.").small().color(pal::FAINT));
            status_line(ui, &status.uplink.edastro);
        });
        ui.add_space(8.0);
        card(ui, "Frontier Companion API", |ui| {
            ui.label(RichText::new("Optional. Adds fleet-carrier market/crew data and remote shipyard info. It needs a client ID that Frontier approves for this app (Developer Zone at user.frontierstore.net); the journal already covers almost everything.").color(pal::DIM));
            ui.horizontal(|ui| badge(ui, "not configured", pal::FAINT));
        });

        let ui = &mut c[1];
        card(ui, "EDSM — flight log & star map", |ui| {
            changed |= ui.checkbox(&mut acc.edsm_enabled, "Sync my journal to EDSM").changed();
            ui.horizontal(|ui| {
                ui.label("Commander name");
                if acc.edsm_commander.is_empty() && !cmdr_name.is_empty() {
                    acc.edsm_commander = cmdr_name.to_string();
                    changed = true;
                }
                changed |= ui.add(egui::TextEdit::singleline(&mut acc.edsm_commander).desired_width(220.0)).changed();
            });
            changed |= secret_field(ui, &mut acc.edsm_api_key, &mut st.edsm_key_input, "EDSM API key");
            ui.hyperlink_to(format!("{} Get your EDSM API key", icon::ARROW_SQUARE_OUT), "https://www.edsm.net/en/settings/api");
            status_line(ui, &status.uplink.edsm);
        });
        ui.add_space(8.0);
        card(ui, "Inara — commander profile", |ui| {
            changed |= ui.checkbox(&mut acc.inara_enabled, "Sync my commander profile to Inara").changed();
            changed |= secret_field(ui, &mut acc.inara_api_key, &mut st.inara_key_input, "Inara API key");
            ui.hyperlink_to(format!("{} Get your Inara API key", icon::ARROW_SQUARE_OUT), "https://inara.cz/elite/cmdr-settings-api/");
            changed |= ui.checkbox(&mut acc.inara_dev_mode, "Developer mode (required until Inara whitelists the app name)").changed();
            ui.label(RichText::new("Inara accepts about 2 requests a minute; Elite Nexus batches updates every 35+ seconds so your key is never locked out.").small().color(pal::FAINT));
            status_line(ui, &status.uplink.inara);
        });
    });
    ui.add_space(8.0);
    ui.label(RichText::new(format!("{} History is never uploaded — only events that happen while Elite Nexus is running.", icon::INFO)).font(display(13.0)).color(pal::DIM));
    changed
}
