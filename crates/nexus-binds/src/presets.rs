//! Preset discovery: which `.binds` files exist, which version of each is current, and which
//! preset the game uses for each control category.
//!
//! The game keeps user presets in `%LOCALAPPDATA%\Frontier Developments\Elite Dangerous\Options\
//! Bindings` as `<Name>.<Major>.<Minor>.binds` (e.g. `Custom.4.2.binds`); older clients' files
//! (`Custom.3.0.binds`) stay around after upgrades. `StartPreset.4.start` holds the active preset
//! per category, one name per line, in the order of the in-game Controls menu: General, Ship,
//! SRV, On foot (pre-Odyssey `StartPreset.start` has a single line used for everything). Built-in
//! presets such as `KeyboardMouseOnly` are not in that folder but in the game's
//! `Products\<product>\ControlSchemes` folder.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use rustc_hash::{FxHashMap, FxHashSet};
use serde::{Deserialize, Serialize};

use crate::bindings::{ActionBinding, ActionKind, Bindings, Version};
use crate::catalogue::action_info;
use crate::error::BindsError;

/// The four independently selectable control presets of the Odyssey client.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum PresetCategory {
    General,
    Ship,
    Srv,
    OnFoot,
}

impl PresetCategory {
    /// In `StartPreset.4.start` line order.
    pub const ALL: [Self; 4] = [Self::General, Self::Ship, Self::Srv, Self::OnFoot];

    pub fn label(self) -> &'static str {
        match self {
            Self::General => "General",
            Self::Ship => "Ship",
            Self::Srv => "SRV",
            Self::OnFoot => "On foot",
        }
    }
}

/// Parsed `StartPreset.*.start`: the active preset name per category.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StartPreset {
    pub general: String,
    pub ship: String,
    pub srv: String,
    pub on_foot: String,
}

impl StartPreset {
    /// Parses the file text. One line applies to every category; otherwise lines map to General,
    /// Ship, SRV, On foot in order (missing lines fall back to the first). `None` if empty.
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        let lines: Vec<&str> = text
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .collect();
        let first = *lines.first()?;
        let line = |i: usize| lines.get(i).copied().unwrap_or(first).to_owned();
        Some(Self {
            general: line(0),
            ship: line(1),
            srv: line(2),
            on_foot: line(3),
        })
    }

    /// Reads and parses a start-preset file; `Ok(None)` if it is empty.
    pub fn load(path: impl AsRef<Path>) -> Result<Option<Self>, BindsError> {
        let path = path.as_ref();
        let bytes = std::fs::read(path).map_err(|e| BindsError::io(path, e))?;
        Ok(Self::parse(&String::from_utf8_lossy(&bytes)))
    }

    /// Preset name for `category`.
    pub fn get(&self, category: PresetCategory) -> &str {
        match category {
            PresetCategory::General => &self.general,
            PresetCategory::Ship => &self.ship,
            PresetCategory::Srv => &self.srv,
            PresetCategory::OnFoot => &self.on_foot,
        }
    }

    /// Categories that use the preset `name`.
    pub fn categories_for(&self, name: &str) -> Vec<PresetCategory> {
        PresetCategory::ALL
            .into_iter()
            .filter(|&c| self.get(c) == name)
            .collect()
    }
}

/// The current file of one preset.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PresetFile {
    /// Preset name as used in `StartPreset.*.start` (the file name without version and
    /// extension).
    pub name: String,
    pub path: PathBuf,
    /// Version encoded in the file name (`Custom.4.2.binds` → 4.2).
    pub version: Option<Version>,
    pub modified: Option<SystemTime>,
    /// Categories for which the game currently uses this preset (empty when inactive).
    pub active_for: Vec<PresetCategory>,
    /// The other files of the same preset (superseded or for another client version), in the
    /// same preference order that picked [`path`](Self::path).
    pub older_versions: Vec<PathBuf>,
}

impl PresetFile {
    pub fn is_active(&self) -> bool {
        !self.active_for.is_empty()
    }

    pub fn load(&self) -> Result<Bindings, BindsError> {
        Bindings::load(&self.path)
    }
}

/// Result of [`find_presets`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PresetDirectory {
    pub dir: PathBuf,
    /// One entry per preset (its current file), active presets first (by category), then by
    /// name.
    pub presets: Vec<PresetFile>,
    pub start_preset: Option<StartPreset>,
    pub start_preset_path: Option<PathBuf>,
}

