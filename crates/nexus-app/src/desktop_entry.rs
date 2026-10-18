//! Linux: Elite Nexus in the desktop's app menu. A `.desktop` file and icons in the user's own data folder
//! (`$XDG_DATA_HOME`, usually `~/.local/share`), written only when the commander asks (Settings › About, or
//! `--install-desktop`). Wayland docks match a window to its entry by the app id, so the entry is also what gives
//! the window its icon there.

use std::path::{Path, PathBuf};

/// The window's app id (main.rs), and so the entry's file name and the icons' name.
pub const APP_ID: &str = "elite-nexus";
const ICON_SIZES: [u32; 5] = [32, 48, 64, 128, 256];

/// What the app menu holds for Elite Nexus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum State {
    Missing,
    /// The entry starts this program.
    Current,
    /// The entry starts a program somewhere else: an older copy, or this one before it moved.
    Elsewhere(String),
}

pub fn entry_path(data_home: &Path) -> PathBuf {
    data_home.join("applications").join(format!("{APP_ID}.desktop"))
}

fn icon_path(data_home: &Path, n: u32) -> PathBuf {
    data_home.join("icons").join("hicolor").join(format!("{n}x{n}")).join("apps").join(format!("{APP_ID}.png"))
}

pub fn state(data_home: &Path, exe: &Path) -> State {
    let Ok(text) = std::fs::read_to_string(entry_path(data_home)) else { return State::Missing };
    let exec = text.lines().find_map(|l| l.strip_prefix("Exec=")).unwrap_or_default();
    if exec == exec_value(exe) { State::Current } else { State::Elsewhere(exec.trim_matches('"').replace("\\\\", "\\")) }
}

/// Writes the icons and the entry, which starts `exe`. Replaces an entry made before.
pub fn install(data_home: &Path, exe: &Path) -> std::io::Result<()> {
    for n in ICON_SIZES {
        let path = icon_path(data_home, n);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        image::save_buffer(&path, &crate::icon::rgba(n), n, n, image::ColorType::Rgba8).map_err(std::io::Error::other)?;
    }
    let entry = entry_path(data_home);
    if let Some(dir) = entry.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(entry, desktop_file(exe))
}

/// Takes out exactly the files [`install`] writes; missing ones are fine.
pub fn uninstall(data_home: &Path) -> std::io::Result<()> {
    for path in std::iter::once(entry_path(data_home)).chain(ICON_SIZES.iter().map(|&n| icon_path(data_home, n))) {
        match std::fs::remove_file(&path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e),
            _ => {}
        }
    }
    Ok(())
}

fn desktop_file(exe: &Path) -> String {
    format!(
        "[Desktop Entry]\nType=Application\nName=Elite Nexus\nGenericName=Elite Dangerous companion\n\
         Comment=Exploration, exobiology, trading, engineering and HUD colours for Elite Dangerous\n\
         Exec={exec}\nIcon={APP_ID}\nTerminal=false\nCategories=Game;\n\
         Keywords=Elite;Dangerous;Odyssey;EDDN;Spansh;Inara;Exobiology;\nStartupWMClass={APP_ID}\n",
        exec = exec_value(exe)
    )
}

/// The program path as the Desktop Entry spec wants it in `Exec`: quoted; `"`, `` ` `` and `$` escaped for the
/// quoting, then every backslash doubled again for the file's own string escapes; `%` doubled (field codes).
fn exec_value(exe: &Path) -> String {
    let mut s = String::from("\"");
    for c in exe.to_string_lossy().chars() {
        match c {
            '"' | '`' | '$' => {
                s.push_str("\\\\");
                s.push(c);
            }
            '\\' => s.push_str("\\\\\\\\"),
            '%' => s.push_str("%%"),
            _ => s.push(c),
        }
    }
    s.push('"');
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("nexus-desktop-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn quotes_the_program_path() {
        assert_eq!(exec_value(Path::new("/home/cmdr/Elite Nexus/EliteNexus")), "\"/home/cmdr/Elite Nexus/EliteNexus\"");
        assert_eq!(exec_value(Path::new("/opt/$x/100%/EliteNexus")), "\"/opt/\\\\$x/100%%/EliteNexus\"");
    }

    #[test]
    fn installs_checks_and_removes() {
        let home = temp("roundtrip");
        let exe = Path::new("/games/Elite Nexus/EliteNexus");
        assert_eq!(state(&home, exe), State::Missing);
        install(&home, exe).unwrap();
        assert_eq!(state(&home, exe), State::Current);
        assert_eq!(state(&home, Path::new("/elsewhere/EliteNexus")), State::Elsewhere("/games/Elite Nexus/EliteNexus".into()));
        let icon = image::open(icon_path(&home, 128)).unwrap();
        assert_eq!((icon.width(), icon.height()), (128, 128));
        let text = std::fs::read_to_string(entry_path(&home)).unwrap();
        assert!(text.starts_with("[Desktop Entry]\n") && text.contains("\nIcon=elite-nexus\n") && text.contains("\nStartupWMClass=elite-nexus\n"));
        // Someone else's file in the same folders stays.
        std::fs::write(home.join("applications").join("other.desktop"), "x").unwrap();
        uninstall(&home).unwrap();
        assert_eq!(state(&home, exe), State::Missing);
        assert!(!icon_path(&home, 32).exists() && home.join("applications").join("other.desktop").exists());
        uninstall(&home).unwrap();
        let _ = std::fs::remove_dir_all(&home);
    }
}
