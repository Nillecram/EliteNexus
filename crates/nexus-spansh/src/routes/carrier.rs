//! Fleet carrier router: `POST /api/fleetcarrier/route`.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::de;
use crate::job::{PlannerBody, Plotted, RoutePlanner};
use crate::routes::{RouteWaypoints, SystemRef, Waypoint, thousands};
use crate::util::{Coords, flag, require};
use crate::{Result, Spansh};

/// How the router treats tritium.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "mode")]
pub enum CarrierFuel {
    /// Work out the tritium needed (website default), optionally refuelling at some of the
    /// destinations.
    Calculate { refuel_destinations: Vec<SystemRef> },
    /// Plan with what is aboard: tritium in the tank and in the carrier market, t.
    Loaded {
        fuel_loaded: u32,
        tritium_stored: u32,
    },
}

impl Default for CarrierFuel {
    fn default() -> Self {
        CarrierFuel::Calculate {
            refuel_destinations: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CarrierRouteRequest {
    pub source: SystemRef,
    /// Ordered waypoints; the last one is the final destination.
    pub destinations: Vec<SystemRef>,
    /// 25 000 for a fleet carrier, 60 000 for a squadron carrier.
    pub capacity: u32,
    /// 25 000 for a fleet carrier, 15 000 for a squadron carrier.
    pub mass: u32,
    /// Market + services capacity in use, t (carrier management screen).
    pub capacity_used: u32,
    pub fuel: CarrierFuel,
}

impl Default for CarrierRouteRequest {
    fn default() -> Self {
        Self {
            source: SystemRef::default(),
            destinations: Vec::new(),
            capacity: 25_000,
            mass: 25_000,
            capacity_used: 0,
            fuel: CarrierFuel::default(),
        }
    }
}

impl CarrierRouteRequest {
    /// A fleet (player) carrier route.
    pub fn new<I, S>(source: impl Into<SystemRef>, destinations: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<SystemRef>,
    {
        Self {
            source: source.into(),
            destinations: destinations.into_iter().map(Into::into).collect(),
            ..Self::default()
        }
    }

    /// A squadron carrier route.
    pub fn squadron<I, S>(source: impl Into<SystemRef>, destinations: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<SystemRef>,
    {
        Self {
            capacity: 60_000,
            mass: 15_000,
            ..Self::new(source, destinations)
        }
    }

    pub fn capacity_used(mut self, tonnes: u32) -> Self {
        self.capacity_used = tonnes;
        self
    }

    /// Plan with the tritium aboard instead of calculating the requirement.
    pub fn fuel_loaded(mut self, tank: u32, market: u32) -> Self {
        self.fuel = CarrierFuel::Loaded {
            fuel_loaded: tank,
            tritium_stored: market,
        };
        self
    }

    /// Calculate the requirement, refuelling at these destinations.
    pub fn refuel_at<I, S>(mut self, systems: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<SystemRef>,
    {
        self.fuel = CarrierFuel::Calculate {
            refuel_destinations: systems.into_iter().map(Into::into).collect(),
        };
        self
    }
}

impl RoutePlanner for CarrierRouteRequest {
    type Route = CarrierRoute;

    fn endpoint(&self) -> &'static str {
        "fleetcarrier/route"
    }

    fn web_path(&self) -> &'static str {
        "fleet-carrier"
    }

    fn body(&self) -> PlannerBody {
        let mut form = vec![("source", self.source.to_string())];
        form.extend(
            self.destinations
                .iter()
                .map(|d| ("destinations", d.to_string())),
        );
        form.push(("capacity", self.capacity.to_string()));
        form.push(("mass", self.mass.to_string()));
        form.push(("capacity_used", self.capacity_used.to_string()));
        match &self.fuel {
            CarrierFuel::Calculate {
                refuel_destinations,
            } => {
                form.push(("calculate_starting_fuel", flag(true)));
                form.extend(
                    refuel_destinations
                        .iter()
                        .map(|d| ("refuel_destinations", d.to_string())),
                );
            }
            CarrierFuel::Loaded {
                fuel_loaded,
                tritium_stored,
            } => {
                form.push(("calculate_starting_fuel", flag(false)));
                form.push(("fuel_loaded", fuel_loaded.to_string()));
                form.push(("tritium_stored", tritium_stored.to_string()));
            }
        }
        PlannerBody::Form(form)
    }

    fn validate(&self) -> Result<()> {
        require(!self.source.is_blank(), "source must not be empty")?;
        require(
            !self.destinations.is_empty(),
            "at least one destination is required",
        )?;
        require(
            self.destinations.iter().all(|d| !d.is_blank()),
            "destinations must not be empty",
        )?;
        require(
            self.capacity > 0 && self.mass > 0,
            "capacity and mass must be positive",
        )?;
        require(
            self.capacity_used <= self.capacity,
            "capacity_used exceeds capacity",
        )?;
        if let CarrierFuel::Loaded { fuel_loaded, .. } = &self.fuel {
            require(*fuel_loaded <= 1000, "a carrier tank holds at most 1000 t")?;
        }
        Ok(())
    }
}

/// `result` of a carrier job (shape verified live).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CarrierRoute {
    /// Rows in order, starting with the source. Every intermediate requested destination
    /// appears twice: the arrival row and a 0 ly "departure" row whose
    /// `distance_to_destination` points at the next destination. [`RouteWaypoints::waypoints`]
    /// merges those.
    #[serde(deserialize_with = "de::nullable")]
    pub jumps: Vec<CarrierJump>,
    // Echo of the request.
    #[serde(deserialize_with = "de::opt_string")]
    pub source: Option<String>,
    #[serde(deserialize_with = "de::name_list")]
    pub destinations: Vec<String>,
    #[serde(deserialize_with = "de::name_list")]
    pub refuel_destinations: Vec<String>,
    #[serde(deserialize_with = "de::opt_u32")]
    pub capacity: Option<u32>,
    #[serde(deserialize_with = "de::opt_u32")]
    pub capacity_used: Option<u32>,
    #[serde(deserialize_with = "de::opt_u32")]
    pub mass: Option<u32>,
    #[serde(deserialize_with = "de::opt_bool")]
    pub calculate_starting_fuel: Option<bool>,
    #[serde(deserialize_with = "de::opt_u32")]
    pub fuel_loaded: Option<u32>,
    #[serde(deserialize_with = "de::opt_u32")]
    pub tritium_stored: Option<u32>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CarrierJump {
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
    /// Length of the jump into this system, ly (0 for the start and "departure" rows).
    #[serde(deserialize_with = "de::f64_or_zero")]
    pub distance: f64,
    /// To the next requested destination, ly.
    #[serde(deserialize_with = "de::f64_or_zero")]
    pub distance_to_destination: f64,
    /// Tritium in the tank on arrival, t.
    #[serde(deserialize_with = "de::u32_or_zero")]
    pub fuel_in_tank: u32,
    /// Tritium burnt by the jump into this system, t.
    #[serde(deserialize_with = "de::u32_or_zero")]
    pub fuel_used: u32,
    /// Tritium left in the carrier market, t.
    #[serde(deserialize_with = "de::u32_or_zero")]
    pub tritium_in_market: u32,
    /// Somewhere to mine tritium.
    #[serde(deserialize_with = "de::bool_or_false")]
    pub has_icy_ring: bool,
    #[serde(deserialize_with = "de::bool_or_false")]
    pub is_system_pristine: bool,
    /// Move tritium from the market into the tank here.
    #[serde(deserialize_with = "de::bool_or_false")]
    pub must_restock: bool,
    #[serde(deserialize_with = "de::u32_or_zero")]
    pub restock_amount: u32,
    /// The source or one of the requested destinations (not an intermediate hop).
    #[serde(deserialize_with = "de::bool_or_false")]
    pub is_desired_destination: bool,
}

impl CarrierJump {
    pub fn coords(&self) -> Option<Coords> {
        Coords::from_parts(self.x, self.y, self.z)
    }

    fn same_system(&self, other: &CarrierJump) -> bool {
        match (self.id64, other.id64) {
            (Some(a), Some(b)) => a == b,
            _ => self.name.eq_ignore_ascii_case(&other.name),
        }
    }
}

impl CarrierRoute {
    /// Carrier jumps actually flown (rows with a distance; the start and the "departure" rows
    /// of intermediate destinations are 0 ly).
    pub fn jump_count(&self) -> usize {
        self.jumps.iter().filter(|j| j.distance > 0.0).count()
    }

    /// Tritium burnt over the route, t.
    pub fn tritium_used(&self) -> u32 {
        self.jumps.iter().map(|j| j.fuel_used).sum()
    }

    pub fn distance(&self) -> f64 {
        self.jumps.iter().map(|j| j.distance).sum()
    }

    /// Rough wall time at the website's 20 minutes per jump.
    pub fn estimated_duration(&self) -> Duration {
        Duration::from_secs(20 * 60 * self.jump_count() as u64)
    }
}

impl RouteWaypoints for CarrierRoute {
    /// One waypoint per system visited; a destination's 0 ly "departure" row is folded into its
    /// arrival (restock instructions are kept).
    fn waypoints(&self) -> Vec<Waypoint> {
        let mut out: Vec<Waypoint> = Vec::with_capacity(self.jumps.len());
        let mut prev: Option<&CarrierJump> = None;
        for j in &self.jumps {
            let restock = j.must_restock.then(|| {
                format!(
                    "restock {} t tritium",
                    thousands(u64::from(j.restock_amount))
                )
            });
            if let (Some(p), Some(last)) = (prev, out.last_mut())
                && j.distance <= 0.0
                && j.same_system(p)
            {
                last.refuel |= j.must_restock;
                last.fuel_in_tank = Some(f64::from(j.fuel_in_tank));
                if let Some(r) = restock {
                    last.note = Some(match last.note.take() {
                        Some(n) => format!("{n}, {r}"),
                        None => r,
                    });
                }
                prev = Some(j);
                continue;
            }
            let mut notes: Vec<String> = restock.into_iter().collect();
            if j.has_icy_ring {
                notes.push(if j.is_system_pristine {
                    "pristine icy ring".to_owned()
                } else {
                    "icy ring".to_owned()
                });
            }
            out.push(Waypoint {
                system: j.name.clone(),
                id64: j.id64,
                coords: j.coords(),
                jumps: u32::from(j.distance > 0.0),
                distance: Some(j.distance),
                distance_remaining: Some(j.distance_to_destination),
                refuel: j.must_restock,
                fuel_in_tank: Some(f64::from(j.fuel_in_tank)),
                fuel_used: Some(f64::from(j.fuel_used)),
                note: (!notes.is_empty()).then(|| notes.join(", ")),
                ..Waypoint::default()
            });
            prev = Some(j);
        }
        out
    }
}

impl Spansh {
    /// Fleet carrier router (`POST /api/fleetcarrier/route`).
    pub async fn carrier_route(&self, req: &CarrierRouteRequest) -> Result<Plotted<CarrierRoute>> {
        self.plot(req).await
    }
}
