//! The cockpit-light dimmer: 3Dmigoto (a Direct3D 11 hook placed next to the game) with a pattern that
//! patches the game's deferred box-light shader as the game loads it, so lights near the eyes (the
//! cockpit's own) come out dimmer.
//!
//! Elite Nexus ships only its pattern and the few instructions it inserts; the game's shader code is
//! never copied (3Dmigoto patches it in memory). 3Dmigoto itself (GPL-3.0) is downloaded from its
//! official release and checked against pinned checksums. Everything Elite Nexus puts in the game
//! folder is listed in a manifest there, and removal deletes only files that still match it.
//!
//! How it was found and tested: docs/research/hud-colours.md.

use std::io::Cursor;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const MIGOTO_VERSION: &str = "1.4.9";
pub const MIGOTO_SOURCE: &str = "https://github.com/bo3b/3Dmigoto";
pub const MIGOTO_URL: &str = "https://github.com/bo3b/3Dmigoto/releases/download/1.4.9/3Dmigoto_v1.4.9.7z";
pub const MIGOTO_ARCHIVE_SHA256: &str = "d5d05e78f0f6df7cce8ef10c669c088db6b8dbe1428ce783d8c3c93ac842dce1";
/// The files taken from the archive's `x64` folder, with their checksums.
pub const MIGOTO_FILES: [(&str, &str); 2] = [
    ("d3d11.dll", "791aa68b9f8c0742c040f9ed5fa61aa0b4d3f9ee7b9e1865bf82ed6b2254378f"),
    ("d3dcompiler_47.dll", "a05f99734f7c4822fefc12b367af21fd0976ed6608752fb1e1e80b6ece7ecbbb"),
];
pub const CONFIG: &str = "d3dx.ini";
pub const MANIFEST: &str = "EliteNexus-3Dmigoto.json";
/// How Elite Nexus's configs start, so a hand-installed one (the first test) is recognised.
const CONFIG_MARK: &str = "; Elite Nexus";
/// The hash-named shader fix of the first hand-installed test (the game's whole shader plus the
/// dimmer); the pattern replaces it.
const LEGACY_FIX: &str = "3f366702d540af3e-ps.txt";
/// Files and folders 3Dmigoto creates next to the game; removed with it.
const BYPRODUCT_FILES: [&str; 3] = ["d3dx_user.ini", "d3d11_log.txt", "d3d11_profile_log.txt"];
const BYPRODUCT_DIRS: [&str; 2] = ["ShaderCache", "ShaderFromGame"];
const FIXES_DIR: &str = "ShaderFixes";

/// Keys bound in the game while the dimmer is installed (only while the game window is in front).
pub const KEYS: [(&str, &str); 3] = [("F9", "dimmer on / off"), ("F10", "strength: 0 %, 15 %, 35 %, 60 %, 100 %"), ("F11", "reach: 5 m, 8 m, 12 m")];

/// What the dimmer does, in the game's `[Constants]` (3Dmigoto's IniParams `x`, `y`, `z`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Dimmer {
    pub on: bool,
    /// How much of a nearby light stays, 0 (off) to 1 (unchanged).
    pub strength: f32,
    /// Lights closer to the eyes than this many metres are dimmed.
    pub reach: f32,
}

impl Default for Dimmer {
    fn default() -> Self {
        Dimmer { on: true, strength: 0.35, reach: 5.0 }
    }
}

/// What Elite Nexus installed, written next to the game as [`MANIFEST`].
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Manifest {
    pub migoto_version: String,
    /// File name and SHA-256 of each file Elite Nexus put there.
    pub files: Vec<(String, String)>,
}

/// What is in a game folder.
#[derive(Debug, Clone, PartialEq)]
pub enum Status {
    /// No 3Dmigoto.
    None,
    /// Installed and managed by Elite Nexus.
    Installed { dimmer: Dimmer },
    /// Elite Nexus's config without a manifest: put there by hand (the first test). Can be taken over.
    Unmanaged { dimmer: Dimmer },
    /// Another 3Dmigoto mod (EDHM or something else): left alone.
    Foreign { edhm: bool },
}

