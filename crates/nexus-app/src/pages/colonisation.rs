use crate::finder::{self, Supplier};
use crate::jobs::Slot;
use crate::market::MarketService;
use crate::theme::{display, pal, size};
use crate::widgets::{badge, bar, button, card, card_with, disclosure, empty, footnote, icon_button, section, short_credits};
use egui::{Grid, RichText, Ui};
use egui_phosphor::regular as icon;
use nexus_journal::Timestamp;
use nexus_model::colonisation::{Depot, Involvement};
use nexus_model::GameState;
use serde::{Deserialize, Serialize};
use std::cmp::Reverse;

#[derive(Default)]
pub struct ColonisationUi {
    /// Supplier search results and the depot (market id) they are for.
    suppliers: Slot<Result<Vec<Supplier>, String>>,
    for_depot: u64,
}

/// The Colonisation page's choices, kept in settings.json.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ColonisationPrefs {
    /// Depots (market ids) hidden with their Hide button.
    pub hidden: Vec<u64>,
    /// Show the depots you only docked at.
    pub show_visited: bool,
    /// List the hidden depots, to bring one back.
    pub show_hidden: bool,
}

/// Returns true when `prefs` changed (to be saved).
pub fn show(ui: &mut Ui, gs: &GameState, market: &MarketService, st: &mut ColonisationUi, prefs: &mut ColonisationPrefs) -> bool {
    st.suppliers.tick();
    let col = &gs.colonisation;
    if col.depots.is_empty() && col.claims.is_empty() {
        empty(ui, icon::BUILDINGS, "No colonisation activity", "Dock at a construction site and its shopping list appears here automatically.");
        return false;
    }
    let before = prefs.clone();
    let now = Timestamp::now();
    let mut depots: Vec<(&Depot, Involvement)> = col.depots.values().map(|d| (d, col.involvement(d))).collect();
    // Up to date first, then out of date, then finished; the latest visit first in each.
    depots.sort_by_key(|(d, _)| (d.finished(), d.is_stale(now), Reverse(d.updated)));
    let (hidden, shown): (Vec<_>, Vec<_>) = depots.into_iter().partition(|(d, _)| prefs.hidden.contains(&d.market_id));
    let (yours, visited): (Vec<_>, Vec<_>) = shown.into_iter().partition(|(_, inv)| inv.is_yours());

    section(ui, "Your constructions");
    if yours.is_empty() {
        footnote(ui, "None in your journals. A construction counts as yours once you've claimed its system or delivered to it.");
    }
    for (d, inv) in &yours {
        if depot_card(ui, gs, d, *inv, now, market, st) {
            prefs.hidden.push(d.market_id);
        }
        ui.add_space(8.0);
    }

    if !visited.is_empty() {
        ui.add_space(4.0);
        disclosure(ui, &mut prefs.show_visited, &format!("Depots you only docked at ({})", visited.len()));
        footnote(
            ui,
            "You docked at these, but your journals show no claim of their system and no delivery from you, so they're most likely other \
             commanders' projects. Deliver to one and it moves up to your constructions.",
        );
        if prefs.show_visited {
            ui.add_space(6.0);
            for (d, inv) in &visited {
                if depot_card(ui, gs, d, *inv, now, market, st) {
                    prefs.hidden.push(d.market_id);
                }
                ui.add_space(8.0);
            }
        }
    }

    if !col.claims.is_empty() {
        ui.add_space(8.0);
        card(ui, "Your claims", |ui| {
            for c in &col.claims {
                ui.horizontal(|ui| {
                    ui.label(&c.system);
                    if c.beacon_deployed {
                        badge(ui, "beacon deployed", pal::GOOD);
                    }
                    if c.released {
                        badge(ui, "released", pal::FAINT);
                    }
                    ui.label(RichText::new(c.ts.to_local_string("%d %b %Y")).color(pal::DIM));
                });
            }
        });
    }

    if !hidden.is_empty() {
        ui.add_space(8.0);
        disclosure(ui, &mut prefs.show_hidden, &format!("Hidden ({})", hidden.len()));
        if prefs.show_hidden {
            for (d, _) in &hidden {
                ui.horizontal(|ui| {
                    if button(ui, "Show again").clicked() {
                        prefs.hidden.retain(|id| *id != d.market_id);
                    }
                    ui.label(depot_title(d));
                    ui.label(RichText::new(format!("updated {}", crate::widgets::ago(d.updated))).color(pal::DIM));
                });
            }
        }
    }

    ui.add_space(10.0);
    footnote(
        ui,
        "The game records a depot's progress only while you're docked there, so every figure is from your last visit. The journal \
         doesn't say whose project a depot is: Elite Nexus counts it as yours when your journals show you claimed its system or \
         delivered to it.",
    );
    *prefs != before
}

