//! Background Simulation (BGS) contributions and Powerplay merits.
//!
//! Every action that moves a faction's influence is recorded with the system and faction it
//! affects; the UI aggregates by period (since the last tick, today, 7 days…).

use nexus_journal::events as ev;
use nexus_journal::Timestamp;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum BgsKind {
    /// Mission influence: `amount` = number of `+` (negative for down-trends).
    MissionInfluence,
    MissionFailed,
    Bounty,
    CombatBond,
    /// Trade profit at a station (credits).
    Trade,
    /// Black-market sale (credits).
    BlackMarket,
    Exploration,
    Exobiology,
    Murder,
    Crime,
}

impl BgsKind {
    pub fn label(self) -> &'static str {
        match self {
            BgsKind::MissionInfluence => "Mission influence",
            BgsKind::MissionFailed => "Missions failed",
            BgsKind::Bounty => "Bounties",
            BgsKind::CombatBond => "Combat bonds",
            BgsKind::Trade => "Trade profit",
            BgsKind::BlackMarket => "Black market",
            BgsKind::Exploration => "Exploration data",
            BgsKind::Exobiology => "Exobiology data",
            BgsKind::Murder => "Murders",
            BgsKind::Crime => "Other crimes",
        }
    }

    /// Does this action help (true) or hurt (false) the faction?
    pub fn positive(self) -> bool {
        !matches!(self, BgsKind::MissionFailed | BgsKind::Murder | BgsKind::Crime)
    }
}

#[derive(Debug, Clone)]
pub struct BgsAction {
    pub ts: Timestamp,
    pub system_address: u64,
    pub system: String,
    pub faction: String,
    pub kind: BgsKind,
    /// Credits for money-based actions, `+` count for influence, 1 per occurrence otherwise.
    pub amount: i64,
}

#[derive(Debug, Clone, Default)]
pub struct Bgs {
    pub actions: Vec<BgsAction>,
    /// (ts, merits gained, total) per PowerplayMerits event.
    pub merits: Vec<(Timestamp, i64, i64)>,
}

/// Where the commander is right now (for attributing station-based actions).
pub struct Here<'a> {
    pub system: &'a str,
    pub system_address: u64,
    pub station_faction: Option<&'a str>,
}

impl Bgs {
    fn push(&mut self, ts: Timestamp, system_address: u64, system: &str, faction: &str, kind: BgsKind, amount: i64) {
        // Fleet/squadron carriers report the pseudo-faction "FleetCarrier": they are outside the BGS.
        if faction.is_empty() || amount == 0 || faction.eq_ignore_ascii_case("FleetCarrier") {
            return;
        }
        self.actions.push(BgsAction { ts, system_address, system: system.to_string(), faction: faction.to_string(), kind, amount });
    }

    /// `FactionEffects[].Influence[]` gives the systems and the `+` strength per faction.
    pub fn on_mission_completed(&mut self, c: &ev::MissionCompleted, ts: Timestamp, resolve_system: impl Fn(u64) -> String) {
        for fe in &c.faction_effects {
            for inf in &fe.influence {
                let plus = inf.influence.as_deref().map(|s| s.chars().filter(|c| *c == '+').count() as i64).unwrap_or(0);
                let sign = if inf.trend.as_deref().is_some_and(|t| t.starts_with("Down")) { -1 } else { 1 };
                let addr = inf.system_address.unwrap_or(0);
                self.push(ts, addr, &resolve_system(addr), &fe.faction, BgsKind::MissionInfluence, plus * sign);
            }
        }
    }

    pub fn on_mission_failed(&mut self, faction: &str, ts: Timestamp, here: &Here) {
        self.push(ts, here.system_address, here.system, faction, BgsKind::MissionFailed, 1);
    }

    pub fn on_redeem_voucher(&mut self, r: &ev::RedeemVoucher, ts: Timestamp, here: &Here) {
        match r.kind.to_ascii_lowercase().as_str() {
            "bounty" => {
                if r.factions.is_empty()
                    && let Some(f) = &r.faction {
                        self.push(ts, here.system_address, here.system, f, BgsKind::Bounty, r.amount);
                    }
                for f in &r.factions {
                    self.push(ts, here.system_address, here.system, &f.faction, BgsKind::Bounty, f.reward);
                }
            }
            "combatbond" => {
                if let Some(f) = &r.faction {
                    self.push(ts, here.system_address, here.system, f, BgsKind::CombatBond, r.amount);
                }
            }
            _ => {}
        }
    }

