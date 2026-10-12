//! Trade planner: `POST /api/trade/route`. Result shape from website/EDDiscovery code (not
//! verified live).

use serde::{Deserialize, Serialize};

use crate::de;
use crate::job::{PlannerBody, Plotted, RoutePlanner};
use crate::routes::{RouteWaypoints, Waypoint, thousands};
use crate::util::{Coords, flag, num, require, require_name, require_positive};
use crate::{Result, Spansh};

/// Website defaults: 5 hops of ≤ 50 ly, 1 000 cr, 7 t, stations within 10 000 000 ls.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TradeRouteRequest {
    pub system: String,
    pub station: String,
    pub max_hops: u32,
    /// Per hop, ly (may exceed the jump range).
    pub max_hop_distance: f64,
    pub starting_capital: u64,
    /// Cargo capacity, t.
    pub max_cargo: u32,
    /// Maximum station distance to arrival, ls.
    pub max_system_distance: u64,
    /// Ignore prices older than this many seconds.
    pub max_price_age_secs: Option<u64>,
    pub requires_large_pad: bool,
    pub allow_prohibited: bool,
    pub allow_planetary: bool,
    /// Fleet carriers.
    pub allow_player_owned: bool,
    pub allow_restricted_access: bool,
    /// Avoid loops (visit each station once).
    pub unique: bool,
    /// Allow permit-locked systems.
    pub permit: bool,
}

impl Default for TradeRouteRequest {
    fn default() -> Self {
        Self {
            system: String::new(),
            station: String::new(),
            max_hops: 5,
            max_hop_distance: 50.0,
            starting_capital: 1_000,
            max_cargo: 7,
            max_system_distance: 10_000_000,
            max_price_age_secs: None,
            requires_large_pad: false,
            allow_prohibited: false,
            allow_planetary: false,
            allow_player_owned: false,
            allow_restricted_access: false,
            unique: false,
            permit: false,
        }
    }
}

impl TradeRouteRequest {
    pub fn new(
        system: impl Into<String>,
        station: impl Into<String>,
        max_cargo: u32,
        starting_capital: u64,
    ) -> Self {
        Self {
            system: system.into(),
            station: station.into(),
            max_cargo,
            starting_capital,
            ..Self::default()
        }
    }

    pub fn hops(mut self, max_hops: u32, max_hop_distance: f64) -> Self {
        self.max_hops = max_hops;
        self.max_hop_distance = max_hop_distance;
        self
    }

    pub fn max_price_age(mut self, secs: u64) -> Self {
        self.max_price_age_secs = Some(secs);
        self
    }

    pub fn large_pad(mut self, required: bool) -> Self {
        self.requires_large_pad = required;
        self
    }
}

impl RoutePlanner for TradeRouteRequest {
    type Route = TradeRoute;

    fn endpoint(&self) -> &'static str {
        "trade/route"
    }

    fn web_path(&self) -> &'static str {
        "trade"
    }

    fn body(&self) -> PlannerBody {
        let mut form = vec![
            ("max_hops", self.max_hops.to_string()),
            ("max_hop_distance", num(self.max_hop_distance)),
            ("system", self.system.trim().to_owned()),
            ("station", self.station.trim().to_owned()),
            ("starting_capital", self.starting_capital.to_string()),
            ("max_cargo", self.max_cargo.to_string()),
            ("max_system_distance", self.max_system_distance.to_string()),
        ];
        if let Some(age) = self.max_price_age_secs {
            form.push(("max_price_age", age.to_string()));
        }
        form.extend([
            ("requires_large_pad", flag(self.requires_large_pad)),
            ("allow_prohibited", flag(self.allow_prohibited)),
            ("allow_planetary", flag(self.allow_planetary)),
            ("allow_player_owned", flag(self.allow_player_owned)),
            (
                "allow_restricted_access",
                flag(self.allow_restricted_access),
            ),
            ("unique", flag(self.unique)),
            ("permit", flag(self.permit)),
        ]);
        PlannerBody::Form(form)
    }

    fn validate(&self) -> Result<()> {
        require_name(&self.system, "system")?;
        require_name(&self.station, "station")?;
        require_positive(self.max_hop_distance, "max_hop_distance")?;
        require(
            self.max_hops > 0 && self.max_cargo > 0,
            "max_hops and max_cargo must be positive",
        )
    }
}