fn depot_title(d: &Depot) -> String {
    let name = d.station.clone().unwrap_or_else(|| if d.colonisation_ship { "Colonisation ship".into() } else { "Construction site".into() });
    if d.system.is_empty() { name } else { format!("{name} — {}", d.system) }
}

/// One depot's card; returns true when its Hide button was clicked.
fn depot_card(ui: &mut Ui, gs: &GameState, d: &Depot, inv: Involvement, now: Timestamp, market: &MarketService, st: &mut ColonisationUi) -> bool {
    let cargo = &gs.inventory.cargo;
    let stale = d.is_stale(now);
    let mut hide = false;
    let right = |ui: &mut Ui| {
        hide = icon_button(ui, icon::EYE_SLASH, "Hide this depot (Hidden, at the bottom of the page, brings it back)").clicked();
    };
    card_with(ui, &depot_title(d), right, |ui| {
        let kind = if d.colonisation_ship { "Colonisation ship (the system's first port)" } else { "Construction site" };
        let whose = match inv {
            Involvement::Claimed => "in a system you claimed".to_string(),
            Involvement::Contributed => format!("you delivered {} t", nexus_data::names::group_digits(d.delivered_tons())),
            Involvement::Visited => "you only docked here".to_string(),
        };
        ui.label(RichText::new(format!("{kind} · {whose} · updated {}", crate::widgets::ago(d.updated))).size(size::FOOTNOTE).color(pal::DIM));
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            crate::widgets::stat(ui, "progress", format!("{:.1}%", d.progress * 100.0), pal::GOOD);
            ui.add_space(16.0);
            crate::widgets::stat(ui, "tonnes remaining", nexus_data::names::group_digits(d.remaining_tons()), pal::ACCENT);
            ui.add_space(16.0);
            crate::widgets::stat(ui, "of", nexus_data::names::group_digits(d.required_tons()), pal::DIM);
            if d.complete {
                badge(ui, "COMPLETE", pal::GOOD);
            } else if d.failed {
                badge(ui, "FAILED", pal::BAD);
            } else if stale {
                badge(ui, "OUT OF DATE", pal::WARN);
            }
        });
        bar(ui, d.progress as f32, pal::GOOD, 10.0, None);
        if stale {
            let days = now.secs_since(d.updated) / 86_400;
            let why = if d.colonisation_ship {
                "A colonisation ship has four weeks to be finished, so this one has been finished or has expired by now."
            } else {
                "The game updates a depot only while you're docked there, so it may have moved on or been finished since."
            };
            ui.add(egui::Label::new(RichText::new(format!("Not updated for {days} days. {why} Dock there to update it, or hide it.")).color(pal::WARN)).wrap());
        }
        ui.add_space(4.0);
        let mut res: Vec<_> = d.resources.iter().filter(|r| r.remaining() > 0).collect();
        res.sort_by_key(|r| Reverse(r.remaining()));
        if !res.is_empty() {
            Grid::new(("depot", d.market_id)).num_columns(5).striped(true).spacing([12.0, 3.0]).show(ui, |ui| {
                ui.label(RichText::new("Commodity").color(pal::DIM));
                ui.label(RichText::new("Still needed").color(pal::DIM));
                ui.label(RichText::new("Progress").color(pal::DIM));
                ui.label(RichText::new("In your hold").color(pal::DIM));
                ui.label(RichText::new("Paid / t").color(pal::DIM));
                ui.end_row();
                for r in res {
                    ui.label(&r.name);
                    ui.label(RichText::new(nexus_data::names::group_digits(r.remaining())).font(display(14.0)).color(pal::ACCENT));
                    ui.scope(|ui| {
                        ui.set_width(140.0);
                        bar(ui, r.provided as f32 / r.required.max(1) as f32, pal::GOOD, 8.0, None);
                    });
                    let have = cargo.count(&r.symbol);
                    ui.label(RichText::new(if have > 0 { have.to_string() } else { "—".into() }).color(if have > 0 { pal::GOOD } else { pal::FAINT }));
                    ui.label(RichText::new(short_credits(r.payment)).color(pal::DIM));
                    ui.end_row();
                }
            });
        }
        // An expired colonisation ship needs nothing more; other out-of-date sites may still want the goods.
        if !d.finished() && !(stale && d.colonisation_ship) {
            ui.add_space(6.0);
            suppliers_section(ui, gs, d, market, st);
        }
    });
    hide
}

