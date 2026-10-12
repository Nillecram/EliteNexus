use crate::theme::{display, pal};
use crate::widgets::{badge, bar, card, credits, duration, kv, short_credits};
use egui::{Grid, RichText, Ui};
use egui_phosphor::regular as icon;
use nexus_data::ranks::RankKind;
use nexus_journal::Timestamp;
use nexus_model::GameState;

pub fn show(ui: &mut Ui, gs: &GameState) {
    ui.columns(3, |c| {
        commander(&mut c[0], gs);
        ship(&mut c[1], gs);
        location(&mut c[2], gs);
    });
    ui.add_space(10.0);
    ui.columns(3, |c| {
        session(&mut c[0], gs);
        system(&mut c[1], gs);
        holdings(&mut c[2], gs);
    });
    ui.add_space(10.0);
    feed(ui, gs);
}

fn commander(ui: &mut Ui, gs: &GameState) {
    let c = &gs.cmdr;
    card(ui, "Commander", |ui| {
        ui.label(RichText::new(format!("CMDR {}", c.name)).font(display(24.0)).color(pal::TEXT));
        ui.horizontal(|ui| {
            ui.label(RichText::new(credits(c.credits)).font(display(18.0)).color(pal::GOLD));
            if let Some(l) = &gs.legal_state {
                let col = if l == "Clean" { pal::GOOD } else { pal::BAD };
                badge(ui, l, col);
            }
            if !c.game_mode.is_empty() {
                badge(ui, &c.game_mode, pal::CYAN);
            }
        });
        ui.add_space(6.0);
        Grid::new("ranks").num_columns(3).spacing([10.0, 3.0]).show(ui, |ui| {
            for k in [RankKind::Combat, RankKind::Trade, RankKind::Explore, RankKind::Soldier, RankKind::Exobiologist, RankKind::Empire, RankKind::Federation] {
                ui.label(RichText::new(k.label()).color(pal::DIM));
                ui.label(RichText::new(c.rank_name(k)).color(pal::TEXT));
                let p = c.progress.get(k) as f32 / 100.0;
                ui.scope(|ui| {
                    ui.set_width(90.0);
                    bar(ui, p, pal::ACCENT, 6.0, None).on_hover_text(format!("{:.0}% to next rank", p * 100.0));
                });
                ui.end_row();
            }
        });
        ui.add_space(4.0);
        let unlocked = c.unlocked_engineers().count();
        ui.label(RichText::new(format!("{} {} engineers unlocked", icon::WRENCH, unlocked)).color(pal::DIM));
        if let Some(sq) = &c.squadron {
            ui.label(RichText::new(format!("{} Squadron: {}", icon::USERS_THREE, sq.name)).color(pal::DIM));
        } else if !c.squadron_applications.is_empty() {
            ui.label(RichText::new(format!("{} Applied: {}", icon::USERS_THREE, c.squadron_applications.join(", "))).color(pal::DIM));
        }
        if let Some(pp) = &c.powerplay {
            ui.label(RichText::new(format!("{} {} · rank {} · {} merits", icon::CROWN, pp.power, pp.rank, pp.merits)).color(pal::PURPLE));
        }
    });
}

