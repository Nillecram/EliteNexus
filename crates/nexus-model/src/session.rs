//! The current play session (since the last LoadGame).

use nexus_journal::Timestamp;

#[derive(Debug, Clone, Default)]
pub struct Session {
    pub start: Timestamp,
    pub end: Option<Timestamp>,
    pub start_credits: i64,
    pub jumps: u32,
    pub distance_ly: f64,
    pub bodies_scanned: u32,
    pub bodies_mapped: u32,
    pub organics_analysed: u32,
    pub systems_visited: u32,
    pub kills: u32,
    pub missions_completed: u32,
    pub income: i64,
    pub expenses: i64,
}

impl Session {
    pub fn start(ts: Timestamp, credits: i64) -> Self {
        Session { start: ts, start_credits: credits, ..Default::default() }
    }

    pub fn active(&self) -> bool {
        !self.start.is_zero() && self.end.is_none()
    }

    pub fn duration_secs(&self, now: Timestamp) -> i64 {
        (self.end.unwrap_or(now).0 - self.start.0).max(0)
    }

    pub fn record_money(&mut self, amount: i64) {
        if amount >= 0 {
            self.income += amount;
        } else {
            self.expenses += amount;
        }
    }
}
