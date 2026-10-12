//! Mining sessions: prospector results, refined tonnage, rates.

use nexus_journal::events as ev;
use nexus_journal::Timestamp;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default)]
pub struct ProspectResult {
    pub ts: Timestamp,
    pub content: String,
    pub motherlode: Option<String>,
    pub materials: Vec<(String, f64)>,
    pub remaining: f64,
}

#[derive(Debug, Clone, Default)]
pub struct MiningSession {
    pub start: Timestamp,
    pub last_activity: Timestamp,
    pub system: String,
    pub body: Option<String>,
    pub prospected: u32,
    pub prospects: Vec<ProspectResult>,
    /// Refined tonnes by commodity display name.
    pub refined: BTreeMap<String, u32>,
    pub cracked: u32,
    pub limpets: BTreeMap<String, u32>,
    pub motherlodes: BTreeMap<String, u32>,
    /// Best proportion seen per material (percent).
    pub best_proportion: BTreeMap<String, f64>,
}

impl MiningSession {
    pub fn total_refined(&self) -> u32 {
        self.refined.values().sum()
    }

    pub fn duration_secs(&self) -> i64 {
        (self.last_activity.0 - self.start.0).max(1)
    }

    pub fn tons_per_hour(&self) -> f64 {
        self.total_refined() as f64 * 3600.0 / self.duration_secs() as f64
    }

    pub fn content_counts(&self) -> (u32, u32, u32) {
        let mut h = (0, 0, 0);
        for p in &self.prospects {
            match p.content.as_str() {
                c if c.contains("High") => h.0 += 1,
                c if c.contains("Medium") => h.1 += 1,
                _ => h.2 += 1,
            }
        }
        h
    }
}

#[derive(Debug, Clone, Default)]
pub struct Mining {
    pub session: Option<MiningSession>,
    pub history: Vec<MiningSession>,
}

impl Mining {
    fn session_mut(&mut self, ts: Timestamp, system: &str, body: Option<&str>) -> &mut MiningSession {
        let s = self.session.get_or_insert_with(|| MiningSession {
            start: ts,
            last_activity: ts,
            system: system.to_string(),
            body: body.map(str::to_string),
            ..Default::default()
        });
        s.last_activity = ts;
        s
    }

    pub fn on_prospected(&mut self, p: &ev::ProspectedAsteroid, ts: Timestamp, system: &str, body: Option<&str>) {
        let s = self.session_mut(ts, system, body);
        s.prospected += 1;
        let materials: Vec<(String, f64)> = p
            .materials
            .iter()
            .map(|m| (m.name_localised.clone().unwrap_or_else(|| nexus_data::names::split_camel(&m.name)), m.proportion))
            .collect();
        for (name, prop) in &materials {
            let best = s.best_proportion.entry(name.clone()).or_insert(0.0);
            if *prop > *best {
                *best = *prop;
            }
        }
        let motherlode = p.motherlode_material_localised.clone().or_else(|| p.motherlode_material.clone());
        if let Some(m) = &motherlode {
            *s.motherlodes.entry(m.clone()).or_default() += 1;
        }
        s.prospects.push(ProspectResult {
            ts,
            content: p.content_localised.clone().unwrap_or_else(|| p.content.clone()),
            motherlode,
            materials,
            remaining: p.remaining,
        });
    }

    pub fn on_refined(&mut self, r: &ev::MiningRefined, ts: Timestamp, system: &str, body: Option<&str>) {
        let s = self.session_mut(ts, system, body);
        let name = r.kind_localised.clone().unwrap_or_else(|| nexus_data::names::humanize_symbol(&r.kind));
        *s.refined.entry(name).or_default() += 1;
    }

    pub fn on_cracked(&mut self, ts: Timestamp, system: &str, body: Option<&str>) {
        self.session_mut(ts, system, body).cracked += 1;
    }

    pub fn on_drone(&mut self, d: &ev::LaunchDrone, ts: Timestamp) {
        if let Some(s) = self.session.as_mut() {
            *s.limpets.entry(d.kind.clone()).or_default() += 1;
            s.last_activity = ts;
        } else if d.kind == "Prospector" || d.kind == "Collection" {
            // first limpet of a session: start tracking right away
            let s = self.session.get_or_insert_with(|| MiningSession { start: ts, last_activity: ts, ..Default::default() });
            *s.limpets.entry(d.kind.clone()).or_default() += 1;
        }
    }

    /// Ends the session (jumped away or docked).
    pub fn end_session(&mut self) {
        if let Some(s) = self.session.take()
            && (s.prospected > 0 || s.total_refined() > 0) {
                self.history.push(s);
            }
    }
}
