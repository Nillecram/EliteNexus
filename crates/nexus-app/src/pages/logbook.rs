use crate::jobs::Slot;
use crate::theme::{display, pal};
use crate::widgets::{card, short_credits};
use egui::{RichText, Sense, Ui};
use egui_extras::{Column, TableBuilder};
use egui_phosphor::regular as icon;
use nexus_journal::{JournalIndex, Timestamp};
use nexus_model::GameState;
use parking_lot::RwLock;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    Activity,
    Journal,
    Jumps,
    Docks,
    Ledger,
}

#[derive(Default)]
pub struct LogbookUi {
    pub tab: Tab,
    pub filter: String,
    pub event: Option<u16>,
    pub selected: Option<usize>,
    pub detail: Option<(usize, String)>,
    pub search: String,
    /// Entry indices matching the full-text search.
    pub search_hits: Slot<Vec<usize>>,
}

pub fn show(ui: &mut Ui, gs: &GameState, st: &mut LogbookUi, index: &Arc<RwLock<JournalIndex>>) {
    st.search_hits.tick();
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(12.0, 10.0);
        if let Some(t) = crate::widgets::tabs(ui, st.tab, &[(Tab::Activity, "Activity"), (Tab::Journal, "Every event"), (Tab::Jumps, "Travel"), (Tab::Docks, "Docking"), (Tab::Ledger, "Credits ledger")]) {
            st.tab = t;
        }
        if st.tab != Tab::Journal {
            crate::widgets::search_field(ui, &mut st.filter, "Filter", 240.0);
        }
    });
    ui.add_space(crate::widgets::GAP);
    let f = st.filter.to_ascii_lowercase();
    let matches = |s: &str| f.is_empty() || s.to_ascii_lowercase().contains(&f);
    match st.tab {
        Tab::Activity => {
            let rows: Vec<_> = gs.feed.iter().rev().filter(|x| matches(&x.text) || matches(&x.event)).collect();
            table(ui, &["When", "Event", "Details"], rows.len(), |i, col, ui| {
                let r = rows[i];
                match col {
                    0 => ui.label(RichText::new(r.ts.to_local_string("%d %b %H:%M:%S")).color(pal::FAINT)),
                    1 => ui.label(RichText::new(&r.event).color(pal::ACCENT_SOFT)),
                    _ => ui.label(&r.text),
                };
            });
        }
        Tab::Journal => journal(ui, st, index),
        Tab::Jumps => {
            let h = &gs.history;
            card(ui, "", |ui| {
                ui.horizontal(|ui| {
                    crate::widgets::stat(ui, "jumps", h.jumps.len().to_string(), pal::TEXT);
                    ui.add_space(16.0);
                    crate::widgets::stat(ui, "distance", format!("{:.0} ly", h.total_distance_ly), pal::ACCENT);
                    ui.add_space(16.0);
                    crate::widgets::stat(ui, "systems visited", h.visited.len().to_string(), pal::TEXT);
                    ui.add_space(16.0);
                    let far = h.visited.values().filter_map(|v| v.pos.map(|p| (v, nexus_model::location::distance(p, [0.0, 0.0, 0.0])))).max_by(|a, b| a.1.total_cmp(&b.1));
                    if let Some((v, d)) = far {
                        crate::widgets::stat(ui, &format!("farthest from Sol ({})", v.name), format!("{d:.0} ly"), pal::PURPLE);
                    }
                });
            });
            ui.add_space(6.0);
            let rows: Vec<_> = h.jumps.iter().rev().filter(|j| matches(&j.system)).collect();
            table(ui, &["When", "System", "Distance", "Fuel used"], rows.len(), |i, col, ui| {
                let j = rows[i];
                match col {
                    0 => ui.label(RichText::new(j.ts.to_local_string("%d %b %H:%M")).color(pal::FAINT)),
                    1 => ui.label(if j.carrier { format!("{} {}", icon::ANCHOR, j.system) } else { j.system.clone() }),
                    2 => ui.label(RichText::new(format!("{:.2} ly", j.dist)).color(pal::DIM)),
                    _ => ui.label(RichText::new(format!("{:.2} t", j.fuel_used)).color(pal::DIM)),
                };
            });
        }
        Tab::Docks => {
            let rows: Vec<_> = gs.history.docks.iter().rev().filter(|d| matches(&d.station) || matches(&d.system)).collect();
            table(ui, &["When", "Station", "Type", "System"], rows.len(), |i, col, ui| {
                let d = rows[i];
                match col {
                    0 => ui.label(RichText::new(d.ts.to_local_string("%d %b %H:%M")).color(pal::FAINT)),
                    1 => ui.label(&d.station),
                    2 => ui.label(RichText::new(&d.station_type).color(pal::DIM)),
                    _ => ui.label(&d.system),
                };
            });
        }
        Tab::Ledger => {
            let rows: Vec<_> = gs.history.ledger.iter().rev().filter(|e| matches(&e.note) || matches(e.category.label())).collect();
            table(ui, &["When", "Category", "Amount", "Note"], rows.len(), |i, col, ui| {
                let e = rows[i];
                match col {
                    0 => ui.label(RichText::new(e.ts.to_local_string("%d %b %H:%M")).color(pal::FAINT)),
                    1 => ui.label(RichText::new(e.category.label()).color(pal::ACCENT_SOFT)),
                    2 => ui.label(RichText::new(short_credits(e.amount)).color(if e.amount >= 0 { pal::GOOD } else { pal::BAD })),
                    _ => ui.label(&e.note),
                };
            });
        }
    }
}

