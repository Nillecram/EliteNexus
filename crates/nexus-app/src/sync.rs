//! Uploads to EDDN / EDSM / Inara / EDAstro: forwards the core's live events to `nexus-uplink`.
//!
//! Only live events ever reach this (the history load never does), and every service stays off
//! until the user enables it in Accounts & sync.

use crate::core::{Core, EventContext, LiveMsg};
use crate::secrets;
use crate::settings::Accounts;
use crossbeam_channel::Receiver;
use nexus_uplink::{EdsmCredentials, InaraCredentials, Uplink, UplinkConfig, UplinkStatus, UploadContext};
use std::path::Path;
use std::sync::Arc;

pub struct SyncService {
    /// None for test runs: the uplink is never started, so its persisted queues are not touched.
    uplink: Option<Arc<Uplink>>,
    applied: UplinkConfig,
}

impl SyncService {
    pub fn start(core: &Core, acc: &Accounts, data_dir: &Path, rt: &tokio::runtime::Handle) -> SyncService {
        let cfg = config(acc);
        let uplink = Arc::new(Uplink::new(cfg.clone(), data_dir, rt));
        let sync = SyncService { uplink: Some(uplink), applied: cfg };
        sync.attach(core);
        sync
    }

    /// No uploads at all (screenshot / replay runs).
    pub fn disabled() -> SyncService {
        SyncService { uplink: None, applied: UplinkConfig::default() }
    }

    /// Follows a (re)started core. The previous forwarder ends when its core stops.
    pub fn attach(&self, core: &Core) {
        if let Some(u) = &self.uplink {
            forward(core.subscribe(), u.clone());
        }
    }

    /// Call after the account settings changed (decrypts the keys, so not every frame).
    pub fn apply(&mut self, acc: &Accounts) {
        let Some(u) = &self.uplink else { return };
        let cfg = config(acc);
        if cfg != self.applied {
            u.set_config(cfg.clone());
            self.applied = cfg;
        }
    }

    pub fn status(&self) -> UplinkStatus {
        self.uplink.as_ref().map(|u| u.status()).unwrap_or_default()
    }

    pub fn any_enabled(&self) -> bool {
        self.applied.any_enabled()
    }

    /// Sends what is batched now (rate limits still apply; the rest stays queued on disk).
    pub fn flush(&self) {
        if let Some(u) = &self.uplink {
            u.flush();
        }
    }
}

fn forward(rx: Receiver<LiveMsg>, uplink: Arc<Uplink>) {
    let spawned = std::thread::Builder::new().name("nexus-sync".into()).spawn(move || {
        // Watcher order is preserved (EDSM swaps Cargo/ShipLocker events for their companion files).
        for msg in rx {
            match msg {
                LiveMsg::Event(e, c) => uplink.on_event(&e, &upload_context(&c)),
                LiveMsg::Companion(cmp, c) => uplink.on_companion(&cmp, &upload_context(&c)),
            }
        }
    });
    if let Err(e) = spawned {
        tracing::error!("sync: cannot start the forwarding thread: {e}");
    }
}

fn upload_context(c: &EventContext) -> UploadContext {
    UploadContext {
        cmdr: c.cmdr.clone(),
        fid: c.fid.clone(),
        game_version: c.game_version.clone(),
        game_build: c.game_build.clone(),
        horizons: c.horizons,
        odyssey: c.odyssey,
        system_name: c.system_name.clone(),
        system_address: c.system_address,
        star_pos: c.star_pos,
        market_id: c.market_id,
        station_name: c.station_name.clone(),
        ship_id: c.ship_id,
        is_beta: nexus_uplink::json::is_beta_version(&c.game_version),
    }
}

/// The uplink configuration for the user's choices (keys decrypted here, never logged).
pub fn config(acc: &Accounts) -> UplinkConfig {
    UplinkConfig {
        eddn: acc.eddn_upload,
        eddn_test_mode: acc.eddn_test_mode,
        eddn_paused: acc.eddn_paused,
        edsm: acc.edsm_enabled.then(|| EdsmCredentials {
            commander_name: acc.edsm_commander.trim().to_string(),
            api_key: secrets::open(&acc.edsm_api_key),
        }),
        inara: acc.inara_enabled.then(|| InaraCredentials { api_key: secrets::open(&acc.inara_api_key), is_being_developed: acc.inara_dev_mode }),
        edastro: acc.edastro_enabled,
        ..UplinkConfig::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_upload_nothing() {
        let s = crate::settings::Settings::default();
        assert!(!config(&s.accounts).any_enabled());
    }

    #[test]
    fn edsm_needs_name_and_key() {
        let acc = Accounts { edsm_enabled: true, edsm_commander: "Jameson".into(), ..Default::default() };
        assert!(!config(&acc).edsm_enabled(), "no key yet");
        let acc = Accounts { edsm_api_key: secrets::seal("k"), ..acc };
        assert!(config(&acc).edsm_enabled());
    }
}
