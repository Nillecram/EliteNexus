//! Key bindings: active presets, conflicts, missing essentials, searchable action list, and a
//! printable reference card (EDRefCard replacement).

use crate::theme::{display, pal, size};
use crate::widgets::{self, badge, card, empty};
use egui::{RichText, Ui};
use egui_extras::{Column, TableBuilder};
use egui_phosphor::regular as icon;
use nexus_binds::{analyze, find_presets, render_html, Bindings, BindsError, Context, Importance, PresetCategory, PresetDirectory, Report, Slot};
use std::path::PathBuf;

/// Said wherever the page could look like it changed something.
const READ_ONLY: &str = "Elite Nexus only reads your bindings: it never changes them. Change them in the game (Options › Controls).";

pub struct Loaded {
    pub dir: PresetDirectory,
    pub bindings: Bindings,
    pub report: Report,
    /// The presets the game uses couldn't all be read, so another preset is shown: why, and which.
    pub fallback: Option<Fallback>,
}

pub struct Fallback {
    pub why: String,
    pub shown: String,
}

#[derive(Default)]
pub struct BindsUi {
    pub loaded: Option<Result<Loaded, String>>,
    pub filter: String,
    pub context: Option<Context>,
    pub only_problems: bool,
    pub last_export: Option<String>,
    /// A preset the game uses wasn't found, and neither was the game folder (built-in presets).
    needs_game_folder: bool,
    /// The game folder chosen in Settings at the last load.
    game_dir_seen: Option<PathBuf>,
    picker: crate::folders::Picker,
}

impl BindsUi {
    pub fn load(&mut self) {
        self.game_dir_seen = nexus_hud::install::game_dir();
        let mut needs_game_folder = false;
        let result = (|| -> Result<Loaded, String> {
            let dir_path = nexus_binds::default_bindings_dir().ok_or("The game's Options folder wasn't found (on Linux: set the journal folder in Settings to the one in the game's Wine prefix)")?;
            let dir = find_presets(&dir_path).map_err(|e| e.to_string())?;
            let builtin = builtin_dirs();
            let builtin_refs: Vec<&std::path::Path> = builtin.iter().map(|p| p.as_path()).collect();
            let (bindings, fallback) = match dir.load_effective(&builtin_refs) {
                Ok(b) => (b, None),
                Err(e) => {
                    let why = match &e {
                        BindsError::PresetNotFound(name) if builtin.is_empty() => {
                            needs_game_folder = true;
                            format!("{name} isn't in your Bindings folder. If it's one of the game's built-in presets (such as KeyboardMouseOnly), it's in the game's folder, which Elite Nexus didn't find.")
                        }
                        BindsError::PresetNotFound(name) => format!("{name} is neither in your Bindings folder nor among the game's built-in presets."),
                        e => format!("Elite Nexus couldn't read them: {e}."),
                    };
                    // Another preset rather than nothing; the page says plainly that it isn't what the game uses.
                    let Some(shown) = dir.primary() else { return Err(why) };
                    let how = if shown.is_active() { "one of the presets the game uses" } else { "the preset in your Bindings folder changed last" };
                    (shown.load().map_err(|e| e.to_string())?, Some(Fallback { why, shown: format!("{} ({how})", shown.name) }))
                }
            };
            let report = analyze(&bindings);
            Ok(Loaded { dir, bindings, report, fallback })
        })();
        self.needs_game_folder = needs_game_folder;
        self.loaded = Some(result);
    }
}

/// The game's ControlSchemes folders (built-in presets): in the game folder chosen in Settings, else in every one found.
fn builtin_dirs() -> Vec<PathBuf> {
    nexus_hud::install::installs().into_iter().map(|i| i.folder.join("ControlSchemes")).filter(|cs| cs.is_dir()).collect()
}

/// The presets the game uses, by category, as one line of text.
fn active_list(dir: &PresetDirectory) -> String {
    let name = |c| dir.active_name(c).unwrap_or("?");
    if PresetCategory::ALL.iter().all(|&c| name(c) == name(PresetCategory::General)) {
        return format!("{} for all its controls", name(PresetCategory::General));
    }
    PresetCategory::ALL.iter().map(|&c| format!("{} {}", c.label(), name(c))).collect::<Vec<_>>().join(", ")
}

