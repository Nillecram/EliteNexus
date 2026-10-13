//! Glare and exposure (a tab of HUD & cockpit): the game's tone-mapping values in
//! `GraphicsConfigurationOverride.xml`, the presets players use, and what each value does.

use crate::theme::{display, pal, size};
use crate::widgets::{self, card, footnote, group, section};
use egui::{RichText, Ui};
use egui_phosphor::regular as icon;
use nexus_hud::OverrideFile;
use nexus_hud::install::GameInstall;
use nexus_hud::tone::{self, Defaults, Tone};

pub struct GlareUi {
    /// What the file sets now (`Tone::default()` = the game's own values).
    current: Result<Tone, String>,
    edit: Tone,
    /// The game's own values, from its `GraphicsConfiguration.xml` when found.
    defaults: Defaults,
    /// That file, or `None` when the built-in 4.4 values are used.
    defaults_from: Option<String>,
    status: Option<(bool, String)>,
    more: bool,
}

impl Default for GlareUi {
    fn default() -> Self {
        GlareUi { current: Ok(Tone::default()), edit: Tone::default(), defaults: tone::GAME_4_4, defaults_from: None, status: None, more: false }
    }
}

impl GlareUi {
    /// Reads the values the override file sets and the game's own from its install.
    pub fn reload(&mut self, file: Option<&Result<OverrideFile, String>>, installs: &[GameInstall]) {
        self.current = match file {
            Some(Ok(f)) => f.text.as_deref().map_or(Ok(Tone::default()), |t| tone::read(t).map_err(|e| e.to_string())),
            Some(Err(e)) => Err(e.clone()),
            None => Ok(Tone::default()),
        };
        let base = installs.iter().find(|i| i.is_live()).map(|i| i.folder.join("GraphicsConfiguration.xml"));
        let read = base.as_ref().and_then(|p| std::fs::read(p).ok().and_then(|b| Defaults::from_base(&String::from_utf8_lossy(&b)).ok()).map(|d| (d, p.display().to_string())));
        (self.defaults, self.defaults_from) = match read {
            Some((d, p)) => (d, Some(p)),
            None => (tone::GAME_4_4, None),
        };
        self.edit = self.current.clone().unwrap_or_default().normalized(&self.defaults);
        self.status = None;
    }

    fn apply(&mut self, file: &mut Option<Result<OverrideFile, String>>) {
        let Some(Ok(f)) = file.as_mut() else { return };
        let target = self.edit.normalized(&self.defaults);
        self.status = Some(match tone::write(f.text.as_deref(), &target).and_then(|t| f.write(t)) {
            Ok(()) => {
                self.current = Ok(target);
                self.edit = target;
                (true, if target.is_default() { "Back to the game's own values the next time Elite Dangerous starts.".into() } else { "Saved. Elite Dangerous uses these values the next time it starts.".into() })
            }
            Err(e) => (false, e.to_string()),
        });
    }
}

pub fn show(ui: &mut Ui, st: &mut GlareUi, file: &mut Option<Result<OverrideFile, String>>, playing: bool) {
    let from = st.defaults_from.clone();
    widgets::split(ui, 0.5, 400.0, |ui| controls(ui, st, file, playing), |ui| explain(ui, from.as_deref()));
}

fn controls(ui: &mut Ui, st: &mut GlareUi, file: &mut Option<Result<OverrideFile, String>>, playing: bool) {
    let d = st.defaults;
    let eff = st.edit.effective(&d);
    card(ui, "Glare and exposure", |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
            for p in tone::PRESETS {
                let selected = st.edit.normalized(&d) == p.tone.normalized(&d);
                if widgets::pill(ui, selected, p.name).on_hover_text(p.note).clicked() {
                    st.edit = p.tone.normalized(&d);
                }
            }
        });
        ui.add_space(6.0);
        group(ui, |g| {
            let detail = format!("The game uses {}; lower means less glare around bright lights.", trim(d.glare));
            g.row("Glare from lights", &detail, |ui| {
                let mut v = eff.glare;
                ui.add_sized([44.0, 20.0], egui::Label::new(RichText::new(trim(v)).color(pal::DIM)));
                if ui.add(egui::Slider::new(&mut v, 0.0..=2.0).show_value(false)).on_hover_text("Frontier publishes no range for this value; players use 1.0.").changed() {
                    st.edit.glare = Some((v * 100.0).round() / 100.0);
                }
            });
            let mut standard = near(&eff.aces, &tone::STANDARD_ACES);
            g.row("Standard ACES curve", &curve_detail(&d), |ui| {
                if widgets::switch(ui, &mut standard).changed() {
                    st.edit.aces = standard.then_some(tone::STANDARD_ACES);
                }
            });
        });
        widgets::disclosure(ui, &mut st.more, "Auto-exposure targets");
        if st.more {
            ui.add(egui::Label::new(RichText::new(format!("The three points of the picture's brightness the auto-exposure is set from. What each one means isn't documented; the game uses {}.", list(&d.percentiles))).size(size::FOOTNOTE).color(pal::DIM)).wrap());
            ui.horizontal(|ui| {
                let mut p = eff.percentiles;
                let mut touched = false;
                for v in p.iter_mut() {
                    touched |= ui.add(egui::DragValue::new(v).range(0.0..=1.0).speed(0.005).max_decimals(3)).changed();
                }
                if touched {
                    st.edit.percentiles = Some(p);
                }
            });
        }
        ui.add_space(8.0);
        apply_bar(ui, st, file, playing);
    });
    footnote(ui, "Changes the whole picture, not only the cockpit: space, stations and planets too.");
}

