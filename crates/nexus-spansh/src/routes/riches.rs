//! Body-scanning routes: Road to Riches and its specialised variants (ammonia, Earth-like,
//! rocky/HMC worlds) via `POST /api/riches/route`, and the exobiology "Expressway to
//! Exomastery" via `POST /api/exobiology/route`. Both return the same shape: an array of
//! systems with the bodies worth visiting. (Result shape from website/EDDiscovery code.)

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::de;
use crate::job::{PlannerBody, Plotted, RoutePlanner};
use crate::records::Landmark;
use crate::routes::{RouteWaypoints, Waypoint, thousands};
use crate::util::{Coords, Form, flag, num, require, require_name, require_positive};
use crate::{Result, Spansh};

/// Road to Riches (and specialised-world) request. Website defaults: radius 25 ly, 100 systems,
/// bodies within 50 000 ls, ≥ 100 000 cr, avoid Thargoids, loop back to the start.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RichesRouteRequest {
    pub from: String,
    /// End here instead of looping (sets `loop_route = false` via [`RichesRouteRequest::to`]).
    pub to: Option<String>,
    /// Jump range, ly.
    pub range: f64,
    /// Search radius around the route, ly (100–500 for circular tours).
    pub radius: u32,
    pub max_results: u32,
    /// Maximum distance to arrival of a body, ls.
    pub max_distance: u32,
    /// Minimum body value, cr (the specialised routes use 1).
    pub min_value: u64,
    /// Rank by mapping (DSS) value instead of FSS value.
    pub use_mapping_value: bool,
    pub avoid_thargoids: bool,
    /// Return to the start.
    pub loop_route: bool,
    /// Restrict to these body sub-types (`Ammonia world`, `Earth-like world`, …).
    pub body_types: Vec<String>,
}

impl Default for RichesRouteRequest {
    fn default() -> Self {
        Self {
            from: String::new(),
            to: None,
            range: 0.0,
            radius: 25,
            max_results: 100,
            max_distance: 50_000,
            min_value: 100_000,
            use_mapping_value: false,
            avoid_thargoids: true,
            loop_route: true,
            body_types: Vec::new(),
        }
    }
}

impl RichesRouteRequest {
    /// Classic Road to Riches.
    pub fn road_to_riches(from: impl Into<String>, range: f64) -> Self {
        Self {
            from: from.into(),
            range,
            ..Self::default()
        }
    }

    /// Only these body sub-types, any value (how the website's specialised planners work).
    pub fn body_types<I, S>(from: impl Into<String>, range: f64, types: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            min_value: 1,
            body_types: types.into_iter().map(Into::into).collect(),
            ..Self::road_to_riches(from, range)
        }
    }

    pub fn ammonia_worlds(from: impl Into<String>, range: f64) -> Self {
        Self::body_types(from, range, ["Ammonia world"])
    }

    pub fn earth_likes(from: impl Into<String>, range: f64) -> Self {
        Self::body_types(from, range, ["Earth-like world"])
    }

    pub fn rocky_hmc(from: impl Into<String>, range: f64) -> Self {
        Self::body_types(from, range, ["Rocky body", "High metal content world"])
    }

    /// End at `system` (no loop).
    pub fn to(mut self, system: impl Into<String>) -> Self {
        self.to = Some(system.into());
        self.loop_route = false;
        self
    }

    pub fn radius(mut self, ly: u32) -> Self {
        self.radius = ly;
        self
    }

    pub fn max_results(mut self, n: u32) -> Self {
        self.max_results = n;
        self
    }

    pub fn max_distance(mut self, ls: u32) -> Self {
        self.max_distance = ls;
        self
    }

    pub fn min_value(mut self, credits: u64) -> Self {
        self.min_value = credits;
        self
    }

    pub fn use_mapping_value(mut self, on: bool) -> Self {
        self.use_mapping_value = on;
        self
    }

    pub fn avoid_thargoids(mut self, on: bool) -> Self {
        self.avoid_thargoids = on;
        self
    }
}

fn common_form(
    from: &str,
    to: Option<&str>,
    range: f64,
    radius: u32,
    max_results: u32,
    max_distance: u32,
    min_value: u64,
) -> Form {
    let mut form = vec![("from", from.trim().to_owned())];
    if let Some(to) = to.map(str::trim).filter(|t| !t.is_empty()) {
        form.push(("to", to.to_owned()));
    }
    form.extend([
        ("range", num(range)),
        ("radius", radius.to_string()),
        ("max_results", max_results.to_string()),
        ("max_distance", max_distance.to_string()),
        ("min_value", min_value.to_string()),
    ]);
    form
}

