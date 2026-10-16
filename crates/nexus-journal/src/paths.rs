//! Locating the game's folders (journals, Options, Steam libraries) and enumerating journal files in order.
//!
//! On Linux the game runs in a Wine prefix (Steam's Proton, Lutris, Heroic). Its Windows user folder is
//! `<prefix>/drive_c/users/<name>`, and the journals and Options sit inside it just as on Windows.

use crate::time::Timestamp;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Elite Dangerous' Steam app id: Proton keeps the game's prefix in `steamapps/compatdata/359320/pfx`.
pub const STEAM_APP_ID: &str = "359320";
/// The journal folder inside a Windows user folder.
const JOURNAL_IN_PROFILE: [&str; 3] = ["Saved Games", "Frontier Developments", "Elite Dangerous"];
/// The game's Options folder inside `%LOCALAPPDATA%`.
const OPTIONS_IN_LOCAL: [&str; 3] = ["Frontier Developments", "Elite Dangerous", "Options"];

/// The journal folder the app reads, when it has said so (see [`set_journal_dir_in_use`]).
static JOURNAL_DIR_IN_USE: Mutex<Option<PathBuf>> = Mutex::new(None);

/// `%USERPROFILE%\Saved Games\Frontier Developments\Elite Dangerous` (honouring folder redirection).
///
/// On Linux, the same folder in the game's Wine prefix: Proton's in every Steam library (native, Flatpak and
/// Snap Steam), Lutris's and Heroic's usual folders, `~/.wine`. With several, the one written to last.
pub fn default_journal_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        if let Some(p) = known_folders::get_known_folder_path(known_folders::KnownFolder::SavedGames) {
            return Some(p.join("Frontier Developments").join("Elite Dangerous"));
        }
        let home = std::env::var_os("USERPROFILE")?;
        Some(JOURNAL_IN_PROFILE.iter().fold(PathBuf::from(home), |p, c| p.join(c)))
    }
    #[cfg(not(windows))]
    {
        let found = wine_prefixes().iter().flat_map(|p| journal_dirs_in_prefix(p)).collect::<Vec<_>>();
        found.into_iter().max_by_key(|d| std::fs::metadata(d).and_then(|m| m.modified()).ok())
    }
}

/// Tells [`options_dir`] which journal folder the app reads: a custom one may sit in a Wine prefix of its own.
pub fn set_journal_dir_in_use(dir: Option<&Path>) {
    if let Ok(mut d) = JOURNAL_DIR_IN_USE.lock() {
        *d = dir.map(Path::to_path_buf);
    }
}

/// The game's Options folder (control bindings, graphics overrides):
/// `%LOCALAPPDATA%\Frontier Developments\Elite Dangerous\Options`.
///
/// On Linux, the same folder in the Wine prefix that holds the journal folder in use, else in the prefix
/// [`default_journal_dir`] finds.
pub fn options_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        let base = std::env::var_os("LOCALAPPDATA")?;
        Some(OPTIONS_IN_LOCAL.iter().fold(PathBuf::from(base), |p, c| p.join(c)))
    }
    #[cfg(not(windows))]
    {
        wine_journal_dir().and_then(|d| options_dir_beside(&d))
    }
}

/// Linux: the `drive_c` folder of the game's Wine prefix (the one holding the journal folder in use, else the
/// one found), where a Frontier or Epic launcher run under Wine installs the game. Windows: `None`.
pub fn wine_drive_c() -> Option<PathBuf> {
    if cfg!(windows) {
        return None;
    }
    // <drive_c>/users/<user>/Saved Games/Frontier Developments/Elite Dangerous
    let journals = wine_journal_dir()?;
    let drive_c = journals.ancestors().nth(5)?;
    drive_c.file_name().is_some_and(|n| n.eq_ignore_ascii_case("drive_c")).then(|| drive_c.to_path_buf())
}

/// The journal folder in use if it sits in a Wine user folder, else the one found automatically.
fn wine_journal_dir() -> Option<PathBuf> {
    let in_use = JOURNAL_DIR_IN_USE.lock().ok().and_then(|d| d.clone());
    in_use.filter(|d| options_dir_beside(d).is_some()).or_else(default_journal_dir)
}

