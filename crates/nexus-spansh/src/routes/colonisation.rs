//! Colonisation plotter: `POST /api/colonisation/route` — a chain of colonisation-range hops to a
//! target system. Result shape from website code (not verified live).

use serde::{Deserialize, Serialize};

use crate::de;
use crate::job::{PlannerBody, Plotted, RoutePlanner};
use crate::routes::{RouteWaypoints, Waypoint};
use crate::util::{Coords, require_name};
use crate::{Result, Spansh};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ColonisationRouteRequest {
    /// Start; `None` = "shortest route to the target regardless of controlling faction".
    pub source_system: Option<String>,
    pub destination_system: String,
}

impl ColonisationRouteRequest {
    pub fn new(source_system: Option<String>, destination_system: impl Into<String>) -> Self {
        Self {
            source_system,
            destination_system: destination_system.into(),
        }
    }
}

impl RoutePlanner for ColonisationRouteRequest {
    type Route = ColonisationRoute;

    fn endpoint(&self) -> &'static str {
        "colonisation/route"
    }

    fn web_path(&self) -> &'static str {
        "colonisation"
    }

    fn body(&self) -> PlannerBody {
        PlannerBody::Form(vec![
            (
                "source_system",
                self.source_system
                    .as_deref()
                    .unwrap_or_default()
                    .trim()
                    .to_owned(),
            ),
            (
                "destination_system",
                self.destination_system.trim().to_owned(),
            ),
        ])
    }

    fn validate(&self) -> Result<()> {
        require_name(&self.destination_system, "destination_system")
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ColonisationRoute {
    #[serde(deserialize_with = "de::nullable")]
    pub jumps: Vec<ColonisationJump>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ColonisationJump {
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
    #[serde(deserialize_with = "de::f64_or_zero")]
    pub distance: f64,
    #[serde(deserialize_with = "de::f64_or_zero")]
    pub distance_to_destination: f64,
    #[serde(deserialize_with = "de::bool_or_false")]
    pub must_refuel: bool,
    #[serde(deserialize_with = "de::opt_u32")]
    pub body_count: Option<u32>,
    #[serde(deserialize_with = "de::opt_u64")]
    pub estimated_scan_value: Option<u64>,
    #[serde(deserialize_with = "de::opt_u64")]
    pub landmark_value: Option<u64>,
}

impl ColonisationJump {
    pub fn coords(&self) -> Option<Coords> {
        Coords::from_parts(self.x, self.y, self.z)
    }
}

impl RouteWaypoints for ColonisationRoute {
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
                refuel: j.must_refuel,
                value: j.estimated_scan_value,
                note: j.body_count.map(|n| format!("{n} bodies")),
                ..Waypoint::default()
            })
            .collect()
    }
}

impl Spansh {
    /// Colonisation plotter (`POST /api/colonisation/route`).
    pub async fn colonisation_route(
        &self,
        req: &ColonisationRouteRequest,
    ) -> Result<Plotted<ColonisationRoute>> {
        self.plot(req).await
    }
}