impl RoutePlanner for RichesRouteRequest {
    type Route = ScanRoute;

    fn endpoint(&self) -> &'static str {
        "riches/route"
    }

    fn web_path(&self) -> &'static str {
        let has = |t: &str| self.body_types.iter().any(|b| b.eq_ignore_ascii_case(t));
        match self.body_types.len() {
            1 if has("Ammonia world") => "ammonia",
            1 if has("Earth-like world") => "earth",
            2 if has("Rocky body") && has("High metal content world") => "rocky-metal",
            _ => "riches",
        }
    }

    fn body(&self) -> PlannerBody {
        let mut form = common_form(
            &self.from,
            self.to.as_deref(),
            self.range,
            self.radius,
            self.max_results,
            self.max_distance,
            self.min_value,
        );
        form.push(("use_mapping_value", flag(self.use_mapping_value)));
        form.push(("avoid_thargoids", flag(self.avoid_thargoids)));
        form.push(("loop", flag(self.loop_route)));
        form.extend(self.body_types.iter().map(|t| ("body_types", t.clone())));
        PlannerBody::Form(form)
    }

    fn validate(&self) -> Result<()> {
        require_name(&self.from, "from")?;
        require_positive(self.range, "range")?;
        require(
            self.radius > 0 && self.max_results > 0,
            "radius and max_results must be positive",
        )
    }
}

/// Exobiology ("Expressway to Exomastery") request. Website defaults as for Road to Riches but
/// with a 10 000 000 cr minimum value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ExobiologyRouteRequest {
    pub from: String,
    pub to: Option<String>,
    pub range: f64,
    pub radius: u32,
    pub max_results: u32,
    pub max_distance: u32,
    pub min_value: u64,
    pub avoid_thargoids: bool,
    pub loop_route: bool,
}

impl Default for ExobiologyRouteRequest {
    fn default() -> Self {
        Self {
            from: String::new(),
            to: None,
            range: 0.0,
            radius: 25,
            max_results: 100,
            max_distance: 50_000,
            min_value: 10_000_000,
            avoid_thargoids: true,
            loop_route: true,
        }
    }
}

impl ExobiologyRouteRequest {
    pub fn new(from: impl Into<String>, range: f64) -> Self {
        Self {
            from: from.into(),
            range,
            ..Self::default()
        }
    }

    pub fn to(mut self, system: impl Into<String>) -> Self {
        self.to = Some(system.into());
        self.loop_route = false;
        self
    }

    pub fn radius(mut self, ly: u32) -> Self {
        self.radius = ly;
        self
    }

    pub fn max_results(mut self, n: u32) -> Self {
        self.max_results = n;
        self
    }

    pub fn max_distance(mut self, ls: u32) -> Self {
        self.max_distance = ls;
        self
    }

    pub fn min_value(mut self, credits: u64) -> Self {
        self.min_value = credits;
        self
    }
}

impl RoutePlanner for ExobiologyRouteRequest {
    type Route = ScanRoute;

    fn endpoint(&self) -> &'static str {
        "exobiology/route"
    }

    fn web_path(&self) -> &'static str {
        "exobiology"
    }

    fn body(&self) -> PlannerBody {
        let mut form = common_form(
            &self.from,
            self.to.as_deref(),
            self.range,
            self.radius,
            self.max_results,
            self.max_distance,
            self.min_value,
        );
        form.push(("avoid_thargoids", flag(self.avoid_thargoids)));
        form.push(("loop", flag(self.loop_route)));
        PlannerBody::Form(form)
    }

    fn validate(&self) -> Result<()> {
        require_name(&self.from, "from")?;
        require_positive(self.range, "range")?;
        require(
            self.radius > 0 && self.max_results > 0,
            "radius and max_results must be positive",
        )
    }
}

/// Road to Riches / specialised / exobiology result: systems in visiting order.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ScanRoute {
    pub systems: Vec<ScanSystem>,
}

