//! Galaxy dump records (camelCase): `/api/dump/{id64}` and the lines of `galaxy*.json.gz`
//! (both are the same per-system record), plus `systems*.json.gz` and `factions.json.gz` lines.
//!
//! Units differ from the journal: `radius` km, `gravity` g, `surfacePressure` atm,
//! `semiMajorAxis` AU, periods in days; ring radii in metres. Body signal keys keep the journal
//! `$SAA_SignalType_…;` identifiers; ring signals use commodity display names.

use std::collections::BTreeMap;

use serde::de::{DeserializeOwned, Deserializer};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::de;
use crate::util::Coords;

/// One system with bodies and stations.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct System {
    #[serde(deserialize_with = "de::u64_or_zero")]
    pub id64: u64,
    #[serde(deserialize_with = "de::string")]
    pub name: String,
    #[serde(deserialize_with = "de::nullable")]
    pub coords: Coords,
    #[serde(deserialize_with = "de::opt_string")]
    pub allegiance: Option<String>,
    #[serde(deserialize_with = "de::opt_string")]
    pub government: Option<String>,
    #[serde(deserialize_with = "de::opt_string")]
    pub primary_economy: Option<String>,
    #[serde(deserialize_with = "de::opt_string")]
    pub secondary_economy: Option<String>,
    #[serde(deserialize_with = "de::opt_string")]
    pub security: Option<String>,
    #[serde(deserialize_with = "de::opt_u64")]
    pub population: Option<u64>,
    #[serde(deserialize_with = "de::opt_u32")]
    pub body_count: Option<u32>,
    /// Last system update.
    #[serde(deserialize_with = "de::opt_string")]
    pub date: Option<String>,
    /// Per-attribute update times (`controllingPower`, `factions`, `powerState`, `powers`).
    #[serde(deserialize_with = "de::string_map")]
    pub timestamps: BTreeMap<String, String>,
    pub controlling_faction: Option<ControllingFaction>,
    #[serde(deserialize_with = "de::nullable")]
    pub factions: Vec<FactionPresence>,
    #[serde(deserialize_with = "de::opt_string")]
    pub controlling_power: Option<String>,
    #[serde(deserialize_with = "de::name_list")]
    pub powers: Vec<String>,
    /// `Exploited`, `Fortified`, `Stronghold`, `Unoccupied`, ….
    #[serde(deserialize_with = "de::opt_string")]
    pub power_state: Option<String>,
    #[serde(deserialize_with = "de::opt_f64")]
    pub power_state_control_progress: Option<f64>,
    #[serde(deserialize_with = "de::opt_f64")]
    pub power_state_reinforcement: Option<f64>,
    #[serde(deserialize_with = "de::opt_f64")]
    pub power_state_undermining: Option<f64>,
    #[serde(deserialize_with = "de::nullable")]
    pub power_conflict_progress: Vec<PowerConflict>,
    pub thargoid_war: Option<ThargoidWar>,
    /// Orbital stations (not bound to a body); body-bound ones are in `bodies[].stations`.
    #[serde(deserialize_with = "de::nullable")]
    pub stations: Vec<Station>,
    #[serde(deserialize_with = "de::nullable")]
    pub bodies: Vec<Body>,
}

impl System {
    /// Every station: orbital ones first, then those on bodies.
    pub fn all_stations(&self) -> impl Iterator<Item = &Station> {
        self.stations
            .iter()
            .chain(self.bodies.iter().flat_map(|b| b.stations.iter()))
    }

    pub fn body(&self, name: &str) -> Option<&Body> {
        self.bodies
            .iter()
            .find(|b| b.name.eq_ignore_ascii_case(name))
    }