/// `<user>/AppData/Local/Frontier Developments/Elite Dangerous/Options` for a journal folder at
/// `<user>/Saved Games/Frontier Developments/Elite Dangerous`; `None` for a journal folder anywhere else.
pub fn options_dir_beside(journal_dir: &Path) -> Option<PathBuf> {
    let mut profile = journal_dir;
    for name in JOURNAL_IN_PROFILE.iter().rev() {
        if !profile.file_name()?.to_string_lossy().eq_ignore_ascii_case(name) {
            return None;
        }
        profile = profile.parent()?;
    }
    Some(OPTIONS_IN_LOCAL.iter().fold(profile.join("AppData").join("Local"), |p, c| p.join(c)))
}

/// `%LOCALAPPDATA%\Frontier Developments\Elite Dangerous\Options\Bindings` (on Linux, in the game's Wine prefix).
pub fn default_bindings_dir() -> Option<PathBuf> {
    Some(options_dir()?.join("Bindings"))
}

/// Every Steam library folder on this PC (each holds `steamapps`), from Steam's `libraryfolders.vdf`.
/// Windows: Steam in Program Files. Linux: native Steam, Flatpak Steam and Snap Steam.
pub fn steam_libraries() -> Vec<PathBuf> {
    let mut out: Vec<(PathBuf, PathBuf)> = Vec::new();
    for root in steam_roots() {
        let listed = std::fs::read_to_string(root.join("steamapps").join("libraryfolders.vdf")).map(|t| parse_library_folders(&t)).unwrap_or_default();
        for lib in std::iter::once(root).chain(listed) {
            if !lib.join("steamapps").is_dir() {
                continue;
            }
            // ~/.steam/steam is usually a link to ~/.local/share/Steam: compare where they lead.
            let key = std::fs::canonicalize(&lib).unwrap_or_else(|_| lib.clone());
            if !out.iter().any(|(k, _)| same_path(k, &key)) {
                out.push((key, lib));
            }
        }
    }
    out.into_iter().map(|(_, lib)| lib).collect()
}

/// The library folders listed in Steam's `libraryfolders.vdf` (`"path"  "D:\\SteamLibrary"`).
pub fn parse_library_folders(vdf: &str) -> Vec<PathBuf> {
    vdf.lines()
        .filter_map(|l| {
            let rest = l.trim().strip_prefix("\"path\"")?.trim();
            let v = rest.strip_prefix('"')?.strip_suffix('"')?;
            Some(PathBuf::from(v.replace("\\\\", "\\")))
        })
        .collect()
}

#[cfg(windows)]
fn steam_roots() -> Vec<PathBuf> {
    ["ProgramFiles(x86)", "ProgramFiles"].iter().filter_map(std::env::var_os).map(|pf| PathBuf::from(pf).join("Steam")).collect()
}

#[cfg(not(windows))]
fn steam_roots() -> Vec<PathBuf> {
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else { return Vec::new() };
    let mut roots: Vec<PathBuf> = std::env::var_os("XDG_DATA_HOME").map(|x| PathBuf::from(x).join("Steam")).into_iter().collect();
    for rel in [".local/share/Steam", ".steam/steam", ".steam/root", ".var/app/com.valvesoftware.Steam/.local/share/Steam", ".var/app/com.valvesoftware.Steam/data/Steam", "snap/steam/common/.local/share/Steam"] {
        roots.push(home.join(rel));
    }
    roots
}

fn same_path(a: &Path, b: &Path) -> bool {
    if cfg!(windows) { a.to_string_lossy().eq_ignore_ascii_case(&b.to_string_lossy()) } else { a == b }
}

/// Wine prefixes that may hold the game: Proton's in every Steam library, Lutris's (`~/Games/<game>`),
/// Heroic's (`~/Games/Heroic/Prefixes/default/<game>`) and `~/.wine`.
#[cfg(not(windows))]
fn wine_prefixes() -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = steam_libraries().iter().map(|l| l.join("steamapps").join("compatdata").join(STEAM_APP_ID).join("pfx")).collect();
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        out.push(home.join(".wine"));
        for parent in [home.join("Games"), home.join("Games/Heroic/Prefixes/default"), home.join("Games/Heroic/Prefixes")] {
            out.extend(std::fs::read_dir(parent).into_iter().flatten().flatten().map(|e| e.path()));
        }
    }
    out
}