/// Trade result: one entry per hop.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TradeRoute {
    pub hops: Vec<TradeHop>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TradeHop {
    pub source: TradeStop,
    pub destination: TradeStop,
    /// ly.
    #[serde(deserialize_with = "de::f64_or_zero")]
    pub distance: f64,
    #[serde(deserialize_with = "de::nullable")]
    pub commodities: Vec<TradeCommodity>,
    /// Profit of this hop, cr.
    #[serde(deserialize_with = "de::i64_or_zero")]
    pub total_profit: i64,
    /// Profit so far including this hop, cr.
    #[serde(deserialize_with = "de::i64_or_zero")]
    pub cumulative_profit: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TradeStop {
    #[serde(deserialize_with = "de::string")]
    pub system: String,
    #[serde(deserialize_with = "de::string")]
    pub station: String,
    #[serde(deserialize_with = "de::opt_u64")]
    pub market_id: Option<u64>,
    #[serde(deserialize_with = "de::opt_u64")]
    pub system_id64: Option<u64>,
    #[serde(deserialize_with = "de::opt_f64")]
    pub distance_to_arrival: Option<f64>,
    #[serde(deserialize_with = "de::opt_string")]
    pub market_updated_at: Option<String>,
    #[serde(deserialize_with = "de::opt_f64")]
    pub x: Option<f64>,
    #[serde(deserialize_with = "de::opt_f64")]
    pub y: Option<f64>,
    #[serde(deserialize_with = "de::opt_f64")]
    pub z: Option<f64>,
}

impl TradeStop {
    pub fn coords(&self) -> Option<Coords> {
        Coords::from_parts(self.x, self.y, self.z)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TradeCommodity {
    #[serde(deserialize_with = "de::string")]
    pub name: String,
    /// t to buy.
    #[serde(deserialize_with = "de::u32_or_zero")]
    pub amount: u32,
    /// Per tonne, cr.
    #[serde(deserialize_with = "de::i64_or_zero")]
    pub profit: i64,
    #[serde(deserialize_with = "de::i64_or_zero")]
    pub total_profit: i64,
    pub source_commodity: TradePrice,
    pub destination_commodity: TradePrice,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TradePrice {
    #[serde(deserialize_with = "de::opt_u64")]
    pub buy_price: Option<u64>,
    #[serde(deserialize_with = "de::opt_u64")]
    pub sell_price: Option<u64>,
    #[serde(deserialize_with = "de::opt_u64")]
    pub supply: Option<u64>,
    #[serde(deserialize_with = "de::opt_u64")]
    pub demand: Option<u64>,
}

impl TradeRoute {
    /// Profit of the whole route, cr.
    pub fn total_profit(&self) -> i64 {
        match self.hops.last() {
            Some(last) if last.cumulative_profit != 0 => last.cumulative_profit,
            _ => self.hops.iter().map(|h| h.total_profit).sum(),
        }
    }
}

impl RouteWaypoints for TradeRoute {
    fn waypoints(&self) -> Vec<Waypoint> {
        let stop = |s: &TradeStop| Waypoint {
            system: s.system.clone(),
            id64: s.system_id64,
            coords: s.coords(),
            bodies: vec![s.station.clone()],
            ..Waypoint::default()
        };
        let mut out = Vec::with_capacity(self.hops.len() + 1);
        if let Some(first) = self.hops.first() {
            let mut start = stop(&first.source);
            let cargo: Vec<String> = first
                .commodities
                .iter()
                .map(|c| format!("buy {} ×{}", c.name, c.amount))
                .collect();
            start.note = (!cargo.is_empty()).then(|| cargo.join(", "));
            out.push(start);
        }
        for (i, hop) in self.hops.iter().enumerate() {
            let mut w = stop(&hop.destination);
            w.jumps = 1;
            w.distance = Some(hop.distance);
            w.value = u64::try_from(hop.total_profit).ok();
            let mut notes = vec![format!(
                "sell for {} cr",
                thousands(u64::try_from(hop.total_profit).unwrap_or(0))
            )];
            if let Some(next) = self.hops.get(i + 1) {
                notes.extend(
                    next.commodities
                        .iter()
                        .map(|c| format!("buy {} ×{}", c.name, c.amount)),
                );
            }
            w.note = Some(notes.join(", "));
            out.push(w);
        }
        out
    }
}

impl Spansh {
    /// Trade planner (`POST /api/trade/route`).
    pub async fn trade_route(&self, req: &TradeRouteRequest) -> Result<Plotted<TradeRoute>> {
        self.plot(req).await
    }
}
