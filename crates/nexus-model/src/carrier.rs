//! The commander's own fleet carrier.

use nexus_journal::events as ev;
use nexus_journal::Timestamp;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default)]
pub struct CarrierJumpRecord {
    pub ts: Timestamp,
    pub system: String,
    pub body: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct PendingJump {
    pub system: String,
    pub system_address: u64,
    pub body: Option<String>,
    pub departure: Option<Timestamp>,
    pub requested: Timestamp,
}

#[derive(Debug, Clone, Default)]
pub struct TradeOrder {
    pub commodity: String,
    pub purchase: Option<i64>,
    pub sale: Option<i64>,
    pub price: Option<i64>,
    pub black_market: bool,
    pub updated: Timestamp,
}

#[derive(Debug, Clone, Default)]
pub struct Carrier {
    pub id: Option<u64>,
    pub carrier_type: Option<String>,
    pub callsign: String,
    pub name: String,
    pub fuel: i64,
    pub jump_range_curr: f64,
    pub jump_range_max: f64,
    pub docking_access: String,
    pub allow_notorious: bool,
    pub pending_decommission: bool,
    pub space: ev::CarrierSpaceUsage,
    pub finance: ev::CarrierFinanceInfo,
    pub crew: Vec<ev::CarrierCrewMember>,
    pub system: Option<String>,
    pub system_address: Option<u64>,
    pub body_id: Option<u32>,
    pub pending_jump: Option<PendingJump>,
    pub jumps: Vec<CarrierJumpRecord>,
    pub trade_orders: BTreeMap<String, TradeOrder>,
    pub stats_updated: Timestamp,
}

impl Carrier {
    pub fn owned(&self) -> bool {
        self.id.is_some()
    }

    fn mine(&self, id: u64) -> bool {
        self.id.is_none_or(|own| own == id)
    }

    pub fn on_stats(&mut self, s: &ev::CarrierStats, ts: Timestamp) {
        // Only squadron carriers can share the event shape; keep the personal one.
        if s.carrier_type.as_deref().is_some_and(|t| t != "FleetCarrier") && self.id.is_some() {
            return;
        }
        self.id = Some(s.carrier_id);
        self.carrier_type = s.carrier_type.clone();
        self.callsign = s.callsign.clone();
        self.name = s.name.clone();
        self.fuel = s.fuel_level;
        self.jump_range_curr = s.jump_range_curr;
        self.jump_range_max = s.jump_range_max;
        self.docking_access = s.docking_access.clone();
        self.allow_notorious = s.allow_notorious;
        self.pending_decommission = s.pending_decommission;
        self.space = s.space_usage.clone();
        self.finance = s.finance.clone();
        self.crew = s.crew.clone();
        self.stats_updated = ts;
    }

    pub fn on_buy(&mut self, b: &ev::CarrierBuy, ts: Timestamp) {
        self.id = Some(b.carrier_id);
        self.callsign = b.callsign.clone();
        self.system = Some(b.location.clone());
        self.system_address = Some(b.system_address);
        self.stats_updated = ts;
    }

    pub fn on_jump_request(&mut self, r: &ev::CarrierJumpRequest, ts: Timestamp) {
        if !self.mine(r.carrier_id) {
            return;
        }
        self.id.get_or_insert(r.carrier_id);
        self.pending_jump = Some(PendingJump {
            system: r.system_name.clone(),
            system_address: r.system_address,
            body: r.body.clone(),
            departure: r.departure_time.as_deref().and_then(Timestamp::parse),
            requested: ts,
        });
    }

    pub fn on_jump_cancelled(&mut self, c: &ev::CarrierLocation) {
        if self.mine(c.carrier_id) {
            self.pending_jump = None;
        }
    }

    pub fn on_location(&mut self, l: &ev::CarrierLocation, ts: Timestamp) {
        if l.carrier_type.as_deref().is_some_and(|t| t != "FleetCarrier") {
            return;
        }
        if !self.mine(l.carrier_id) {
            return;
        }
        self.id.get_or_insert(l.carrier_id);
        let moved = self.system_address.is_some() && self.system_address != l.system_address;
        if moved
            && let Some(sys) = &l.star_system {
                self.jumps.push(CarrierJumpRecord { ts, system: sys.clone(), body: None });
            }
        if l.star_system.is_some() {
            self.system = l.star_system.clone();
        }
        self.system_address = l.system_address.or(self.system_address);
        self.body_id = l.body_id;
        if let Some(p) = &self.pending_jump
            && Some(p.system_address) == l.system_address {
                self.pending_jump = None;
            }
    }

