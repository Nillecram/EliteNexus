//! Tourist planner: `POST /api/tourist/route` — visit a set of systems in an optimised order.
//! Result shape from the website code (not verified live).

use serde::{Deserialize, Serialize};

use crate::de;
use crate::job::{PlannerBody, Plotted, RoutePlanner};
use crate::routes::neutron::SystemJump;
use crate::routes::{RouteWaypoints, Waypoint};
use crate::util::{flag, num, require, require_name, require_positive};
use crate::{Result, Spansh};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TouristRouteRequest {
    pub source: String,
    /// Systems to visit (Spansh picks the order).
    pub destinations: Vec<String>,
    /// Fixed last stop (optional).
    pub final_destination: Option<String>,
    /// Jump range, ly.
    pub range: f64,
    /// Return to the source at the end (website default).
    pub loop_route: bool,
}

impl Default for TouristRouteRequest {
    fn default() -> Self {
        Self {
            source: String::new(),
            destinations: Vec::new(),
            final_destination: None,
            range: 0.0,
            loop_route: true,
        }
    }
}

impl TouristRouteRequest {
    pub fn new<I, S>(source: impl Into<String>, destinations: I, range: f64) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            source: source.into(),
            destinations: destinations.into_iter().map(Into::into).collect(),
            range,
            ..Self::default()
        }
    }

    /// End at `system` instead of looping back.
    pub fn finish_at(mut self, system: impl Into<String>) -> Self {
        self.final_destination = Some(system.into());
        self.loop_route = false;
        self
    }

    pub fn loop_route(mut self, on: bool) -> Self {
        self.loop_route = on;
        self
    }
}

impl RoutePlanner for TouristRouteRequest {
    type Route = TouristRoute;

    fn endpoint(&self) -> &'static str {
        "tourist/route"
    }

    fn web_path(&self) -> &'static str {
        "tourist"
    }

    fn body(&self) -> PlannerBody {
        let mut form = vec![
            ("range", num(self.range)),
            ("source", self.source.trim().to_owned()),
        ];
        form.extend(
            self.destinations
                .iter()
                .map(|d| ("destination", d.trim().to_owned())),
        );
        if let Some(last) = self
            .final_destination
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            form.push(("final_destination", last.to_owned()));
        }
        form.push(("loop", flag(self.loop_route)));
        PlannerBody::Form(form)
    }

    fn validate(&self) -> Result<()> {
        require_name(&self.source, "source")?;
        require_positive(self.range, "range")?;
        require(
            !self.destinations.is_empty(),
            "at least one destination is required",
        )?;
        require(
            self.destinations.iter().all(|d| !d.trim().is_empty()),
            "destinations must not be empty",
        )
    }
}

/// `result` of a tourist job.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TouristRoute {
    #[serde(deserialize_with = "de::opt_string")]
    pub job: Option<String>,
    #[serde(deserialize_with = "de::string")]
    pub source_system: String,
    #[serde(deserialize_with = "de::opt_string")]
    pub destination_system: Option<String>,
    #[serde(deserialize_with = "de::opt_f64")]
    pub range: Option<f64>,
    #[serde(deserialize_with = "de::opt_f64")]
    pub distance: Option<f64>,
    #[serde(deserialize_with = "de::opt_u32")]
    pub total_jumps: Option<u32>,
    #[serde(deserialize_with = "de::nullable")]
    pub system_jumps: Vec<SystemJump>,
}

impl RouteWaypoints for TouristRoute {
    fn waypoints(&self) -> Vec<Waypoint> {
        self.system_jumps.iter().map(SystemJump::waypoint).collect()
    }
}

impl Spansh {
    /// Tourist planner (`POST /api/tourist/route`).
    pub async fn tourist_route(&self, req: &TouristRouteRequest) -> Result<Plotted<TouristRoute>> {
        self.plot(req).await
    }
}