/// Same shape for the Road-to-Riches family.
pub type RichesRoute = ScanRoute;
/// Same shape for the exobiology planner.
pub type ExobiologyRoute = ScanRoute;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ScanSystem {
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
    /// Estimated jumps from the previous system.
    #[serde(deserialize_with = "de::u32_or_zero")]
    pub jumps: u32,
    #[serde(deserialize_with = "de::nullable")]
    pub bodies: Vec<ScanBody>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ScanBody {
    #[serde(deserialize_with = "de::string")]
    pub name: String,
    #[serde(deserialize_with = "de::opt_u64")]
    pub id64: Option<u64>,
    #[serde(rename = "type", deserialize_with = "de::opt_string")]
    pub body_type: Option<String>,
    #[serde(deserialize_with = "de::opt_string")]
    pub subtype: Option<String>,
    #[serde(deserialize_with = "de::bool_or_false")]
    pub is_terraformable: bool,
    /// ls.
    #[serde(deserialize_with = "de::f64_or_zero")]
    pub distance_to_arrival: f64,
    #[serde(deserialize_with = "de::u64_or_zero")]
    pub estimated_scan_value: u64,
    #[serde(deserialize_with = "de::u64_or_zero")]
    pub estimated_mapping_value: u64,
    /// Exobiology: value of all landmarks (species) on the body.
    #[serde(deserialize_with = "de::u64_or_zero")]
    pub landmark_value: u64,
    /// Exobiology: species (`type` = genus, `subtype` = species); may repeat per species.
    #[serde(deserialize_with = "de::nullable")]
    pub landmarks: Vec<Landmark>,
}

/// One species on a body, with repeated landmarks summed (as the website does).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Species {
    pub genus: String,
    pub species: String,
    pub count: u32,
    /// Value of one sample set, cr.
    pub value: u64,
}

impl ScanBody {
    /// Exploration payout: the mapped value when mapping is worth more than the FSS scan.
    pub fn exploration_value(&self) -> u64 {
        self.estimated_scan_value.max(self.estimated_mapping_value)
    }

    /// Species grouped by name.
    pub fn species(&self) -> Vec<Species> {
        let mut by_name: BTreeMap<&str, Species> = BTreeMap::new();
        for lm in &self.landmarks {
            let entry = by_name
                .entry(lm.subtype.as_str())
                .or_insert_with(|| Species {
                    genus: lm.kind.clone(),
                    species: lm.subtype.clone(),
                    count: 0,
                    value: 0,
                });
            entry.count += lm.count.unwrap_or(1);
            entry.value = entry.value.max(lm.value);
        }
        by_name.into_values().collect()
    }
}

impl ScanSystem {
    pub fn coords(&self) -> Option<Coords> {
        Coords::from_parts(self.x, self.y, self.z)
    }

    /// Exploration + landmark (exobiology) value of the listed bodies, cr.
    pub fn value(&self) -> u64 {
        self.bodies
            .iter()
            .map(|b| b.exploration_value() + b.landmark_value)
            .sum()
    }
}

impl ScanRoute {
    pub fn total_value(&self) -> u64 {
        self.systems.iter().map(ScanSystem::value).sum()
    }

    pub fn body_count(&self) -> usize {
        self.systems.iter().map(|s| s.bodies.len()).sum()
    }

    pub fn total_jumps(&self) -> u32 {
        self.systems.iter().map(|s| s.jumps).sum()
    }
}

impl RouteWaypoints for ScanRoute {
    fn waypoints(&self) -> Vec<Waypoint> {
        self.systems
            .iter()
            .map(|s| {
                let species: Vec<String> = s
                    .bodies
                    .iter()
                    .flat_map(ScanBody::species)
                    .map(|sp| sp.species)
                    .collect();
                let note = if species.is_empty() {
                    (!s.bodies.is_empty())
                        .then(|| format!("{} bodies, {} cr", s.bodies.len(), thousands(s.value())))
                } else {
                    Some(species.join(", "))
                };
                Waypoint {
                    system: s.name.clone(),
                    id64: s.id64,
                    coords: s.coords(),
                    jumps: s.jumps,
                    value: Some(s.value()),
                    bodies: s.bodies.iter().map(|b| b.name.clone()).collect(),
                    note,
                    ..Waypoint::default()
                }
            })
            .collect()
    }

    fn total_jumps(&self) -> u32 {
        ScanRoute::total_jumps(self)
    }
}

impl Spansh {
    /// Road to Riches and the specialised-world planners (`POST /api/riches/route`).
    pub async fn riches_route(&self, req: &RichesRouteRequest) -> Result<Plotted<ScanRoute>> {
        self.plot(req).await
    }

    /// Exobiology planner (`POST /api/exobiology/route`).
    pub async fn exobiology_route(
        &self,
        req: &ExobiologyRouteRequest,
    ) -> Result<Plotted<ScanRoute>> {
        self.plot(req).await
    }
}