#[derive(Debug, thiserror::Error)]
pub enum DimmerError {
    #[error("the download doesn't match the official 3Dmigoto {MIGOTO_VERSION} release (checksum {0})")]
    BadArchive(String),
    #[error("can't read the 3Dmigoto archive: {0}")]
    Archive(String),
    #[error("{0} is missing from the 3Dmigoto archive")]
    MissingFile(&'static str),
    #[error("{0} in the archive doesn't match the official release")]
    BadFile(&'static str),
    #[error("another 3Dmigoto mod is installed{}: Elite Nexus leaves it alone", if *.0 { " (EDHM)" } else { "" })]
    Foreign(bool),
    #[error("{0} in the game folder isn't the file Elite Nexus expects; it was left alone")]
    UnknownFile(String),
    #[error("Elite Nexus didn't install 3Dmigoto in this folder")]
    NotOurs,
    #[error("{path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

fn io(path: &Path) -> impl FnOnce(std::io::Error) -> DimmerError + '_ {
    move |source| DimmerError::Io { path: path.to_path_buf(), source }
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// The two DLLs from the downloaded release, after checking the archive and each file.
pub fn extract(archive: &[u8]) -> Result<Vec<(&'static str, Vec<u8>)>, DimmerError> {
    let sum = sha256_hex(archive);
    if sum != MIGOTO_ARCHIVE_SHA256 {
        return Err(DimmerError::BadArchive(sum));
    }
    let mut reader = sevenz_rust2::ArchiveReader::new(Cursor::new(archive), sevenz_rust2::Password::empty()).map_err(|e| DimmerError::Archive(e.to_string()))?;
    let names: Vec<String> = reader.archive().files.iter().map(|f| f.name().to_string()).collect();
    let mut out = Vec::new();
    for (file, expected) in MIGOTO_FILES {
        // Entry names use either separator depending on the tool that made the archive.
        let name = names.iter().find(|n| n.replace('\\', "/").eq_ignore_ascii_case(&format!("x64/{file}"))).ok_or(DimmerError::MissingFile(file))?;
        let bytes = reader.read_file(name).map_err(|e| DimmerError::Archive(e.to_string()))?;
        if sha256_hex(&bytes) != expected {
            return Err(DimmerError::BadFile(file));
        }
        out.push((file, bytes));
    }
    Ok(out)
}

/// What is in `game` (the folder holding `EliteDangerous64.exe`).
pub fn status(game: &Path) -> Status {
    let config = std::fs::read_to_string(game.join(CONFIG)).ok();
    match config {
        Some(text) if text.starts_with(CONFIG_MARK) => {
            let dimmer = read_settings(&text);
            if game.join(MANIFEST).is_file() { Status::Installed { dimmer } } else { Status::Unmanaged { dimmer } }
        }
        Some(_) => Status::Foreign { edhm: game.join("EDHM-ini").is_dir() },
        None if game.join("d3d11.dll").exists() => Status::Foreign { edhm: game.join("EDHM-ini").is_dir() },
        None => Status::None,
    }
}

/// Installs 3Dmigoto (`files` from [`extract`]) and the dimmer with `dimmer`, or takes over a
/// hand-installed Elite Nexus config (then `files` may be empty: the DLLs there are checked instead).
pub fn install(game: &Path, files: &[(&'static str, Vec<u8>)], dimmer: &Dimmer) -> Result<(), DimmerError> {
    match status(game) {
        Status::Foreign { edhm } => return Err(DimmerError::Foreign(edhm)),
        Status::None | Status::Installed { .. } | Status::Unmanaged { .. } => {}
    }
    let mut manifest = Manifest { migoto_version: MIGOTO_VERSION.to_string(), files: Vec::new() };
    for (name, expected) in MIGOTO_FILES {
        let path = game.join(name);
        let written = match files.iter().find(|(f, _)| *f == name) {
            Some((_, bytes)) => {
                if path.exists() && !matches_hash(&path, expected) {
                    return Err(DimmerError::UnknownFile(name.to_string()));
                }
                write_atomic(&path, bytes)?;
                sha256_hex(bytes)
            }
            // Taking over: the file must already be the official one.
            None if matches_hash(&path, expected) => expected.to_string(),
            None => return Err(DimmerError::UnknownFile(name.to_string())),
        };
        manifest.files.push((name.to_string(), written));
    }
    let fixes = game.join(FIXES_DIR);
    std::fs::create_dir_all(&fixes).map_err(io(&fixes))?;
    // The pattern replaces the first test's hash-named fix (which holds the game's own shader code).
    let legacy = fixes.join(LEGACY_FIX);
    if legacy.is_file() {
        std::fs::remove_file(&legacy).map_err(io(&legacy))?;
    }
    let ini = render_config(dimmer);
    write_atomic(&game.join(CONFIG), ini.as_bytes())?;
    manifest.files.push((CONFIG.to_string(), sha256_hex(ini.as_bytes())));
    let json = serde_json::to_vec_pretty(&manifest).unwrap_or_default();
    // Written last: a manifest means the install is complete.
    write_atomic(&game.join(MANIFEST), &json)?;
    Ok(())
}

/// Writes new dimmer settings (the game reads them when it starts).
pub fn save_settings(game: &Path, dimmer: &Dimmer) -> Result<(), DimmerError> {
    match status(game) {
        Status::Installed { .. } => {}
        _ => return Err(DimmerError::NotOurs),
    }
    let ini = render_config(dimmer);
    write_atomic(&game.join(CONFIG), ini.as_bytes())?;
    // Keep the manifest's checksum of the config in step, so removal recognises it.
    let path = game.join(MANIFEST);
    let mut manifest: Manifest = std::fs::read(&path).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
    match manifest.files.iter_mut().find(|(n, _)| n == CONFIG) {
        Some(entry) => entry.1 = sha256_hex(ini.as_bytes()),
        None => manifest.files.push((CONFIG.to_string(), sha256_hex(ini.as_bytes()))),
    }
    write_atomic(&path, &serde_json::to_vec_pretty(&manifest).unwrap_or_default())
}

/// What [`uninstall`] did.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Removed {
    pub removed: Vec<String>,
    /// Files that changed since Elite Nexus installed them: left in place.
    pub kept: Vec<String>,
}

/// Removes 3Dmigoto and the dimmer: the files Elite Nexus installed (only while they still match),
/// 3Dmigoto's own by-products, and the frame captures made with it.
pub fn uninstall(game: &Path) -> Result<Removed, DimmerError> {
    let manifest = match status(game) {
        Status::Installed { .. } => std::fs::read(game.join(MANIFEST)).ok().and_then(|b| serde_json::from_slice::<Manifest>(&b).ok()).unwrap_or_default(),
        // A hand-installed config: the DLLs are recognised by their checksums alone.
        Status::Unmanaged { .. } => Manifest { migoto_version: MIGOTO_VERSION.to_string(), files: MIGOTO_FILES.iter().map(|(n, h)| (n.to_string(), h.to_string())).collect() },
        _ => return Err(DimmerError::NotOurs),
    };
    let mut done = Removed::default();
    for (name, expected) in &manifest.files {
        let path = game.join(name);
        if !path.exists() {
            continue;
        }
        if name == CONFIG || matches_hash(&path, expected) {
            std::fs::remove_file(&path).map_err(io(&path))?;
            done.removed.push(name.clone());
        } else {
            done.kept.push(name.clone());
        }
    }
    // A hand-installed config isn't in the list above.
    let config = game.join(CONFIG);
    if config.is_file() && std::fs::read_to_string(&config).is_ok_and(|t| t.starts_with(CONFIG_MARK)) {
        std::fs::remove_file(&config).map_err(io(&config))?;
        done.removed.push(CONFIG.to_string());
    }
    for name in BYPRODUCT_FILES {
        let path = game.join(name);
        if path.is_file() {
            std::fs::remove_file(&path).map_err(io(&path))?;
            done.removed.push(name.to_string());
        }
    }
    let mut dirs: Vec<String> = BYPRODUCT_DIRS.iter().map(|d| d.to_string()).collect();
    dirs.extend(frame_captures(game));
    for name in dirs {
        let path = game.join(&name);
        if path.is_dir() {
            std::fs::remove_dir_all(&path).map_err(io(&path))?;
            done.removed.push(format!("{name}\\"));
        }
    }
    let fixes = game.join(FIXES_DIR);
    let legacy = fixes.join(LEGACY_FIX);
    if legacy.is_file() {
        std::fs::remove_file(&legacy).map_err(io(&legacy))?;
        done.removed.push(format!("{FIXES_DIR}\\{LEGACY_FIX}"));
    }
    // Only an empty ShaderFixes goes: anything else in it isn't ours.
    if std::fs::read_dir(&fixes).is_ok_and(|mut d| d.next().is_none()) {
        std::fs::remove_dir(&fixes).map_err(io(&fixes))?;
        done.removed.push(format!("{FIXES_DIR}\\"));
    }
    let manifest_path = game.join(MANIFEST);
    if manifest_path.is_file() {
        std::fs::remove_file(&manifest_path).map_err(io(&manifest_path))?;
        done.removed.push(MANIFEST.to_string());
    }
    Ok(done)
}

/// The `FrameAnalysis-…` folders 3Dmigoto's frame analysis wrote next to the game.
fn frame_captures(game: &Path) -> Vec<String> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(game).into_iter().flatten().flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if name.starts_with("FrameAnalysis-") && e.file_type().is_ok_and(|t| t.is_dir()) {
            out.push(name);
        }
    }
    out.sort();
    out
}

/// Folders of capture files the dimmer doesn't use: frame captures, and the shaders 3Dmigoto
/// exported (`ShaderCache`, unless the config still exports or caches them; `ShaderFromGame`, from
/// hunting mode). They hold the game's own shader code and can run to hundreds of MB.
fn leftover_dirs(game: &Path) -> Vec<String> {
    let mut dirs = frame_captures(game);
    let config = std::fs::read_to_string(game.join(CONFIG)).unwrap_or_default();
    let in_use = section(&config, "Rendering").any(|(k, v)| (k.eq_ignore_ascii_case("export_shaders") || k.eq_ignore_ascii_case("cache_shaders")) && v != "0");
    if !in_use {
        dirs.push("ShaderCache".to_string());
    }
    dirs.push("ShaderFromGame".to_string());
    dirs.retain(|d| game.join(d).symlink_metadata().is_ok_and(|m| m.is_dir()));
    dirs
}

/// Bytes of capture files left in the game folder ([`remove_leftovers`] deletes them); 0 unless
/// Elite Nexus manages the install there.
pub fn leftovers(game: &Path) -> u64 {
    if !matches!(status(game), Status::Installed { .. }) {
        return 0;
    }
    leftover_dirs(game).iter().map(|d| dir_size(&game.join(d))).sum()
}

/// Deletes the capture files [`leftovers`] counts; everything the dimmer needs stays.
pub fn remove_leftovers(game: &Path) -> Result<Removed, DimmerError> {
    if !matches!(status(game), Status::Installed { .. }) {
        return Err(DimmerError::NotOurs);
    }
    let mut done = Removed::default();
    for name in leftover_dirs(game) {
        let path = game.join(&name);
        std::fs::remove_dir_all(&path).map_err(io(&path))?;
        done.removed.push(format!("{name}\\"));
    }
    Ok(done)
}

/// Total size of the files under `dir`; links aren't followed.
fn dir_size(dir: &Path) -> u64 {
    let mut total = 0;
    let mut todo = vec![dir.to_path_buf()];
    while let Some(d) = todo.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            match e.file_type() {
                Ok(t) if t.is_dir() => todo.push(e.path()),
                Ok(t) if t.is_file() => total += e.metadata().map(|m| m.len()).unwrap_or(0),
                _ => {}
            }
        }
    }
    total
}