/// What the file holds now, and Apply / Undo.
fn apply_bar(ui: &mut Ui, st: &mut GlareUi, file: &mut Option<Result<OverrideFile, String>>, playing: bool) {
    let d = st.defaults;
    match &st.current {
        Ok(cur) => {
            let text = if cur.is_default() { "In the game: its own values.".to_string() } else { format!("In the game: {}.", describe(cur, &d)) };
            ui.label(RichText::new(text).font(display(size::BODY)).color(pal::TEXT));
        }
        Err(e) => {
            ui.add(egui::Label::new(RichText::new(format!("{}  {e}", icon::WARNING)).color(pal::BAD)).wrap());
            return;
        }
    }
    let can_save = matches!(file, Some(Ok(_)));
    let unsaved = st.current.as_ref().is_ok_and(|c| c.normalized(&d) != st.edit.normalized(&d));
    if unsaved && can_save {
        ui.horizontal_wrapped(|ui| {
            if widgets::primary_button(ui, &format!("{} Apply to Elite Dangerous", icon::CHECK)).clicked() {
                st.apply(file);
            }
            if widgets::button(ui, "Undo changes").clicked() {
                st.edit = st.current.clone().unwrap_or_default().normalized(&d);
                st.status = None;
            }
        });
    }
    if let Some((ok, msg)) = &st.status {
        ui.add(egui::Label::new(RichText::new(msg).size(size::FOOTNOTE).color(if *ok { pal::GOOD } else { pal::BAD })).wrap());
    } else if unsaved && can_save {
        ui.label(RichText::new("Not applied yet.").size(size::FOOTNOTE).color(pal::WARN));
    }
    let restart = if playing { "The game is running: it reads these values only when it starts, so quit to the desktop and start it again." } else { "The game reads these values when it starts." };
    ui.add(egui::Label::new(RichText::new(restart).size(size::FOOTNOTE).color(pal::DIM)).wrap());
}

/// `defaults_from`: the game file its own values were read from, if found.
fn explain(ui: &mut Ui, defaults_from: Option<&str>) {
    card(ui, "What these settings do, and what they can't", |ui| {
        let lines = [
            "Odyssey's auto-exposure (brightness adaptation) reacts to the cockpit's own lights: with a bright dashboard in view, everything outside is drawn darker. Commanders have reported it on Frontier's feedback thread since 2021.",
            "It can't be switched off. What players change instead is how much glare bright lights produce and the shape of the tone curve, with the values on the left.",
            "Glare compensation: lower values reduce the peak brightness of glare from lights (CMDR Morbad, Frontier forums).",
            "Standard ACES curve: the widely used filmic curve. Compared with the game's own it draws darks and mid-tones brighter and leaves highlights alone, which offsets some of the darkening.",
            "These are the game's own tone-mapping values (HDRNode_Reference). Frontier doesn't document them, so treat them as an experiment: if you don't like the result, pick Game default and apply.",
        ];
        for l in lines {
            ui.add(egui::Label::new(RichText::new(format!("•  {l}")).color(pal::TEXT)).wrap());
        }
        section(ui, "Not here");
        let not = [
            "The cockpit lamps keep their own brightness: the game has no setting for them.",
            "Bloom, in the game's Graphics options, sets how much bright lights glow; Off removes that glow altogether.",
        ];
        for l in not {
            ui.add(egui::Label::new(RichText::new(format!("•  {l}")).color(pal::DIM)).wrap());
        }
    });
    let from = match defaults_from {
        Some(p) => format!("The game's own values are read from {p}."),
        None => "The game's install wasn't found, so the values the game shipped with in 4.4 are shown as its own.".to_string(),
    };
    footnote(ui, &from);
}

/// "glare 1.0 · standard ACES curve · auto-exposure targets 0.6, 0.54, 0.9".
fn describe(t: &Tone, d: &Defaults) -> String {
    let t = t.normalized(d);
    let mut parts = Vec::new();
    if let Some(g) = t.glare {
        parts.push(format!("glare {}", trim(g)));
    }
    if let Some(a) = t.aces {
        parts.push(if near(&a, &tone::STANDARD_ACES) { "standard ACES curve".to_string() } else { format!("ACES curve {}", list(&a)) });
    }
    if let Some(p) = t.percentiles {
        parts.push(format!("auto-exposure targets {}", list(&p)));
    }
    parts.join(" · ")
}

/// What switching to the standard curve does, computed from the two curves.
fn curve_detail(d: &Defaults) -> String {
    let [dark, mid, _] = tone::curve_ratios(tone::STANDARD_ACES, d.aces);
    if (dark - 1.0).abs() < 0.05 && (mid - 1.0).abs() < 0.05 {
        return "The game already uses this curve.".into();
    }
    format!("Darks about {dark:.1}× and mid-tones {mid:.1}× as bright as with the game's curve; highlights the same.")
}

fn near(a: &[f32; 3], b: &[f32; 3]) -> bool {
    a.iter().zip(b).all(|(x, y)| (x - y).abs() < 5e-4)
}

fn trim(v: f32) -> String {
    let s = format!("{v:.4}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() || s == "-0" { "0".into() } else { s.to_string() }
}

fn list(v: &[f32; 3]) -> String {
    v.iter().map(|x| trim(*x)).collect::<Vec<_>>().join(", ")
}
