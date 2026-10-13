//! Cockpit lights (a tab of HUD & cockpit): installs, sets up and removes the cockpit-light dimmer,
//! 3Dmigoto plus a pattern for the game's box-light shader (`nexus_hud::dimmer`).

use crate::jobs::Slot;
use crate::theme::{display, pal, size};
use crate::widgets::{self, card, footnote, group, section};
use egui::{RichText, Ui};
use egui_phosphor::regular as icon;
use nexus_binds::{Bindings, Input};
use nexus_hud::dimmer::{self, Dimmer, Removed, Status};
use nexus_hud::install::GameInstall;
use std::path::{Path, PathBuf};

type Files = Vec<(&'static str, Vec<u8>)>;

#[derive(Default)]
pub struct CockpitUi {
    loaded: bool,
    /// The game folder (the one holding `EliteDangerous64.exe`) and what's in it.
    folder: Option<PathBuf>,
    status: Option<Status>,
    /// Settings being edited; Save writes them to the game's config.
    edit: Dimmer,
    /// Install was clicked: the page explains what happens before anything is downloaded.
    confirm_install: bool,
    confirm_remove: bool,
    download: Slot<Result<Files, String>>,
    /// Bytes of capture files left in the game folder, measured in the background.
    leftovers: Slot<u64>,
    cleanup: Slot<Result<Removed, String>>,
    /// The dimmer's keys that the game's own controls use too, with the actions bound to them.
    clashes: Vec<(&'static str, String)>,
    message: Option<(bool, String)>,
}

impl CockpitUi {
    fn refresh(&mut self, ctx: &egui::Context, installs: &[GameInstall]) {
        self.loaded = true;
        self.folder = installs.iter().find(|i| i.is_live()).or(installs.first()).map(|i| i.folder.clone());
        self.status = self.folder.as_deref().map(dimmer::status);
        if let Some(Status::Installed { dimmer } | Status::Unmanaged { dimmer }) = &self.status {
            self.edit = *dimmer;
        }
        self.leftovers = Slot::default();
        self.clashes.clear();
        if let (Some(folder), Some(Status::Installed { .. })) = (self.folder.clone(), &self.status) {
            self.clashes = key_clashes(&folder);
            self.leftovers.run(ctx, move || dimmer::leftovers(&folder));
        }
    }
}

/// The dimmer's keys that the commander's active control presets bind too.
fn key_clashes(game: &Path) -> Vec<(&'static str, String)> {
    let Some(dir) = nexus_binds::default_bindings_dir() else { return Vec::new() };
    let Ok(presets) = nexus_binds::find_presets(&dir) else { return Vec::new() };
    // Built-in presets (KeyboardMouseOnly, ...) live in the game's ControlSchemes folder.
    let schemes = game.join("ControlSchemes");
    let bindings = presets.load_effective(&[schemes.as_path()]).ok().or_else(|| presets.primary()?.load().ok());
    bindings.map(|b| clashes(&b)).unwrap_or_default()
}

fn clashes(b: &Bindings) -> Vec<(&'static str, String)> {
    let mut out = Vec::new();
    for (key, _) in dimmer::KEYS {
        let name = format!("Key_{key}");
        let mut actions: Vec<String> = Vec::new();
        for a in b.actions.iter().filter(|a| a.inputs().any(|(_, i)| fires_with(i, &name))) {
            let info = a.info();
            let what = format!("{} ({})", info.name, info.context.label());
            if !actions.contains(&what) {
                actions.push(what);
            }
        }
        if !actions.is_empty() {
            out.push((key, actions.join(", ")));
        }
    }
    out
}

/// Whether pressing `key` on its own also triggers the binding `i`. 3Dmigoto's keys fire only with
/// no Shift, Ctrl, Alt or Windows key held, so a binding that needs one of those doesn't clash.
fn fires_with(i: &Input, key: &str) -> bool {
    let keyboard = |d: &str| d.eq_ignore_ascii_case("Keyboard");
    let held = i.modifiers.iter().any(|(d, k)| keyboard(d) && ["Shift", "Control", "Alt", "Win"].iter().any(|m| k.ends_with(m)));
    (keyboard(&i.device) && i.key == key && !held) || i.modifiers.iter().any(|(d, k)| keyboard(d) && k == key)
}

fn megabytes(bytes: u64) -> String {
    let mb = bytes as f64 / (1u64 << 20) as f64;
    if mb >= 10.0 { format!("{mb:.0} MB") } else { format!("{mb:.1} MB") }
}

/// Downloads the official 3Dmigoto release and unpacks the two DLLs (checked against pinned
/// checksums).
async fn fetch() -> Result<Files, String> {
    let http = reqwest::Client::builder().user_agent(concat!("EliteNexus/", env!("CARGO_PKG_VERSION"))).build().map_err(|e| e.to_string())?;
    let resp = http.get(dimmer::MIGOTO_URL).send().await.map_err(|e| e.to_string())?.error_for_status().map_err(|e| e.to_string())?;
    let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
    tokio::task::spawn_blocking(move || dimmer::extract(&bytes).map_err(|e| e.to_string())).await.map_err(|e| e.to_string())?
}

pub fn show(ui: &mut Ui, st: &mut CockpitUi, installs: &[GameInstall], playing: bool, rt: &tokio::runtime::Handle) {
    if !st.loaded {
        st.refresh(ui.ctx(), installs);
    }
    if st.download.tick() {
        match (st.download.last.take(), st.folder.clone()) {
            (Some(Ok(files)), Some(folder)) => {
                st.message = Some(match dimmer::install(&folder, &files, &st.edit) {
                    Ok(()) => (true, "Installed. Start Elite Dangerous: the cockpit lights come out dimmer from the first frame.".into()),
                    Err(e) => (false, e.to_string()),
                });
                st.confirm_install = false;
                st.refresh(ui.ctx(), installs);
            }
            (Some(Err(e)), _) => st.message = Some((false, format!("The download didn't work: {e}"))),
            _ => {}
        }
    }
    st.leftovers.tick();
    if st.cleanup.tick() {
        let bytes = st.leftovers.last.unwrap_or(0);
        st.message = match st.cleanup.last.take() {
            Some(Ok(done)) => Some((true, format!("Deleted {} of capture files: {}.", megabytes(bytes), done.removed.join(", ")))),
            Some(Err(e)) => Some((false, format!("Not all capture files could be deleted: {e}"))),
            None => None,
        };
        st.refresh(ui.ctx(), installs);
    }
    widgets::split(ui, 0.5, 400.0, |ui| controls(ui, st, installs, playing, rt), explain);
}

fn controls(ui: &mut Ui, st: &mut CockpitUi, installs: &[GameInstall], playing: bool, rt: &tokio::runtime::Handle) {
    card(ui, "Cockpit-light dimmer", |ui| {
        let Some(folder) = st.folder.clone() else {
            ui.add(egui::Label::new(RichText::new("Elite Dangerous wasn't found in the usual Steam, Epic or Frontier folders, so the dimmer can't be installed from here.").color(pal::DIM)).wrap());
            return;
        };
        match st.status.clone() {
            Some(Status::Installed { .. }) => installed(ui, st, &folder, installs, playing, false),
            Some(Status::Unmanaged { .. }) => installed(ui, st, &folder, installs, playing, true),
            Some(Status::Foreign { edhm }) => {
                let what = if edhm { "EDHM is installed in the game folder." } else { "Another 3Dmigoto mod is installed in the game folder." };
                ui.label(RichText::new(what).font(display(size::BODY)).color(pal::TEXT));
                ui.add(egui::Label::new(RichText::new("Elite Nexus leaves it alone: two mods can't share the game's d3d11.dll. To use Elite Nexus's dimmer instead, remove that mod first.").color(pal::DIM)).wrap());
            }
            Some(Status::None) | None => not_installed(ui, st, playing, rt),
        }
        if let Some((ok, msg)) = &st.message {
            ui.add_space(4.0);
            ui.add(egui::Label::new(RichText::new(msg).size(size::FOOTNOTE).color(if *ok { pal::GOOD } else { pal::BAD })).wrap());
        }
        ui.add_space(4.0);
        let path = folder.display().to_string();
        ui.add(egui::Label::new(RichText::new(&path).size(size::FOOTNOTE).color(pal::DIM)).truncate()).on_hover_text(path);
    });
}

fn not_installed(ui: &mut Ui, st: &mut CockpitUi, playing: bool, rt: &tokio::runtime::Handle) {
    ui.add(egui::Label::new(RichText::new("Dims your cockpit's own lights (lamps, panel glow, indicator lights) and nothing else: stations, carriers, planets and the HUD stay as they are.").color(pal::TEXT)).wrap());
    ui.add_space(6.0);
    if !st.confirm_install {
        if widgets::primary_button(ui, &format!("{} Install the dimmer", icon::DOWNLOAD_SIMPLE)).clicked() {
            st.confirm_install = true;
            st.message = None;
        }
        return;
    }
    section(ui, "Before you install");
    let points = [
        format!("Elite Nexus downloads 3Dmigoto {} (4.7 MB) from its official GitHub release and checks it against a pinned checksum. 3Dmigoto is an open-source (GPL-3.0) graphics modding tool; Elite Nexus uses it to load its own pattern, nothing else.", dimmer::MIGOTO_VERSION),
        format!("It puts d3d11.dll, d3dcompiler_47.dll, {}, {} and a ShaderFixes folder next to EliteDangerous64.exe, in the folder below. Remove takes them out again.", dimmer::CONFIG, dimmer::MANIFEST),
        "No game file is changed: 3Dmigoto adjusts the light shader in memory while the game runs. Frontier tolerates 3Dmigoto mods (commanders have used them for years) but has never approved them.".to_string(),
    ];
    for p in points {
        ui.add(egui::Label::new(RichText::new(format!("•  {p}")).color(pal::TEXT)).wrap());
    }
    if playing {
        ui.label(RichText::new("The game is running: the dimmer takes effect the next time it starts.").size(size::FOOTNOTE).color(pal::WARN));
    }
    ui.add_space(4.0);
    ui.horizontal_wrapped(|ui| {
        if st.download.busy() {
            ui.spinner();
            ui.label(RichText::new("Downloading and checking 3Dmigoto…").color(pal::DIM));
        } else {
            if widgets::primary_button(ui, &format!("{} Download and install", icon::CHECK)).clicked() {
                st.message = None;
                st.download.run_async(ui.ctx(), rt, fetch());
            }
            if widgets::button(ui, "Cancel").clicked() {
                st.confirm_install = false;
            }
        }
    });
}

fn installed(ui: &mut Ui, st: &mut CockpitUi, folder: &Path, installs: &[GameInstall], playing: bool, unmanaged: bool) {
    if unmanaged {
        ui.label(RichText::new("The test version you put in by hand is installed.").font(display(size::BODY)).color(pal::TEXT));
        ui.add(
            egui::Label::new(
                RichText::new("Let Elite Nexus manage it: it switches to the pattern (which keeps working through most game updates and replaces the copy of the game's shader the test used), keeps your settings and can remove it again. Nothing is downloaded.")
                    .color(pal::DIM),
            )
            .wrap(),
        );
        if widgets::primary_button(ui, &format!("{} Let Elite Nexus manage it", icon::CHECK)).clicked() {
            st.message = Some(match dimmer::install(folder, &[], &st.edit) {
                Ok(()) => (true, "Done: Elite Nexus manages the dimmer now. The game picks up the change the next time it starts.".into()),
                Err(e) => (false, e.to_string()),
            });
            st.refresh(ui.ctx(), installs);
        }
        return;
    }
    let on = st.edit.on;
    ui.horizontal(|ui| {
        ui.label(RichText::new(if on { icon::LIGHTBULB_FILAMENT } else { icon::LIGHTBULB }).size(18.0).color(if on { pal::ACCENT } else { pal::DIM }));
        ui.label(RichText::new("Installed").font(display(size::BODY)).color(pal::TEXT));
    });
    ui.add_space(4.0);
    let saved = matches!(&st.status, Some(Status::Installed { dimmer }) if *dimmer == st.edit);
    group(ui, |g| {
        g.switch(&mut st.edit.on, "Dim the cockpit lights", "Off keeps 3Dmigoto installed but changes nothing.");
        g.row("Light kept", "0 % switches the cockpit lights off.", |ui| {
            let mut pct = st.edit.strength * 100.0;
            if slider(ui, &mut pct, 0.0..=100.0, 0, " %") {
                st.edit.strength = (pct / 100.0).clamp(0.0, 1.0);
            }
        });
        g.row("Reach", "Lights closer to your eyes than this are dimmed. About 5 m covers most cockpits; big ships need more.", |ui| {
            if slider(ui, &mut st.edit.reach, 2.0..=15.0, 1, " m") {
                st.edit.reach = (st.edit.reach * 2.0).round() / 2.0;
            }
        });
    });
    ui.add_space(4.0);
    ui.horizontal_wrapped(|ui| {
        if !saved && widgets::primary_button(ui, &format!("{} Save", icon::CHECK)).clicked() {
            st.message = Some(match dimmer::save_settings(folder, &st.edit) {
                Ok(()) => (true, "Saved. The game uses it the next time it starts; F9, F10 and F11 change it while you play.".into()),
                Err(e) => (false, e.to_string()),
            });
            st.refresh(ui.ctx(), installs);
        }
        if !st.confirm_remove {
            if widgets::button(ui, &format!("{} Remove", icon::TRASH)).clicked() {
                st.confirm_remove = true;
            }
        } else {
            ui.label(RichText::new("Remove 3Dmigoto and the dimmer from the game folder?").color(pal::WARN));
            if widgets::button(ui, "Remove").clicked() {
                st.message = Some(match dimmer::uninstall(folder) {
                    Ok(done) if done.kept.is_empty() => (true, "Removed. The cockpit lights are back to normal the next time the game starts.".into()),
                    Ok(done) => (false, format!("Removed, except {}: changed since Elite Nexus installed it, so it was left in place.", done.kept.join(", "))),
                    Err(e) if playing => (false, format!("{e}. Quit the game first: it keeps these files open.")),
                    Err(e) => (false, e.to_string()),
                });
                st.confirm_remove = false;
                st.refresh(ui.ctx(), installs);
            }
            if widgets::button(ui, "Cancel").clicked() {
                st.confirm_remove = false;
            }
        }
    });
    if !saved {
        ui.label(RichText::new("Not saved yet.").size(size::FOOTNOTE).color(pal::WARN));
    }
    let leftover = st.leftovers.last.unwrap_or(0);
    if leftover > 0 && !st.cleanup.busy() {
        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(format!("Capture files from the setup are still in the game folder ({}).", megabytes(leftover))).color(pal::DIM));
            let tip = "Frame captures (FrameAnalysis-…) and the game's shaders as 3Dmigoto exported them (ShaderCache), written while the dimmer was being worked out. The dimmer doesn't use them.";
            if widgets::button(ui, &format!("{} Delete them", icon::TRASH)).on_hover_text(tip).clicked() {
                let folder = folder.to_path_buf();
                st.message = None;
                st.cleanup.run(ui.ctx(), move || dimmer::remove_leftovers(&folder).map_err(|e| e.to_string()));
            }
        });
    } else if st.cleanup.busy() {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label(RichText::new("Deleting the capture files…").color(pal::DIM));
        });
    }
    section(ui, "Keys in the game");
    for (key, what) in dimmer::KEYS {
        let clash = st.clashes.iter().find(|(k, _)| *k == key);
        ui.horizontal(|ui| {
            widgets::badge(ui, key, if clash.is_some() { pal::WARN } else { pal::ACCENT });
            ui.label(RichText::new(what).color(pal::DIM));
        });
        if let Some((_, actions)) = clash {
            ui.add(egui::Label::new(RichText::new(format!("{}  {key} is bound in your game controls too: {actions}. Pressing it does both, so give the game's action another key (Options › Controls in the game).", icon::WARNING)).size(size::FOOTNOTE).color(pal::WARN)).wrap());
        }
    }
    footnote(ui, "The keys work while the game window is in front; what they set lasts until the game closes.");
}