fn ship(ui: &mut Ui, gs: &GameState) {
    card(ui, "Ship", |ui| {
        let Some(s) = gs.fleet.current() else {
            ui.label(RichText::new("No ship information yet").color(pal::DIM));
            return;
        };
        let name = if s.name.trim().is_empty() { s.type_name.clone() } else { s.name.clone() };
        ui.label(RichText::new(name).font(display(22.0)).color(pal::TEXT));
        ui.label(RichText::new(format!("{} · {}", s.type_name, s.ident)).color(pal::DIM));
        ui.add_space(6.0);
        let fuel_cap = s.fuel_main_capacity.max(0.001);
        let fuel = gs.fleet.fuel_main;
        let fuel_frac = (fuel / fuel_cap) as f32;
        let fuel_col = if fuel_frac < 0.25 { pal::BAD } else if fuel_frac < 0.5 { pal::WARN } else { pal::CYAN };
        Grid::new("shipgrid").num_columns(2).spacing([10.0, 5.0]).show(ui, |ui| {
            ui.label(RichText::new(format!("{} Fuel", icon::GAS_PUMP)).color(pal::DIM));
            bar(ui, fuel_frac, fuel_col, 14.0, Some(&format!("{:.1} / {:.0} t", fuel, s.fuel_main_capacity)));
            ui.end_row();
            ui.label(RichText::new(format!("{} Hull", icon::SHIELD)).color(pal::DIM));
            let hc = if s.hull_health < 0.5 { pal::BAD } else { pal::GOOD };
            bar(ui, s.hull_health as f32, hc, 14.0, Some(&format!("{:.0}%", s.hull_health * 100.0)));
            ui.end_row();
            let used = gs.inventory.cargo.total();
            ui.label(RichText::new(format!("{} Cargo", icon::PACKAGE)).color(pal::DIM));
            let cap = s.cargo_capacity.max(1) as f32;
            bar(ui, used as f32 / cap, pal::ACCENT, 14.0, Some(&format!("{} / {} t", used, s.cargo_capacity)));
            ui.end_row();
        });
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            crate::widgets::stat(ui, "max jump", format!("{:.1} ly", s.max_jump_range), pal::ACCENT);
            ui.add_space(14.0);
            crate::widgets::stat(ui, "rebuy", short_credits(s.rebuy), pal::TEXT);
            ui.add_space(14.0);
            crate::widgets::stat(ui, "value", short_credits(s.total_value()), pal::TEXT);
        });
        ui.label(RichText::new(format!("{} engineered modules", s.engineered_modules())).small().color(pal::DIM));
    });
}

fn location(ui: &mut Ui, gs: &GameState) {
    let l = &gs.location;
    card(ui, "Location", |ui| {
        ui.label(RichText::new(if l.system.is_empty() { "Unknown" } else { &l.system }).font(display(22.0)).color(pal::TEXT));
        ui.label(RichText::new(l.situation()).color(pal::ACCENT_SOFT));
        if let Some(b) = &l.body
            && !b.is_empty() && l.station.as_ref().is_none_or(|s| !l.docked || s.name != *b) {
                ui.label(RichText::new(format!("{} {}", icon::PLANET, b)).color(pal::DIM));
            }
        ui.add_space(4.0);
        let i = &l.info;
        Grid::new("locgrid").num_columns(2).spacing([10.0, 3.0]).show(ui, |ui| {
            if i.population > 0 {
                kv(ui, "Population", nexus_data::names::compact_credits(i.population as i64));
                kv(ui, "Allegiance", i.allegiance.clone());
                kv(ui, "Government", i.government.clone());
                kv(ui, "Economy", if i.second_economy.is_empty() || i.second_economy == "None" { i.economy.clone() } else { format!("{} / {}", i.economy, i.second_economy) });
                kv(ui, "Security", i.security.clone());
                if let Some(f) = &i.controlling_faction {
                    kv(ui, "Controlled by", f.clone());
                }
            } else if !l.system.is_empty() {
                kv(ui, "Population", "Unpopulated");
            }
            if let Some(p) = &i.controlling_power {
                kv(ui, "Power", format!("{} ({})", p, i.powerplay_state.clone().unwrap_or_default()));
            }
            if let Some(pos) = l.star_pos {
                kv(ui, "Coordinates", format!("{:.2}, {:.2}, {:.2}", pos[0], pos[1], pos[2]));
                kv(ui, "From Sol", format!("{:.1} ly", nexus_model::location::distance(pos, [0.0, 0.0, 0.0])));
            }
        });
        if let Some(st) = l.station.as_ref().filter(|_| l.docked) {
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                badge(ui, &st.station_type, pal::CYAN);
                if let Some(p) = &st.pads {
                    badge(ui, &format!("Pads L{} M{} S{}", p.large, p.medium, p.small), pal::DIM);
                }
                for (svc, label) in [("shipyard", "Shipyard"), ("outfitting", "Outfitting"), ("commodities", "Market"), ("engineer", "Engineer"), ("vistagenomics", "Vista Genomics"), ("exploration", "Cartographics")] {
                    if st.has_service(svc) {
                        badge(ui, label, pal::GOOD);
                    }
                }
            });
        }
        if let Some(t) = &l.fsd_target {
            ui.add_space(4.0);
            ui.label(RichText::new(format!("{} Target: {} ({}){}", icon::CROSSHAIR, t.name, t.star_class, t.remaining_jumps.map(|j| format!(" · {j} jumps left")).unwrap_or_default())).color(pal::CYAN));
        }
        if !gs.nav_route.is_empty() {
            let dest = gs.nav_route.last().map(|h| h.star_system.clone()).unwrap_or_default();
            ui.label(RichText::new(format!("{} Route: {} jumps → {}", icon::PATH, gs.nav_route.len(), dest)).color(pal::DIM));
        }
    });
}

