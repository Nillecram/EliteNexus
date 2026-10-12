//! Neutron plotter: `POST /api/route` (shape verified live).

use serde::{Deserialize, Serialize};

use crate::de;
use crate::job::{PlannerBody, Plotted, RoutePlanner};
use crate::routes::{RouteWaypoints, Waypoint};
use crate::ship::ShipProfile;
use crate::util::{Coords, num, require, require_name, require_positive};
use crate::{Result, Spansh};

/// Neutron-highway route request (website defaults: efficiency 60, supercharge ×4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct NeutronRouteRequest {
    pub from: String,
    pub to: String,
    /// Laden jump range, ly.
    pub range: f64,
    /// 1–100: how closely to follow the straight line (100 never detours for neutrons).
    pub efficiency: u8,
    /// 4 = normal neutron boost; 6 = SCO Mk II ("overcharge") drives.
    pub supercharge_multiplier: u8,
    /// Ordered intermediate waypoints.
    pub via: Vec<String>,
}

impl Default for NeutronRouteRequest {
    fn default() -> Self {
        Self {
            from: String::new(),
            to: String::new(),
            range: 0.0,
            efficiency: 60,
            supercharge_multiplier: 4,
            via: Vec::new(),
        }
    }
}

impl NeutronRouteRequest {
    pub fn new(from: impl Into<String>, to: impl Into<String>, range: f64) -> Self {
        Self {
            from: from.into(),
            to: to.into(),
            range,
            ..Self::default()
        }
    }

    /// Range = laden range with a full tank and `cargo` t aboard; supercharge from the FSD.
    pub fn for_ship(
        from: impl Into<String>,
        to: impl Into<String>,
        ship: &ShipProfile,
        cargo: f64,
    ) -> Self {
        Self::new(from, to, ship.laden_range(cargo))
            .supercharge_multiplier(ship.fsd.supercharge_multiplier)
    }

    pub fn efficiency(mut self, efficiency: u8) -> Self {
        self.efficiency = efficiency;
        self
    }

    pub fn supercharge_multiplier(mut self, multiplier: u8) -> Self {
        self.supercharge_multiplier = multiplier;
        self
    }

    /// Append an intermediate waypoint.
    pub fn via(mut self, system: impl Into<String>) -> Self {
        self.via.push(system.into());
        self
    }
}

impl RoutePlanner for NeutronRouteRequest {
    type Route = NeutronRoute;

    fn endpoint(&self) -> &'static str {
        "route"
    }

    fn web_path(&self) -> &'static str {
        "plotter"
    }

    fn body(&self) -> PlannerBody {
        let mut form = vec![
            ("efficiency", self.efficiency.to_string()),
            ("range", num(self.range)),
            ("from", self.from.trim().to_owned()),
            ("to", self.to.trim().to_owned()),
            (
                "supercharge_multiplier",
                self.supercharge_multiplier.to_string(),
            ),
        ];
        form.extend(self.via.iter().map(|v| ("via", v.trim().to_owned())));
        PlannerBody::Form(form)
    }

    fn validate(&self) -> Result<()> {
        require_name(&self.from, "from")?;
        require_name(&self.to, "to")?;
        require_positive(self.range, "range")?;
        require(
            (1..=100).contains(&self.efficiency),
            "efficiency must be 1-100",
        )?;
        require(
            self.supercharge_multiplier >= 1,
            "supercharge_multiplier must be at least 1",
        )?;
        require(
            self.via.iter().all(|v| !v.trim().is_empty()),
            "via systems must not be empty",
        )
    }
}

/// `result` of a neutron job.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct NeutronRoute {
    #[serde(deserialize_with = "de::string")]
    pub job: String,
    #[serde(deserialize_with = "de::string")]
    pub source_system: String,
    #[serde(deserialize_with = "de::string")]
    pub destination_system: String,
    /// Straight-line distance, ly.
    #[serde(deserialize_with = "de::f64_or_zero")]
    pub distance: f64,
    /// Echo of the request (Spansh sends these as strings).
    #[serde(deserialize_with = "de::opt_f64")]
    pub range: Option<f64>,
    #[serde(deserialize_with = "de::opt_f64")]
    pub efficiency: Option<f64>,
    /// Spansh's figure — in practice the number of waypoints minus one; see
    /// [`NeutronRoute::estimated_jumps`] for the real jump estimate.
    #[serde(deserialize_with = "de::u32_or_zero")]
    pub total_jumps: u32,
    #[serde(deserialize_with = "de::name_list")]
    pub via: Vec<String>,
    /// Waypoints in order, starting with the source.
    #[serde(deserialize_with = "de::nullable")]
    pub system_jumps: Vec<SystemJump>,
}

/// One `system_jumps` row (neutron and tourist planners): a place to open the galaxy map and
/// plot again.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SystemJump {
    #[serde(deserialize_with = "de::string")]
    pub system: String,
    #[serde(deserialize_with = "de::opt_u64")]
    pub id64: Option<u64>,
    #[serde(deserialize_with = "de::opt_f64")]
    pub x: Option<f64>,
    #[serde(deserialize_with = "de::opt_f64")]
    pub y: Option<f64>,
    #[serde(deserialize_with = "de::opt_f64")]
    pub z: Option<f64>,
    /// From the previous waypoint, ly.
    #[serde(deserialize_with = "de::f64_or_zero")]
    pub distance_jumped: f64,
    /// Still to go, ly.
    #[serde(deserialize_with = "de::f64_or_zero")]
    pub distance_left: f64,
    /// Estimated jumps from the previous waypoint.
    #[serde(deserialize_with = "de::u32_or_zero")]
    pub jumps: u32,
    #[serde(deserialize_with = "de::bool_or_false")]
    pub neutron_star: bool,
}

impl SystemJump {
    pub fn coords(&self) -> Option<Coords> {
        Coords::from_parts(self.x, self.y, self.z)
    }

    pub(crate) fn waypoint(&self) -> Waypoint {
        Waypoint {
            system: self.system.clone(),
            id64: self.id64,
            coords: self.coords(),
            jumps: self.jumps,
            distance: Some(self.distance_jumped),
            distance_remaining: Some(self.distance_left),
            neutron: self.neutron_star,
            ..Waypoint::default()
        }
    }
}

impl NeutronRoute {
    /// Σ estimated jumps over all legs (the realistic total).
    pub fn estimated_jumps(&self) -> u32 {
        self.system_jumps.iter().map(|j| j.jumps).sum()
    }

    /// Waypoints that are neutron stars.
    pub fn neutron_count(&self) -> usize {
        self.system_jumps.iter().filter(|j| j.neutron_star).count()
    }
}

impl RouteWaypoints for NeutronRoute {
    fn waypoints(&self) -> Vec<Waypoint> {
        self.system_jumps.iter().map(SystemJump::waypoint).collect()
    }

    fn total_jumps(&self) -> u32 {
        self.estimated_jumps()
    }
}

impl Spansh {
    /// Neutron plotter (`POST /api/route`).
    pub async fn neutron_route(&self, req: &NeutronRouteRequest) -> Result<Plotted<NeutronRoute>> {
        self.plot(req).await
    }
}
