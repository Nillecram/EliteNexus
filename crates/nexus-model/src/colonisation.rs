//! System colonisation: construction depots and claims.
//!
//! The game writes a depot's snapshot (`ColonisationConstructionDepot`) only while you're docked there, so every
//! figure here is from your last visit. The journal doesn't say whose project a depot is: a depot counts as yours
//! when you claimed its system or delivered to it (see [`Involvement`]).

use crate::location::StationInfo;
use nexus_journal::events as ev;
use nexus_journal::Timestamp;
use std::collections::BTreeMap;

/// A depot not updated for this long is out of date: a colonisation ship has four weeks to be finished, so one
/// last seen this long ago has been finished or has expired by now.
pub const STALE_AFTER_SECS: i64 = 28 * 86_400;

#[derive(Debug, Clone, Default)]
pub struct DepotResource {
    pub symbol: String,
    pub name: String,
    pub required: i64,
    pub provided: i64,
    pub payment: i64,
}

impl DepotResource {
    pub fn remaining(&self) -> i64 {
        (self.required - self.provided).max(0)
    }
}

#[derive(Debug, Clone, Default)]
pub struct Depot {
    pub market_id: u64,
    pub system: String,
    pub system_address: u64,
    pub station: Option<String>,
    /// A System Colonisation Ship: the primary port of a newly claimed system.
    pub colonisation_ship: bool,
    pub progress: f64,
    pub complete: bool,
    pub failed: bool,
    pub resources: Vec<DepotResource>,
    pub updated: Timestamp,
    /// What this commander contributed (by commodity display name).
    pub my_contributions: BTreeMap<String, i64>,
}

impl Depot {
    pub fn remaining_tons(&self) -> i64 {
        self.resources.iter().map(DepotResource::remaining).sum()
    }

    pub fn required_tons(&self) -> i64 {
        self.resources.iter().map(|r| r.required).sum()
    }

    pub fn finished(&self) -> bool {
        self.complete || self.failed
    }

    /// Unfinished, and not updated for [`STALE_AFTER_SECS`] or more.
    pub fn is_stale(&self, now: Timestamp) -> bool {
        !self.finished() && now.secs_since(self.updated) >= STALE_AFTER_SECS
    }

    pub fn delivered_tons(&self) -> i64 {
        self.my_contributions.values().sum()
    }
}

/// How the commander is involved with a depot, as far as the journal shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Involvement {
    /// In a system you claimed (and haven't released).
    Claimed,
    /// You delivered to it.
    Contributed,
    /// You only docked there: most likely someone else's project.
    Visited,
}

impl Involvement {
    pub fn is_yours(self) -> bool {
        self != Involvement::Visited
    }
}

#[derive(Debug, Clone, Default)]
pub struct Claim {
    pub system: String,
    pub system_address: u64,
    pub ts: Timestamp,
    pub beacon_deployed: bool,
    pub released: bool,
}

#[derive(Debug, Clone, Default)]
pub struct Colonisation {
    pub depots: BTreeMap<u64, Depot>,
    pub claims: Vec<Claim>,
}

impl Colonisation {
    /// A depot snapshot. `docked_at` is the station you're docked at, if any: a depot is placed only by the station
    /// with its MarketID, since the snapshot itself names no system or station. A snapshot of a depot you aren't
    /// docked at updates it if it's known, and is ignored otherwise.
    pub fn on_depot(&mut self, d: &ev::ColonisationConstructionDepot, ts: Timestamp, docked_at: Option<&StationInfo>) {
        if let Some(st) = docked_at.filter(|st| st.market_id == d.market_id && d.market_id != 0) {
            let depot = self.depots.entry(d.market_id).or_insert_with(|| Depot { market_id: d.market_id, ..Default::default() });
            depot.system.clone_from(&st.system);
            depot.system_address = st.system_address;
            depot.station = Some(st.name.clone()).filter(|n| !n.is_empty());
            depot.colonisation_ship = st.is_colonisation_ship();
        }
        let Some(depot) = self.depots.get_mut(&d.market_id) else { return };
        depot.progress = d.construction_progress;
        depot.complete = d.construction_complete;
        depot.failed = d.construction_failed;
        depot.resources = d
            .resources_required
            .iter()
            .map(|r| DepotResource {
                symbol: nexus_journal::companion::commodity_symbol(&r.name),
                name: r.name_localised.clone().unwrap_or_else(|| nexus_data::names::humanize_symbol(&r.name)),
                required: r.required_amount,
                provided: r.provided_amount,
                payment: r.payment,
            })
            .collect();
        depot.updated = ts;
    }

    pub fn on_contribution(&mut self, c: &ev::ColonisationContribution) {
        if let Some(depot) = self.depots.get_mut(&c.market_id) {
            for item in &c.contributions {
                let name = item.name_localised.clone().unwrap_or_else(|| nexus_data::names::humanize_symbol(&item.name));
                *depot.my_contributions.entry(name).or_default() += item.amount;
            }
        }
    }

    pub fn on_claim(&mut self, c: &ev::ColonisationSystem, ts: Timestamp) {
        self.claims.push(Claim {
            system: c.star_system.clone(),
            system_address: c.system_address,
            ts,
            beacon_deployed: false,
            released: false,
        });
    }

    pub fn on_claim_release(&mut self, c: &ev::ColonisationSystem) {
        for claim in self.claims.iter_mut().filter(|cl| cl.system_address == c.system_address) {
            claim.released = true;
        }
    }

    pub fn on_beacon(&mut self, c: &ev::ColonisationSystem, ts: Timestamp) {
        match self.claims.iter_mut().find(|cl| cl.system_address == c.system_address) {
            Some(claim) => claim.beacon_deployed = true,
            None => self.claims.push(Claim {
                system: c.star_system.clone(),
                system_address: c.system_address,
                ts,
                beacon_deployed: true,
                released: false,
            }),
        }
    }

    pub fn involvement(&self, d: &Depot) -> Involvement {
        if d.system_address != 0 && self.claims.iter().any(|c| c.system_address == d.system_address && !c.released) {
            Involvement::Claimed
        } else if d.delivered_tons() > 0 {
            Involvement::Contributed
        } else {
            Involvement::Visited
        }
    }

    /// Your unfinished, up-to-date constructions (claimed or delivered to), most recently updated first.
    pub fn active_depots(&self, now: Timestamp) -> impl Iterator<Item = &Depot> {
        let mut v: Vec<&Depot> = self.depots.values().filter(|d| !d.finished() && !d.is_stale(now) && self.involvement(d).is_yours()).collect();
        v.sort_by_key(|d| std::cmp::Reverse(d.updated));
        v.into_iter()
    }
}
