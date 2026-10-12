//! Persistent user settings (JSON in the data directory).

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Override for the journal folder (None = auto-detect Saved Games).
    pub journal_dir: Option<PathBuf>,
    /// Speak alerts out loud.
    pub tts_enabled: bool,
    /// 0 = low .. 3 = critical: minimum priority that gets spoken.
    pub tts_min_priority: u8,
    pub tts_rate: f32,
    pub tts_volume: f32,
    pub toast_seconds: f32,
    pub ui_scale: f32,
    /// Exploration: bodies worth at least this (mapped) are highlighted.
    pub high_value_threshold: i64,
    /// Copy the next route system to the clipboard after each jump.
    pub auto_copy_next_system: bool,
    pub accounts: Accounts,
    /// The first-run welcome has been completed.
    pub welcomed: bool,
    /// Engineering wishlist (blueprint planner).
    pub wishlist: Vec<WishItem>,
    /// Market price alerts, checked against every live market update.
    pub price_watches: Vec<PriceWatch>,
    /// The "Ask" assistant's model service.
    pub ai: AiSettings,
}

/// Which model answers "Ask" questions. The key is DPAPI-sealed (see secrets.rs).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AiSettings {
    pub provider: nexus_ai::ProviderKind,
    pub api_key: String,
    /// Empty = automatic (the cheapest suitable model the service offers).
    pub model: String,
    /// Empty = the provider's default address (needed only for "Other").
    pub base_url: String,
}

impl AiSettings {
    /// The provider configuration with the key unsealed (for one request; never logged).
    pub fn provider(&self) -> nexus_ai::ProviderConfig {
        nexus_ai::ProviderConfig { kind: self.provider, api_key: crate::secrets::open(&self.api_key), model: self.model.trim().to_string(), base_url: self.base_url.trim().to_string() }
    }

    pub fn is_ready(&self) -> bool {
        !self.provider.needs_key() || !self.api_key.is_empty()
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct WishItem {
    pub label: String,
    pub blueprint: String,
    pub module: String,
    pub from: u8,
    pub to: u8,
    pub rolls: u32,
    pub experimental: Option<String>,
}

/// "Tell me when …": a market rule checked against every live market update.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PriceWatch {
    /// Display name or symbol (`Tritium`, `lowtemperaturediamond`).
    pub commodity: String,
    /// true: a station pays at least `price` (for selling); false: sells for at most `price`.
    pub sell: bool,
    pub price: i64,
    /// From the current position; 0 = anywhere.
    pub max_ly: f64,
    pub min_pad: Option<char>,
    pub carriers: bool,
    /// Minimum demand (selling) or stock (buying).
    pub min_qty: i64,
    pub enabled: bool,
}

impl Default for PriceWatch {
    fn default() -> Self {
        PriceWatch { commodity: "Tritium".into(), sell: true, price: 50_000, max_ly: 50.0, min_pad: Some('L'), carriers: false, min_qty: 100, enabled: true }
    }
}

/// Website/service connections. Secrets are DPAPI-sealed (see secrets.rs).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Accounts {
    /// Contribute market/outfitting/shipyard/exploration data to EDDN (opt-in).
    pub eddn_upload: bool,
    /// Send to EDDN's /test schemas only (for verifying the setup).
    pub eddn_test_mode: bool,
    /// Receive the live EDDN market feed into the local galaxy database.
    pub eddn_listen: bool,
    pub edsm_enabled: bool,
    pub edsm_commander: String,
    pub edsm_api_key: String,
    pub inara_enabled: bool,
    pub inara_api_key: String,
    /// Inara requires new apps to run in "being developed" mode until whitelisted.
    pub inara_dev_mode: bool,
    pub edastro_enabled: bool,
    /// EDDN sharing paused by the commander (the header's Pause button).
    pub eddn_paused: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            journal_dir: None,
            tts_enabled: true,
            tts_min_priority: 1,
            tts_rate: 1.0,
            tts_volume: 0.9,
            toast_seconds: 7.0,
            ui_scale: 1.0,
            high_value_threshold: 1_000_000,
            auto_copy_next_system: true,
            accounts: Accounts { eddn_listen: true, inara_dev_mode: true, ..Default::default() },
            welcomed: false,
            wishlist: Vec::new(),
            price_watches: Vec::new(),
            ai: AiSettings::default(),
        }
    }
}

impl Settings {
    pub fn load(dir: &Path) -> Self {
        std::fs::read_to_string(dir.join("settings.json"))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, dir: &Path) {
        if let Ok(s) = serde_json::to_string_pretty(self) {
            let tmp = dir.join("settings.json.tmp");
            if std::fs::write(&tmp, s).is_ok() {
                let _ = std::fs::rename(&tmp, dir.join("settings.json"));
            }
        }
    }
}

/// Where Elite Nexus keeps its data.
///
/// Portable by default: a `nexus-data` folder next to the executable (keeps everything on the
/// drive the app lives on). Falls back to `%LOCALAPPDATA%\EliteNexus` if that isn't writable.
/// `ELITE_NEXUS_HOME` overrides both.
pub fn data_dir() -> PathBuf {
    if let Some(p) = std::env::var_os("ELITE_NEXUS_HOME") {
        let p = PathBuf::from(p);
        let _ = std::fs::create_dir_all(&p);
        return p;
    }
    if let Some(exe_dir) = std::env::current_exe().ok().and_then(|e| e.parent().map(Path::to_path_buf)) {
        let p = exe_dir.join("nexus-data");
        if std::fs::create_dir_all(&p).is_ok() && is_writable(&p) {
            return p;
        }
    }
    let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
    let p = base.join("EliteNexus");
    let _ = std::fs::create_dir_all(&p);
    p
}

fn is_writable(dir: &Path) -> bool {
    let probe = dir.join(".write-test");
    let ok = std::fs::write(&probe, b"ok").is_ok();
    let _ = std::fs::remove_file(probe);
    ok
}
