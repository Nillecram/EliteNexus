//! Folders the commander points Elite Nexus at (the journal folder, the game folder): typed or pasted into a field,
//! or picked with the system's folder picker (Windows' own; on Linux the desktop's file-chooser portal, else zenity).

use crate::jobs::Job;
use crate::theme::{pal, size};
use crate::widgets;
use egui::{RichText, Ui};
use egui_phosphor::regular as icon;
use std::path::PathBuf;

/// Where the game folder is, for the pages that ask for it.
pub const GAME_FOLDER_HINT: &str = "The game's folder is the one holding its Products folder: steamapps › common › Elite Dangerous in a Steam library, Frontier › EDLaunch for the Frontier launcher, Epic Games › EliteDangerous for Epic. Products itself, or the game's folder inside it, works too.";

const GAME_PICKER_TITLE: &str = "Elite Dangerous's folder";

/// Where Elite Nexus looked for the game folder, for the pages that didn't find it.
pub fn game_not_found() -> &'static str {
    if cfg!(windows) {
        "It wasn't found in your Steam libraries, in Windows' list of installed programs or in the usual Epic and Frontier folders."
    } else {
        "It wasn't found in your Steam libraries or in Program Files in the game's Wine prefix."
    }
}

/// A folder as typed or pasted: spaces and quotes around it go (Explorer's "Copy as path" adds quotes), spaces inside
/// stay. Network folders (`\\PC\Users\…`) are kept as they are. Empty: `None`.
pub fn parse_folder(text: &str) -> Option<PathBuf> {
    let t = text.trim().trim_matches('"').trim();
    // Linux file managers quote a path with spaces in single quotes.
    let t = t.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')).map_or(t, str::trim);
    (!t.is_empty()).then(|| PathBuf::from(t))
}

/// Opens a folder in the system's file manager (Explorer; on Linux whatever xdg-open picks).
pub fn reveal(dir: &std::path::Path) {
    let program = if cfg!(windows) {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    match std::process::Command::new(program).arg(dir).spawn() {
        // Waited for on its own thread, so no finished process is left behind.
        Ok(mut child) => {
            let _ = std::thread::Builder::new().name("reveal".into()).spawn(move || child.wait());
        }
        Err(e) => tracing::warn!("couldn't open {} with {program}: {e}", dir.display()),
    }
}

/// The system's folder picker, run off the UI thread so the window keeps drawing (and following the game) while
/// it's open.
#[derive(Default)]
pub struct Picker {
    job: Option<Job<Option<PathBuf>>>,
}

impl Picker {
    /// Opens the picker at `start` (when that folder exists), unless it's open already.
    pub fn open(&mut self, ctx: &egui::Context, title: &str, start: Option<PathBuf>) {
        if self.job.is_some() {
            return;
        }
        let title = title.to_owned();
        self.job = Some(Job::spawn(ctx, move || {
            let dialog = rfd::FileDialog::new().set_title(title);
            // Checked here, not on the UI thread: an offline network folder can take a while to answer.
            let dialog = match start.filter(|d| d.is_dir()) {
                Some(dir) => dialog.set_directory(dir),
                None => dialog,
            };
            dialog.pick_folder()
        }));
    }

    pub fn busy(&self) -> bool {
        self.job.is_some()
    }

    /// The folder picked, in the frame the picker closes with one.
    pub fn poll(&mut self) -> Option<PathBuf> {
        let picked = self.job.as_mut()?.poll()?;
        self.job = None;
        picked
    }
}

/// A folder typed or picked. The text stays exactly as typed while the field has focus, and is read (spaces and
/// quotes around it removed) when Enter is pressed, the field loses focus or a folder is picked. Escape puts the
/// saved folder back.
#[derive(Default)]
pub struct FolderField {
    text: String,
    /// The value the text was last filled from or read as.
    shown: Option<Option<PathBuf>>,
    picker: Picker,
}

impl FolderField {
    /// The text field, full width. Returns the new value when one is entered.
    pub fn field(&mut self, ui: &mut Ui, id_salt: &str, value: &Option<PathBuf>, hint: &str) -> Option<Option<PathBuf>> {
        let id = ui.make_persistent_id(("folder-field", id_salt));
        let focused = ui.memory(|m| m.has_focus(id));
        if self.shown.as_ref() != Some(value) && !focused {
            self.text = value.as_ref().map(|p| p.display().to_string()).unwrap_or_default();
            self.shown = Some(value.clone());
        }
        let resp = ui.add(egui::TextEdit::singleline(&mut self.text).id(id).hint_text(hint).desired_width(ui.available_width()).margin(egui::Margin::symmetric(10, 6)));
        if !resp.lost_focus() {
            return None;
        }
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.shown = None;
            return None;
        }
        let entered = parse_folder(&self.text);
        self.text = entered.as_ref().map(|p| p.display().to_string()).unwrap_or_default();
        self.shown = Some(entered.clone());
        (entered != *value).then_some(entered)
    }

    /// The Browse button (or, while the picker is open, a note saying so). Returns the folder picked.
    pub fn browse(&mut self, ui: &mut Ui, title: &str, value: &Option<PathBuf>) -> Option<Option<PathBuf>> {
        if self.picker.busy() {
            ui.spinner();
            ui.label(RichText::new("Pick the folder in the window that opened…").size(size::FOOTNOTE).color(pal::DIM));
        } else if widgets::button(ui, &format!("{} Browse…", icon::FOLDER_OPEN)).clicked() {
            let start = parse_folder(&self.text).or_else(|| value.clone());
            self.picker.open(ui.ctx(), title, start);
        }
        let dir = self.picker.poll()?;
        self.text = dir.display().to_string();
        self.shown = Some(Some(dir.clone()));
        (value.as_ref() != Some(&dir)).then_some(Some(dir))
    }
}