    pub fn main_star(&self) -> Option<&Body> {
        self.bodies.iter().find(|b| b.main_star == Some(true))
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct StateTrend {
    #[serde(deserialize_with = "de::string")]
    pub state: String,
    #[serde(deserialize_with = "de::opt_f64")]
    pub trend: Option<f64>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ControllingFaction {
    #[serde(deserialize_with = "de::string")]
    pub name: String,
    #[serde(deserialize_with = "de::opt_string")]
    pub allegiance: Option<String>,
    #[serde(deserialize_with = "de::opt_string")]
    pub government: Option<String>,
    #[serde(deserialize_with = "de::opt_string")]
    pub state: Option<String>,
    #[serde(deserialize_with = "de::opt_f64")]
    pub influence: Option<f64>,
    #[serde(deserialize_with = "de::nullable")]
    pub active_states: Vec<StateTrend>,
    #[serde(deserialize_with = "de::nullable")]
    pub pending_states: Vec<StateTrend>,
    #[serde(deserialize_with = "de::nullable")]
    pub recovering_states: Vec<StateTrend>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct FactionPresence {
    #[serde(deserialize_with = "de::string")]
    pub name: String,
    #[serde(deserialize_with = "de::opt_string")]
    pub allegiance: Option<String>,
    #[serde(deserialize_with = "de::opt_string")]
    pub government: Option<String>,
    /// 0..1.
    #[serde(deserialize_with = "de::opt_f64")]
    pub influence: Option<f64>,
    #[serde(deserialize_with = "de::opt_string")]
    pub state: Option<String>,
    #[serde(deserialize_with = "de::nullable")]
    pub active_states: Vec<StateTrend>,
    #[serde(deserialize_with = "de::nullable")]
    pub pending_states: Vec<StateTrend>,
    #[serde(deserialize_with = "de::nullable")]
    pub recovering_states: Vec<StateTrend>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PowerConflict {
    #[serde(deserialize_with = "de::string")]
    pub power: String,
    #[serde(deserialize_with = "de::opt_f64")]
    pub progress: Option<f64>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ThargoidWar {
    #[serde(deserialize_with = "de::opt_string")]
    pub current_state: Option<String>,
    #[serde(deserialize_with = "de::opt_string")]
    pub success_state: Option<String>,
    #[serde(deserialize_with = "de::opt_string")]
    pub failure_state: Option<String>,
    #[serde(deserialize_with = "de::opt_f64")]
    pub progress: Option<f64>,
    #[serde(deserialize_with = "de::opt_f64")]
    pub days_remaining: Option<f64>,
    #[serde(deserialize_with = "de::opt_f64")]
    pub ports_remaining: Option<f64>,
    #[serde(deserialize_with = "de::opt_bool")]
    pub success_reached: Option<bool>,
}

/// Journal-style parent link: `{"Star": 0}` → `kind = "Star", body_id = 0`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParentRef {
    /// `Star`, `Planet`, `Null` (barycentre), `Ring`.
    pub kind: String,
    pub body_id: u32,
}

fn parents<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<ParentRef>, D::Error> {
    let raw = Value::deserialize(d)?;
    let Value::Array(items) = raw else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for item in items {
        match item {
            // Dump form `{"Star": 0}`.
            Value::Object(m) if !m.contains_key("kind") => {
                for (kind, id) in m {
                    if let Some(body_id) = id.as_u64().and_then(|n| u32::try_from(n).ok()) {
                        out.push(ParentRef { kind, body_id });
                    }
                }
            }
            // Our own serialized form `{"kind": "Star", "body_id": 0}`.
            other => {
                if let Ok(p) = serde_json::from_value::<ParentRef>(other) {
                    out.push(p);
                }
            }
        }
    }
    Ok(out)
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct BodySignals {
    /// `$SAA_SignalType_Biological;` → count, `$PlanetaryMiningLocation_Name;` → count, ….
    #[serde(deserialize_with = "de::u32_map")]
    pub signals: BTreeMap<String, u32>,
    /// `$Codex_Ent_Sphere_Name;`, ….
    #[serde(deserialize_with = "de::name_list")]
    pub genuses: Vec<String>,
    #[serde(deserialize_with = "de::opt_string")]
    pub update_time: Option<String>,
}

impl BodySignals {
    /// Count for a signal type: `Biological` matches `$SAA_SignalType_Biological;`; full keys
    /// (`$PlanetaryMiningLocation_Name;`) match themselves.
    pub fn count(&self, kind: &str) -> u32 {
        let bare = |s: &str| {
            s.trim()
                .trim_start_matches('$')
                .trim_end_matches(';')
                .to_owned()
        };
        let kind = bare(kind);
        self.signals
            .iter()
            .filter(|(k, _)| {
                let k = bare(k);
                k.eq_ignore_ascii_case(&kind)
                    || k.strip_prefix("SAA_SignalType_")
                        .is_some_and(|t| t.eq_ignore_ascii_case(&kind))
            })
            .map(|(_, n)| *n)
            .sum()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct RingSignals {
    /// Commodity display name → hotspot count.
    #[serde(deserialize_with = "de::u32_map")]
    pub signals: BTreeMap<String, u32>,
    #[serde(deserialize_with = "de::opt_string")]
    pub update_time: Option<String>,
}

/// Ring or belt.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Ring {
    #[serde(deserialize_with = "de::string")]
    pub name: String,
    /// `Icy`, `Metal Rich`, `Metallic`, `Rocky`.
    #[serde(rename = "type", deserialize_with = "de::opt_string")]
    pub ring_type: Option<String>,
    /// Mt.
    #[serde(deserialize_with = "de::opt_f64")]
    pub mass: Option<f64>,
    /// m.
    #[serde(deserialize_with = "de::opt_f64")]
    pub inner_radius: Option<f64>,
    /// m.
    #[serde(deserialize_with = "de::opt_f64")]
    pub outer_radius: Option<f64>,
    #[serde(deserialize_with = "de::opt_u64")]
    pub id64: Option<u64>,
    pub signals: Option<RingSignals>,
}

impl Ring {
    pub fn hotspots(&self, commodity: &str) -> u32 {
        self.signals
            .as_ref()
            .map(|s| {
                s.signals
                    .iter()
                    .filter(|(k, _)| k.eq_ignore_ascii_case(commodity))
                    .map(|(_, n)| *n)
                    .sum()
            })
            .unwrap_or(0)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Body {
    #[serde(deserialize_with = "de::u64_or_zero")]
    pub id64: u64,
    #[serde(deserialize_with = "de::opt_u32")]
    pub body_id: Option<u32>,
    #[serde(deserialize_with = "de::string")]
    pub name: String,
    /// `Star`, `Planet`, `Barycentre`.
    #[serde(rename = "type", deserialize_with = "de::opt_string")]
    pub body_type: Option<String>,
    #[serde(deserialize_with = "de::opt_string")]
    pub sub_type: Option<String>,
    #[serde(deserialize_with = "de::opt_string")]
    pub update_time: Option<String>,
    /// ls.
    #[serde(deserialize_with = "de::opt_f64")]
    pub distance_to_arrival: Option<f64>,

    // stars
    #[serde(deserialize_with = "de::opt_bool")]
    pub main_star: Option<bool>,
    /// Myr.
    #[serde(deserialize_with = "de::opt_f64")]
    pub age: Option<f64>,
    #[serde(deserialize_with = "de::opt_string")]
    pub spectral_class: Option<String>,
    #[serde(deserialize_with = "de::opt_string")]
    pub luminosity: Option<String>,
    #[serde(deserialize_with = "de::opt_f64")]
    pub absolute_magnitude: Option<f64>,
    #[serde(deserialize_with = "de::opt_f64")]
    pub solar_masses: Option<f64>,
    #[serde(deserialize_with = "de::opt_f64")]
    pub solar_radius: Option<f64>,

    // planets
    #[serde(deserialize_with = "de::opt_bool")]
    pub is_landable: Option<bool>,
    /// g.
    #[serde(deserialize_with = "de::opt_f64")]
    pub gravity: Option<f64>,
    #[serde(deserialize_with = "de::opt_f64")]
    pub earth_masses: Option<f64>,
    /// km.
    #[serde(deserialize_with = "de::opt_f64")]
    pub radius: Option<f64>,
    /// atm.
    #[serde(deserialize_with = "de::opt_f64")]
    pub surface_pressure: Option<f64>,
    /// K.
    #[serde(deserialize_with = "de::opt_f64")]
    pub surface_temperature: Option<f64>,
    #[serde(deserialize_with = "de::opt_string")]
    pub volcanism_type: Option<String>,
    #[serde(deserialize_with = "de::opt_string")]
    pub atmosphere_type: Option<String>,
    /// Gas → %.
    #[serde(deserialize_with = "de::f64_map")]
    pub atmosphere_composition: BTreeMap<String, f64>,
    /// `Ice` / `Rock` / `Metal` → %.
    #[serde(deserialize_with = "de::f64_map")]
    pub solid_composition: BTreeMap<String, f64>,
    #[serde(deserialize_with = "de::opt_string")]
    pub terraforming_state: Option<String>,
    /// Material → %.
    #[serde(deserialize_with = "de::f64_map")]
    pub materials: BTreeMap<String, f64>,
    #[serde(deserialize_with = "de::opt_string")]
    pub reserve_level: Option<String>,

    // orbit
    /// days.
    #[serde(deserialize_with = "de::opt_f64")]
    pub orbital_period: Option<f64>,
    /// AU.
    #[serde(deserialize_with = "de::opt_f64")]
    pub semi_major_axis: Option<f64>,
    #[serde(deserialize_with = "de::opt_f64")]
    pub orbital_eccentricity: Option<f64>,
    #[serde(deserialize_with = "de::opt_f64")]
    pub orbital_inclination: Option<f64>,
    #[serde(deserialize_with = "de::opt_f64")]
    pub arg_of_periapsis: Option<f64>,
    #[serde(deserialize_with = "de::opt_f64")]
    pub mean_anomaly: Option<f64>,
    #[serde(deserialize_with = "de::opt_f64")]
    pub ascending_node: Option<f64>,
    /// rad.
    #[serde(deserialize_with = "de::opt_f64")]
    pub axial_tilt: Option<f64>,
    /// days.
    #[serde(deserialize_with = "de::opt_f64")]
    pub rotational_period: Option<f64>,
    #[serde(deserialize_with = "de::opt_bool")]
    pub rotational_period_tidally_locked: Option<bool>,
    #[serde(deserialize_with = "parents")]
    pub parents: Vec<ParentRef>,

    #[serde(deserialize_with = "de::string_map")]
    pub timestamps: BTreeMap<String, String>,
    pub signals: Option<BodySignals>,
    #[serde(deserialize_with = "de::nullable")]
    pub rings: Vec<Ring>,
    #[serde(deserialize_with = "de::nullable")]
    pub belts: Vec<Ring>,
    /// Planetary ports and settlements on this body.
    #[serde(deserialize_with = "de::nullable")]
    pub stations: Vec<Station>,
}

impl Body {
    pub fn is_star(&self) -> bool {
        self.body_type.as_deref() == Some("Star")
    }

    /// Body signal count (`Biological`, `Geological`, `Human`, …).
    pub fn signal_count(&self, kind: &str) -> u32 {
        self.signals.as_ref().map(|s| s.count(kind)).unwrap_or(0)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LandingPads {
    #[serde(deserialize_with = "de::u32_or_zero")]
    pub large: u32,
    #[serde(deserialize_with = "de::u32_or_zero")]
    pub medium: u32,
    #[serde(deserialize_with = "de::u32_or_zero")]
    pub small: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Commodity {
    #[serde(deserialize_with = "de::string")]
    pub name: String,
    /// FDev symbol (`AdvancedCatalysers`).
    #[serde(deserialize_with = "de::opt_string")]
    pub symbol: Option<String>,
    #[serde(deserialize_with = "de::opt_string")]
    pub category: Option<String>,
    #[serde(deserialize_with = "de::opt_u64")]
    pub commodity_id: Option<u64>,
    #[serde(deserialize_with = "de::u64_or_zero")]
    pub demand: u64,
    #[serde(deserialize_with = "de::u64_or_zero")]
    pub supply: u64,
    #[serde(deserialize_with = "de::u64_or_zero")]
    pub buy_price: u64,
    #[serde(deserialize_with = "de::u64_or_zero")]
    pub sell_price: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Market {
    #[serde(deserialize_with = "de::nullable")]
    pub commodities: Vec<Commodity>,
    #[serde(deserialize_with = "de::name_list")]
    pub prohibited_commodities: Vec<String>,
    #[serde(deserialize_with = "de::opt_string")]
    pub update_time: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ShipyardShip {
    #[serde(deserialize_with = "de::string")]
    pub name: String,
    #[serde(deserialize_with = "de::opt_string")]
    pub symbol: Option<String>,
    #[serde(deserialize_with = "de::opt_u64")]
    pub ship_id: Option<u64>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Shipyard {
    #[serde(deserialize_with = "de::nullable")]
    pub ships: Vec<ShipyardShip>,
    #[serde(deserialize_with = "de::opt_string")]
    pub update_time: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct OutfittingModule {
    #[serde(deserialize_with = "de::string")]
    pub name: String,
    #[serde(deserialize_with = "de::opt_string")]
    pub symbol: Option<String>,
    #[serde(deserialize_with = "de::opt_u64")]
    pub module_id: Option<u64>,
    #[serde(deserialize_with = "de::opt_u32")]
    pub class: Option<u32>,
    #[serde(deserialize_with = "de::opt_string")]
    pub rating: Option<String>,
    /// `hardpoint`, `internal`, `utility`, `standard`, `mercgear`.
    #[serde(deserialize_with = "de::opt_string")]
    pub category: Option<String>,
    #[serde(deserialize_with = "de::opt_string")]
    pub ship: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Outfitting {
    #[serde(deserialize_with = "de::nullable")]
    pub modules: Vec<OutfittingModule>,
    #[serde(deserialize_with = "de::opt_string")]
    pub update_time: Option<String>,
}

/// Station, carrier or settlement.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Station {
    #[serde(deserialize_with = "de::string")]
    pub name: String,
    /// Market id.
    #[serde(deserialize_with = "de::u64_or_zero")]
    pub id: u64,
    #[serde(deserialize_with = "de::opt_string")]
    pub update_time: Option<String>,
    /// Missing on some settlements (treat as `Settlement` when `latitude` is present).
    #[serde(rename = "type", deserialize_with = "de::opt_string")]
    pub station_type: Option<String>,
    #[serde(deserialize_with = "de::opt_string")]
    pub state: Option<String>,
    #[serde(deserialize_with = "de::opt_string")]
    pub real_name: Option<String>,
    #[serde(deserialize_with = "de::opt_string")]
    pub carrier_name: Option<String>,
    #[serde(deserialize_with = "de::opt_string")]
    pub carrier_docking_access: Option<String>,
    #[serde(deserialize_with = "de::opt_string")]
    pub controlling_faction: Option<String>,
    #[serde(deserialize_with = "de::opt_string")]
    pub controlling_faction_state: Option<String>,
    #[serde(deserialize_with = "de::opt_string")]
    pub allegiance: Option<String>,
    #[serde(deserialize_with = "de::opt_string")]
    pub government: Option<String>,
    #[serde(deserialize_with = "de::opt_string")]
    pub primary_economy: Option<String>,
    #[serde(deserialize_with = "de::opt_string")]
    pub secondary_economy: Option<String>,
    /// Economy → %.
    #[serde(deserialize_with = "de::f64_map")]
    pub economies: BTreeMap<String, f64>,
    /// ls.
    #[serde(deserialize_with = "de::opt_f64")]
    pub distance_to_arrival: Option<f64>,
    #[serde(deserialize_with = "de::opt_f64")]
    pub latitude: Option<f64>,
    #[serde(deserialize_with = "de::opt_f64")]
    pub longitude: Option<f64>,
    pub landing_pads: Option<LandingPads>,
    #[serde(deserialize_with = "de::name_list")]
    pub services: Vec<String>,
    pub market: Option<Market>,
    pub shipyard: Option<Shipyard>,
    pub outfitting: Option<Outfitting>,
}

impl Station {
    pub fn is_fleet_carrier(&self) -> bool {
        self.station_type
            .as_deref()
            .is_some_and(|t| t.contains("Carrier"))
            || self.carrier_name.is_some()
    }

    pub fn has_service(&self, service: &str) -> bool {
        self.services
            .iter()
            .any(|s| s.eq_ignore_ascii_case(service))
    }

    pub fn commodity(&self, name: &str) -> Option<&Commodity> {
        self.market.as_ref()?.commodities.iter().find(|c| {
            c.name.eq_ignore_ascii_case(name)
                || c.symbol
                    .as_deref()
                    .is_some_and(|s| s.eq_ignore_ascii_case(name))
        })
    }
}

/// A `systems*.json.gz` line: system without bodies or stations.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SystemSummary {
    #[serde(deserialize_with = "de::u64_or_zero")]
    pub id64: u64,
    #[serde(deserialize_with = "de::string")]
    pub name: String,
    #[serde(deserialize_with = "de::nullable")]
    pub coords: Coords,
    /// Main star sub-type (absent for ~0.7 % of systems).
    #[serde(deserialize_with = "de::opt_string")]
    pub main_star: Option<String>,
    #[serde(deserialize_with = "de::opt_bool")]
    pub needs_permit: Option<bool>,
    #[serde(deserialize_with = "de::opt_string")]
    pub update_time: Option<String>,
}

/// A `factions.json.gz` line.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Faction {
    #[serde(deserialize_with = "de::string")]
    pub name: String,
    #[serde(deserialize_with = "de::opt_string")]
    pub allegiance: Option<String>,
    #[serde(deserialize_with = "de::opt_string")]
    pub government: Option<String>,
    /// As in the file: controlled systems appear twice (see [`Faction::systems_deduped`]).
    #[serde(deserialize_with = "de::nullable")]
    pub systems: Vec<FactionSystem>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct FactionSystem {
    #[serde(deserialize_with = "de::string")]
    pub system_name: String,
    #[serde(deserialize_with = "de::u64_or_zero")]
    pub system_id64: u64,
    #[serde(deserialize_with = "de::bool_or_false")]
    pub is_controlling_faction: bool,
    #[serde(deserialize_with = "de::nullable")]
    pub coords: Coords,
}

impl Faction {
    /// One entry per system with the controlling flags OR-ed (the dump lists every controlled
    /// system twice), in first-seen order.
    pub fn systems_deduped(&self) -> Vec<FactionSystem> {
        let mut out: Vec<FactionSystem> = Vec::with_capacity(self.systems.len());
        for s in &self.systems {
            match out.iter_mut().find(|o| o.system_id64 == s.system_id64) {
                Some(existing) => existing.is_controlling_faction |= s.is_controlling_faction,
                None => out.push(s.clone()),
            }
        }
        out
    }
}

/// Parse one line of a dump file. The files are a JSON array with one record per line
/// (`[`, then `\t{…},` lines, then `]`); returns `None` for the bracket/blank lines.
pub fn parse_line<T: DeserializeOwned>(line: &str) -> Option<serde_json::Result<T>> {
    let trimmed = line.trim();
    let trimmed = trimmed.strip_suffix(',').unwrap_or(trimmed).trim_end();
    if trimmed.is_empty() || trimmed == "[" || trimmed == "]" {
        return None;
    }
    Some(serde_json::from_str(trimmed))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dump_lines() {
        assert!(parse_line::<SystemSummary>("[").is_none());
        assert!(parse_line::<SystemSummary>("]").is_none());
        assert!(parse_line::<SystemSummary>("   ").is_none());
        let s = parse_line::<SystemSummary>(
            "\t{\"id64\":1802559,\"name\":\"Muena AA-A h0\",\"mainStar\":\"Wolf-Rayet NC Star\",\"coords\":{\"x\":-15399.875,\"y\":600.125,\"z\":26977.4375},\"updateTime\":\"2026-10-11T15:51:43Z\"},",
        )
        .expect("record line")
        .expect("valid json");
        assert_eq!(s.id64, 1_802_559);
        assert_eq!(s.coords.z, 26977.4375);
    }

    #[test]
    fn parent_forms() {
        let b: Body = serde_json::from_str(r#"{"name":"x","parents":[{"Null":1},{"Star":0}]}"#)
            .expect("parse");
        assert_eq!(
            b.parents,
            vec![
                ParentRef {
                    kind: "Null".into(),
                    body_id: 1
                },
                ParentRef {
                    kind: "Star".into(),
                    body_id: 0
                }
            ]
        );
        let again: Body =
            serde_json::from_str(&serde_json::to_string(&b).expect("ser")).expect("roundtrip");
        assert_eq!(again.parents, b.parents);
    }
}
