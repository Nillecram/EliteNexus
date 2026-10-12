//! BGS contributions per system/faction and Powerplay merits (OD Elite Tracker style).

use crate::theme::{display, pal};
use crate::widgets::{card, empty, short_credits};
use egui::{Grid, RichText, Ui};
use egui_phosphor::regular as icon;
use egui_plot::{Bar, BarChart, Plot};
use nexus_journal::Timestamp;
use nexus_model::bgs::{last_tick, BgsKind};
use nexus_model::GameState;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Period {
    #[default]
    SinceTick,
    Day,
    Week,
    All,
}

pub struct BgsUi {
    pub period: Period,
    pub tick_hour_utc: u32,
}

impl Default for BgsUi {
    fn default() -> Self {
        BgsUi { period: Period::SinceTick, tick_hour_utc: 13 }
    }
}

const COLUMNS: [BgsKind; 9] = [
    BgsKind::MissionInfluence,
    BgsKind::Bounty,
    BgsKind::CombatBond,
    BgsKind::Trade,
    BgsKind::Exploration,
    BgsKind::Exobiology,
    BgsKind::BlackMarket,
    BgsKind::MissionFailed,
    BgsKind::Murder,
];

pub fn show(ui: &mut Ui, gs: &GameState, st: &mut BgsUi) {
    let now = Timestamp::now();
    ui.horizontal(|ui| {
        crate::widgets::segmented(ui, &mut st.period, &[(Period::SinceTick, "Since last tick"), (Period::Day, "Last 24 h"), (Period::Week, "7 days"), (Period::All, "All time")]);
        if st.period == Period::SinceTick {
            ui.label(RichText::new("tick at").color(pal::DIM));
            ui.add(egui::DragValue::new(&mut st.tick_hour_utc).range(0..=23).suffix(":00 UTC"));
        }
    });
    let since = match st.period {
        Period::SinceTick => last_tick(now, st.tick_hour_utc),
        Period::Day => Timestamp(now.0 - 86_400),
        Period::Week => Timestamp(now.0 - 7 * 86_400),
        Period::All => Timestamp::ZERO,
    };
    ui.label(RichText::new(format!("Counting from {}", since.to_local_string("%d %b %H:%M"))).small().color(pal::DIM));
    ui.add_space(6.0);

    powerplay(ui, gs, since);
    ui.add_space(8.0);

    let summary = gs.bgs.summary(since);
    if summary.is_empty() {
        empty(ui, icon::SCALES, "No BGS activity in this period", "Missions, bounties, bonds, trade and data sales are attributed to the factions they affect.");
        return;
    }
    for sys in &summary {
        card(ui, &sys.system, |ui| {
            Grid::new(("bgs", &sys.system)).num_columns(COLUMNS.len() + 1).striped(true).spacing([14.0, 4.0]).show(ui, |ui| {
                ui.label(RichText::new("Faction").color(pal::DIM));
                for k in COLUMNS {
                    ui.label(RichText::new(k.label()).small().color(pal::DIM));
                }
                ui.end_row();
                for f in &sys.factions {
                    ui.label(RichText::new(&f.faction).color(pal::TEXT));
                    for k in COLUMNS {
                        match f.kinds.iter().find(|(kk, _, _)| *kk == k) {
                            Some((_, amount, n)) => {
                                let text = match k {
                                    BgsKind::MissionInfluence => format!("{}{} ({n})", if *amount >= 0 { "+" } else { "" }, amount),
                                    BgsKind::MissionFailed | BgsKind::Murder | BgsKind::Crime => n.to_string(),
                                    _ => short_credits(*amount),
                                };
                                let col = if k.positive() { pal::GOOD } else { pal::BAD };
                                ui.label(RichText::new(text).font(display(13.0)).color(col));
                            }
                            None => {
                                ui.label(RichText::new("·").color(pal::FAINT));
                            }
                        }
                    }
                    ui.end_row();
                }
            });
        });
        ui.add_space(6.0);
    }
}

fn powerplay(ui: &mut Ui, gs: &GameState, since: Timestamp) {
    card(ui, "Powerplay", |ui| {
        let Some(pp) = &gs.cmdr.powerplay else {
            ui.label(RichText::new("Not pledged to a power.").color(pal::DIM));
            return;
        };
        ui.horizontal(|ui| {
            crate::widgets::stat(ui, "power", pp.power.clone(), pal::PURPLE);
            ui.add_space(16.0);
            crate::widgets::stat(ui, "rank", pp.rank.to_string(), pal::TEXT);
            ui.add_space(16.0);
            crate::widgets::stat(ui, "total merits", nexus_data::names::group_digits(pp.merits), pal::GOLD);
            ui.add_space(16.0);
            crate::widgets::stat(ui, "merits this period", nexus_data::names::group_digits(gs.bgs.merits_since(since)), pal::ACCENT);
            ui.add_space(16.0);
            crate::widgets::stat(ui, "pledged", crate::widgets::duration(pp.time_pledged_secs), pal::DIM);
        });
        if gs.bgs.merits.len() > 1 {
            let mut per_day: std::collections::BTreeMap<i64, i64> = Default::default();
            for (t, g, _) in &gs.bgs.merits {
                *per_day.entry(t.0 / 86_400).or_default() += g;
            }
            let bars: Vec<Bar> = per_day.iter().map(|(d, m)| Bar::new(*d as f64, *m as f64).width(0.8).fill(pal::PURPLE)).collect();
            Plot::new("merits")
                    .show_background(false)
                    .show_grid([false, true])
                .height(140.0)
                .allow_scroll(false)
                .x_axis_formatter(|m, _| Timestamp((m.value * 86_400.0) as i64).to_local_string("%d %b"))
                .show(ui, |p| p.bar_chart(BarChart::new("merits", bars)));
        }
    });
}
