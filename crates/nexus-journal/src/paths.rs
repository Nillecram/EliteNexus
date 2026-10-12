//! Locating the journal folder, the bindings folder, and enumerating journal files in order.

use crate::time::Timestamp;
use std::path::{Path, PathBuf};

/// `%USERPROFILE%\Saved Games\Frontier Developments\Elite Dangerous` (honouring folder redirection).
pub fn default_journal_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        if let Some(p) = known_folders::get_known_folder_path(known_folders::KnownFolder::SavedGames) {
            return Some(p.join("Frontier Developments").join("Elite Dangerous"));
        }
    }
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"))?;
    Some(PathBuf::from(home).join("Saved Games").join("Frontier Developments").join("Elite Dangerous"))
}

/// `%LOCALAPPDATA%\Frontier Developments\Elite Dangerous\Options\Bindings`.
pub fn default_bindings_dir() -> Option<PathBuf> {
    let base = std::env::var_os("LOCALAPPDATA")?;
    Some(PathBuf::from(base).join("Frontier Developments").join("Elite Dangerous").join("Options").join("Bindings"))
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
}