fn journal(ui: &mut Ui, st: &mut LogbookUi, index: &Arc<RwLock<JournalIndex>>) {
    let idx = index.read();
    ui.horizontal(|ui| {
        let current = st.event.map(|id| idx.name(id).to_string()).unwrap_or_else(|| "All events".into());
        egui::ComboBox::from_id_salt("evfilter").width(240.0).selected_text(current).show_ui(ui, |ui| {
            if ui.selectable_label(st.event.is_none(), format!("All events ({})", idx.len())).clicked() {
                st.event = None;
                st.selected = None;
            }
            for (id, name, n) in idx.name_counts() {
                if ui.selectable_label(st.event == Some(id), format!("{name} ({n})")).clicked() {
                    st.event = Some(id);
                    st.selected = None;
                }
            }
        });
        let resp = crate::widgets::search_field(ui, &mut st.search, "Search every event: a system, station, CMDR…", 380.0);
        let go = crate::widgets::primary_button(ui, "Search").clicked() || (resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)));
        if go {
            st.selected = None;
            if st.search.trim().is_empty() {
                st.search_hits = Slot::default();
            } else {
                let needle = st.search.trim().to_ascii_lowercase();
                let index = index.clone();
                st.search_hits.run(ui.ctx(), move || full_text_search(&index, &needle));
            }
        }
        if st.search_hits.busy() {
            ui.spinner();
        }
    });
    // Rows: newest first, optionally filtered by event type and/or search hits.
    let mut rows: Vec<usize> = match &st.search_hits.last {
        Some(hits) if !st.search.trim().is_empty() => hits.iter().rev().copied().collect(),
        _ => (0..idx.len()).rev().collect(),
    };
    if let Some(id) = st.event {
        rows.retain(|&i| idx.entries[i].name == id);
    }
    ui.label(RichText::new(format!("{} events", rows.len())).small().color(pal::DIM));
    let detail_w = 460.0;
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width((ui.available_width() - detail_w - 12.0).max(300.0));
            TableBuilder::new(ui)
                .id_salt("journalgrid")
                .striped(true)
                .sense(Sense::click())
                .column(Column::initial(150.0))
                .column(Column::initial(230.0))
                .column(Column::remainder())
                .header(30.0, |mut h| {
                    for t in ["When", "Event", "File"] {
                        h.col(|ui| {
                            crate::widgets::th(ui, t);
                        });
                    }
                })
                .body(|body| {
                    body.rows(32.0, rows.len(), |mut row| {
                        let i = rows[row.index()];
                        let e = idx.entries[i];
                        row.set_selected(st.selected == Some(i));
                        row.col(|ui| {
                            ui.label(RichText::new(Timestamp(e.ts).to_local_string("%Y-%m-%d %H:%M:%S")).color(pal::FAINT));
                        });
                        row.col(|ui| {
                            ui.label(RichText::new(idx.name(e.name)).color(pal::ACCENT_SOFT));
                        });
                        row.col(|ui| {
                            let f = idx.files.get(e.file as usize).and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                            ui.label(RichText::new(f).small().color(pal::DIM));
                        });
                        if row.response().clicked() {
                            st.selected = Some(i);
                        }
                    });
                });
        });
        ui.vertical(|ui| {
            ui.set_width(detail_w);
            if let Some(i) = st.selected {
                if st.detail.as_ref().is_none_or(|(di, _)| *di != i) {
                    let text = idx
                        .read_line(&idx.entries[i])
                        .map(|l| serde_json::from_str::<serde_json::Value>(&l).ok().and_then(|v| serde_json::to_string_pretty(&v).ok()).unwrap_or(l))
                        .unwrap_or_else(|| "(could not read the journal file)".into());
                    st.detail = Some((i, text));
                }
                if let Some((_, text)) = &st.detail {
                    card(ui, idx.name(idx.entries[i].name), |ui| {
                        if ui.small_button(format!("{} Copy JSON", icon::COPY)).clicked() {
                            ui.ctx().copy_text(text.clone());
                        }
                        egui::ScrollArea::vertical().id_salt("jsondetail").max_height(ui.available_height() - 40.0).show(ui, |ui| {
                            ui.label(RichText::new(text).monospace().size(12.0).color(pal::TEXT));
                        });
                    });
                }
            } else {
                card(ui, "Event details", |ui| {
                    ui.label(RichText::new("Select an event to see its full JSON.").color(pal::DIM));
                    ui.label(RichText::new(format!("{} events indexed from {} journal files.", idx.len(), idx.files.len())).font(display(13.0)).color(pal::FAINT));
                });
            }
        });
    });
}

