//! System colonisation: construction depots and claims.

use nexus_journal::events as ev;
use nexus_journal::Timestamp;
use std::collections::BTreeMap;

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
    pub fn on_depot(
        &mut self,
        d: &ev::ColonisationConstructionDepot,
        ts: Timestamp,
        system: &str,
        system_address: u64,
        station: Option<&str>,
    ) {
        let depot = self.depots.entry(d.market_id).or_insert_with(|| Depot {
            market_id: d.market_id,
            system: system.to_string(),
            system_address,
            station: station.map(str::to_string),
            ..Default::default()
        });
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

    pub fn active_depots(&self) -> impl Iterator<Item = &Depot> {
        self.depots.values().filter(|d| !d.complete && !d.failed)
    }
}