fn session(ui: &mut Ui, gs: &GameState) {
    let s = &gs.session;
    card(ui, "Session", |ui| {
        let now = Timestamp::now();
        let dur = if s.active() { s.duration_secs(now) } else { s.duration_secs(s.end.unwrap_or(gs.last_event)) };
        ui.horizontal(|ui| {
            crate::widgets::stat(ui, "time", duration(dur), pal::TEXT);
            ui.add_space(12.0);
            crate::widgets::stat(ui, "jumps", s.jumps.to_string(), pal::TEXT);
            ui.add_space(12.0);
            crate::widgets::stat(ui, "light years", format!("{:.0}", s.distance_ly), pal::TEXT);
        });
        ui.add_space(6.0);
        Grid::new("sessgrid").num_columns(2).spacing([10.0, 3.0]).show(ui, |ui| {
            kv(ui, "Bodies scanned / mapped", format!("{} / {}", s.bodies_scanned, s.bodies_mapped));
            kv(ui, "Organics analysed", s.organics_analysed.to_string());
            kv(ui, "Missions completed", s.missions_completed.to_string());
            kv(ui, "Income", RichText::new(short_credits(s.income)).color(pal::GOOD));
            kv(ui, "Spent", RichText::new(short_credits(-s.expenses)).color(pal::BAD));
            let delta = gs.cmdr.credits - s.start_credits;
            kv(ui, "Balance change", RichText::new(short_credits(delta)).color(if delta >= 0 { pal::GOOD } else { pal::BAD }));
        });
    });
}

fn system(ui: &mut Ui, gs: &GameState) {
    card(ui, "Current system", |ui| {
        let Some(sys) = gs.exploration.current_system() else {
            ui.label(RichText::new("No scan data for this system").color(pal::DIM));
            return;
        };
        let scanned = sys.scanned_bodies();
        let total = sys.fss_body_count.unwrap_or(0) as usize;
        ui.horizontal(|ui| {
            crate::widgets::stat(ui, "bodies scanned", if total > 0 { format!("{scanned}/{total}") } else { scanned.to_string() }, pal::TEXT);
            ui.add_space(12.0);
            crate::widgets::stat(ui, "value now", short_credits(sys.value()), pal::GOLD);
            ui.add_space(12.0);
            crate::widgets::stat(ui, "if mapped", short_credits(sys.potential_value()), pal::ACCENT);
        });
        if total > 0 {
            bar(ui, scanned as f32 / total as f32, pal::CYAN, 8.0, None);
        }
        let bio = sys.bio_signal_total();
        if bio > 0 {
            ui.label(RichText::new(format!("{} {} biological signals", icon::PLANT, bio)).color(pal::GOOD));
        }
        let mut notable: Vec<_> = sys.bodies.values().filter(|b| b.has_scan).collect();
        notable.sort_by_key(|b| std::cmp::Reverse(b.value.mapped));
        for b in notable.iter().take(4) {
            let tags = nexus_model::exploration::interesting(b, sys);
            if tags.is_empty() && b.value.mapped < 300_000 {
                continue;
            }
            ui.horizontal(|ui| {
                ui.label(RichText::new(b.short_name(&sys.name)).color(pal::TEXT));
                ui.label(RichText::new(short_credits(b.value.mapped)).color(pal::GOLD));
                for t in tags.iter().take(2) {
                    badge(ui, t.label(), pal::ACCENT);
                }
            });
        }
        let unsold: i64 = gs.exploration.unsold_value();
        ui.add_space(4.0);
        ui.label(RichText::new(format!("Unsold cartographic data: {}", short_credits(unsold))).color(pal::DIM));
        ui.label(RichText::new(format!("Unsold exobiology: {}", short_credits(gs.exobio.unsold_value()))).color(pal::DIM));
    });
}

