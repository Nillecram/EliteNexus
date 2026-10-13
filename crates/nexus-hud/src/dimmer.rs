//! Elite Nexus's graphics hook: 3Dmigoto (a Direct3D 11 hook placed next to the game) with patterns
//! that patch game shaders as the game loads them.
//!
//! * The cockpit-light dimmer ([`Dimmer`]): the deferred box-light shader, so lights near the eyes
//!   (the cockpit's own) come out dimmer.
//! * HUD colour protection ([`Protect`], optional): the HUD shaders that apply the game's colour
//!   matrix apply it only to the HUD's orange, so hostile red, friendly green and shield blue keep
//!   their colours under a theme.
//!
//! Elite Nexus ships only its patterns and the few instructions it inserts; the game's shader code is
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

/// Keeps hostile red, friendly green and shield blue out of the HUD colour theme: the game's colour
/// matrix is applied only to colours like the HUD's orange ([`crate::palette::is_hud_orange`]), in
/// the game's `[Constants]` as 3Dmigoto's IniParams `x1`, `y1`, `z1`, `w1`. The shares compare the
/// colour's channels with its red, so they don't depend on brightness. Frontier documents none of
/// this; the defaults come from the colours measured in a frame capture.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Protect {
    pub on: bool,
    /// The most green, as a share of red, a colour can have and still count as the HUD's orange.
    pub max_gr: f32,
    /// The most blue, as a share of red.
    pub max_br: f32,
    /// The least green, as a share of red: redder colours (hostile red) keep their colour.
    pub min_gr: f32,
}

impl Default for Protect {
    fn default() -> Self {
        Protect { on: false, max_gr: 0.6, max_br: 0.3, min_gr: 0.035 }
    }
}

/// Everything Elite Nexus sets in the game's 3Dmigoto config.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct HookConfig {
    pub dimmer: Dimmer,
    pub protect: Protect,
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
    Installed { config: HookConfig },
    /// Elite Nexus's config without a manifest: put there by hand (the first test). Can be taken over.
    Unmanaged { config: HookConfig },
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
            let config = read_config(&text);
            if game.join(MANIFEST).is_file() { Status::Installed { config } } else { Status::Unmanaged { config } }
        }
        Some(_) => Status::Foreign { edhm: game.join("EDHM-ini").is_dir() },
        None if game.join("d3d11.dll").exists() => Status::Foreign { edhm: game.join("EDHM-ini").is_dir() },
        None => Status::None,
    }
}