/// The journal folders in a Wine prefix: one per Windows user that has played (Proton's is `steamuser`).
#[cfg(not(windows))]
fn journal_dirs_in_prefix(prefix: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(prefix.join("drive_c").join("users"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|user| JOURNAL_IN_PROFILE.iter().fold(user.path(), |p, c| p.join(c)))
        .filter(|d| d.is_dir())
        .collect()
}

/// A journal file with the session start time and part number decoded from its name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JournalFile {
    pub path: PathBuf,
    pub file_name: String,
    /// Session start time encoded in the file name (local time of the game client, treated as UTC).
    pub started: Timestamp,
    pub part: u32,
    pub size: u64,
}

/// Decodes `Journal.2026-10-12T073031.01.log` (current) and `Journal.170404170616.01.log` (pre-2021).
pub fn parse_journal_file_name(name: &str) -> Option<(Timestamp, u32)> {
    let rest = name.strip_prefix("Journal.")?.strip_suffix(".log")?;
    let (stamp, part) = rest.rsplit_once('.')?;
    let part: u32 = part.parse().ok()?;
    let ts = if stamp.len() == 17 && stamp.as_bytes()[10] == b'T' {
        // 2026-10-12T073031
        let iso = format!("{}T{}:{}:{}Z", &stamp[..10], &stamp[11..13], &stamp[13..15], &stamp[15..17]);
        Timestamp::parse(&iso)?
    } else if stamp.len() == 12 && stamp.bytes().all(|b| b.is_ascii_digit()) {
        // 170404170616 -> 2017-04-04T17:06:16
        let iso = format!(
            "20{}-{}-{}T{}:{}:{}Z",
            &stamp[0..2],
            &stamp[2..4],
            &stamp[4..6],
            &stamp[6..8],
            &stamp[8..10],
            &stamp[10..12]
        );
        Timestamp::parse(&iso)?
    } else {
        return None;
    };
    Some((ts, part))
}

/// All `Journal.*.log` files in `dir`, oldest first.
pub fn list_journal_files(dir: &Path) -> std::io::Result<Vec<JournalFile>> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let file_name = entry.file_name().to_string_lossy().into_owned();
        let Some((started, part)) = parse_journal_file_name(&file_name) else { continue };
        let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
        out.push(JournalFile { path: entry.path(), file_name, started, part, size });
    }
    out.sort_by(|a, b| (a.started, a.part, &a.file_name).cmp(&(b.started, b.part, &b.file_name)));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_both_file_name_styles() {
        let (t, p) = parse_journal_file_name("Journal.2026-10-12T073031.01.log").unwrap();
        assert_eq!(t.to_iso(), "2026-10-12T07:30:31Z");
        assert_eq!(p, 1);
        let (t, p) = parse_journal_file_name("Journal.170404170616.02.log").unwrap();
        assert_eq!(t.to_iso(), "2017-04-04T17:06:16Z");
        assert_eq!(p, 2);
        assert!(parse_journal_file_name("Status.json").is_none());
    }

    #[test]
    fn reads_steam_library_folders() {
        let vdf = "\"libraryfolders\"\n{\n\t\"0\"\n\t{\n\t\t\"path\"\t\t\"C:\\\\Program Files (x86)\\\\Steam\"\n\t\t\"label\"\t\t\"\"\n\t}\n\t\"1\"\n\t{\n\t\t\"path\"\t\t\"D:\\\\SteamLibrary\"\n\t}\n}";
        assert_eq!(parse_library_folders(vdf), vec![PathBuf::from(r"C:\Program Files (x86)\Steam"), PathBuf::from(r"D:\SteamLibrary")]);
        let linux = "\"libraryfolders\"\n{\n\t\"0\"\n\t{\n\t\t\"path\"\t\t\"/home/cmdr/.local/share/Steam\"\n\t}\n\t\"1\"\n\t{\n\t\t\"path\"\t\t\"/mnt/games/SteamLibrary\"\n\t}\n}";
        assert_eq!(parse_library_folders(linux), vec![PathBuf::from("/home/cmdr/.local/share/Steam"), PathBuf::from("/mnt/games/SteamLibrary")]);
    }

    #[test]
    fn finds_options_next_to_the_journals() {
        let pfx = PathBuf::from("steamapps").join("compatdata").join(STEAM_APP_ID).join("pfx").join("drive_c").join("users").join("steamuser");
        let journals = pfx.join("Saved Games").join("Frontier Developments").join("Elite Dangerous");
        let options = pfx.join("AppData").join("Local").join("Frontier Developments").join("Elite Dangerous").join("Options");
        assert_eq!(options_dir_beside(&journals), Some(options));
        assert_eq!(options_dir_beside(&PathBuf::from("replays").join("Elite Dangerous")), None);
    }
}