/// Scans every journal file for `needle` (lower-case) and returns matching entry indices.
fn full_text_search(index: &Arc<RwLock<JournalIndex>>, needle: &str) -> Vec<usize> {
    let (files, entries) = {
        let idx = index.read();
        (idx.files.clone(), idx.entries.clone())
    };
    let mut by_pos: rustc_hash::FxHashMap<(u32, u32), usize> = Default::default();
    for (i, e) in entries.iter().enumerate() {
        by_pos.insert((e.file, e.offset), i);
    }
    let mut hits = Vec::new();
    for (fi, path) in files.iter().enumerate() {
        let Ok(bytes) = std::fs::read(path) else { continue };
        let text = String::from_utf8_lossy(&bytes);
        let mut pos = 0usize;
        for line in text.split_inclusive('\n') {
            if line.to_ascii_lowercase().contains(needle)
                && let Some(&i) = by_pos.get(&(fi as u32, pos as u32)) {
                    hits.push(i);
                }
            pos += line.len();
        }
    }
    hits.sort_unstable();
    hits
}

fn table(ui: &mut Ui, headers: &[&str], rows: usize, mut cell: impl FnMut(usize, usize, &mut Ui)) {
    let mut b = TableBuilder::new(ui).striped(true).resizable(false)
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center));
    for (i, _) in headers.iter().enumerate() {
        b = b.column(if i + 1 == headers.len() { Column::remainder() } else { Column::initial(if i == 0 { 130.0 } else { 220.0 }) });
    }
    b.header(30.0, |mut h| {
        for t in headers {
            h.col(|ui| {
                crate::widgets::th(ui, t);
            });
        }
    })
    .body(|body| {
        body.rows(32.0, rows, |mut row| {
            let i = row.index();
            for c in 0..headers.len() {
                row.col(|ui| cell(i, c, ui));
            }
        });
    });
}