/// The `key = value` lines of one `[name]` section of an ini (comments skipped).
fn section<'a>(ini: &'a str, name: &'a str) -> impl Iterator<Item = (&'a str, &'a str)> + 'a {
    let mut inside = false;
    ini.lines().filter_map(move |line| {
        let line = line.trim();
        if let Some(head) = line.strip_prefix('[') {
            inside = head.strip_suffix(']').is_some_and(|h| h.trim().eq_ignore_ascii_case(name));
            return None;
        }
        if !inside || line.starts_with(';') {
            return None;
        }
        let (key, value) = line.split_once('=')?;
        Some((key.trim(), value.trim()))
    })
}

fn matches_hash(path: &Path, expected: &str) -> bool {
    std::fs::read(path).is_ok_and(|b| sha256_hex(&b) == expected)
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), DimmerError> {
    let tmp = path.with_extension("elite-nexus-tmp");
    std::fs::write(&tmp, bytes).map_err(io(&tmp))?;
    std::fs::rename(&tmp, path).map_err(io(path))
}

/// The dimmer settings in a config's `[Constants]` (defaults for anything missing).
pub fn read_settings(ini: &str) -> Dimmer {
    let mut d = Dimmer::default();
    for (key, value) in section(ini, "Constants") {
        let Ok(v) = value.parse::<f32>() else { continue };
        match key {
            "x" => d.strength = v.clamp(0.0, 1.0),
            "y" => d.reach = v.max(0.0),
            "z" => d.on = v != 0.0,
            _ => {}
        }
    }
    d
}