fn explain(ui: &mut Ui) {
    card(ui, "What the dimmer does, and what it can't", |ui| {
        let lines = [
            "The game has no setting for its cockpit lights. 3Dmigoto, a Direct3D 11 hook that sits next to the game, lets Elite Nexus change one step of how the game lights the cockpit: lights within the reach of your eyes keep only part of their light.",
            "Only the cockpit's own lights change. Stations, carriers, planets, the HUD and everything else stay exactly as the game draws them.",
            "Elite Nexus never copies the game's code: a pattern finds the light shader by what its code looks like as the game loads it. A game update that only rebuilds shaders keeps it working; if Frontier rewrites how lights are drawn, the dimmer simply stops (the game is unaffected) until Elite Nexus is updated.",
            "Settings saved here apply the next time the game starts; the keys change them while you play.",
            "It can't run alongside EDHM or another 3Dmigoto mod: they all need the game's d3d11.dll.",
            "Frontier tolerates mods like this but has never approved them.",
        ];
        for l in lines {
            ui.add(egui::Label::new(RichText::new(format!("•  {l}")).color(pal::TEXT)).wrap());
        }
        ui.add_space(4.0);
        ui.hyperlink_to(format!("{} 3Dmigoto on GitHub", icon::ARROW_SQUARE_OUT), dimmer::MIGOTO_SOURCE);
    });
}

