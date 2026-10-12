use crate::finder::{self, Supplier};
use crate::jobs::Slot;
use crate::market::MarketService;
use crate::theme::{display, pal};
use crate::widgets::{badge, bar, card, empty, short_credits};
use egui::{Grid, RichText, Ui};
use egui_phosphor::regular as icon;
use nexus_model::GameState;

#[derive(Default)]
pub struct ColonisationUi {
    /// Supplier search results and the depot (market id) they are for.
    suppliers: Slot<Result<Vec<Supplier>, String>>,
    for_depot: u64,
}

pub fn show(ui: &mut Ui, gs: &GameState, market: &MarketService, st: &mut ColonisationUi) {
    st.suppliers.tick();
    let col = &gs.colonisation;
    if col.depots.is_empty() && col.claims.is_empty() {
        empty(ui, icon::BUILDINGS, "No colonisation activity", "Dock at a construction site and its shopping list appears here automatically.");
        return;
    }
    let cargo = &gs.inventory.cargo;
    let mut depots: Vec<_> = col.depots.values().collect();
    depots.sort_by(|a, b| (a.complete || a.failed).cmp(&(b.complete || b.failed)).then(b.updated.cmp(&a.updated)));
    for d in depots {
        let title = format!("{} — {}", d.station.clone().unwrap_or_else(|| "Construction site".into()), d.system);
        card(ui, &title, |ui| {
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
                }
            });
            bar(ui, d.progress as f32, pal::GOOD, 10.0, None);
            ui.label(RichText::new(format!("Updated {}", crate::widgets::ago(d.updated))).small().color(pal::DIM));
            ui.add_space(4.0);
            let mut res: Vec<_> = d.resources.iter().filter(|r| r.remaining() > 0).collect();
            res.sort_by_key(|r| std::cmp::Reverse(r.remaining()));
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
            if !d.my_contributions.is_empty() {
                let total: i64 = d.my_contributions.values().sum();
                ui.label(RichText::new(format!("You delivered {} t here", nexus_data::names::group_digits(total))).color(pal::GOOD));
            }
            if !(d.complete || d.failed) {
                ui.add_space(6.0);
                suppliers_section(ui, gs, d, market, st);
            }
        });
        ui.add_space(8.0);
    }
    if !col.claims.is_empty() {
        card(ui, "Claims", |ui| {
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
}

/// "Where do I buy this?": the nearest stations with stock for each commodity still needed,
/// searched around the construction site (from the local market database).
fn suppliers_section(ui: &mut Ui, gs: &GameState, d: &nexus_model::colonisation::Depot, market: &MarketService, st: &mut ColonisationUi) {
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
