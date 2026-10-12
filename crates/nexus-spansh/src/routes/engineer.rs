//! Engineer plotter: `POST /api/engineer/route` with a JSON body — the shortest tour through the
//! engineers that can apply a set of blueprints. Request from website code; the result shape is
//! unverified, so blueprint details are kept as raw JSON.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::de;
use crate::job::{PlannerBody, Plotted, RoutePlanner};
use crate::routes::{RouteWaypoints, Waypoint};
use crate::util::{Coords, require, require_name};
use crate::{Result, Spansh};

/// One blueprint to apply (names as in the `/api/modules`, `/api/blueprints`, `/api/effects`
/// vocabularies, e.g. slot `Frame Shift Drive`, name `Increased Range`, effect `Mass Manager`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct EngineerBlueprint {
    pub slot: String,
    pub name: String,
    pub grade: u8,
    pub effect: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct EngineerRouteRequest {
    pub source: String,
    pub blueprints: Vec<EngineerBlueprint>,
    /// Restrict to these engineers (empty = all).
    pub engineers: Vec<String>,
}

impl RoutePlanner for EngineerRouteRequest {
    type Route = EngineerRoute;

    fn endpoint(&self) -> &'static str {
        "engineer/route"
    }

    fn web_path(&self) -> &'static str {
        "engineer-plotter"
    }

    fn body(&self) -> PlannerBody {
        let blueprints: Vec<Value> = self
            .blueprints
            .iter()
            .map(|b| {
                let mut v = json!({"slot": b.slot, "name": b.name, "grade": b.grade});
                if let Some(effect) = &b.effect {
                    v["effect"] = json!(effect);
                }
                v
            })
            .collect();
        let mut body = json!({"source": self.source.trim(), "blueprints": blueprints});
        if !self.engineers.is_empty() {
            body["engineers"] = json!(self.engineers);
        }
        PlannerBody::Json(body)
    }

    fn validate(&self) -> Result<()> {
        require_name(&self.source, "source")?;
        require(
            !self.blueprints.is_empty(),
            "at least one blueprint is required",
        )
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EngineerRoute {
    #[serde(deserialize_with = "de::nullable")]
    pub jumps: Vec<EngineerStop>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EngineerStop {
    #[serde(alias = "name", deserialize_with = "de::string")]
    pub system: String,
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
    #[serde(deserialize_with = "de::opt_string")]
    pub engineer: Option<String>,
    /// Blueprints applied here (raw; the website reads `blueprints[].to.{slot,name,grade,effect}`).
    #[serde(deserialize_with = "de::nullable")]
    pub blueprints: Vec<Value>,
}

impl EngineerStop {
    pub fn coords(&self) -> Option<Coords> {
        Coords::from_parts(self.x, self.y, self.z)
    }
}

impl RouteWaypoints for EngineerRoute {
    fn waypoints(&self) -> Vec<Waypoint> {
        self.jumps
            .iter()
            .enumerate()
            .map(|(i, j)| Waypoint {
                system: j.system.clone(),
                id64: j.id64,
                coords: j.coords(),
                jumps: u32::from(i > 0),
                distance: Some(j.distance),
                note: j.engineer.clone(),
                ..Waypoint::default()
            })
            .collect()
    }
}

/// Vocabularies for the engineer plotter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EngineerVocabulary {
    Engineers,
    Modules,
    Blueprints,
    Effects,
}

impl Spansh {
    /// Engineer plotter (`POST /api/engineer/route`, JSON body).
    pub async fn engineer_route(
        &self,
        req: &EngineerRouteRequest,
    ) -> Result<Plotted<EngineerRoute>> {
        self.plot(req).await
    }

    /// `GET /api/{engineers|modules|blueprints|effects}` → the `values` list (raw JSON items).
    pub async fn engineer_vocabulary(&self, list: EngineerVocabulary) -> Result<Vec<Value>> {
        #[derive(Deserialize)]
        struct Values {
            #[serde(default, deserialize_with = "de::nullable")]
            values: Vec<Value>,
        }
        let path = match list {
            EngineerVocabulary::Engineers => "engineers",
            EngineerVocabulary::Modules => "modules",
            EngineerVocabulary::Blueprints => "blueprints",
            EngineerVocabulary::Effects => "effects",
        };
        Ok(self.get_json::<Values>(path, &[], path).await?.values)
    }
}