    /// The commander is docked on a carrier that jumped (CarrierJump event).
    pub fn on_carrier_jump(&mut self, a: &ev::SystemArrival, ts: Timestamp) {
        if a.market_id.is_some() && a.market_id == self.id {
            self.system = Some(a.star_system.clone());
            self.system_address = Some(a.system_address);
            self.body_id = a.body_id;
            self.jumps.push(CarrierJumpRecord { ts, system: a.star_system.clone(), body: a.body.clone() });
            self.pending_jump = None;
        }
    }

    /// Our own carrier jump has departed long enough ago that it must have arrived.
    pub fn settle_pending(&mut self, now: Timestamp) {
        let arrived = self.pending_jump.as_ref().and_then(|p| p.departure).is_some_and(|d| now.0 > d.0 + 60 * 5);
        if arrived
            && let Some(p) = self.pending_jump.take() {
                self.system = Some(p.system.clone());
                self.system_address = Some(p.system_address);
                self.jumps.push(CarrierJumpRecord { ts: p.departure.unwrap_or(now), system: p.system, body: p.body });
            }
    }

    pub fn on_finance(&mut self, f: &ev::CarrierFinance) {
        if !self.mine(f.carrier_id) {
            return;
        }
        self.finance.carrier_balance = f.carrier_balance;
        self.finance.reserve_balance = f.reserve_balance;
        self.finance.available_balance = f.available_balance;
        if f.reserve_percent.is_some() {
            self.finance.reserve_percent = f.reserve_percent;
        }
    }

    pub fn on_bank_transfer(&mut self, t: &ev::CarrierBankTransfer) {
        if self.mine(t.carrier_id) {
            self.finance.carrier_balance = t.carrier_balance;
        }
    }

    pub fn on_deposit_fuel(&mut self, d: &ev::CarrierDepositFuel) {
        if self.mine(d.carrier_id) {
            self.fuel = d.total;
        }
    }

    pub fn on_trade_order(&mut self, o: &ev::CarrierTradeOrder, ts: Timestamp) {
        if !self.mine(o.carrier_id) {
            return;
        }
        let name = o.commodity_localised.clone().unwrap_or_else(|| o.commodity.clone());
        if o.cancel_trade.unwrap_or(false) {
            self.trade_orders.remove(&name);
            return;
        }
        self.trade_orders.insert(
            name.clone(),
            TradeOrder {
                commodity: name,
                purchase: o.purchase_order,
                sale: o.sale_order,
                price: o.price,
                black_market: o.black_market,
                updated: ts,
            },
        );
    }

    pub fn on_name_change(&mut self, n: &ev::CarrierNameChange) {
        if self.mine(n.carrier_id) {
            self.name = n.name.clone();
            self.callsign = n.callsign.clone();
        }
    }

    pub fn on_crew_services(&mut self, c: &ev::CarrierCrewServices) {
        if c.carrier_type.as_deref().is_some_and(|t| t != "FleetCarrier") {
            return;
        }
        if !self.mine(c.carrier_id) {
            return;
        }
        self.id.get_or_insert(c.carrier_id);
        let entry = match self.crew.iter_mut().position(|m| m.crew_role == c.crew_role) {
            Some(i) => &mut self.crew[i],
            None => {
                self.crew.push(ev::CarrierCrewMember { crew_role: c.crew_role.clone(), ..Default::default() });
                self.crew.last_mut().expect("just pushed")
            }
        };
        match c.operation.as_str() {
            "Activate" => {
                entry.activated = true;
                entry.enabled = Some(true);
            }
            "Deactivate" => entry.activated = false,
            "Pause" => entry.enabled = Some(false),
            "Resume" => entry.enabled = Some(true),
            _ => {}
        }
        if c.crew_name.is_some() {
            entry.crew_name = c.crew_name.clone();
        }
    }

    /// Tritium burned per jump is roughly 5 t + 1 t per ~7.6 kt of mass… the game reports the
    /// range; for planning we use Spansh. Here: how many max-range jumps the tank allows (~ 90 t
    /// for a laden carrier is a common rule of thumb, so stay conservative).
    pub fn rough_jumps_on_fuel(&self) -> i64 {
        if self.fuel <= 0 { 0 } else { self.fuel / 90 }
    }
}