/// The section names of the pattern (one name, so nothing else in an ini can collide with it).
const REGEX: &str = "ShaderRegexEliteNexusCockpitLights";

/// The box-light shader, recognised by its inputs (light position and range in camera space, packed
/// colour, view ray, pixel position) and by how it ends (colour and luminance scaled by `cb2[0].x`).
/// Lines are joined after trimming, as 3Dmigoto does; `\n` matches the end of each shader line.
pub const PATTERN: [&str; 8] = [
    r"dcl_input_ps constant v0\.xyzw\n",
    r"dcl_input_ps constant v1\.xyzw\n",
    r"dcl_input_ps linear v2\.xyz\n",
    r"dcl_input_ps_siv linear noperspective v3\.xy, position\n",
    r"(?P<body>[\s\S]*?)",
    r"mul o0\.xyz, (?P<colour>r\d+)\.xyzx, cb2\[0\]\.xxxx\n",
    r"dp3 (?P<lumout>r\d+)\.x, (?P<lumin>r\d+)\.xyzx, l\(0\.308600, 0\.609400, 0\.082000, 0\.000000\)\n",
    r"mul o1\.x, (?P<lumres>r\d+)\.x, cb2\[0\]\.x\n",
];

/// The same shader with the dimmer before its last three lines: `IniParams[0]` = (strength, reach,
/// on); `v0.xyz` is the light's position relative to the eyes, so its length is the distance.
pub const REPLACE: [&str; 19] = [
    r"dcl_input_ps constant v0.xyzw\n",
    r"dcl_input_ps constant v1.xyzw\n",
    r"dcl_input_ps linear v2.xyz\n",
    r"dcl_input_ps_siv linear noperspective v3.xy, position\n",
    r"${body}",
    r"// Elite Nexus cockpit-light dimmer: lights nearer than IniParams y (metres) keep x of their light while z is not 0.\n",
    r"ld_indexable(texture1d)(float,float,float,float) ${nexus_settings}.xyzw, l(0, 0, 0, 0), t120.xyzw\n",
    r"dp3 ${nexus_light}.x, v0.xyzx, v0.xyzx\n",
    r"mul ${nexus_light}.y, ${nexus_settings}.y, ${nexus_settings}.y\n",
    r"lt ${nexus_light}.z, ${nexus_light}.x, ${nexus_light}.y\n",
    r"ne ${nexus_light}.w, ${nexus_settings}.z, l(0.000000)\n",
    r"and ${nexus_light}.z, ${nexus_light}.z, ${nexus_light}.w\n",
    r"movc ${nexus_light}.x, ${nexus_light}.z, ${nexus_settings}.x, l(1.000000)\n",
    r"mul ${colour}.xyz, ${colour}.xyzx, ${nexus_light}.xxxx\n",
    r"mul ${lumin}.xyz, ${lumin}.xyzx, ${nexus_light}.xxxx\n",
    r"mul o0.xyz, ${colour}.xyzx, cb2[0].xxxx\n",
    r"dp3 ${lumout}.x, ${lumin}.xyzx, l(0.308600, 0.609400, 0.082000, 0.000000)\n",
    r"mul o1.x, ${lumres}.x, cb2[0].x\n",
    "",
];
pub const TEMPS: [&str; 2] = ["nexus_settings", "nexus_light"];
pub const INSERT_DECLARATIONS: &str = "dcl_resource_texture1d (float,float,float,float) t120";

