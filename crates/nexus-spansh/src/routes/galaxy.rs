//! Galaxy plotter ("exact plotter"): `POST /api/generic/route`. Plans every jump for a specific
//! ship, including fuel, scooping, neutron/white-dwarf boosts and FSD injections.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::de;
use crate::job::{PlannerBody, Plotted, RoutePlanner};
use crate::routes::{RouteWaypoints, SystemRef, Waypoint};
use crate::ship::ShipProfile;
use crate::util::{Coords, flag, num, require, require_positive};
use crate::{Result, Spansh};

/// Routing strategy (website help text, paraphrased).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GalaxyAlgorithm {
    /// Favours neutron jumps, penalises sparse regions; usually the fastest route (default).
    #[default]
    Optimistic,
    /// Over-estimates star spacing: faster to compute, possibly less optimal.
    Pessimistic,
    /// Follows a neutron-plotter route as a guide (penalises > 100 ly off it).
    Guided,
    /// Minimum fuel: no scooping, no supercharging.
    Fuel,
    /// Like `Fuel`, then cuts the jump count using the whole tank.
    FuelJumps,
}

impl GalaxyAlgorithm {
    pub fn as_str(self) -> &'static str {
        match self {
            GalaxyAlgorithm::Optimistic => "optimistic",
            GalaxyAlgorithm::Pessimistic => "pessimistic",
            GalaxyAlgorithm::Guided => "guided",
            GalaxyAlgorithm::Fuel => "fuel",
            GalaxyAlgorithm::FuelJumps => "fuel_jumps",
        }
    }
}

/// Exact-plotter request. Build it with [`GalaxyRouteRequest::new`] from a [`ShipProfile`]
/// (journal `Loadout`), or fill the FSD fields yourself.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GalaxyRouteRequest {
    pub source: SystemRef,
    pub destination: SystemRef,

    // --- ship / FSD
    pub optimal_mass: f64,
    pub max_fuel_per_jump: f64,
    pub fuel_multiplier: f64,
    pub fuel_power: f64,
    /// Ship mass without fuel and cargo, plus the reservoir (`UnladenMass + Reserve`).
    pub base_mass: f64,
    /// Main tank, t.
    pub tank_size: f64,
    /// Reservoir, t.
    pub internal_tank_size: f64,
    /// Guardian FSD booster bonus, ly.
    pub range_boost: Option<f64>,
    /// 4, or 6 for SCO Mk II drives.
    pub supercharge_multiplier: u8,
    /// FSD injection boost multiplier (2).
    pub injection_multiplier: f64,
    /// SLEF build JSON, sent as `ship_build` like the website and EDDiscovery do.
    pub ship_build: Option<String>,

    // --- trip
    /// Cargo carried, t.
    pub cargo: u32,
    /// Fuel the router must not use, t.
    pub reserve_size: f64,
    /// Already supercharged at the start.
    pub is_supercharged: bool,
    /// Use neutron / white-dwarf boosts.
    pub use_supercharge: bool,
    /// Use FSD injections whenever no neutron is available.
    pub use_injections: bool,
    /// Use injections only when there is no other way.
    pub use_injections_when_required: bool,
    /// Do not use secondary neutron / scoopable stars.
    pub exclude_secondary: bool,
    /// Top up at every scoopable star.
    pub refuel_every_scoopable: bool,
    pub algorithm: GalaxyAlgorithm,
    /// Server calculation budget, s (website: 60–120).
    pub max_time: u32,
}

impl Default for GalaxyRouteRequest {
    fn default() -> Self {
        Self {
            source: SystemRef::default(),
            destination: SystemRef::default(),
            optimal_mass: 0.0,
            max_fuel_per_jump: 0.0,
            fuel_multiplier: 0.0,
            fuel_power: 0.0,
            base_mass: 0.0,
            tank_size: 0.0,
            internal_tank_size: 0.0,
            range_boost: None,
            supercharge_multiplier: 4,
            injection_multiplier: 2.0,
            ship_build: None,
            cargo: 0,
            reserve_size: 0.0,
            is_supercharged: false,
            use_supercharge: true,
            use_injections: false,
            use_injections_when_required: false,
            exclude_secondary: false,
            refuel_every_scoopable: true,
            algorithm: GalaxyAlgorithm::Optimistic,
            max_time: 60,
        }
    }
}

impl GalaxyRouteRequest {
    pub fn new(
        source: impl Into<SystemRef>,
        destination: impl Into<SystemRef>,
        ship: &ShipProfile,
    ) -> Self {
        Self {
            source: source.into(),
            destination: destination.into(),
            ..Self::default()
        }
        .ship(ship)
    }