/// "Where do I buy this?": the nearest stations with stock for each commodity still needed,
/// searched around the construction site (from the local market database).
fn suppliers_section(ui: &mut Ui, gs: &GameState, d: &Depot, market: &MarketService, st: &mut ColonisationUi) {
    let Some(db) = market.db.clone() else { return };
    let hold = gs.fleet.current().map(|s| i64::from(s.cargo_capacity)).filter(|c| *c > 0).unwrap_or(100);
    let pad = finder::ship_pad(gs);
    ui.horizontal(|ui| {
        let busy = st.suppliers.busy() && st.for_depot == d.market_id;
        let label = format!("{} Where to buy (nearest {} pad, {} t hold)", icon::MAGNIFYING_GLASS, pad.unwrap_or('L'), hold);
        if ui.add_enabled(!busy, egui::Button::new(RichText::new(label).color(pal::ACCENT))).clicked() {
            let needs: Vec<finder::Line> = d.resources.iter().filter(|r| r.remaining() > 0).map(|r| (r.symbol.clone(), r.name.clone(), r.remaining())).collect();
            let (addr, here) = (d.system_address, gs.location.star_pos);
            st.for_depot = d.market_id;
            st.suppliers.run(ui.ctx(), move || {
                let origin = db.system_by_id(addr).ok().flatten().and_then(|s| s.pos).or(here).ok_or("The construction site's position is not known yet.")?;
                finder::suppliers(&db, origin, &needs, pad, hold, 7.0 * 24.0)
            });
        }
        if busy {
            ui.spinner();
        }
    });
    if st.for_depot != d.market_id {
        return;
    }
    match &st.suppliers.last {
        Some(Err(e)) => {
            ui.label(RichText::new(e).color(pal::BAD));
        }
        Some(Ok(list)) => {
            Grid::new(("suppliers", d.market_id)).num_columns(5).striped(true).spacing([12.0, 3.0]).show(ui, |ui| {
                for h in ["Commodity", "Nearest seller", "Distance", "Price / stock", "Data age"] {
                    ui.label(RichText::new(h).color(pal::DIM));
                }
                ui.end_row();
                for s in list {
                    ui.label(&s.name);
                    match s.hits.first() {
                        Some(h) => {
                            ui.horizontal(|ui| {
                                if h.is_carrier {
                                    ui.label(RichText::new(icon::ANCHOR).color(pal::CYAN)).on_hover_text("Fleet carrier");
                                }
                                ui.label(&h.station);
                                if ui.link(RichText::new(&h.system).color(pal::ACCENT_SOFT)).on_hover_text("Click to copy the system name").clicked() {
                                    ui.ctx().copy_text(h.system.clone());
                                }
                            })
                            .response
                            .on_hover_text(s.hits.iter().skip(1).map(|o| format!("{} — {} ({:.1} ly)", o.station, o.system, o.distance_ly.unwrap_or(0.0))).collect::<Vec<_>>().join("\n"));
                            ui.label(RichText::new(h.distance_ly.map(|d| format!("{d:.1} ly")).unwrap_or_default()).color(pal::DIM));
                            ui.label(RichText::new(format!("{} Cr · {} t", nexus_data::names::group_digits(h.price), nexus_data::names::group_digits(h.quantity))).color(pal::GOLD));
                            ui.label(RichText::new(crate::widgets::ago(nexus_journal::Timestamp(h.updated_unix))).color(pal::FAINT));
                        }
                        None => {
                            ui.label(RichText::new("no known seller with enough stock").color(pal::FAINT));
                            ui.label("");
                            ui.label("");
                            ui.label("");
                        }
                    }
                    ui.end_row();
                }
            });
            if list.iter().all(|s| s.hits.is_empty()) {
                ui.label(RichText::new("The market database has no sellers yet: turn on the live feed or run the Spansh import (Market → Database & feed).").small().color(pal::DIM));
            }
        }
        None => {}
    }
}