impl PresetDirectory {
    /// The preset called `name`, if it has a file in this folder.
    pub fn get(&self, name: &str) -> Option<&PresetFile> {
        self.presets.iter().find(|p| p.name == name)
    }

    /// Name of the active preset for `category` according to `StartPreset.*.start`.
    pub fn active_name(&self, category: PresetCategory) -> Option<&str> {
        self.start_preset.as_ref().map(|s| s.get(category))
    }

    /// The user preset file active for `category`; `None` when the category uses a built-in
    /// preset (not in this folder) or there is no start file.
    pub fn active(&self, category: PresetCategory) -> Option<&PresetFile> {
        self.get(self.active_name(category)?)
    }

    /// The preset to show when only one can be shown: the active Ship preset, else any active
    /// preset in this folder, else the most recently modified one.
    pub fn primary(&self) -> Option<&PresetFile> {
        self.active(PresetCategory::Ship)
            .or_else(|| PresetCategory::ALL.into_iter().find_map(|c| self.active(c)))
            .or_else(|| self.presets.iter().max_by_key(|p| p.modified))
    }

    /// Finds the `.binds` file for preset `name`: this folder first, then each of
    /// `builtin_dirs` (e.g. the game's `ControlSchemes` folder) as `<name>.binds`.
    pub fn resolve(&self, name: &str, builtin_dirs: &[&Path]) -> Option<PathBuf> {
        if let Some(p) = self.get(name) {
            return Some(p.path.clone());
        }
        builtin_dirs
            .iter()
            .map(|dir| dir.join(format!("{name}.binds")))
            .find(|p| p.is_file())
    }

    /// Loads the bindings the game actually uses: every category's actions come from the preset
    /// selected for it (see [`merge_by_category`]). Without a start file this is just the
    /// [`primary`](Self::primary) preset.
    pub fn load_effective(&self, builtin_dirs: &[&Path]) -> Result<Bindings, BindsError> {
        let Some(start) = &self.start_preset else {
            return self
                .primary()
                .ok_or_else(|| BindsError::NoPresets(self.dir.clone()))?
                .load();
        };
        let mut loaded: Vec<(&str, Bindings)> = Vec::with_capacity(4);
        for category in PresetCategory::ALL {
            let name = start.get(category);
            if loaded.iter().all(|(n, _)| *n != name) {
                let path = self
                    .resolve(name, builtin_dirs)
                    .ok_or_else(|| BindsError::PresetNotFound(name.to_owned()))?;
                loaded.push((name, Bindings::load(&path)?));
            }
        }
        let parts: Vec<(PresetCategory, &Bindings)> = PresetCategory::ALL
            .into_iter()
            .filter_map(|c| {
                loaded
                    .iter()
                    .find(|(n, _)| *n == start.get(c))
                    .map(|(_, b)| (c, b))
            })
            .collect();
        Ok(merge_by_category(&parts))
    }
}

/// `%LOCALAPPDATA%\Frontier Developments\Elite Dangerous\Options\Bindings`.
pub fn default_bindings_dir() -> Option<PathBuf> {
    let base = std::env::var_os("LOCALAPPDATA")?;
    Some(
        PathBuf::from(base)
            .join("Frontier Developments")
            .join("Elite Dangerous")
            .join("Options")
            .join("Bindings"),
    )
}

/// Splits a preset file name: `Custom.4.2.binds` → (`Custom`, 4.2), `My.Preset.3.binds` →
/// (`My.Preset`, 3.0), `KeyboardMouseOnly.binds` → (`KeyboardMouseOnly`, none). `None` for
/// anything that is not a `.binds` file.
pub fn parse_preset_file_name(file_name: &str) -> Option<(String, Option<Version>)> {
    let split = file_name.len().checked_sub(".binds".len())?;
    let (stem, ext) = (file_name.get(..split)?, file_name.get(split..)?);
    if !ext.eq_ignore_ascii_case(".binds") || stem.is_empty() {
        return None;
    }
    let number = |s: &str| -> Option<u32> {
        if !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()) {
            s.parse().ok()
        } else {
            None
        }
    };
    if let Some((rest, minor)) = stem.rsplit_once('.')
        && let Some(minor) = number(minor)
    {
        if let Some((name, major)) = rest.rsplit_once('.')
            && let Some(major) = number(major)
            && !name.is_empty()
        {
            return Some((name.to_owned(), Some(Version { major, minor })));
        }
        if !rest.is_empty() {
            return Some((
                rest.to_owned(),
                Some(Version {
                    major: minor,
                    minor: 0,
                }),
            ));
        }
    }
    Some((stem.to_owned(), None))
}

