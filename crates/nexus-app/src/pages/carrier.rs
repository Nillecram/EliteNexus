use crate::theme::{display, pal};
use crate::widgets::{badge, bar, card, empty, kv, short_credits};
use egui::{Grid, RichText, Ui};
use egui_phosphor::regular as icon;
use nexus_journal::Timestamp;
use nexus_model::GameState;

pub fn show(ui: &mut Ui, gs: &GameState) {
    let c = &gs.carrier;
    if !c.owned() {
        empty(ui, icon::ANCHOR, "No fleet carrier", "Carrier data appears when the game writes CarrierStats (open the carrier management screen).");
        return;
    }
    ui.columns(3, |col| {
        card(&mut col[0], "Carrier", |ui| {
            ui.label(RichText::new(&c.name).font(display(24.0)));
            ui.label(RichText::new(&c.callsign).font(display(16.0)).color(pal::ACCENT));
            ui.add_space(4.0);
            ui.label(RichText::new(format!("{} {}", icon::MAP_PIN, c.system.clone().unwrap_or_else(|| "Unknown".into()))).color(pal::TEXT));
            if let Some(j) = &c.pending_jump {
                let now = Timestamp::now();
                let when = j.departure.map(|d| {
                    let left = d.0 - now.0;
                    if left > 0 { format!("in {}", crate::widgets::duration(left)) } else { "now".into() }
                });
                ui.label(RichText::new(format!("{} Jump to {} {}", icon::ARROW_FAT_RIGHT, j.system, when.unwrap_or_default())).color(pal::WARN));
            }
            ui.horizontal(|ui| {
                badge(ui, &format!("Docking: {}", c.docking_access), pal::CYAN);
                if c.allow_notorious {
                    badge(ui, "Notorious allowed", pal::WARN);
                }
                if c.pending_decommission {
                    badge(ui, "DECOMMISSIONING", pal::BAD);
                }
            });
        });
        card(&mut col[1], "Tritium", |ui| {
            ui.label(RichText::new(format!("{} t", c.fuel)).font(display(24.0)).color(pal::CYAN));
            bar(ui, c.fuel as f32 / 1000.0, pal::CYAN, 10.0, None);
            ui.label(RichText::new(format!("Jump range {:.0} / {:.0} ly", c.jump_range_curr, c.jump_range_max)).color(pal::DIM));
            ui.label(RichText::new("Use Navigation → Carrier route for exact tritium per jump (Spansh).").small().color(pal::FAINT));
        });
        card(&mut col[2], "Finance", |ui| {
            ui.label(RichText::new(short_credits(c.finance.carrier_balance)).font(display(24.0)).color(pal::GOLD));
            Grid::new("cfin").num_columns(2).spacing([10.0, 3.0]).show(ui, |ui| {
                kv(ui, "Available", short_credits(c.finance.available_balance));
                kv(ui, "Reserved", short_credits(c.finance.reserve_balance));
                if let Some(p) = c.finance.reserve_percent {
                    kv(ui, "Reserve %", format!("{p:.0}%"));
                }
            });
        });
    });
    ui.add_space(8.0);
    ui.columns(2, |col| {
        card(&mut col[0], "Capacity", |ui| {
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
        card(&mut col[1], "Crew & services", |ui| {
            Grid::new("ccrew").num_columns(3).striped(true).spacing([10.0, 3.0]).show(ui, |ui| {
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
    ui.add_space(8.0);
    ui.columns(2, |col| {
        card(&mut col[0], "Market orders", |ui| {
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
        card(&mut col[1], "Jump history", |ui| {
            if c.jumps.is_empty() {
                ui.label(RichText::new("No jumps recorded yet.").color(pal::DIM));
            }
            Grid::new("cjumps").num_columns(2).striped(true).show(ui, |ui| {
                for j in c.jumps.iter().rev().take(30) {
                    ui.label(RichText::new(j.ts.to_local_string("%d %b %H:%M")).color(pal::FAINT));
                    ui.label(&j.system);
                    ui.end_row();
                }
            });
        });
    });
}
