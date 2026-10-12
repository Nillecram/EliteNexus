//! Alerts raised by the model for live events (toasts, the alert feed, text-to-speech).

use nexus_journal::Timestamp;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Priority {
    Low,
    Normal,
    High,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AlertKind {
    Exploration,
    Exobiology,
    Navigation,
    Trade,
    Mining,
    Missions,
    Combat,
    Carrier,
    Colonisation,
    Engineering,
    Commander,
    System,
}

impl AlertKind {
    pub fn label(self) -> &'static str {
        match self {
            AlertKind::Exploration => "Exploration",
            AlertKind::Exobiology => "Exobiology",
            AlertKind::Navigation => "Navigation",
            AlertKind::Trade => "Trade",
            AlertKind::Mining => "Mining",
            AlertKind::Missions => "Missions",
            AlertKind::Combat => "Combat",
            AlertKind::Carrier => "Carrier",
            AlertKind::Colonisation => "Colonisation",
            AlertKind::Engineering => "Engineering",
            AlertKind::Commander => "Commander",
            AlertKind::System => "System",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Alert {
    pub ts: Timestamp,
    pub kind: AlertKind,
    pub priority: Priority,
    pub title: String,
    pub detail: String,
    /// Text for text-to-speech; `None` means the alert is visual only.
    pub speech: Option<String>,
}

impl Alert {
    pub fn new(ts: Timestamp, kind: AlertKind, priority: Priority, title: impl Into<String>) -> Self {
        Alert { ts, kind, priority, title: title.into(), detail: String::new(), speech: None }
    }

    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = detail.into();
        self
    }

    pub fn speak(mut self, text: impl Into<String>) -> Self {
        self.speech = Some(text.into());
        self
    }
}

/// Sink for alerts. Historical replays use a disabled sink so nothing fires for old events.
#[derive(Debug, Default)]
pub struct Alerts {
    pub enabled: bool,
    pub items: Vec<Alert>,
}

impl Alerts {
    pub fn live() -> Self {
        Alerts { enabled: true, items: Vec::new() }
    }

    pub fn disabled() -> Self {
        Alerts { enabled: false, items: Vec::new() }
    }

    #[inline]
    pub fn push(&mut self, a: Alert) {
        if self.enabled {
            self.items.push(a);
        }
    }

    /// Only builds the alert when alerts are enabled (avoids formatting work during replays).
    #[inline]
    pub fn push_with(&mut self, f: impl FnOnce() -> Alert) {
        if self.enabled {
            self.items.push(f());
        }
    }
}