/// Installs 3Dmigoto (`files` from [`extract`]) with `config`, or takes over a hand-installed Elite
/// Nexus config (then `files` may be empty: the DLLs there are checked instead).
pub fn install(game: &Path, files: &[(&'static str, Vec<u8>)], config: &HookConfig) -> Result<(), DimmerError> {
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
    let ini = render_config(config);
    write_atomic(&game.join(CONFIG), ini.as_bytes())?;
    manifest.files.push((CONFIG.to_string(), sha256_hex(ini.as_bytes())));
    let json = serde_json::to_vec_pretty(&manifest).unwrap_or_default();
    // Written last: a manifest means the install is complete.
    write_atomic(&game.join(MANIFEST), &json)?;
    Ok(())
}

/// Writes new settings (the game reads them when it starts).
pub fn save_config(game: &Path, config: &HookConfig) -> Result<(), DimmerError> {
    match status(game) {
        Status::Installed { .. } => {}
        _ => return Err(DimmerError::NotOurs),
    }
    let ini = render_config(config);
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

/// Removes 3Dmigoto with the dimmer and the HUD colour protection: the files Elite Nexus installed
/// (only while they still match), 3Dmigoto's own by-products, and the frame captures made with it.
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

/// The settings in a config's `[Constants]` (defaults for anything missing).
pub fn read_config(ini: &str) -> HookConfig {
    let mut c = HookConfig::default();
    for (key, value) in section(ini, "Constants") {
        let Ok(v) = value.parse::<f32>() else { continue };
        if !v.is_finite() {
            continue;
        }
        match key {
            "x" => c.dimmer.strength = v.clamp(0.0, 1.0),
            "y" => c.dimmer.reach = v.max(0.0),
            "z" => c.dimmer.on = v != 0.0,
            "x1" => c.protect.on = v != 0.0,
            "y1" => c.protect.max_gr = v.clamp(0.0, 1.0),
            "z1" => c.protect.max_br = v.clamp(0.0, 1.0),
            "w1" => c.protect.min_gr = v.clamp(0.0, 1.0),
            _ => {}
        }
    }
    c
}

/// The section names of each pattern (own names, so nothing else in an ini can collide with them).
const LIGHT_REGEX: &str = "ShaderRegexEliteNexusCockpitLights";
const HUD_REGEX: &str = "ShaderRegexEliteNexusHudColours";

/// The box-light shader, recognised by its inputs (light position and range in camera space, packed
/// colour, view ray, pixel position) and by how it ends (colour and luminance scaled by `cb2[0].x`).
/// Lines are joined after trimming, as 3Dmigoto does; `\n` matches the end of each shader line.
pub const LIGHT_PATTERN: [&str; 8] = [
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
pub const LIGHT_REPLACE: [&str; 18] = [
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
];
pub const LIGHT_TEMPS: [&str; 2] = ["nexus_settings", "nexus_light"];

/// Where a HUD shader applies the game's colour matrix to an element's standard colour (`cb1[85]`,
/// `[86]`, `[87]` are the MatrixRed, MatrixGreen and MatrixBlue rows): green times row 86, plus red
/// times row 85, plus blue times row 87, three lines in a row. Registers, masks and component order
/// vary between shaders; the third line's destination components hold the themed red, green, blue.
pub const HUD_PATTERN: [&str; 3] = [
    r"(?P<l1>mul r\d+\.[xyzw]{3}, (?P<gs>r\d+)\.(?P<gc>[xyzw])[xyzw]{3}, cb1\[86\]\.[xyzw]{4}\n)",
    r"(?P<l2>mad r\d+\.[xyzw]{3}, (?P<rs>r\d+)\.(?P<rc>[xyzw])[xyzw]{3}, cb1\[85\]\.[xyzw]{4}, r\d+\.[xyzw]{4}\n)",
    r"(?P<l3>mad (?P<d>r\d+)\.(?P<d0>[xyzw])(?P<d1>[xyzw])(?P<d2>[xyzw]), (?P<bs>r\d+)\.(?P<bc>[xyzw])[xyzw]{3}, cb1\[87\]\.[xyzw]{4}, r\d+\.[xyzw]{4}\n)",
];

/// The same three lines with the standard colour copied first (the third line can overwrite it) and,
/// after them, the rule of [`crate::palette::is_hud_orange`] with `IniParams[1]` = (on, max_gr,
/// max_br, min_gr): a colour that isn't the HUD's orange gets its standard colour back.
pub const HUD_REPLACE: [&str; 22] = [
    r"mov ${nexus_in}.x, ${rs}.${rc}\n",
    r"mov ${nexus_in}.y, ${gs}.${gc}\n",
    r"mov ${nexus_in}.z, ${bs}.${bc}\n",
    r"${l1}${l2}${l3}",
    r"// Elite Nexus: keep hostile red, friendly green and shield blue out of the HUD colour theme.\n",
    r"ld_indexable(texture1d)(float,float,float,float) ${nexus_set}.xyzw, l(1, 0, 0, 0), t120.xyzw\n",
    r"max ${nexus_tmp}.w, ${nexus_in}.y, ${nexus_in}.z\n",
    r"lt ${nexus_tmp}.x, ${nexus_tmp}.w, ${nexus_in}.x\n",
    r"mul ${nexus_tmp}.y, ${nexus_in}.x, ${nexus_set}.y\n",
    r"ge ${nexus_tmp}.y, ${nexus_tmp}.y, ${nexus_in}.y\n",
    r"mul ${nexus_tmp}.z, ${nexus_in}.x, ${nexus_set}.w\n",
    r"ge ${nexus_tmp}.z, ${nexus_in}.y, ${nexus_tmp}.z\n",
    r"mul ${nexus_tmp}.w, ${nexus_in}.x, ${nexus_set}.z\n",
    r"ge ${nexus_tmp}.w, ${nexus_tmp}.w, ${nexus_in}.z\n",
    r"and ${nexus_tmp}.x, ${nexus_tmp}.x, ${nexus_tmp}.y\n",
    r"and ${nexus_tmp}.x, ${nexus_tmp}.x, ${nexus_tmp}.z\n",
    r"and ${nexus_tmp}.x, ${nexus_tmp}.x, ${nexus_tmp}.w\n",
    r"eq ${nexus_tmp}.y, ${nexus_set}.x, l(0.000000)\n",
    r"or ${nexus_tmp}.x, ${nexus_tmp}.x, ${nexus_tmp}.y\n",
    r"movc ${d}.${d0}, ${nexus_tmp}.x, ${d}.${d0}, ${nexus_in}.x\n",
    r"movc ${d}.${d1}, ${nexus_tmp}.x, ${d}.${d1}, ${nexus_in}.y\n",
    r"movc ${d}.${d2}, ${nexus_tmp}.x, ${d}.${d2}, ${nexus_in}.z\n",
];
pub const HUD_TEMPS: [&str; 3] = ["nexus_in", "nexus_tmp", "nexus_set"];

/// Both patterns read their settings from 3Dmigoto's IniParams texture.
pub const INSERT_DECLARATIONS: &str = "dcl_resource_texture1d (float,float,float,float) t120";

fn number(v: f32) -> String {
    let s = format!("{v:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() || s == "-0" { "0".into() } else { s.to_string() }
}

/// The game's `d3dx.ini` for `c`. The HUD pattern is in it only while the protection is on, so the
/// HUD shaders are patched only when that's wanted.
pub fn render_config(c: &HookConfig) -> String {
    let (d, p) = (&c.dimmer, &c.protect);
    let mut s = String::new();
    let mut line = |l: &str| {
        s.push_str(l);
        s.push_str("\r\n");
    };
    line("; Elite Nexus: graphics hook for the cockpit-light dimmer and the HUD colours. Installed and managed by");
    line(&format!("; Elite Nexus (HUD & cockpit); change it there. 3Dmigoto {MIGOTO_VERSION} ({MIGOTO_SOURCE}, GPL-3.0)"));
    line("; patches game shaders as the game loads them: the box-light shader, so lights within the reach of your");
    line("; eyes (the cockpit's own) come out dimmer, and, while \"Keep hostile red, friendly green and shield blue\"");
    line("; is on, the HUD shaders that apply the game's colour matrix, so a theme colours only the HUD's orange.");
    line("; No game file is changed.");
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
    line("; x1: 1 = keep hostile red, friendly green and shield blue out of the HUD colour theme. y1, z1: the most");
    line("; green and blue (shares of red) the HUD's orange has; w1: the least green (redder colours keep theirs).");
    line(&format!("x1 = {}", if p.on { 1 } else { 0 }));
    line(&format!("y1 = {}", number(p.max_gr)));
    line(&format!("z1 = {}", number(p.max_br)));
    line(&format!("w1 = {}", number(p.min_gr)));
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
    regex_sections(&mut line, LIGHT_REGEX, &LIGHT_TEMPS, &LIGHT_PATTERN, &LIGHT_REPLACE);
    if p.on {
        regex_sections(&mut line, HUD_REGEX, &HUD_TEMPS, &HUD_PATTERN, &HUD_REPLACE);
    }
    line("[Stereo]");
    line("automatic_mode=0");
    s
}

/// One `[ShaderRegex…]` group: the main section, the pattern, its replacement and the declaration
/// of the IniParams texture.
fn regex_sections(line: &mut dyn FnMut(&str), name: &str, temps: &[&str], pattern: &[&str], replace: &[&str]) {
    line(&format!("[{name}]"));
    line("shader_model = ps_5_0");
    line(&format!("temps = {}", temps.join(" ")));
    line("");
    line(&format!("[{name}.Pattern]"));
    for l in pattern {
        line(l);
    }
    line("");
    line(&format!("[{name}.Pattern.Replace]"));
    for l in replace {
        line(l);
    }
    line("");
    line(&format!("[{name}.InsertDeclarations]"));
    line(INSERT_DECLARATIONS);
    line("");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::matrix::{HudMatrix, Rgb};
    use crate::palette::is_hud_orange;
    use std::collections::HashMap;

    /// A pattern applied the way 3Dmigoto does it: lines joined, case-insensitive, multi-line, `\n` in
    /// the replacement is a new line, temps become free registers (r90, r91, ... here).
    struct Patched {
        shader: String,
        /// The replacement on its own.
        inserted: String,
        groups: HashMap<String, String>,
    }

    fn regex(pattern: &[&str]) -> regex::Regex {
        regex::Regex::new(&format!("(?im){}", pattern.concat())).unwrap()
    }

    fn patch_with(pattern: &[&str], replace: &[&str], temps: &[&str], shader: &str) -> Option<Patched> {
        let re = regex(pattern);
        let caps = re.captures(shader)?;
        let mut replacement = replace.concat().replace(r"\n", "\n");
        for (i, t) in temps.iter().enumerate() {
            replacement = replacement.replace(&format!("${{{t}}}"), &format!("r{}", 90 + i));
        }
        let mut inserted = String::new();
        caps.expand(&replacement, &mut inserted);
        let whole = caps.get(0)?;
        let groups = re.capture_names().flatten().filter_map(|n| Some((n.to_string(), caps.name(n)?.as_str().to_string()))).collect();
        Some(Patched { shader: format!("{}{}{}", &shader[..whole.start()], inserted, &shader[whole.end()..]), inserted, groups })
    }

    fn patch_lights(shader: &str) -> Option<Patched> {
        patch_with(&LIGHT_PATTERN, &LIGHT_REPLACE, &LIGHT_TEMPS, shader)
    }

    fn patch_hud(shader: &str) -> Option<Patched> {
        patch_with(&HUD_PATTERN, &HUD_REPLACE, &HUD_TEMPS, shader)
    }

    /// Just enough of a pixel shader machine to run the instructions the patterns match and insert,
    /// so the tests check what the inserted code computes. Values are kept as bits, as on the GPU:
    /// comparisons give all-ones masks and `and`/`or` are bitwise.
    #[derive(Default)]
    struct Machine {
        regs: HashMap<String, [u32; 4]>,
        /// Constant buffers by (buffer, index).
        cb: HashMap<(u32, u32), [f32; 4]>,
        /// 3Dmigoto's IniParams (t120).
        ini: Vec<[f32; 4]>,
    }

    const LANES: &str = "xyzw";

    fn lane(c: char) -> usize {
        LANES.find(c).expect("a component")
    }

    /// Operands split at the commas between them (not inside `l(...)`).
    fn operands(s: &str) -> Vec<&str> {
        let (mut out, mut depth, mut start) = (Vec::new(), 0, 0);
        for (i, ch) in s.char_indices() {
            match ch {
                '(' => depth += 1,
                ')' => depth -= 1,
                ',' if depth == 0 => {
                    out.push(s[start..i].trim());
                    start = i + 1;
                }
                _ => {}
            }
        }
        out.push(s[start..].trim());
        out
    }

    impl Machine {
        fn set(&mut self, reg: &str, values: [f32; 4]) {
            self.regs.insert(reg.to_string(), values.map(f32::to_bits));
        }

        fn set_component(&mut self, reg: &str, component: &str, value: f32) {
            self.regs.entry(reg.to_string()).or_default()[lane(component.chars().next().unwrap())] = value.to_bits();
        }

        fn get(&self, reg: &str) -> [f32; 4] {
            self.regs.get(reg).copied().unwrap_or_default().map(f32::from_bits)
        }

        /// A source operand, swizzled to four lanes (a short swizzle repeats its last component).
        fn read(&self, operand: &str) -> [u32; 4] {
            if let Some(list) = operand.strip_prefix("l(").and_then(|o| o.strip_suffix(')')) {
                let v: Vec<u32> = list.split(',').map(|n| n.trim().parse::<f32>().unwrap().to_bits()).collect();
                return if v.len() == 1 { [v[0]; 4] } else { [v[0], v[1], v[2], v[3]] };
            }
            let (base, swizzle) = operand.split_once('.').unwrap();
            let values = match base.strip_prefix("cb").and_then(|r| r.trim_end_matches(']').split_once('[')) {
                Some((buf, idx)) => self.cb.get(&(buf.parse().unwrap(), idx.parse().unwrap())).copied().unwrap_or_default().map(f32::to_bits),
                None => self.regs.get(base).copied().unwrap_or_default(),
            };
            let s: Vec<char> = swizzle.chars().collect();
            std::array::from_fn(|i| values[lane(s[i.min(s.len() - 1)])])
        }

        /// Runs the instructions of `code`, skipping declarations and comments.
        fn run(&mut self, code: &str) {
            let float = f32::from_bits;
            let mask = |b: bool| if b { u32::MAX } else { 0 };
            for line in code.lines().map(str::trim) {
                if line.is_empty() || line.starts_with("//") || line.starts_with("dcl_") || line.starts_with("ps_") || line == "ret" {
                    continue;
                }
                let (op, rest) = line.split_once(' ').unwrap();
                let args = operands(rest);
                let (dest, write) = args[0].split_once('.').unwrap();
                let src: Vec<[u32; 4]> = if op.starts_with("ld_indexable") {
                    let texel = float(self.read(args[1])[0]) as usize;
                    vec![self.ini.get(texel).copied().unwrap_or_default().map(f32::to_bits)]
                } else {
                    args[1..].iter().map(|a| self.read(a)).collect()
                };
                let out: [u32; 4] = std::array::from_fn(|i| {
                    let a = |k: usize| src[k][i];
                    match op {
                        "mov" => a(0),
                        "mul" => (float(a(0)) * float(a(1))).to_bits(),
                        "mad" => (float(a(0)) * float(a(1)) + float(a(2))).to_bits(),
                        "max" => float(a(0)).max(float(a(1))).to_bits(),
                        "dp3" => (0..3).map(|k| float(src[0][k]) * float(src[1][k])).sum::<f32>().to_bits(),
                        "lt" => mask(float(a(0)) < float(a(1))),
                        "ge" => mask(float(a(0)) >= float(a(1))),
                        "eq" => mask(float(a(0)) == float(a(1))),
                        "ne" => mask(float(a(0)) != float(a(1))),
                        "and" => a(0) & a(1),
                        "or" => a(0) | a(1),
                        "movc" => {
                            if a(0) != 0 {
                                a(1)
                            } else {
                                a(2)
                            }
                        }
                        o if o.starts_with("ld_indexable") => a(0),
                        o => panic!("{o} isn't modelled"),
                    }
                });
                let reg = self.regs.entry(dest.to_string()).or_default();
                for c in write.chars() {
                    reg[lane(c)] = out[lane(c)];
                }
            }
        }
    }

    // A made-up shader with the box light's inputs and ending (not the game's code).
    const BOX: &str = "ps_5_0\ndcl_constantbuffer CB2[47], immediateIndexed\ndcl_input_ps constant v0.xyzw\ndcl_input_ps constant v1.xyzw\ndcl_input_ps linear v2.xyz\ndcl_input_ps_siv linear noperspective v3.xy, position\ndcl_output o0.xyz\ndcl_output o1.x\ndcl_temps 3\nmov r2.xyz, v1.xyzx\nmul r0.xyz, r2.xyzx, v0.wwww\nmul r1.xyz, r2.xyzx, v0.wwww\nmul o0.xyz, r0.xyzx, cb2[0].xxxx\ndp3 r0.x, r1.xyzx, l(0.308600, 0.609400, 0.082000, 0.000000)\nmul o1.x, r0.x, cb2[0].x\nret\n";

    #[test]
    fn the_pattern_dims_only_the_end_of_a_box_light() {
        let out = patch_lights(BOX).expect("pattern should match").shader;
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
        assert!(patch_lights(&sphere).is_none());
        let other_end = BOX.replace("cb2[0].xxxx", "cb2[1].xxxx");
        assert!(patch_lights(&other_end).is_none());
    }

    #[test]
    fn the_inserted_dimmer_code_dims_only_lights_within_reach() {
        let patched = patch_lights(BOX).unwrap().shader;
        let colour = [0.5, 0.25, 1.0];
        let luminance = 0.3086 * colour[0] + 0.6094 * colour[1] + 0.082 * colour[2];
        // Light position relative to the eyes, dimmer on, light kept.
        for (at, on, kept) in [([1.0, 2.0, 2.0], true, 0.35), ([1.0, 2.0, 2.0], false, 1.0), ([3.0, 4.0, 5.0], true, 1.0)] {
            let mut m = Machine { ini: vec![[0.35, 5.0, if on { 1.0 } else { 0.0 }, 0.0]], ..Default::default() };
            m.cb.insert((2, 0), [1.0, 0.0, 0.0, 0.0]);
            m.set("v0", [at[0], at[1], at[2], 1.0]);
            m.set("v1", [colour[0], colour[1], colour[2], 0.0]);
            m.run(&patched);
            let o0 = m.get("o0");
            assert!(o0.iter().zip(colour).all(|(got, c)| (got - c * kept).abs() < 1e-6), "{at:?} {on}: {o0:?}");
            assert!((m.get("o1")[0] - luminance * kept).abs() < 1e-6, "{at:?} {on}");
        }
    }

    // Made-up HUD shaders (not the game's code) around the two shapes in which the game's HUD
    // shaders apply the colour matrix.
    const HUD_SHAPE_1: &str = "ps_5_0\ndcl_constantbuffer CB1[91], immediateIndexed\ndcl_input_ps linear v1.xyzw\ndcl_output o0.xyzw\ndcl_temps 3\nmov r1.xyz, v1.xyzx\nmul r2.xyz, r1.yyyy, cb1[86].xyzx\nmad r1.xyw, r1.xxxx, cb1[85].xyxz, r2.xyxz\nmad r0.xyz, r1.zzzz, cb1[87].xyzx, r1.xywx\nmul o0.xyz, r0.xyzx, cb1[90].yyyy\nmov o0.w, v1.w\nret\n";
    const HUD_SHAPE_2: &str = "ps_5_0\ndcl_constantbuffer CB1[91], immediateIndexed\ndcl_input_ps linear v1.xyzw\ndcl_output o0.xyzw\ndcl_temps 2\nmov r0.xyzw, v1.xwyz\nmul r1.xyz, r0.zzzz, cb1[86].xyzx\nmad r1.xyz, r0.xxxx, cb1[85].xyzx, r1.xyzx\nmad r0.xzw, r0.wwww, cb1[87].xxyz, r1.xxyz\nmul o0.xyz, r0.xzwx, cb1[90].yyyy\nmov o0.w, r0.y\nret\n";

    #[test]
    fn the_hud_pattern_wraps_the_three_matrix_lines_and_nothing_else() {
        for shader in [HUD_SHAPE_1, HUD_SHAPE_2] {
            let found: Vec<_> = regex(&HUD_PATTERN).find_iter(shader).collect();
            assert_eq!(found.len(), 1, "{shader}");
            let (start, end) = (found[0].start(), found[0].end());
            let p = patch_hud(shader).unwrap();
            // Byte for byte the same outside the three lines, which stay in the middle as they were.
            assert!(p.shader.starts_with(&shader[..start]) && p.shader.ends_with(&shader[end..]), "{}", p.shader);
            assert!(p.inserted.contains(&shader[start..end]));
            assert!(!p.inserted.contains("${") && !p.inserted.contains(r"\n"), "{}", p.inserted);
            assert!(p.inserted.contains("ld_indexable(texture1d)(float,float,float,float) r92.xyzw, l(1, 0, 0, 0), t120.xyzw\n"));
            // Neither pattern takes the other's shaders.
            assert!(patch_lights(shader).is_none());
        }
        assert!(patch_hud(BOX).is_none());
        // Shape 2 writes its result over its own blue input (r0.w): the inputs are copied first.
        let p = patch_hud(HUD_SHAPE_2).unwrap();
        assert!(p.inserted.starts_with("mov r90.x, r0.x\nmov r90.y, r0.z\nmov r90.z, r0.w\nmul r1.xyz, r0.zzzz, cb1[86].xyzx\n"), "{}", p.inserted);
        assert!(p.inserted.ends_with("movc r0.x, r91.x, r0.x, r90.x\nmovc r0.z, r91.x, r0.z, r90.y\nmovc r0.w, r91.x, r0.w, r90.z\n"), "{}", p.inserted);
        let p = patch_hud(HUD_SHAPE_1).unwrap();
        assert!(p.inserted.starts_with("mov r90.x, r1.x\nmov r90.y, r1.y\nmov r90.z, r1.z\nmul r2.xyz"), "{}", p.inserted);
        assert!(p.inserted.ends_with("movc r0.x, r91.x, r0.x, r90.x\nmovc r0.y, r91.x, r0.y, r90.y\nmovc r0.z, r91.x, r0.z, r90.z\n"), "{}", p.inserted);
        // Only two matrix rows, or another buffer: not the HUD's matrix.
        assert!(patch_hud(&HUD_SHAPE_1.replace("cb1[87]", "cb1[88]")).is_none());
        assert!(patch_hud(&HUD_SHAPE_1.replace("cb1[86]", "cb2[86]")).is_none());
    }

    /// What the HUD should show for the standard colour `c` (linear light): the theme `m` when the
    /// protection is off or `c` is the HUD's orange, else `c` itself.
    fn expected(m: &HudMatrix, c: Rgb, p: &Protect) -> Rgb {
        if p.on && !is_hud_orange(c, p) {
            return c;
        }
        std::array::from_fn(|k| c[2] * m.blue[k] + (c[0] * m.red[k] + c[1] * m.green[k]))
    }

    /// Random colours from a fixed seed, plus the measured and reference ones and the edges.
    fn test_colours() -> Vec<Rgb> {
        let mut out: Vec<Rgb> = vec![[1.0, 0.068, 0.025], [0.234, 0.044, 0.025], [1.0, 1.0, 1.0], [0.0, 1.0, 0.0], [0.0, 0.6, 1.0], [1.0, 0.01, 0.01], [0.0, 0.0, 0.0], [1.0, 0.0144, 0.0052], [1.0, 0.6, 0.3], [1.0, 0.035, 0.0]];
        let mut seed = 0x2545_f491_u32;
        let mut next = || {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            (seed % 10_001) as f32 / 10_000.0
        };
        for _ in 0..500 {
            let r = next() * 4.0;
            out.push([r, r * next() * 0.8, r * next() * 0.5]);
            out.push([next(), next(), next()]);
        }
        out
    }

    /// Runs the matrix lines of a patched shader (made-up or the game's) for many colours and themes
    /// and checks the output against [`expected`].
    fn check_hud_rule(p: &Patched, what: &str) {
        let g = |k: &str| p.groups[k].as_str();
        let themes = [HudMatrix { red: [0.0, 0.0, 1.0], green: [0.0, 1.0, 0.0], blue: [1.0, 0.0, 0.0] }, HudMatrix::hue_rotation(200.0), HudMatrix::IDENTITY.scaled(0.4)];
        let settings = [Protect { on: true, ..Protect::default() }, Protect::default(), Protect { on: true, max_gr: 0.3, max_br: 0.1, min_gr: 0.1 }];
        for theme in themes {
            for protect in settings {
                for c in test_colours() {
                    let mut m = Machine { ini: vec![[0.35, 5.0, 1.0, 0.0], [if protect.on { 1.0 } else { 0.0 }, protect.max_gr, protect.max_br, protect.min_gr]], ..Default::default() };
                    for (i, row) in theme.rows().iter().enumerate() {
                        m.cb.insert((1, 85 + i as u32), [row[0], row[1], row[2], 0.0]);
                    }
                    m.set_component(g("rs"), g("rc"), c[0]);
                    m.set_component(g("gs"), g("gc"), c[1]);
                    m.set_component(g("bs"), g("bc"), c[2]);
                    m.run(&p.inserted);
                    let d = m.get(g("d"));
                    let got = [d[lane(g("d0").chars().next().unwrap())], d[lane(g("d1").chars().next().unwrap())], d[lane(g("d2").chars().next().unwrap())]];
                    let want = expected(&theme, c, &protect);
                    assert!(got.iter().zip(want).all(|(a, b)| (a - b).abs() <= 1e-5 * b.abs().max(1.0)), "{what}: {c:?} with {protect:?}: got {got:?}, want {want:?}");
                }
            }
        }
    }

    #[test]
    fn the_inserted_hud_code_follows_the_rule() {
        for shader in [HUD_SHAPE_1, HUD_SHAPE_2] {
            check_hud_rule(&patch_hud(shader).unwrap(), shader);
        }
        // The whole made-up shader, run end to end: orange is themed, hostile red keeps its colour.
        let swap = HudMatrix { red: [0.0, 0.0, 1.0], green: [0.0, 1.0, 0.0], blue: [1.0, 0.0, 0.0] };
        let patched = patch_hud(HUD_SHAPE_2).unwrap().shader;
        for (c, want) in [([1.0, 0.068, 0.025], [0.025, 0.068, 1.0]), ([1.0, 0.0144, 0.0052], [1.0, 0.0144, 0.0052])] {
            let mut m = Machine { ini: vec![[0.0; 4], [1.0, 0.6, 0.3, 0.035]], ..Default::default() };
            for (i, row) in swap.rows().iter().enumerate() {
                m.cb.insert((1, 85 + i as u32), [row[0], row[1], row[2], 0.0]);
            }
            m.cb.insert((1, 90), [0.0, 1.0, 0.0, 0.0]);
            m.set("v1", [c[0], c[1], c[2], 0.75]);
            m.run(&patched);
            let o0 = m.get("o0");
            assert_eq!(&o0[..3], &want[..], "{c:?}");
            assert_eq!(o0[3], 0.75);
        }
    }

    #[test]
    fn config_round_trips_settings_and_carries_the_patterns() {
        let c = HookConfig { dimmer: Dimmer { on: false, strength: 0.15, reach: 7.5 }, protect: Protect { on: true, max_gr: 0.55, max_br: 0.25, min_gr: 0.04 } };
        let ini = render_config(&c);
        assert!(ini.starts_with(CONFIG_MARK));
        assert!(ini.is_ascii());
        assert!(!ini.replace("\r\n", "").contains('\n'), "CRLF only");
        assert_eq!(read_config(&ini), c);
        assert!(ini.contains("\r\nx = 0.15\r\ny = 7.5\r\nz = 0\r\n"));
        assert!(ini.contains("\r\nx1 = 1\r\ny1 = 0.55\r\nz1 = 0.25\r\nw1 = 0.04\r\n"));
        // F9 switches the dimmer the other way.
        assert!(ini.contains("type = toggle\r\nz = 1\r\n"));
        assert!(ini.contains("[ShaderRegexEliteNexusCockpitLights.Pattern]\r\ndcl_input_ps constant v0\\.xyzw\\n\r\n"));
        assert!(ini.contains("[ShaderRegexEliteNexusCockpitLights.InsertDeclarations]\r\ndcl_resource_texture1d (float,float,float,float) t120\r\n"));
        assert!(ini.contains("[ShaderRegexEliteNexusHudColours]\r\nshader_model = ps_5_0\r\ntemps = nexus_in nexus_tmp nexus_set\r\n"));
        assert!(ini.contains("[ShaderRegexEliteNexusHudColours.Pattern.Replace]\r\nmov ${nexus_in}.x, ${rs}.${rc}\\n\r\n"));
        assert!(ini.contains("\r\n${l1}${l2}${l3}\r\n// Elite Nexus: keep hostile red"));
        assert!(ini.contains("[ShaderRegexEliteNexusHudColours.InsertDeclarations]\r\ndcl_resource_texture1d (float,float,float,float) t120\r\n"));
        // Instruction lines carry no trailing comments (3Dmigoto's assembler reads them as they are).
        for l in HUD_REPLACE.iter().chain(LIGHT_REPLACE.iter()).filter(|l| !l.starts_with("//")) {
            assert!(!l.contains("//") && !l.contains(';'), "{l}");
        }
        // Protection off: its settings are kept, but no HUD shader is patched.
        let off = HookConfig { protect: Protect { on: false, ..c.protect }, ..c };
        let ini = render_config(&off);
        assert!(!ini.contains("ShaderRegexEliteNexusHudColours"));
        assert_eq!(read_config(&ini), off);
        assert_eq!(read_config(&render_config(&HookConfig::default())), HookConfig::default());
        // A config from before the protection existed reads as protection off, with the defaults.
        assert_eq!(read_config("; Elite Nexus\r\n[Constants]\r\nx = 0.6\r\ny = 8\r\nz = 1\r\n"), HookConfig { dimmer: Dimmer { on: true, strength: 0.6, reach: 8.0 }, protect: Protect::default() });
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
        assert!(matches!(install(&game, &fake_files(), &HookConfig::default()), Err(DimmerError::Foreign(true))));
        assert!(matches!(save_config(&game, &HookConfig::default()), Err(DimmerError::NotOurs)));
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
        assert!(matches!(install(&game, &fake_files(), &HookConfig::default()), Err(DimmerError::Foreign(false))));
        std::fs::remove_file(game.join("d3d11.dll")).unwrap();

        install(&game, &fake_files(), &HookConfig::default()).unwrap();
        assert_eq!(status(&game), Status::Installed { config: HookConfig::default() });
        assert_eq!(std::fs::read(game.join("d3d11.dll")).unwrap(), b"fake d3d11");
        assert!(game.join(FIXES_DIR).is_dir());
        let changed = HookConfig { dimmer: Dimmer { on: true, strength: 0.6, reach: 9.0 }, protect: Protect { on: true, ..Protect::default() } };
        save_config(&game, &changed).unwrap();
        assert_eq!(status(&game), Status::Installed { config: changed });
        assert!(std::fs::read_to_string(game.join(CONFIG)).unwrap().contains("[ShaderRegexEliteNexusHudColours]"));
        // The manifest follows the config, so removal still recognises it.
        let manifest: Manifest = serde_json::from_slice(&std::fs::read(game.join(MANIFEST)).unwrap()).unwrap();
        assert!(manifest.files.iter().any(|(n, h)| n == CONFIG && matches_hash(&game.join(CONFIG), h)));
        let off = HookConfig { protect: Protect::default(), ..changed };
        save_config(&game, &off).unwrap();
        assert_eq!(status(&game), Status::Installed { config: off });
        assert!(!std::fs::read_to_string(game.join(CONFIG)).unwrap().contains("[ShaderRegexEliteNexusHudColours]"));

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
        install(&game, &fake_files(), &HookConfig::default()).unwrap();
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
        assert_eq!(status(&game), Status::Unmanaged { config: HookConfig::default() });
        // Taking over without a download needs the official DLLs in place; fakes are refused.
        std::fs::write(game.join("d3d11.dll"), b"fake").unwrap();
        assert!(matches!(install(&game, &[], &HookConfig::default()), Err(DimmerError::UnknownFile(_))));
        // With files supplied it installs over them only if they're the official ones.
        assert!(matches!(install(&game, &fake_files(), &HookConfig::default()), Err(DimmerError::UnknownFile(_))));
        std::fs::remove_file(game.join("d3d11.dll")).unwrap();
        install(&game, &fake_files(), &HookConfig::default()).unwrap();
        assert!(!game.join(FIXES_DIR).join(LEGACY_FIX).exists());
        assert!(matches!(status(&game), Status::Installed { .. }));
        let _ = std::fs::remove_dir_all(&game);
    }

    /// The shaders each pattern must take among the 309 pixel shaders the game loaded on 2026-10-13
    /// (game 4.4): the box-light shader and its variant, and the 12 HUD shaders that apply the colour
    /// matrix.
    const REAL_LIGHTS: [&str; 2] = ["3f366702d540af3e", "7d85f63f9ab11daf"];
    const REAL_HUD: [&str; 12] = [
        "08517b7d32fa1f80",
        "1a696e8bae6eec02",
        "1d5a317f0e831e9d",
        "3582a66bcb0903ae",
        "4a533b975e4998bf",
        "70e91536d34607c7",
        "9798f7cba8194a6c",
        "ad5ad611d07db185",
        "c4f04894e3f6ae79",
        "cc0e441e849ebe4e",
        "ef44eee0c6ab9506",
        "f2ce89f2c5d390ce",
    ];

    /// Checks both patterns against the shaders the game loaded (exported with 3Dmigoto's
    /// `export_shaders`; not in git), and runs the HUD rule through each real HUD shader's own
    /// registers. Run with NEXUS_SHADER_DIR set:
    ///   cargo test -p nexus-hud real_shaders -- --ignored --nocapture
    #[test]
    #[ignore]
    fn real_shaders() {
        let dir = std::env::var("NEXUS_SHADER_DIR").expect("set NEXUS_SHADER_DIR to a ShaderCache folder");
        let (mut lights, mut hud, mut total) = (Vec::new(), Vec::new(), 0);
        for e in std::fs::read_dir(&dir).unwrap().flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if !name.ends_with("-ps.txt") {
                continue;
            }
            total += 1;
            let text = std::fs::read_to_string(e.path()).unwrap().replace("\r\n", "\n");
            let hash = name.trim_end_matches("-ps.txt").to_string();
            let (l, h) = (regex(&LIGHT_PATTERN).find_iter(&text).count(), regex(&HUD_PATTERN).find_iter(&text).count());
            assert!(l <= 1 && h <= 1, "{name}: a pattern matches {l} / {h} times");
            assert!(l + h < 2, "{name}: both patterns match");
            if l == 1 {
                lights.push(hash.clone());
            }
            if h == 1 {
                let p = patch_hud(&text).unwrap();
                println!("{hash}: {}", p.inserted.lines().take(6).skip(3).collect::<Vec<_>>().join(" | "));
                check_hud_rule(&p, &name);
                hud.push(hash);
            }
        }
        lights.sort();
        hud.sort();
        println!("{total} pixel shaders; box lights {lights:?}; HUD {hud:?}");
        assert_eq!(lights, REAL_LIGHTS);
        assert_eq!(hud, REAL_HUD);
    }

    /// Prints the config Elite Nexus writes with the HUD colour protection on, to read it over:
    ///   cargo test -p nexus-hud print_config -- --ignored --nocapture
    #[test]
    #[ignore]
    fn print_config() {
        print!("{}", render_config(&HookConfig { protect: Protect { on: true, ..Protect::default() }, ..HookConfig::default() }));
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