/// Sets the game folder chosen in Settings, and tells every page that uses it (`nexus_hud::install::installs`).
pub fn set_game_dir(setting: &mut Option<PathBuf>, dir: Option<PathBuf>) {
    nexus_hud::install::set_game_dir(dir.as_deref());
    *setting = dir;
}

/// For a page that needs the game folder and has none: why, and the way to choose it. Returns true when the choice
/// changed (the setting is then saved, and every page uses it).
pub fn choose_game_folder(ui: &mut Ui, picker: &mut Picker, game_dir: &mut Option<PathBuf>) -> bool {
    let mut changed = false;
    if let Some(dir) = game_dir.as_ref() {
        let text = format!("{} The game folder chosen in Settings, {}, doesn't hold the game: no EliteDangerous64.exe in it or under it.", icon::WARNING, dir.display());
        ui.add(egui::Label::new(RichText::new(text).color(pal::WARN)).wrap());
    }
    ui.add_space(4.0);
    ui.horizontal_wrapped(|ui| {
        if picker.busy() {
            ui.spinner();
            ui.label(RichText::new("Pick the folder in the window that opened…").color(pal::DIM));
        } else if widgets::primary_button(ui, &format!("{} Choose the game folder…", icon::FOLDER_OPEN)).clicked() {
            picker.open(ui.ctx(), GAME_PICKER_TITLE, game_dir.clone());
        }
        if game_dir.is_some() && widgets::button(ui, "Find it automatically").clicked() {
            set_game_dir(game_dir, None);
            changed = true;
        }
    });
    if let Some(dir) = picker.poll() {
        set_game_dir(game_dir, Some(dir));
        changed = true;
    }
    widgets::footnote(ui, GAME_FOLDER_HINT);
    changed
}

/// The game folder in use, for a one-line display.
pub fn game_folder_text(installs: &[nexus_hud::install::GameInstall]) -> Option<String> {
    let live = installs.iter().find(|i| i.is_live()).or(installs.first())?;
    Some(live.folder.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_folders_as_typed_or_pasted() {
        assert_eq!(parse_folder(""), None);
        assert_eq!(parse_folder("   "), None);
        assert_eq!(parse_folder("\"\""), None);
        // Spaces inside stay; around, they go.
        let saved = r"C:\Users\Cmdr\Saved Games\Frontier Developments\Elite Dangerous";
        assert_eq!(parse_folder(&format!("  {saved}  ")), Some(PathBuf::from(saved)));
        // Explorer's "Copy as path".
        assert_eq!(parse_folder(&format!("\"{saved}\"")), Some(PathBuf::from(saved)));
        assert_eq!(parse_folder(&format!(" \" {saved} \" ")), Some(PathBuf::from(saved)));
        // A network folder on another PC.
        let unc = r"\\MAINPC\Users\Cmdr\Saved Games\Frontier Developments\Elite Dangerous";
        assert_eq!(parse_folder(&format!("\"{unc}\"\n")), Some(PathBuf::from(unc)));
        // A Linux file manager's quoting; an apostrophe inside a name stays.
        assert_eq!(parse_folder("'/home/cmdr/Games/elite dangerous'"), Some(PathBuf::from("/home/cmdr/Games/elite dangerous")));
        assert_eq!(parse_folder("/home/cmdr/Cmdr's games"), Some(PathBuf::from("/home/cmdr/Cmdr's games")));
    }
}