/// A settings-row slider with its value on the left.
fn slider(ui: &mut Ui, value: &mut f32, range: std::ops::RangeInclusive<f32>, decimals: usize, suffix: &str) -> bool {
    ui.add_sized([56.0, 20.0], egui::Label::new(RichText::new(format!("{:.*}{suffix}", decimals, *value)).color(pal::DIM)));
    ui.add(egui::Slider::new(value, range).show_value(false)).changed()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_dimmer_keys_the_game_uses_too() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8" ?>
<Root PresetName="Test" MajorVersion="4" MinorVersion="2">
	<GalaxyMapOpen>
		<Primary Device="Keyboard" Key="Key_F10" />
		<Secondary Device="{NoDevice}" Key="" />
	</GalaxyMapOpen>
	<SystemMapOpen>
		<Primary Device="Keyboard" Key="Key_F9">
			<Modifier Device="Keyboard" Key="Key_LeftShift" />
		</Primary>
		<Secondary Device="{NoDevice}" Key="" />
	</SystemMapOpen>
	<FocusLeftPanel>
		<Primary Device="Keyboard" Key="Key_1">
			<Modifier Device="Keyboard" Key="Key_F11" />
		</Primary>
		<Secondary Device="Keyboard" Key="Key_F12" />
	</FocusLeftPanel>
</Root>
"#;
        let b = Bindings::parse(xml).unwrap();
        // F9 needs Shift in the game, and the dimmer's keys fire only without modifiers: no clash.
        assert_eq!(clashes(&b), vec![("F10", "Open Galaxy Map (Ship)".to_string()), ("F11", "Left Panel (Navigation) (Ship)".to_string())]);
        assert!(clashes(&Bindings::default()).is_empty());
    }
}