fn number(v: f32) -> String {
    let s = format!("{v:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() || s == "-0" { "0".into() } else { s.to_string() }
}

/// The game's `d3dx.ini` with the dimmer set to `d`.
pub fn render_config(d: &Dimmer) -> String {
    let mut s = String::new();
    let mut line = |l: &str| {
        s.push_str(l);
        s.push_str("\r\n");
    };
    line("; Elite Nexus: cockpit-light dimmer. Installed and managed by Elite Nexus (HUD & cockpit > Cockpit");
    line(&format!("; lights); change it there. 3Dmigoto {MIGOTO_VERSION} ({MIGOTO_SOURCE}, GPL-3.0) patches the game's"));
    line("; box-light shader as the game loads it, so lights within the reach of your eyes (the cockpit's own)");
    line("; come out dimmer. No game file is changed.");
    line(";");
    line("; Keys, while the game window is in front:");
    for (key, what) in KEYS {
        line(&format!(";   {key:<4} {what}"));
    }
    line(";");
    line("; To remove it: Elite Nexus > HUD & cockpit > Cockpit lights > Remove. By hand: quit the game, then delete");
    line(&format!("; d3d11.dll, d3dcompiler_47.dll, {CONFIG}, {MANIFEST} and the ShaderFixes and ShaderCache folders."));
    line("");
    for l in ["[Logging]", "calls=0", "input=0", "debug=0", "", "[System]", "load_library_redirect=2", "check_foreground_window=1", "allow_check_interface=1", "allow_platform_update=1", ""] {
        line(l);
    }
    for l in ["[Rendering]", "override_directory=ShaderFixes", "cache_directory=ShaderCache", "storage_directory=ShaderFromGame", "cache_shaders=0", "export_shaders=0", "ini_params = 120", ""] {
        line(l);
    }
    line("[Hunting]");
    line("hunting=0");
    line("");
    line("[Constants]");
    line("; x: how much of a nearby light stays (0-1). y: reach in metres. z: 1 = on, 0 = off.");
    line(&format!("x = {}", number(d.strength)));
    line(&format!("y = {}", number(d.reach)));
    line(&format!("z = {}", if d.on { 1 } else { 0 }));
    line("");
    line("[KeyEliteNexusDimmerOnOff]");
    line("Key = no_modifiers VK_F9");
    line("type = toggle");
    line(&format!("z = {}", if d.on { 0 } else { 1 }));
    line("");
    line("[KeyEliteNexusDimmerStrength]");
    line("Key = no_modifiers VK_F10");
    line("type = cycle");
    line("x = 0, 0.15, 0.35, 0.6, 1");
    line("");
    line("[KeyEliteNexusDimmerReach]");
    line("Key = no_modifiers VK_F11");
    line("type = cycle");
    line("y = 5, 8, 12");
    line("");
    line(&format!("[{REGEX}]"));
    line("shader_model = ps_5_0");
    line(&format!("temps = {}", TEMPS.join(" ")));
    line("");
    line(&format!("[{REGEX}.Pattern]"));
    for l in PATTERN {
        line(l);
    }
    line("");
    line(&format!("[{REGEX}.Pattern.Replace]"));
    for l in REPLACE.iter().filter(|l| !l.is_empty()) {
        line(l);
    }
    line("");
    line(&format!("[{REGEX}.InsertDeclarations]"));
    line(INSERT_DECLARATIONS);
    line("");
    line("[Stereo]");
    line("automatic_mode=0");
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Applies the pattern the way 3Dmigoto does (lines joined, case-insensitive, multi-line, `\n`
    /// in the replacement is a new line, temps become free registers).
    fn patch(shader: &str) -> Option<String> {
        let re = regex::Regex::new(&format!("(?im){}", PATTERN.concat())).ok()?;
        let caps = re.captures(shader)?;
        let mut replacement = REPLACE.concat().replace(r"\n", "\n");
        for (i, t) in TEMPS.iter().enumerate() {
            replacement = replacement.replace(&format!("${{{t}}}"), &format!("r{}", 90 + i));
        }
        let mut out = String::new();
        caps.expand(&replacement, &mut out);
        Some(format!("{}{}{}", &shader[..caps.get(0)?.start()], out, &shader[caps.get(0)?.end()..]))
    }

    // A made-up shader with the box light's inputs and ending (not the game's code).
    const BOX: &str = "ps_5_0\ndcl_constantbuffer CB2[47], immediateIndexed\ndcl_input_ps constant v0.xyzw\ndcl_input_ps constant v1.xyzw\ndcl_input_ps linear v2.xyz\ndcl_input_ps_siv linear noperspective v3.xy, position\ndcl_output o0.xyz\ndcl_output o1.x\ndcl_temps 3\nmov r2.xyz, v1.xyzx\nmul r0.xyz, r2.xyzx, v0.wwww\nmul r1.xyz, r2.xyzx, v0.wwww\nmul o0.xyz, r0.xyzx, cb2[0].xxxx\ndp3 r0.x, r1.xyzx, l(0.308600, 0.609400, 0.082000, 0.000000)\nmul o1.x, r0.x, cb2[0].x\nret\n";

    #[test]
    fn the_pattern_dims_only_the_end_of_a_box_light() {
        let out = patch(BOX).expect("pattern should match");
        // Everything before the end is untouched.
        assert!(out.starts_with(&BOX[..BOX.find("mul o0.xyz").unwrap()]));
        assert!(out.contains("ld_indexable(texture1d)(float,float,float,float) r90.xyzw, l(0, 0, 0, 0), t120.xyzw\n"));
        assert!(out.contains("dp3 r91.x, v0.xyzx, v0.xyzx\n"));
        assert!(out.contains("mul r0.xyz, r0.xyzx, r91.xxxx\nmul r1.xyz, r1.xyzx, r91.xxxx\nmul o0.xyz, r0.xyzx, cb2[0].xxxx\n"));
        assert!(out.ends_with("dp3 r0.x, r1.xyzx, l(0.308600, 0.609400, 0.082000, 0.000000)\nmul o1.x, r0.x, cb2[0].x\nret\n"), "{out}");
        assert!(!out.contains("${"), "{out}");
    }

    #[test]
    fn other_light_shapes_are_left_alone() {
        // Round lights read their data elsewhere; v0 isn't a light position there.
        let sphere = BOX.replace("dcl_input_ps constant v0.xyzw\n", "dcl_input_ps constant v0.xyz\n");
        assert!(patch(&sphere).is_none());
        let other_end = BOX.replace("cb2[0].xxxx", "cb2[1].xxxx");
        assert!(patch(&other_end).is_none());
    }

    #[test]
    fn config_round_trips_settings_and_carries_the_pattern() {
        let d = Dimmer { on: false, strength: 0.15, reach: 7.5 };
        let ini = render_config(&d);
        assert!(ini.starts_with(CONFIG_MARK));
        assert_eq!(read_settings(&ini), d);
        assert!(ini.contains("\r\nx = 0.15\r\ny = 7.5\r\nz = 0\r\n"));
        // F9 switches it the other way.
        assert!(ini.contains("type = toggle\r\nz = 1\r\n"));
        assert!(ini.contains("[ShaderRegexEliteNexusCockpitLights.Pattern]\r\ndcl_input_ps constant v0\\.xyzw\\n\r\n"));
        assert!(ini.contains("[ShaderRegexEliteNexusCockpitLights.InsertDeclarations]\r\ndcl_resource_texture1d (float,float,float,float) t120\r\n"));
        assert!(ini.is_ascii());
        assert_eq!(read_settings(&render_config(&Dimmer::default())), Dimmer::default());
    }

    fn temp_game(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("nexus-dimmer-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("EliteDangerous64.exe"), b"game").unwrap();
        dir
    }

    /// Stand-ins for the DLLs: install checks hashes, so tests use the pinned names with fake content
    /// and compare against these instead.
    fn fake_files() -> Vec<(&'static str, Vec<u8>)> {
        vec![("d3d11.dll", b"fake d3d11".to_vec()), ("d3dcompiler_47.dll", b"fake compiler".to_vec())]
    }

    #[test]
    fn foreign_mods_are_left_alone() {
        let game = temp_game("foreign");
        std::fs::write(game.join(CONFIG), "[Constants]\nx150 = 1\n").unwrap();
        std::fs::create_dir_all(game.join("EDHM-ini")).unwrap();
        assert_eq!(status(&game), Status::Foreign { edhm: true });
        assert!(matches!(install(&game, &fake_files(), &Dimmer::default()), Err(DimmerError::Foreign(true))));
        assert!(matches!(uninstall(&game), Err(DimmerError::NotOurs)));
        assert!(game.join(CONFIG).is_file() && game.join("EDHM-ini").is_dir());
        let _ = std::fs::remove_dir_all(&game);
    }

    #[test]
    fn install_settings_and_removal() {
        let game = temp_game("cycle");
        assert_eq!(status(&game), Status::None);
        // Real installs check the DLLs against the pinned hashes; the fakes don't match, so a
        // stray "official" file already there would be refused.
        std::fs::write(game.join("d3d11.dll"), b"something else").unwrap();
        assert!(matches!(install(&game, &fake_files(), &Dimmer::default()), Err(DimmerError::Foreign(false))));
        std::fs::remove_file(game.join("d3d11.dll")).unwrap();

        install(&game, &fake_files(), &Dimmer::default()).unwrap();
        assert_eq!(status(&game), Status::Installed { dimmer: Dimmer::default() });
        assert_eq!(std::fs::read(game.join("d3d11.dll")).unwrap(), b"fake d3d11");
        assert!(game.join(FIXES_DIR).is_dir());
        let changed = Dimmer { on: true, strength: 0.6, reach: 9.0 };
        save_settings(&game, &changed).unwrap();
        assert_eq!(status(&game), Status::Installed { dimmer: changed });

        // 3Dmigoto's by-products and captures go too; a file someone changed stays.
        std::fs::create_dir_all(game.join("ShaderCache")).unwrap();
        std::fs::write(game.join("ShaderCache").join("x-ps.txt"), b"asm").unwrap();
        std::fs::create_dir_all(game.join("FrameAnalysis-2026-10-13-083851")).unwrap();
        std::fs::write(game.join("d3dx_user.ini"), b"").unwrap();
        std::fs::write(game.join("d3dcompiler_47.dll"), b"replaced by someone").unwrap();
        let done = uninstall(&game).unwrap();
        assert_eq!(done.kept, vec!["d3dcompiler_47.dll".to_string()]);
        for gone in ["d3d11.dll", CONFIG, MANIFEST, "d3dx_user.ini", "ShaderCache", "FrameAnalysis-2026-10-13-083851", FIXES_DIR] {
            assert!(!game.join(gone).exists(), "{gone} should be gone: {done:?}");
        }
        assert!(game.join("d3dcompiler_47.dll").is_file() && game.join("EliteDangerous64.exe").is_file());
        let _ = std::fs::remove_dir_all(&game);
    }

    #[test]
    fn capture_leftovers_are_measured_and_deleted() {
        let game = temp_game("leftovers");
        let capture = game.join("FrameAnalysis-2026-10-13-083851");
        std::fs::create_dir_all(capture.join("deduped")).unwrap();
        std::fs::write(capture.join("log.txt"), [0u8; 1000]).unwrap();
        std::fs::write(capture.join("deduped").join("000001-ps-t0.jpg"), [0u8; 500]).unwrap();
        std::fs::create_dir_all(game.join("ShaderCache")).unwrap();
        std::fs::write(game.join("ShaderCache").join("x-ps.txt"), [0u8; 200]).unwrap();
        // Not Elite Nexus's install (yet): nothing is counted or deleted.
        assert_eq!(leftovers(&game), 0);
        assert!(matches!(remove_leftovers(&game), Err(DimmerError::NotOurs)));
        install(&game, &fake_files(), &Dimmer::default()).unwrap();
        assert_eq!(leftovers(&game), 1700);
        // While the config exports shaders, ShaderCache is in use and stays.
        let config = std::fs::read_to_string(game.join(CONFIG)).unwrap();
        std::fs::write(game.join(CONFIG), config.replace("export_shaders=0", "export_shaders=1")).unwrap();
        assert_eq!(leftovers(&game), 1500);
        std::fs::write(game.join(CONFIG), &config).unwrap();
        let done = remove_leftovers(&game).unwrap();
        assert_eq!(done.removed, vec!["FrameAnalysis-2026-10-13-083851\\".to_string(), "ShaderCache\\".to_string()]);
        assert_eq!(leftovers(&game), 0);
        // What the dimmer needs stays.
        for kept in ["d3d11.dll", "d3dcompiler_47.dll", CONFIG, MANIFEST, FIXES_DIR, "EliteDangerous64.exe"] {
            assert!(game.join(kept).exists(), "{kept} should stay");
        }
        assert!(matches!(status(&game), Status::Installed { .. }));
        let _ = std::fs::remove_dir_all(&game);
    }

    #[test]
    fn a_hand_installed_test_is_recognised_and_its_fix_replaced() {
        let game = temp_game("legacy");
        std::fs::create_dir_all(game.join(FIXES_DIR)).unwrap();
        std::fs::write(game.join(FIXES_DIR).join(LEGACY_FIX), b"asm").unwrap();
        std::fs::write(game.join(CONFIG), "; Elite Nexus: cockpit-light dimmer, test version.\n[Constants]\nx = 0.35\ny = 5.0\nz = 1\n").unwrap();
        assert_eq!(status(&game), Status::Unmanaged { dimmer: Dimmer::default() });
        // Taking over without a download needs the official DLLs in place; fakes are refused.
        std::fs::write(game.join("d3d11.dll"), b"fake").unwrap();
        assert!(matches!(install(&game, &[], &Dimmer::default()), Err(DimmerError::UnknownFile(_))));
        // With files supplied it installs over them only if they're the official ones.
        assert!(matches!(install(&game, &fake_files(), &Dimmer::default()), Err(DimmerError::UnknownFile(_))));
        std::fs::remove_file(game.join("d3d11.dll")).unwrap();
        install(&game, &fake_files(), &Dimmer::default()).unwrap();
        assert!(!game.join(FIXES_DIR).join(LEGACY_FIX).exists());
        assert!(matches!(status(&game), Status::Installed { .. }));
        let _ = std::fs::remove_dir_all(&game);
    }

    /// Checks the pattern against the shaders the game loaded (exported with 3Dmigoto's
    /// `export_shaders`; not in git). Run with NEXUS_SHADER_DIR set:
    ///   cargo test -p nexus-hud real_shaders -- --ignored --nocapture
    #[test]
    #[ignore]
    fn real_shaders() {
        let dir = std::env::var("NEXUS_SHADER_DIR").expect("set NEXUS_SHADER_DIR to a ShaderCache folder");
        let mut matched = Vec::new();
        let mut total = 0;
        for e in std::fs::read_dir(&dir).unwrap().flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if !name.ends_with("-ps.txt") {
                continue;
            }
            total += 1;
            let text = std::fs::read_to_string(e.path()).unwrap().replace("\r\n", "\n");
            if patch(&text).is_some() {
                matched.push(name);
            }
        }
        println!("{} of {total} pixel shaders match: {matched:?}", matched.len());
        assert!(matched.iter().any(|n| n.starts_with("3f366702d540af3e")), "the cockpit's box-light shader must match");
    }

    /// Checks the real download (network, ~5 MB). Run with:
    ///   cargo test -p nexus-hud real_archive -- --ignored --nocapture   (NEXUS_MIGOTO_ARCHIVE = a local copy)
    #[test]
    #[ignore]
    fn real_archive() {
        let path = std::env::var("NEXUS_MIGOTO_ARCHIVE").expect("set NEXUS_MIGOTO_ARCHIVE to 3Dmigoto_v1.4.9.7z");
        let files = extract(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(files.iter().map(|(n, _)| *n).collect::<Vec<_>>(), vec!["d3d11.dll", "d3dcompiler_47.dll"]);
    }
}
