//! Route planners. Each `*RouteRequest` implements [`RoutePlanner`](crate::RoutePlanner) and has
//! a typed result; every result also converts to the common [`Waypoint`] list used by the route
//! follower ([`RouteWaypoints`]).

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::util::Coords;

pub mod carrier;
pub mod colonisation;
pub mod engineer;
pub mod galaxy;
pub mod neutron;
pub mod riches;
pub mod tourist;
pub mod trade;

/// A system given by name or by id64 (planners that take either: galaxy plotter, carrier router).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SystemRef {
    Id64(u64),
    Name(String),
}

impl Default for SystemRef {
    fn default() -> Self {
        SystemRef::Name(String::new())
    }
}

impl SystemRef {
    pub(crate) fn is_blank(&self) -> bool {
        matches!(self, SystemRef::Name(n) if n.trim().is_empty())
    }
}

impl fmt::Display for SystemRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SystemRef::Id64(id) => write!(f, "{id}"),
            SystemRef::Name(name) => f.write_str(name),
        }
    }
}

impl From<&str> for SystemRef {
    fn from(name: &str) -> Self {
        SystemRef::Name(name.to_owned())
    }
}

impl From<String> for SystemRef {
    fn from(name: String) -> Self {
        SystemRef::Name(name)
    }
}

impl From<&String> for SystemRef {
    fn from(name: &String) -> Self {
        SystemRef::Name(name.clone())
    }
}

impl From<u64> for SystemRef {
    fn from(id64: u64) -> Self {
        SystemRef::Id64(id64)
    }
}

/// One stop of any route, in the shape the route follower needs.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Waypoint {
    pub system: String,
    pub id64: Option<u64>,
    pub coords: Option<Coords>,
    /// Jumps from the previous waypoint (estimated for the neutron/tourist/riches planners,
    /// exactly 1 for the exact and carrier planners, 0 for the start).
    pub jumps: u32,
    /// Distance from the previous waypoint, ly.
    pub distance: Option<f64>,
    /// Remaining distance to the destination, ly.
    pub distance_remaining: Option<f64>,
    /// Neutron star here (supercharge).
    pub neutron: bool,
    /// Fuel-scoopable primary star (exact plotter only).
    pub scoopable: Option<bool>,
    /// Refuel here (scoop for ships, restock tritium for carriers).
    pub refuel: bool,
    /// Use an FSD injection for the jump from here.
    pub inject: bool,
    /// Fuel (ships, t) or tritium (carriers, t) in the tank on arrival.
    pub fuel_in_tank: Option<f64>,
    /// Fuel or tritium used to get here.
    pub fuel_used: Option<f64>,
    /// Credits on offer here: scan + mapping (+ landmark) value, or trade profit.
    pub value: Option<u64>,
    /// Bodies to visit (riches/exobiology) or the station to dock at (trade).
    pub bodies: Vec<String>,
    /// Short human-readable extra ("restock 1,200 t", "Felicity Farseer", "Gold ×720").
    pub note: Option<String>,
}

/// Common view over every planner result.
pub trait RouteWaypoints {
    /// Ordered stops, starting with the origin when the planner includes it.
    fn waypoints(&self) -> Vec<Waypoint>;

    /// Total jumps along the route.
    fn total_jumps(&self) -> u32 {
        self.waypoints().iter().map(|w| w.jumps).sum()
    }
}

pub(crate) fn thousands(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_ref_serde() {
        assert_eq!(
            serde_json::to_string(&SystemRef::from(10_477_373_803u64)).expect("ser"),
            "10477373803"
        );
        assert_eq!(
            serde_json::from_str::<SystemRef>("\"Sol\"").expect("de"),
            SystemRef::Name("Sol".into())
        );
        assert_eq!(SystemRef::from("Sol").to_string(), "Sol");
    }

    #[test]
    fn thousands_sep() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(1000), "1,000");
        assert_eq!(thousands(12_345_678), "12,345,678");
    }
}