    pub fn on_market_sell(&mut self, s: &ev::MarketSell, ts: Timestamp, here: &Here) {
        let Some(f) = here.station_faction else { return };
        let profit = s.total_sale - s.avg_price_paid * s.count;
        let kind = if s.black_market.unwrap_or(false) { BgsKind::BlackMarket } else { BgsKind::Trade };
        if profit > 0 {
            self.push(ts, here.system_address, here.system, f, kind, profit);
        }
    }

    pub fn on_data_sale(&mut self, kind: BgsKind, earnings: i64, ts: Timestamp, here: &Here) {
        if let Some(f) = here.station_faction {
            self.push(ts, here.system_address, here.system, f, kind, earnings);
        }
    }

    pub fn on_crime(&mut self, c: &ev::CommitCrime, ts: Timestamp, here: &Here) {
        let kind = if c.crime_type.to_ascii_lowercase().contains("murder") { BgsKind::Murder } else { BgsKind::Crime };
        // Murders hurt the victim's faction; other crimes the faction whose law was broken.
        self.push(ts, here.system_address, here.system, &c.faction, kind, 1);
    }

    pub fn on_merits(&mut self, m: &ev::PowerplayMerits, ts: Timestamp) {
        self.merits.push((ts, m.merits_gained, m.total_merits));
    }

    /// Totals per (system, faction, kind) since `since`.
    pub fn summary(&self, since: Timestamp) -> Vec<SystemSummary> {
        use std::collections::BTreeMap;
        let mut map: BTreeMap<(String, String), BTreeMap<BgsKind, (i64, u32)>> = BTreeMap::new();
        for a in self.actions.iter().filter(|a| a.ts >= since) {
            let e = map.entry((a.system.clone(), a.faction.clone())).or_default().entry(a.kind).or_default();
            e.0 += a.amount;
            e.1 += 1;
        }
        let mut out: Vec<SystemSummary> = Vec::new();
        for ((system, faction), kinds) in map {
            let f = FactionSummary { faction, kinds: kinds.into_iter().map(|(k, (amt, n))| (k, amt, n)).collect() };
            match out.last_mut() {
                Some(s) if s.system == system => s.factions.push(f),
                _ => out.push(SystemSummary { system, factions: vec![f] }),
            }
        }
        out
    }

    pub fn merits_since(&self, since: Timestamp) -> i64 {
        self.merits.iter().filter(|(t, _, _)| *t >= since).map(|(_, g, _)| *g).sum()
    }
}

#[derive(Debug, Clone)]
pub struct FactionSummary {
    pub faction: String,
    /// (kind, total amount, occurrences)
    pub kinds: Vec<(BgsKind, i64, u32)>,
}

impl FactionSummary {
    pub fn influence_plus(&self) -> i64 {
        self.kinds.iter().filter(|(k, _, _)| *k == BgsKind::MissionInfluence).map(|(_, a, _)| *a).sum()
    }
}

#[derive(Debug, Clone)]
pub struct SystemSummary {
    pub system: String,
    pub factions: Vec<FactionSummary>,
}

/// Start of the current BGS day. The tick moves around; a fixed UTC hour is a good default and the
/// UI lets the commander override it.
pub fn last_tick(now: Timestamp, tick_hour_utc: u32) -> Timestamp {
    let day = now.0.div_euclid(86_400) * 86_400;
    let tick = day + i64::from(tick_hour_utc.min(23)) * 3600;
    Timestamp(if tick <= now.0 { tick } else { tick - 86_400 })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tick_math() {
        // 2026-10-12T10:00:00Z
        let now = Timestamp::parse("2026-10-12T10:00:00Z").unwrap();
        assert_eq!(last_tick(now, 8).to_iso(), "2026-10-12T08:00:00Z");
        assert_eq!(last_tick(now, 14).to_iso(), "2026-10-11T14:00:00Z");
    }

    #[test]
    fn influence_plus_counting() {
        let mut b = Bgs::default();
        let c = ev::MissionCompleted {
            faction: "A".into(),
            faction_effects: vec![ev::FactionEffect {
                faction: "A".into(),
                influence: vec![ev::InfluenceEffect { system_address: Some(1), trend: Some("UpGood".into()), influence: Some("+++".into()) }],
                ..Default::default()
            }],
            ..Default::default()
        };
        b.on_mission_completed(&c, Timestamp(10), |_| "Sys".into());
        let s = b.summary(Timestamp::ZERO);
        assert_eq!(s[0].factions[0].influence_plus(), 3);
    }
}
