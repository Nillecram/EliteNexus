//! Per-service counters shown in the UI.

use nexus_journal::Timestamp;
use parking_lot::Mutex;
use serde::Serialize;

/// State of one upload service.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct ServiceStatus {
    /// Enabled in the configuration (and credentials present where required).
    pub enabled: bool,
    /// Messages/events waiting in the persistent queue.
    pub queued: usize,
    /// Messages/events accepted by the service.
    pub sent: u64,
    /// Messages/events rejected by the service or dropped after retries/expiry.
    pub failed: u64,
    /// The most recent error (rejection text, HTTP status, transport error).
    pub last_error: Option<String>,
    /// When the service last accepted something.
    pub last_success: Option<Timestamp>,
    /// Set while sending is suspended (bad credentials, blocked software, rate limit, backoff).
    pub paused: Option<String>,
}

/// Snapshot of every service, from [`crate::Uplink::status`].
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct UplinkStatus {
    pub eddn: ServiceStatus,
    pub edsm: ServiceStatus,
    pub inara: ServiceStatus,
    pub edastro: ServiceStatus,
}

/// Live counters for one service (the `queued`/`enabled` fields are filled in at snapshot time).
#[derive(Debug, Default)]
pub(crate) struct Counters {
    inner: Mutex<ServiceStatus>,
}

impl Counters {
    pub fn sent(&self, n: u64) {
        let mut s = self.inner.lock();
        s.sent += n;
        s.last_success = Some(Timestamp::now());
    }

    pub fn failed(&self, n: u64, error: impl Into<String>) {
        let mut s = self.inner.lock();
        s.failed += n;
        s.last_error = Some(error.into());
    }

    pub fn error(&self, error: impl Into<String>) {
        self.inner.lock().last_error = Some(error.into());
    }

    pub fn set_paused(&self, reason: Option<String>) {
        self.inner.lock().paused = reason;
    }

    pub fn snapshot(&self, enabled: bool, queued: usize) -> ServiceStatus {
        let mut s = self.inner.lock().clone();
        s.enabled = enabled;
        s.queued = queued;
        s
    }
}
