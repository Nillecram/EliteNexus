//! Long-term history: jumps, visited systems, docks, deaths, the credit ledger.

use nexus_journal::Timestamp;
use rustc_hash::FxHashMap;

#[derive(Debug, Clone)]
pub struct JumpRecord {
    pub ts: Timestamp,
    pub system: String,
    pub address: u64,
    pub pos: Option<[f64; 3]>,
    pub dist: f64,
    pub fuel_used: f64,
    pub carrier: bool,
}

#[derive(Debug, Clone)]
pub struct VisitedSystem {
    pub name: String,
    pub pos: Option<[f64; 3]>,
    pub first: Timestamp,
    pub last: Timestamp,
    pub visits: u32,
}

#[derive(Debug, Clone)]
pub struct DockRecord {
    pub ts: Timestamp,
    pub station: String,
    pub station_type: String,
    pub system: String,
    pub market_id: u64,
}

#[derive(Debug, Clone)]
pub struct DeathRecord {
    pub ts: Timestamp,
    pub system: String,
    pub killers: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum LedgerCategory {
    Trade,
    Mining,
    Exploration,
    Exobiology,
    Bounties,
    CombatBonds,
    Missions,
    Ships,
    Outfitting,
    Repairs,
    Fuel,
    Ammo,
    Fines,
    Rebuy,
    Carrier,
    Colonisation,
    Materials,
    Taxi,
    /// Moving money between the commander and their own carrier: not income or spending.
    Transfer,
    Other,
}

impl LedgerCategory {
    pub fn label(self) -> &'static str {
        match self {
            LedgerCategory::Trade => "Trade",
            LedgerCategory::Mining => "Mining",
            LedgerCategory::Exploration => "Exploration",
            LedgerCategory::Exobiology => "Exobiology",
            LedgerCategory::Bounties => "Bounties",
            LedgerCategory::CombatBonds => "Combat bonds",
            LedgerCategory::Missions => "Missions",
            LedgerCategory::Ships => "Ships",
            LedgerCategory::Outfitting => "Outfitting",
            LedgerCategory::Repairs => "Repairs",
            LedgerCategory::Fuel => "Fuel",
            LedgerCategory::Ammo => "Ammo & restock",
            LedgerCategory::Fines => "Fines",
            LedgerCategory::Rebuy => "Rebuy",
            LedgerCategory::Carrier => "Carrier",
            LedgerCategory::Colonisation => "Colonisation",
            LedgerCategory::Materials => "Materials",
            LedgerCategory::Taxi => "Taxi",
            LedgerCategory::Transfer => "Carrier transfers",
            LedgerCategory::Other => "Other",
        }
    }
}

#[derive(Debug, Clone)]
pub struct LedgerEntry {
    pub ts: Timestamp,
    pub category: LedgerCategory,
    /// Positive = income, negative = expense.
    pub amount: i64,
    pub note: String,
}

#[derive(Debug, Clone, Default)]
pub struct History {
    pub jumps: Vec<JumpRecord>,
    pub visited: FxHashMap<u64, VisitedSystem>,
    pub total_distance_ly: f64,
    pub docks: Vec<DockRecord>,
    pub deaths: Vec<DeathRecord>,
    pub ledger: Vec<LedgerEntry>,
    /// Absolute credit balance samples (LoadGame + live Status balance changes).
    pub credit_samples: Vec<(Timestamp, i64)>,
}

impl History {
    pub fn visit(&mut self, address: u64, name: &str, pos: Option<[f64; 3]>, ts: Timestamp) {
        let v = self.visited.entry(address).or_insert_with(|| VisitedSystem {
            name: name.to_string(),
            pos,
            first: ts,
            last: ts,
            visits: 0,
        });
        v.visits += 1;
        v.last = ts;
        if v.pos.is_none() {
            v.pos = pos;
        }
    }

    pub fn jump(&mut self, rec: JumpRecord) {
        self.total_distance_ly += rec.dist;
        self.visit(rec.address, &rec.system, rec.pos, rec.ts);
        self.jumps.push(rec);
    }

    pub fn ledger(&mut self, ts: Timestamp, category: LedgerCategory, amount: i64, note: impl Into<String>) {
        if amount != 0 {
            self.ledger.push(LedgerEntry { ts, category, amount, note: note.into() });
        }
    }

    pub fn credit_sample(&mut self, ts: Timestamp, credits: i64) {
        if self.credit_samples.last().is_none_or(|(_, c)| *c != credits) {
            self.credit_samples.push((ts, credits));
        }
    }

    /// Income/expense totals per category since `since`.
    pub fn totals_since(&self, since: Timestamp) -> Vec<(LedgerCategory, i64, i64)> {
        let mut map: std::collections::BTreeMap<LedgerCategory, (i64, i64)> = Default::default();
        for e in self.ledger.iter().filter(|e| e.ts >= since) {
            let t = map.entry(e.category).or_default();
            if e.amount >= 0 {
                t.0 += e.amount;
            } else {
                t.1 += e.amount;
            }
        }
        map.into_iter().map(|(k, (i, x))| (k, i, x)).collect()
    }
}