/// Returns true when the game folder was chosen here (settings to save).
pub fn show(ui: &mut Ui, st: &mut BindsUi, data_dir: &std::path::Path, game_dir: &mut Option<PathBuf>) -> bool {
    let mut changed = false;
    // The game folder (built-in presets) was chosen here or in Settings.
    if st.loaded.is_none() || st.game_dir_seen != nexus_hud::install::game_dir() {
        st.load();
    }
    ui.horizontal(|ui| {
        if ui.button(format!("{} Reload", icon::ARROW_CLOCKWISE)).clicked() {
            st.load();
        }
        if let Some(Ok(l)) = &st.loaded
            && ui.button(format!("{} Printable reference card", icon::PRINTER)).on_hover_text("Writes an HTML card and opens it in your browser").clicked() {
                let html = render_html(&l.bindings, &l.report);
                let path = data_dir.join("bindings-refcard.html");
                match std::fs::write(&path, html) {
                    Ok(()) => {
                        // file:///C:/... on Windows, file:///home/... on Linux (whose paths already start with /).
                        let url = format!("file:///{}", path.display().to_string().replace('\\', "/").trim_start_matches('/'));
                        ui.ctx().open_url(egui::OpenUrl::new_tab(url));
                        st.last_export = Some(path.display().to_string());
                    }
                    Err(e) => st.last_export = Some(format!("Could not write card: {e}")),
                }
            }
        if let Some(p) = &st.last_export {
            ui.label(RichText::new(p).small().color(pal::DIM));
        }
    });
    ui.add_space(6.0);
    let loaded = match &st.loaded {
        Some(Ok(l)) => l,
        Some(Err(e)) => {
            empty(ui, icon::GAME_CONTROLLER, "Couldn't read your bindings", e);
            if st.needs_game_folder {
                let mut dir = game_dir.clone();
                if crate::folders::choose_game_folder(ui, &mut st.picker, &mut dir) {
                    *game_dir = dir;
                    changed = true;
                }
            }
            return changed;
        }
        None => return changed,
    };
    let r = &loaded.report;
    if let Some(f) = &loaded.fallback {
        let mut dir = game_dir.clone();
        let picked = widgets::card_tinted(ui, icon::WARNING, "These aren't the bindings your game uses", pal::WARN, |ui| {
            let start = loaded.dir.start_preset_path.as_ref().and_then(|p| p.file_name()).map_or("its start file".into(), |n| n.to_string_lossy());
            let text = format!("The game uses {} (from {start}). {}", active_list(&loaded.dir), f.why);
            ui.add(egui::Label::new(RichText::new(text).color(pal::TEXT)).wrap());
            let shown = format!("Shown instead: {}. The conflicts and missing essentials below are that preset's. {READ_ONLY}", f.shown);
            ui.add(egui::Label::new(RichText::new(shown).size(size::FOOTNOTE).color(pal::DIM)).wrap());
            st.needs_game_folder && crate::folders::choose_game_folder(ui, &mut st.picker, &mut dir)
        });
        if picked {
            *game_dir = dir;
            changed = true;
        }
        ui.add_space(8.0);
    }

    crate::widgets::cards(ui, 3, 280.0, |c| {
        c.add(|ui| {
            card(ui, "Active presets", |ui| {
            for cat in [PresetCategory::General, PresetCategory::Ship, PresetCategory::Srv, PresetCategory::OnFoot] {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(cat.label()).color(pal::DIM));
                    ui.label(RichText::new(loaded.dir.active_name(cat).unwrap_or("?")).color(pal::TEXT));
                });
            }
            ui.label(RichText::new(format!("{} actions bound · {} bindings", r.totals.bound_actions, r.totals.bindings)).small().color(pal::DIM));
            ui.add(egui::Label::new(RichText::new(READ_ONLY).size(size::FOOTNOTE).color(pal::DIM)).wrap()).on_hover_text(loaded.dir.dir.display().to_string());
        });
        });
        c.add(|ui| {
            card(ui, "Health", |ui| {
            let essentials = r.missing_essentials().count();
            ui.horizontal(|ui| {
                crate::widgets::stat(ui, "conflicts", r.conflicts.len().to_string(), if r.conflicts.is_empty() { pal::GOOD } else { pal::BAD });
                ui.add_space(14.0);
                crate::widgets::stat(ui, "missing essentials", essentials.to_string(), if essentials == 0 { pal::GOOD } else { pal::WARN });
                ui.add_space(14.0);
                crate::widgets::stat(ui, "recommended", (r.missing.len() - essentials).to_string(), pal::DIM);
            });
        });
        });
        c.add(|ui| {
            card(ui, "Devices", |ui| {
            ui.horizontal_wrapped(|ui| {
                for d in &r.devices {
                    let col = if d.conflicts > 0 { pal::WARN } else { pal::CYAN };
                    badge(ui, &format!("{} · {} btn · {} axes", d.name, d.buttons, d.axes), col);
                }
            });
        });
        });
    });
    ui.add_space(8.0);
    if !r.conflicts.is_empty() || r.missing.iter().any(|m| m.importance == Importance::Essential) {
        crate::widgets::cards(ui, 2, 360.0, |c| {
            c.add(|ui| {
                card(ui, "Conflicts", |ui| {
                if r.conflicts.is_empty() {
                    ui.label(RichText::new("None — nice.").color(pal::GOOD));
                }
                for cf in &r.conflicts {
                    ui.horizontal_wrapped(|ui| {
                        badge(ui, &cf.input.display(), pal::BAD);
                        ui.label(RichText::new(cf.actions.iter().map(|a| format!("{} ({})", a.name, a.context.label())).collect::<Vec<_>>().join("  ·  ")).color(pal::TEXT));
                    });
                }
            });
            });
            c.add(|ui| {
                card(ui, "Missing essentials", |ui| {
                let mut any = false;
                for m in r.missing_essentials() {
                    any = true;
                    ui.horizontal_wrapped(|ui| {
                        badge(ui, m.context.label(), pal::WARN);
                        ui.label(RichText::new(m.label).color(pal::TEXT));
                    });
                }
                if !any {
                    ui.label(RichText::new("All essentials bound.").color(pal::GOOD));
                }
            });
            });
        });
        ui.add_space(8.0);
    }

    ui.horizontal(|ui| {
        crate::widgets::search_field(ui, &mut st.filter, "Search actions or keys", 280.0);
        egui::ComboBox::from_id_salt("bindctx")
            .selected_text(st.context.map(|c| c.label()).unwrap_or("All contexts"))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut st.context, None, "All contexts");
                for c in [Context::Ship, Context::Srv, Context::OnFoot, Context::Interface, Context::GalaxyMap, Context::Scanners, Context::Fighter, Context::Multicrew, Context::Camera, Context::HoloMe, Context::General, Context::Colonisation, Context::Store] {
                    ui.selectable_value(&mut st.context, Some(c), c.label());
                }
            });
        crate::widgets::toggle(ui, &mut st.only_problems, "Only conflicts");
    });
    let f = st.filter.to_ascii_lowercase();
    let rows: Vec<_> = loaded
        .bindings
        .actions
        .iter()
        .filter(|a| a.is_bound())
        .map(|a| (a, a.info()))
        .filter(|(_, i)| !i.legacy)
        .filter(|(_, i)| st.context.is_none_or(|c| i.context == c))
        .filter(|(a, _)| !st.only_problems || r.is_conflicted(&a.action, Slot::Primary) || r.is_conflicted(&a.action, Slot::Secondary))
        .filter(|(a, i)| {
            f.is_empty()
                || i.name.to_ascii_lowercase().contains(&f)
                || a.inputs().any(|(_, inp)| inp.display().to_ascii_lowercase().contains(&f))
        })
        .collect();
    TableBuilder::new(ui)
        .striped(true)
        .resizable(false)
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .column(Column::initial(110.0))
        .column(Column::initial(170.0))
        .column(Column::initial(260.0))
        .column(Column::initial(200.0))
        .column(Column::initial(200.0))
        .column(Column::remainder())
        .header(30.0, |mut h| {
            for t in ["Context", "Category", "Action", "Primary", "Secondary", "Axis"] {
                h.col(|ui| {
                    crate::widgets::th(ui, t);
                });
            }
        })
        .body(|body| {
            body.rows(32.0, rows.len(), |mut row| {
                let (a, info) = &rows[row.index()];
                row.col(|ui| {
                    ui.label(RichText::new(info.context.label()).color(pal::DIM));
                });
                row.col(|ui| {
                    ui.label(RichText::new(info.category.label()).color(pal::DIM));
                });
                row.col(|ui| {
                    ui.label(RichText::new(info.name.as_ref()).color(pal::TEXT));
                });
                for slot in [Slot::Primary, Slot::Secondary] {
                    row.col(|ui| {
                        if let Some(inp) = a.input(slot) {
                            let bad = r.is_conflicted(&a.action, slot);
                            ui.label(RichText::new(inp.display()).font(display(13.0)).color(if bad { pal::BAD } else { pal::ACCENT_SOFT }));
                        }
                    });
                }
                row.col(|ui| {
                    if let Some(ax) = &a.axis {
                        ui.label(RichText::new(format!("{}{}", ax.input.display(), if ax.inverted { " (inverted)" } else { "" })).color(pal::CYAN));
                    }
                });
            });
        });
    changed
}