    /// Copy FSD, tank and mass parameters (and the SLEF build) from a ship profile.
    pub fn ship(mut self, ship: &ShipProfile) -> Self {
        self.optimal_mass = ship.fsd.optimal_mass;
        self.max_fuel_per_jump = ship.fsd.max_fuel_per_jump;
        self.fuel_multiplier = ship.fsd.fuel_multiplier;
        self.fuel_power = ship.fsd.fuel_power;
        self.supercharge_multiplier = ship.fsd.supercharge_multiplier;
        self.base_mass = ship.base_mass();
        self.tank_size = ship.tank_size;
        self.internal_tank_size = ship.reservoir_size;
        self.range_boost = ship.range_boost;
        self.ship_build = ship.slef.clone();
        self
    }

    pub fn cargo(mut self, tonnes: u32) -> Self {
        self.cargo = tonnes;
        self
    }

    pub fn algorithm(mut self, algorithm: GalaxyAlgorithm) -> Self {
        self.algorithm = algorithm;
        self
    }

    pub fn max_time(mut self, seconds: u32) -> Self {
        self.max_time = seconds;
        self
    }

    pub fn use_injections(mut self, always: bool, when_required: bool) -> Self {
        self.use_injections = always;
        self.use_injections_when_required = when_required;
        self
    }

    pub fn use_supercharge(mut self, on: bool) -> Self {
        self.use_supercharge = on;
        self
    }

    pub fn already_supercharged(mut self, on: bool) -> Self {
        self.is_supercharged = on;
        self
    }

    pub fn refuel_every_scoopable(mut self, on: bool) -> Self {
        self.refuel_every_scoopable = on;
        self
    }

    pub fn exclude_secondary(mut self, on: bool) -> Self {
        self.exclude_secondary = on;
        self
    }

    pub fn reserve(mut self, tonnes: f64) -> Self {
        self.reserve_size = tonnes;
        self
    }
}

impl RoutePlanner for GalaxyRouteRequest {
    type Route = GalaxyRoute;

    fn endpoint(&self) -> &'static str {
        "generic/route"
    }

    fn web_path(&self) -> &'static str {
        "exact-plotter"
    }

    fn body(&self) -> PlannerBody {
        let mut form = vec![
            ("source", self.source.to_string()),
            ("destination", self.destination.to_string()),
            ("is_supercharged", flag(self.is_supercharged)),
            ("use_supercharge", flag(self.use_supercharge)),
            ("use_injections", flag(self.use_injections)),
            (
                "use_injections_when_required",
                flag(self.use_injections_when_required),
            ),
            ("exclude_secondary", flag(self.exclude_secondary)),
            ("refuel_every_scoopable", flag(self.refuel_every_scoopable)),
            ("fuel_power", num(self.fuel_power)),
            ("fuel_multiplier", num(self.fuel_multiplier)),
            ("optimal_mass", num(self.optimal_mass)),
            ("base_mass", num(self.base_mass)),
            ("tank_size", num(self.tank_size)),
            ("internal_tank_size", num(self.internal_tank_size)),
            ("max_fuel_per_jump", num(self.max_fuel_per_jump)),
            ("reserve_size", num(self.reserve_size)),
            ("cargo", self.cargo.to_string()),
            ("max_time", self.max_time.to_string()),
            ("algorithm", self.algorithm.as_str().to_owned()),
            (
                "supercharge_multiplier",
                self.supercharge_multiplier.to_string(),
            ),
            ("injection_multiplier", num(self.injection_multiplier)),
        ];
        if let Some(boost) = self.range_boost.filter(|b| *b > 0.0) {
            form.push(("range_boost", num(boost)));
        }
        if let Some(build) = &self.ship_build {
            form.push(("ship_build", build.clone()));
        }
        PlannerBody::Form(form)
    }

    fn validate(&self) -> Result<()> {
        require(!self.source.is_blank(), "source must not be empty")?;
        require(
            !self.destination.is_blank(),
            "destination must not be empty",
        )?;
        require_positive(self.optimal_mass, "optimal_mass")?;
        require_positive(self.max_fuel_per_jump, "max_fuel_per_jump")?;
        require_positive(self.fuel_multiplier, "fuel_multiplier")?;
        require_positive(self.fuel_power, "fuel_power")?;
        require_positive(self.base_mass, "base_mass")?;
        require_positive(self.tank_size, "tank_size")?;
        require(
            self.internal_tank_size >= 0.0 && self.reserve_size >= 0.0,
            "tank sizes must not be negative",
        )?;
        require(
            (1..=600).contains(&self.max_time),
            "max_time must be 1-600 s",
        )
    }

    fn timeout(&self) -> Duration {
        Duration::from_secs(u64::from(self.max_time) + 60)
    }

    /// Observed live: the plotter keeps optimising for its whole `max_time` (a 582 ly route
    /// with `max_time` 60 finished after 61 s).
    fn expected_duration(&self) -> Option<Duration> {
        Some(Duration::from_secs(u64::from(self.max_time)))
    }
}