/// `StartPreset.4.start` → `Some(Some(4))`, `StartPreset.start` → `Some(None)`.
fn parse_start_file_name(file_name: &str) -> Option<Option<u32>> {
    let lower = file_name.to_ascii_lowercase();
    let middle = lower.strip_prefix("startpreset")?.strip_suffix(".start")?;
    if middle.is_empty() {
        return Some(None);
    }
    middle.strip_prefix('.')?.parse().ok().map(Some)
}

/// Lists the presets in a bindings folder, keeping the newest file per preset and marking the
/// active ones from the highest-numbered `StartPreset.*.start`.
///
/// When the start file is versioned (`StartPreset.4.start`), files with the same major version
/// are preferred over newer majors, since that is what the matching client reads.
pub fn find_presets(dir: impl AsRef<Path>) -> Result<PresetDirectory, BindsError> {
    let dir = dir.as_ref();
    let entries = std::fs::read_dir(dir).map_err(|e| BindsError::io(dir, e))?;

    let mut files: Vec<(String, Option<Version>, PathBuf, Option<SystemTime>)> = Vec::new();
    let mut start_file: Option<(Option<u32>, PathBuf)> = None;
    for entry in entries {
        let entry = entry.map_err(|e| BindsError::io(dir, e))?;
        let file_name = entry.file_name().to_string_lossy().into_owned();
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if let Some(number) = parse_start_file_name(&file_name) {
            if start_file.as_ref().is_none_or(|(best, _)| number > *best) {
                start_file = Some((number, path));
            }
            continue;
        }
        let Some((name, version)) = parse_preset_file_name(&file_name) else {
            continue;
        };
        let modified = entry.metadata().ok().and_then(|m| m.modified().ok());
        files.push((name, version, path, modified));
    }

    let (start_preset, start_preset_path, preferred_major) = match start_file {
        Some((number, path)) => match StartPreset::load(&path) {
            Ok(start) => (start, Some(path), number),
            Err(err) => {
                tracing::warn!("cannot read {}: {err}", path.display());
                (None, Some(path), number)
            }
        },
        None => (None, None, None),
    };

    // Newest file per preset name.
    let rank = |v: &Option<Version>, m: &Option<SystemTime>| {
        (
            preferred_major.is_some_and(|major| v.is_some_and(|v| v.major == major)),
            *v,
            *m,
        )
    };
    files.sort_by(|a, b| {
        a.0.cmp(&b.0)
            .then_with(|| rank(&b.1, &b.3).cmp(&rank(&a.1, &a.3)))
    });
    let mut presets: Vec<PresetFile> = Vec::new();
    for (name, version, path, modified) in files {
        match presets.last_mut() {
            Some(last) if last.name == name => last.older_versions.push(path),
            _ => {
                let active_for = start_preset
                    .as_ref()
                    .map(|s| s.categories_for(&name))
                    .unwrap_or_default();
                presets.push(PresetFile {
                    name,
                    path,
                    version,
                    modified,
                    active_for,
                    older_versions: Vec::new(),
                });
            }
        }
    }
    presets.sort_by(|a, b| {
        let key = |p: &PresetFile| {
            p.active_for
                .first()
                .copied()
                .map_or(usize::MAX, |c| c as usize)
        };
        key(a)
            .cmp(&key(b))
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    Ok(PresetDirectory {
        dir: dir.to_path_buf(),
        presets,
        start_preset,
        start_preset_path,
    })
}

/// Merges per-category presets into the bindings the game effectively uses: each action is
/// taken from the preset selected for its category
/// ([`Context::preset_category`](crate::Context::preset_category)); an action missing from that
/// preset is unbound, and legacy actions that only an older (built-in) preset carries are
/// dropped. Settings come from the Ship preset, except on-foot and SRV settings which come from
/// their presets.
pub fn merge_by_category(parts: &[(PresetCategory, &Bindings)]) -> Bindings {
    let preset_for =
        |category: PresetCategory| parts.iter().find(|(c, _)| *c == category).map(|(_, b)| *b);
    let Some(base) = preset_for(PresetCategory::Ship).or_else(|| parts.first().map(|(_, b)| *b))
    else {
        return Bindings::default();
    };

    let indexes: Vec<FxHashMap<&str, &ActionBinding>> = parts
        .iter()
        .map(|(_, b)| b.actions.iter().map(|a| (a.action.as_str(), a)).collect())
        .collect();
    let index_of = |b: &Bindings| parts.iter().position(|(_, p)| std::ptr::eq(*p, b));

    // The base preset's actions, then anything only the others have — except legacy actions
    // that older built-in presets still carry but the game no longer reads.
    let mut seen: FxHashSet<&str> = FxHashSet::default();
    let mut order: Vec<&str> = Vec::with_capacity(base.actions.len());
    for a in &base.actions {
        if seen.insert(a.action.as_str()) {
            order.push(a.action.as_str());
        }
    }
    for (_, b) in parts {
        for a in &b.actions {
            if !action_info(&a.action).legacy && seen.insert(a.action.as_str()) {
                order.push(a.action.as_str());
            }
        }
    }

    let actions = order
        .into_iter()
        .map(|id| {
            let source = preset_for(action_info(id).context.preset_category()).unwrap_or(base);
            let found = index_of(source).and_then(|i| indexes[i].get(id));
            match found {
                Some(a) => (*a).clone(),
                None => {
                    let kind = indexes
                        .iter()
                        .find_map(|ix| ix.get(id))
                        .map_or(ActionKind::Button, |a| a.kind);
                    ActionBinding::unbound(id, kind)
                }
            }
        })
        .collect();

    let setting_category = |name: &str| {
        if name.contains("Humanoid") || name.contains("OnFoot") {
            PresetCategory::OnFoot
        } else if name.contains("Buggy")
            || name.contains("SRV")
            || name.contains("Turret")
            || name.starts_with("DriveAssist")
        {
            PresetCategory::Srv
        } else {
            PresetCategory::Ship
        }
    };
    let mut settings: Vec<(String, String)> = base
        .settings
        .iter()
        .filter(|(n, _)| setting_category(n) == PresetCategory::Ship)
        .cloned()
        .collect();
    for category in [PresetCategory::Srv, PresetCategory::OnFoot] {
        let source = preset_for(category).unwrap_or(base);
        settings.extend(
            source
                .settings
                .iter()
                .filter(|(n, _)| setting_category(n) == category)
                .cloned(),
        );
    }

    let mut names: Vec<&str> = Vec::new();
    for (_, b) in parts {
        if !names.contains(&b.preset_name.as_str()) {
            names.push(&b.preset_name);
        }
    }

    let mut merged = Bindings {
        preset_name: names.join(" + "),
        version: base.version,
        keyboard_layout: base.keyboard_layout.clone(),
        actions,
        settings,
        devices: Default::default(),
        sources: parts
            .iter()
            .map(|(c, b)| (*c, b.preset_name.clone()))
            .collect(),
    };
    merged.refresh_devices();
    merged
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preset_file_names() {
        assert_eq!(
            parse_preset_file_name("Custom.4.2.binds"),
            Some(("Custom".into(), Some(Version { major: 4, minor: 2 })))
        );
        assert_eq!(
            parse_preset_file_name("AsterEDAP.4.2.BINDS"),
            Some(("AsterEDAP".into(), Some(Version { major: 4, minor: 2 })))
        );
        assert_eq!(
            parse_preset_file_name("My.Preset.3.0.binds"),
            Some(("My.Preset".into(), Some(Version { major: 3, minor: 0 })))
        );
        assert_eq!(
            parse_preset_file_name("Old.3.binds"),
            Some(("Old".into(), Some(Version { major: 3, minor: 0 })))
        );
        assert_eq!(
            parse_preset_file_name("KeyboardMouseOnly.binds"),
            Some(("KeyboardMouseOnly".into(), None))
        );
        assert_eq!(
            parse_preset_file_name("My.Preset.binds"),
            Some(("My.Preset".into(), None))
        );
        assert_eq!(parse_preset_file_name(".binds"), None);
        assert_eq!(parse_preset_file_name("StartPreset.4.start"), None);
        assert_eq!(parse_preset_file_name("binds"), None);
    }

    #[test]
    fn start_file_names() {
        assert_eq!(parse_start_file_name("StartPreset.4.start"), Some(Some(4)));
        assert_eq!(parse_start_file_name("StartPreset.start"), Some(None));
        assert_eq!(parse_start_file_name("startpreset.5.START"), Some(Some(5)));
        assert_eq!(parse_start_file_name("StartPreset.x.start"), None);
        assert_eq!(parse_start_file_name("Custom.4.2.binds"), None);
    }

    #[test]
    fn start_preset_lines() {
        let s = StartPreset::parse("AsterEDAP\r\nAsterEDAP\r\nCustom\r\nKeyboardMouseOnly\r\n")
            .unwrap();
        assert_eq!(s.get(PresetCategory::General), "AsterEDAP");
        assert_eq!(s.get(PresetCategory::Ship), "AsterEDAP");
        assert_eq!(s.get(PresetCategory::Srv), "Custom");
        assert_eq!(s.get(PresetCategory::OnFoot), "KeyboardMouseOnly");
        assert_eq!(
            s.categories_for("AsterEDAP"),
            [PresetCategory::General, PresetCategory::Ship]
        );
        assert!(s.categories_for("Nope").is_empty());

        let single = StartPreset::parse("\u{feff}Custom\n").unwrap();
        assert!(
            PresetCategory::ALL
                .iter()
                .all(|&c| single.get(c) == "Custom")
        );
        assert_eq!(StartPreset::parse(" \n\n"), None);
    }

    fn binds(name: &str, body: &str) -> Bindings {
        Bindings::parse(&format!(
            r#"<Root PresetName="{name}" MajorVersion="4" MinorVersion="2">{body}</Root>"#
        ))
        .unwrap()
    }

    #[test]
    fn merge_takes_each_category_from_its_preset() {
        let ship = binds(
            "ShipPreset",
            r#"<MouseXMode Value="Bindings_MouseYaw"/><MouseHumanoidXMode Value="ShipValue"/>
               <LandingGearToggle><Primary Device="Keyboard" Key="Key_L"/><Secondary Device="{NoDevice}" Key=""/></LandingGearToggle>
               <UI_Select><Primary Device="Keyboard" Key="Key_Space"/><Secondary Device="{NoDevice}" Key=""/></UI_Select>
               <HumanoidJumpButton><Primary Device="Keyboard" Key="Key_J"/><Secondary Device="{NoDevice}" Key=""/></HumanoidJumpButton>"#,
        );
        let foot = binds(
            "FootPreset",
            r#"<MouseHumanoidXMode Value="Bindings_MouseYaw"/>
               <LandingGearToggle><Primary Device="Keyboard" Key="Key_G"/><Secondary Device="{NoDevice}" Key=""/></LandingGearToggle>
               <HumanoidJumpButton><Primary Device="Keyboard" Key="Key_Space"/><Secondary Device="{NoDevice}" Key=""/></HumanoidJumpButton>
               <HumanoidPulseScanButton><Primary Device="Keyboard" Key="Key_B"/><Secondary Device="{NoDevice}" Key=""/></HumanoidPulseScanButton>"#,
        );
        let merged = merge_by_category(&[
            (PresetCategory::General, &ship),
            (PresetCategory::Ship, &ship),
            (PresetCategory::Srv, &ship),
            (PresetCategory::OnFoot, &foot),
        ]);
        assert_eq!(merged.preset_name, "ShipPreset + FootPreset");
        assert_eq!(merged.sources.len(), 4);
        assert_eq!(
            merged
                .action("LandingGearToggle")
                .unwrap()
                .primary
                .as_ref()
                .unwrap()
                .key,
            "Key_L"
        );
        assert_eq!(
            merged
                .action("HumanoidJumpButton")
                .unwrap()
                .primary
                .as_ref()
                .unwrap()
                .key,
            "Key_Space"
        );
        assert!(merged.is_bound("UI_Select"));
        assert_eq!(merged.setting("MouseXMode"), Some("Bindings_MouseYaw"));
        assert_eq!(
            merged.setting("MouseHumanoidXMode"),
            Some("Bindings_MouseYaw")
        );
        // Legacy actions only an older preset carries are dropped.
        assert!(merged.action("HumanoidPulseScanButton").is_none());

        // An on-foot action the on-foot preset lacks is unbound, even if another preset binds it.
        let empty_foot = binds("Bare", "");
        let merged = merge_by_category(&[
            (PresetCategory::Ship, &ship),
            (PresetCategory::OnFoot, &empty_foot),
        ]);
        let jump = merged.action("HumanoidJumpButton").unwrap();
        assert!(!jump.is_bound());
        assert_eq!(merge_by_category(&[]), Bindings::default());
    }
}