fn holdings(ui: &mut Ui, gs: &GameState) {
    card(ui, "Holdings", |ui| {
        Grid::new("holdgrid").num_columns(2).spacing([10.0, 3.0]).show(ui, |ui| {
            kv(ui, "Ships owned", format!("{} ({})", gs.fleet.ships.len(), short_credits(gs.fleet.total_value())));
            let m = &gs.inventory.materials;
            kv(ui, "Materials", format!("{} raw · {} mfd · {} enc", m.total(nexus_model::inventory::MaterialCategory::Raw), m.total(nexus_model::inventory::MaterialCategory::Manufactured), m.total(nexus_model::inventory::MaterialCategory::Encoded)));
            kv(ui, "Ship locker", format!("{} items", gs.inventory.ship_locker.total()));
            kv(ui, "Active missions", gs.missions.active.len().to_string());
        });
        let c = &gs.carrier;
        if c.owned() {
            ui.add_space(6.0);
            ui.label(RichText::new(format!("{} {} {}", icon::ANCHOR, c.name, c.callsign)).font(display(16.0)).color(pal::TEXT));
            ui.label(RichText::new(format!("{} · {} t tritium · {}", c.system.clone().unwrap_or_else(|| "?".into()), c.fuel, short_credits(c.finance.carrier_balance))).color(pal::DIM));
            if let Some(j) = &c.pending_jump {
                let when = j.departure.map(|d| d.to_local_string("%H:%M")).unwrap_or_default();
                ui.label(RichText::new(format!("{} Jumping to {} at {}", icon::ARROW_FAT_RIGHT, j.system, when)).color(pal::WARN));
            }
        }
        for d in gs.colonisation.active_depots().take(2) {
            ui.add_space(4.0);
            ui.label(RichText::new(format!("{} {} — {:.0}%", icon::BUILDINGS, d.station.clone().unwrap_or_else(|| d.system.clone()), d.progress * 100.0)).color(pal::TEXT));
            bar(ui, d.progress as f32, pal::GOOD, 6.0, None);
        }
    });
}

fn feed(ui: &mut Ui, gs: &GameState) {
    card(ui, "Activity", |ui| {
        if gs.feed.is_empty() {
            ui.label(RichText::new("Nothing yet — events appear here as you play.").color(pal::DIM));
            return;
        }
        egui::ScrollArea::vertical().max_height(260.0).auto_shrink([false, true]).show(ui, |ui| {
            Grid::new("feedgrid").num_columns(3).spacing([12.0, 4.0]).striped(true).show(ui, |ui| {
                for f in gs.feed.iter().rev().take(60) {
                    ui.label(RichText::new(f.ts.to_local_string("%d %b %H:%M")).monospace().color(pal::FAINT));
                    ui.label(RichText::new(&f.event).color(pal::ACCENT_SOFT));
                    ui.label(RichText::new(&f.text).color(pal::TEXT));
                    ui.end_row();
                }
            });
        });
    });
}
