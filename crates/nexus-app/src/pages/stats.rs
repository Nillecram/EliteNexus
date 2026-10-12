use crate::theme::{display, pal, size};
use crate::widgets::{card, short_credits};
use egui::{Grid, RichText, Ui};
use egui_plot::{Bar, BarChart, Line, Plot, PlotPoints};
use nexus_journal::Timestamp;
use nexus_model::GameState;

pub fn show(ui: &mut Ui, gs: &GameState) {
    crate::widgets::cards(ui, 2, 360.0, |c| {
        c.add(|ui| {
            card(ui, "Credits over time", |ui| {
            let pts: Vec<[f64; 2]> = gs.history.credit_samples.iter().map(|(t, v)| [t.0 as f64 / 86_400.0, *v as f64 / 1e6]).collect();
            if pts.len() < 2 {
                ui.label(RichText::new("Not enough data yet.").color(pal::DIM));
            } else {
                Plot::new("credits")
                    .show_background(false)
                    .show_grid([false, true])
                    .height(240.0)
                    .allow_scroll(false)
                    .x_axis_formatter(|m, _| Timestamp((m.value * 86_400.0) as i64).to_local_string("%d %b"))
                    .y_axis_label("M Cr")
                    .label_formatter(|h| { let (x, y) = crate::widgets::hover_xy(h); Some(format!("{}\n{:.1} M Cr", Timestamp((x * 86_400.0) as i64).to_local_string("%d %b %H:%M"), y)) })
                    .show(ui, |p| {
                        p.line(Line::new("credits", PlotPoints::from(pts)).color(pal::GOLD).width(2.0));
                    });
            }
        });
        });
        c.add(|ui| {
            card(ui, "Income & spending by category", |ui| {
            let totals = gs.history.totals_since(Timestamp::ZERO);
            let bars: Vec<Bar> = totals
                .iter()
                .enumerate()
                .flat_map(|(i, (cat, inc, exp))| {
                    [
                        Bar::new(i as f64 - 0.2, *inc as f64 / 1e6).width(0.38).fill(pal::GOOD).name(format!("{} income", cat.label())),
                        Bar::new(i as f64 + 0.2, *exp as f64 / 1e6).width(0.38).fill(pal::BAD).name(format!("{} spent", cat.label())),
                    ]
                })
                .collect();
            let labels: Vec<String> = totals.iter().map(|(c, _, _)| c.label().to_string()).collect();
            Plot::new("ledger")
                    .show_background(false)
                    .show_grid([false, true])
                .height(240.0)
                .allow_scroll(false)
                .y_axis_label("M Cr")
                .x_axis_formatter(move |m, _| {
                    let i = m.value.round();
                    if (m.value - i).abs() < 0.01 && i >= 0.0 { labels.get(i as usize).cloned().unwrap_or_default() } else { String::new() }
                })
                .show(ui, |p| p.bar_chart(BarChart::new("ledger", bars)));
        });
        });
    });
    ui.add_space(10.0);
    crate::widgets::cards(ui, 2, 360.0, |c| {
        c.add(|ui| {
            card(ui, "Jumps per day", |ui| {
            let mut per_day: std::collections::BTreeMap<i64, (u32, f64)> = Default::default();
            for j in &gs.history.jumps {
                let e = per_day.entry(j.ts.0 / 86_400).or_default();
                e.0 += 1;
                e.1 += j.dist;
            }
            let bars: Vec<Bar> = per_day.iter().map(|(d, (n, ly))| Bar::new(*d as f64, *n as f64).width(0.8).fill(pal::CYAN).name(format!("{n} jumps, {ly:.0} ly"))).collect();
            Plot::new("jumpsday")
                    .show_background(false)
                    .show_grid([false, true])
                .height(200.0)
                .allow_scroll(false)
                .x_axis_formatter(|m, _| Timestamp((m.value * 86_400.0) as i64).to_local_string("%d %b"))
                .show(ui, |p| p.bar_chart(BarChart::new("jumps", bars)));
        });
        });
        c.add(|ui| {
            card(ui, "Totals", |ui| {
            let h = &gs.history;
            Grid::new("totals").max_col_width(ui.available_width()).num_columns(2).spacing([12.0, 4.0]).striped(true).show(ui, |ui| {
                for (cat, inc, exp) in h.totals_since(Timestamp::ZERO) {
                    ui.label(RichText::new(cat.label()).color(pal::DIM));
                    ui.horizontal(|ui| {
                        if inc != 0 {
                            ui.label(RichText::new(format!("+{}", short_credits(inc))).font(display(14.0)).color(pal::GOOD));
                        }
                        if exp != 0 {
                            ui.label(RichText::new(short_credits(exp)).font(display(14.0)).color(pal::BAD));
                        }
                    });
                    ui.end_row();
                }
                ui.label(RichText::new("Deaths").color(pal::DIM));
                ui.label(h.deaths.len().to_string());
                ui.end_row();
            });
        });
        });
    });
    if let Some(stats) = &gs.cmdr.statistics {
        ui.add_space(10.0);
        card(ui, "In-game statistics (last Statistics event)", |ui| {
            crate::widgets::cards(ui, 3, 260.0, |c| {
                let sections: [(&str, &serde_json::Value); 9] = [
                    ("Bank account", &stats.bank_account),
                    ("Combat", &stats.combat),
                    ("Exploration", &stats.exploration),
                    ("Trading", &stats.trading),
                    ("Mining", &stats.mining),
                    ("Exobiology", &stats.exobiology),
                    ("Crafting", &stats.crafting),
                    ("Passengers", &stats.passengers),
                    ("Fleet carrier", &stats.fleetcarrier),
                ];
                for (i, (title, v)) in sections.iter().enumerate() {
                    c.add(|ui| {
                    egui::CollapsingHeader::new(RichText::new(*title).font(display(size::CALLOUT)).color(pal::TEXT)).id_salt(("stat", i)).show(ui, |ui| {
                        if let Some(obj) = v.as_object() {
                            Grid::new(("statgrid", i)).max_col_width(ui.available_width()).num_columns(2).striped(true).show(ui, |ui| {
                                for (k, val) in obj {
                                    ui.label(RichText::new(k.replace('_', " ")).small().color(pal::DIM));
                                    let txt = match val {
                                        serde_json::Value::Number(n) => n.as_i64().map(nexus_data::names::group_digits).unwrap_or_else(|| n.to_string()),
                                        other => other.to_string(),
                                    };
                                    ui.label(RichText::new(txt).small());
                                    ui.end_row();
                                }
                            });
                        }
                    });
                    });
                }
            });
        });
    }
}