/// `result` of an exact-plotter job.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GalaxyRoute {
    #[serde(deserialize_with = "de::opt_string")]
    pub job: Option<String>,
    #[serde(deserialize_with = "de::opt_string")]
    pub source_system: Option<String>,
    #[serde(deserialize_with = "de::opt_string")]
    pub destination_system: Option<String>,
    /// The planner ran out of time or options; `jumps` ends early.
    #[serde(deserialize_with = "de::bool_or_false")]
    pub incomplete: bool,
    #[serde(deserialize_with = "de::name_list")]
    pub errors: Vec<String>,
    /// Every system to jump to, starting with the source.
    #[serde(deserialize_with = "de::nullable")]
    pub jumps: Vec<GalaxyJump>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GalaxyJump {
    #[serde(deserialize_with = "de::string")]
    pub name: String,
    #[serde(deserialize_with = "de::opt_u64")]
    pub id64: Option<u64>,
    #[serde(deserialize_with = "de::opt_f64")]
    pub x: Option<f64>,
    #[serde(deserialize_with = "de::opt_f64")]
    pub y: Option<f64>,
    #[serde(deserialize_with = "de::opt_f64")]
    pub z: Option<f64>,
    /// Length of the jump into this system, ly.
    #[serde(deserialize_with = "de::f64_or_zero")]
    pub distance: f64,
    #[serde(deserialize_with = "de::f64_or_zero")]
    pub distance_to_destination: f64,
    /// Fuel on arrival, t.
    #[serde(deserialize_with = "de::f64_or_zero")]
    pub fuel_in_tank: f64,
    /// Fuel used by the jump into this system, t.
    #[serde(deserialize_with = "de::f64_or_zero")]
    pub fuel_used: f64,
    /// Scoop here.
    #[serde(deserialize_with = "de::bool_or_false")]
    pub must_refuel: bool,
    /// Neutron star here (supercharge).
    #[serde(deserialize_with = "de::bool_or_false")]
    pub has_neutron: bool,
    #[serde(deserialize_with = "de::bool_or_false")]
    pub is_scoopable: bool,
    /// Use an FSD injection for the next jump.
    #[serde(deserialize_with = "de::bool_or_false")]
    pub must_inject: bool,
}

impl GalaxyJump {
    pub fn coords(&self) -> Option<Coords> {
        Coords::from_parts(self.x, self.y, self.z)
    }
}

impl GalaxyRoute {
    /// Jumps to fly (the first row is the start system).
    pub fn jump_count(&self) -> usize {
        self.jumps.len().saturating_sub(1)
    }

    pub fn fuel_used(&self) -> f64 {
        self.jumps.iter().map(|j| j.fuel_used).sum()
    }

    pub fn distance(&self) -> f64 {
        self.jumps.iter().map(|j| j.distance).sum()
    }

    pub fn refuel_stops(&self) -> usize {
        self.jumps.iter().filter(|j| j.must_refuel).count()
    }

    pub fn neutron_boosts(&self) -> usize {
        self.jumps.iter().filter(|j| j.has_neutron).count()
    }

    pub fn injections(&self) -> usize {
        self.jumps.iter().filter(|j| j.must_inject).count()
    }
}

impl RouteWaypoints for GalaxyRoute {
    fn waypoints(&self) -> Vec<Waypoint> {
        self.jumps
            .iter()
            .enumerate()
            .map(|(i, j)| Waypoint {
                system: j.name.clone(),
                id64: j.id64,
                coords: j.coords(),
                jumps: u32::from(i > 0),
                distance: Some(j.distance),
                distance_remaining: Some(j.distance_to_destination),
                neutron: j.has_neutron,
                scoopable: Some(j.is_scoopable),
                refuel: j.must_refuel,
                inject: j.must_inject,
                fuel_in_tank: Some(j.fuel_in_tank),
                fuel_used: Some(j.fuel_used),
                ..Waypoint::default()
            })
            .collect()
    }
}

impl Spansh {
    /// Galaxy / exact plotter (`POST /api/generic/route`).
    pub async fn galaxy_route(&self, req: &GalaxyRouteRequest) -> Result<Plotted<GalaxyRoute>> {
        self.plot(req).await
    }
}
