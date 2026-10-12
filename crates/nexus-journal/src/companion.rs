//! The companion JSON files the game rewrites next to the journal: Status.json, Market.json,
//! Outfitting.json, Shipyard.json, NavRoute.json, ModulesInfo.json, Cargo.json, ShipLocker.json,
//! Backpack.json and FCMaterials.json.

#![allow(non_snake_case)]

use crate::events::{Cargo, MicroResourceInventory};
use crate::time::Timestamp;
use serde::Deserialize;
use std::sync::Arc;

// ---------------------------------------------------------------------------------------------
// Status.json
// ---------------------------------------------------------------------------------------------

/// Bits of `Status.Flags`.
pub mod flags {
    pub const DOCKED: u32 = 1 << 0;
    pub const LANDED: u32 = 1 << 1;
    pub const LANDING_GEAR_DOWN: u32 = 1 << 2;
    pub const SHIELDS_UP: u32 = 1 << 3;
    pub const SUPERCRUISE: u32 = 1 << 4;
    pub const FLIGHT_ASSIST_OFF: u32 = 1 << 5;
    pub const HARDPOINTS_DEPLOYED: u32 = 1 << 6;
    pub const IN_WING: u32 = 1 << 7;
    pub const LIGHTS_ON: u32 = 1 << 8;
    pub const CARGO_SCOOP_DEPLOYED: u32 = 1 << 9;
    pub const SILENT_RUNNING: u32 = 1 << 10;
    pub const SCOOPING_FUEL: u32 = 1 << 11;
    pub const SRV_HANDBRAKE: u32 = 1 << 12;
    pub const SRV_TURRET_VIEW: u32 = 1 << 13;
    pub const SRV_TURRET_RETRACTED: u32 = 1 << 14;
    pub const SRV_DRIVE_ASSIST: u32 = 1 << 15;
    pub const FSD_MASS_LOCKED: u32 = 1 << 16;
    pub const FSD_CHARGING: u32 = 1 << 17;
    pub const FSD_COOLDOWN: u32 = 1 << 18;
    pub const LOW_FUEL: u32 = 1 << 19;
    pub const OVER_HEATING: u32 = 1 << 20;
    pub const HAS_LAT_LONG: u32 = 1 << 21;
    pub const IS_IN_DANGER: u32 = 1 << 22;
    pub const BEING_INTERDICTED: u32 = 1 << 23;
    pub const IN_MAIN_SHIP: u32 = 1 << 24;
    pub const IN_FIGHTER: u32 = 1 << 25;
    pub const IN_SRV: u32 = 1 << 26;
    pub const HUD_ANALYSIS_MODE: u32 = 1 << 27;
    pub const NIGHT_VISION: u32 = 1 << 28;
    pub const ALTITUDE_FROM_AVERAGE_RADIUS: u32 = 1 << 29;
    pub const FSD_JUMP: u32 = 1 << 30;
    pub const SRV_HIGH_BEAM: u32 = 1 << 31;

    pub const NAMES: [&str; 32] = [
        "Docked",
        "Landed",
        "Landing gear down",
        "Shields up",
        "Supercruise",
        "Flight assist off",
        "Hardpoints deployed",
        "In wing",
        "Lights on",
        "Cargo scoop deployed",
        "Silent running",
        "Scooping fuel",
        "SRV handbrake",
        "SRV turret view",
        "SRV turret retracted",
        "SRV drive assist",
        "FSD mass locked",
        "FSD charging",
        "FSD cooldown",
        "Low fuel",
        "Over heating",
        "Has lat/long",
        "In danger",
        "Being interdicted",
        "In main ship",
        "In fighter",
        "In SRV",
        "Analysis mode",
        "Night vision",
        "Altitude from average radius",
        "FSD jump",
        "SRV high beam",
    ];
}

/// Bits of `Status.Flags2` (Odyssey).
pub mod flags2 {
    pub const ON_FOOT: u32 = 1 << 0;
    pub const IN_TAXI: u32 = 1 << 1;
    pub const IN_MULTICREW: u32 = 1 << 2;
    pub const ON_FOOT_IN_STATION: u32 = 1 << 3;
    pub const ON_FOOT_ON_PLANET: u32 = 1 << 4;
    pub const AIM_DOWN_SIGHT: u32 = 1 << 5;
    pub const LOW_OXYGEN: u32 = 1 << 6;
    pub const LOW_HEALTH: u32 = 1 << 7;
    pub const COLD: u32 = 1 << 8;
    pub const HOT: u32 = 1 << 9;
    pub const VERY_COLD: u32 = 1 << 10;
    pub const VERY_HOT: u32 = 1 << 11;
    pub const GLIDE_MODE: u32 = 1 << 12;
    pub const ON_FOOT_IN_HANGAR: u32 = 1 << 13;
    pub const ON_FOOT_SOCIAL_SPACE: u32 = 1 << 14;
    pub const ON_FOOT_EXTERIOR: u32 = 1 << 15;
    pub const BREATHABLE_ATMOSPHERE: u32 = 1 << 16;
    pub const TELEPRESENCE_MULTICREW: u32 = 1 << 17;
    pub const PHYSICAL_MULTICREW: u32 = 1 << 18;
    pub const FSD_HYPERDRIVE_CHARGING: u32 = 1 << 19;
    pub const SUPERCRUISE_OVERCHARGE: u32 = 1 << 20;
    pub const SUPERCRUISE_ASSIST: u32 = 1 << 21;
    pub const NPC_CREW_ACTIVE: u32 = 1 << 22;

    pub const NAMES: [&str; 23] = [
        "On foot",
        "In taxi",
        "In multicrew",
        "On foot in station",
        "On foot on planet",
        "Aim down sight",
        "Low oxygen",
        "Low health",
        "Cold",
        "Hot",
        "Very cold",
        "Very hot",
        "Glide mode",
        "On foot in hangar",
        "On foot in social space",
        "On foot exterior",
        "Breathable atmosphere",
        "Telepresence multicrew",
        "Physical multicrew",
        "FSD hyperdrive charging",
        "Supercruise overcharge",
        "Supercruise assist",
        "NPC crew active",
    ];
}

/// `Status.GuiFocus` values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuiFocus {
    None,
    InternalPanel,
    ExternalPanel,
    CommsPanel,
    RolePanel,
    StationServices,
    GalaxyMap,
    SystemMap,
    Orrery,
    Fss,
    Saa,
    Codex,
    Unknown(u32),
}

impl GuiFocus {
    pub fn from_u32(v: u32) -> Self {
        match v {
            0 => GuiFocus::None,
            1 => GuiFocus::InternalPanel,
            2 => GuiFocus::ExternalPanel,
            3 => GuiFocus::CommsPanel,
            4 => GuiFocus::RolePanel,
            5 => GuiFocus::StationServices,
            6 => GuiFocus::GalaxyMap,
            7 => GuiFocus::SystemMap,
            8 => GuiFocus::Orrery,
            9 => GuiFocus::Fss,
            10 => GuiFocus::Saa,
            11 => GuiFocus::Codex,
            other => GuiFocus::Unknown(other),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(default)]
pub struct StatusFuel {
    #[serde(rename = "FuelMain")]
    pub main: f64,
    #[serde(rename = "FuelReservoir")]
    pub reservoir: f64,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct StatusDestination {
    #[serde(rename = "System")]
    pub system: u64,
    #[serde(rename = "Body")]
    pub body: u32,
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Name_Localised")]
    pub name_localised: Option<String>,
}

/// Status.json — rewritten by the game whenever the cockpit state changes.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Status {
    pub timestamp: Timestamp,
    #[serde(rename = "Flags")]
    pub flags: u32,
    #[serde(rename = "Flags2")]
    pub flags2: u32,
    #[serde(rename = "Pips")]
    pub pips: Option<[u8; 3]>,
    #[serde(rename = "FireGroup")]
    pub fire_group: Option<u32>,
    #[serde(rename = "GuiFocus")]
    pub gui_focus: Option<u32>,
    #[serde(rename = "Fuel")]
    pub fuel: Option<StatusFuel>,
    #[serde(rename = "Cargo")]
    pub cargo: Option<f64>,
    #[serde(rename = "LegalState")]
    pub legal_state: Option<String>,
    #[serde(rename = "Latitude")]
    pub latitude: Option<f64>,
    #[serde(rename = "Longitude")]
    pub longitude: Option<f64>,
    #[serde(rename = "Altitude")]
    pub altitude: Option<f64>,
    #[serde(rename = "Heading")]
    pub heading: Option<f64>,
    #[serde(rename = "BodyName")]
    pub body_name: Option<String>,
    #[serde(rename = "PlanetRadius")]
    pub planet_radius: Option<f64>,
    #[serde(rename = "Balance")]
    pub balance: Option<i64>,
    #[serde(rename = "Destination")]
    pub destination: Option<StatusDestination>,
    #[serde(rename = "Oxygen")]
    pub oxygen: Option<f64>,
    #[serde(rename = "Health")]
    pub health: Option<f64>,
    #[serde(rename = "Temperature")]
    pub temperature: Option<f64>,
    #[serde(rename = "SelectedWeapon")]
    pub selected_weapon: Option<String>,
    #[serde(rename = "SelectedWeapon_Localised")]
    pub selected_weapon_localised: Option<String>,
    #[serde(rename = "Gravity")]
    pub gravity: Option<f64>,
}

impl Status {
    #[inline]
    pub fn has(&self, flag: u32) -> bool {
        self.flags & flag != 0
    }
    #[inline]
    pub fn has2(&self, flag: u32) -> bool {
        self.flags2 & flag != 0
    }
    pub fn gui_focus(&self) -> GuiFocus {
        GuiFocus::from_u32(self.gui_focus.unwrap_or(0))
    }
    /// The game writes an almost-empty Status.json when it's at the main menu / not running.
    pub fn is_in_game(&self) -> bool {
        self.flags != 0 || self.flags2 != 0
    }
    pub fn on_foot(&self) -> bool {
        self.has2(flags2::ON_FOOT)
    }
    pub fn lat_lon(&self) -> Option<(f64, f64)> {
        Some((self.latitude?, self.longitude?))
    }
}

// ---------------------------------------------------------------------------------------------
// Market.json / Outfitting.json / Shipyard.json / FCMaterials.json
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct MarketItem {
    pub id: u64,
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Name_Localised")]
    pub name_localised: Option<String>,
    #[serde(rename = "Category")]
    pub category: String,
    #[serde(rename = "Category_Localised")]
    pub category_localised: Option<String>,
    #[serde(rename = "BuyPrice")]
    pub buy_price: i64,
    #[serde(rename = "SellPrice")]
    pub sell_price: i64,
    #[serde(rename = "MeanPrice")]
    pub mean_price: i64,
    /// 0-3; the game writes `""` for goods the station does not trade in that direction.
    #[serde(rename = "StockBracket", deserialize_with = "lenient_i64")]
    pub stock_bracket: i64,
    #[serde(rename = "DemandBracket", deserialize_with = "lenient_i64")]
    pub demand_bracket: i64,
    #[serde(rename = "Stock")]
    pub stock: i64,
    #[serde(rename = "Demand")]
    pub demand: i64,
    #[serde(rename = "Consumer")]
    pub consumer: bool,
    #[serde(rename = "Producer")]
    pub producer: bool,
    #[serde(rename = "Rare")]
    pub rare: bool,
}

/// A number that may also arrive as a string (`""` reads as 0).
fn lenient_i64<'de, D: serde::Deserializer<'de>>(d: D) -> Result<i64, D::Error> {
    Ok(match serde_json::Value::deserialize(d)? {
        serde_json::Value::Number(n) => n.as_i64().or_else(|| n.as_f64().map(|f| f as i64)).unwrap_or(0),
        serde_json::Value::String(s) => s.trim().parse().unwrap_or(0),
        _ => 0,
    })
}

impl MarketItem {
    /// `$gold_name;` -> `gold`: the symbol used by EDDN, FDevIDs and our galaxy database.
    pub fn symbol(&self) -> String {
        commodity_symbol(&self.name)
    }
}

/// Normalises a journal commodity name (`$gold_name;`, `Gold`, `gold`) to the lowercase symbol.
pub fn commodity_symbol(name: &str) -> String {
    let s = name.strip_prefix('$').unwrap_or(name);
    let s = s.strip_suffix(';').unwrap_or(s);
    let s = s.strip_suffix("_name").or_else(|| s.strip_suffix("_Name")).unwrap_or(s);
    s.to_ascii_lowercase()
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct MarketFile {
    pub timestamp: Timestamp,
    #[serde(rename = "MarketID")]
    pub market_id: u64,
    #[serde(rename = "StationName")]
    pub station_name: String,
    #[serde(rename = "StationType")]
    pub station_type: Option<String>,
    #[serde(rename = "CarrierDockingAccess")]
    pub carrier_docking_access: Option<String>,
    #[serde(rename = "StarSystem")]
    pub star_system: String,
    #[serde(rename = "Items")]
    pub items: Vec<MarketItem>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct OutfittingItem {
    pub id: u64,
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "BuyPrice")]
    pub buy_price: i64,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct OutfittingFile {
    pub timestamp: Timestamp,
    #[serde(rename = "MarketID")]
    pub market_id: u64,
    #[serde(rename = "StationName")]
    pub station_name: String,
    #[serde(rename = "StarSystem")]
    pub star_system: String,
    #[serde(rename = "Horizons")]
    pub horizons: Option<bool>,
    #[serde(rename = "Items")]
    pub items: Vec<OutfittingItem>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ShipyardItem {
    pub id: u64,
    #[serde(rename = "ShipType")]
    pub ship_type: String,
    #[serde(rename = "ShipType_Localised")]
    pub ship_type_localised: Option<String>,
    #[serde(rename = "ShipPrice")]
    pub ship_price: i64,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ShipyardFile {
    pub timestamp: Timestamp,
    #[serde(rename = "MarketID")]
    pub market_id: u64,
    #[serde(rename = "StationName")]
    pub station_name: String,
    #[serde(rename = "StarSystem")]
    pub star_system: String,
    #[serde(rename = "Horizons")]
    pub horizons: Option<bool>,
    #[serde(rename = "AllowCobraMkIV")]
    pub allow_cobra_mk_iv: Option<bool>,
    #[serde(rename = "PriceList")]
    pub price_list: Vec<ShipyardItem>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct FCMaterialItem {
    pub id: u64,
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Name_Localised")]
    pub name_localised: Option<String>,
    #[serde(rename = "Price")]
    pub price: i64,
    #[serde(rename = "Stock")]
    pub stock: i64,
    #[serde(rename = "Demand")]
    pub demand: i64,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct FCMaterialsFile {
    pub timestamp: Timestamp,
    #[serde(rename = "MarketID")]
    pub market_id: u64,
    #[serde(rename = "CarrierName")]
    pub carrier_name: String,
    #[serde(rename = "CarrierID")]
    pub carrier_id: String,
    #[serde(rename = "Items")]
    pub items: Vec<FCMaterialItem>,
}

// ---------------------------------------------------------------------------------------------
// NavRoute.json / ModulesInfo.json
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct RouteHop {
    #[serde(rename = "StarSystem")]
    pub star_system: String,
    #[serde(rename = "SystemAddress")]
    pub system_address: u64,
    #[serde(rename = "StarPos")]
    pub star_pos: [f64; 3],
    #[serde(rename = "StarClass")]
    pub star_class: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct NavRouteFile {
    pub timestamp: Timestamp,
    pub event: String,
    #[serde(rename = "Route")]
    pub route: Vec<RouteHop>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ModuleInfoItem {
    #[serde(rename = "Slot")]
    pub slot: String,
    #[serde(rename = "Item")]
    pub item: String,
    #[serde(rename = "Power")]
    pub power: f64,
    #[serde(rename = "Priority")]
    pub priority: Option<u32>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ModulesInfoFile {
    pub timestamp: Timestamp,
    #[serde(rename = "Modules")]
    pub modules: Vec<ModuleInfoItem>,
}

/// Cargo.json / ShipLocker.json / Backpack.json wrappers that keep the file timestamp.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct CargoFile {
    pub timestamp: Timestamp,
    #[serde(flatten)]
    pub cargo: Cargo,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct MicroResourceFile {
    pub timestamp: Timestamp,
    #[serde(flatten)]
    pub inventory: MicroResourceInventory,
}

// ---------------------------------------------------------------------------------------------
// Generic wrapper
// ---------------------------------------------------------------------------------------------

/// Which companion file something came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CompanionKind {
    Status,
    Market,
    Outfitting,
    Shipyard,
    NavRoute,
    ModulesInfo,
    Cargo,
    ShipLocker,
    Backpack,
    FCMaterials,
}

impl CompanionKind {
    pub const ALL: [CompanionKind; 10] = [
        CompanionKind::Status,
        CompanionKind::Market,
        CompanionKind::Outfitting,
        CompanionKind::Shipyard,
        CompanionKind::NavRoute,
        CompanionKind::ModulesInfo,
        CompanionKind::Cargo,
        CompanionKind::ShipLocker,
        CompanionKind::Backpack,
        CompanionKind::FCMaterials,
    ];

    pub fn file_name(self) -> &'static str {
        match self {
            CompanionKind::Status => "Status.json",
            CompanionKind::Market => "Market.json",
            CompanionKind::Outfitting => "Outfitting.json",
            CompanionKind::Shipyard => "Shipyard.json",
            CompanionKind::NavRoute => "NavRoute.json",
            CompanionKind::ModulesInfo => "ModulesInfo.json",
            CompanionKind::Cargo => "Cargo.json",
            CompanionKind::ShipLocker => "ShipLocker.json",
            CompanionKind::Backpack => "Backpack.json",
            CompanionKind::FCMaterials => "FCMaterials.json",
        }
    }

    /// The journal event that announces a fresh write of this file.
    pub fn from_event_name(name: &str) -> Option<Self> {
        Some(match name {
            "Market" => CompanionKind::Market,
            "Outfitting" => CompanionKind::Outfitting,
            "Shipyard" => CompanionKind::Shipyard,
            "NavRoute" | "NavRouteClear" => CompanionKind::NavRoute,
            "ModuleInfo" => CompanionKind::ModulesInfo,
            "Cargo" => CompanionKind::Cargo,
            "ShipLocker" => CompanionKind::ShipLocker,
            "Backpack" => CompanionKind::Backpack,
            "FCMaterials" => CompanionKind::FCMaterials,
            _ => return None,
        })
    }
}

/// A parsed companion file, with its raw text kept for uploaders (EDDN wants the original data).
#[derive(Debug, Clone)]
pub enum Companion {
    Status(Arc<Status>),
    Market(Arc<MarketFile>, Arc<str>),
    Outfitting(Arc<OutfittingFile>, Arc<str>),
    Shipyard(Arc<ShipyardFile>, Arc<str>),
    NavRoute(Arc<NavRouteFile>, Arc<str>),
    ModulesInfo(Arc<ModulesInfoFile>, Arc<str>),
    Cargo(Arc<CargoFile>, Arc<str>),
    ShipLocker(Arc<MicroResourceFile>, Arc<str>),
    Backpack(Arc<MicroResourceFile>, Arc<str>),
    FCMaterials(Arc<FCMaterialsFile>, Arc<str>),
}

impl Companion {
    pub fn kind(&self) -> CompanionKind {
        match self {
            Companion::Status(_) => CompanionKind::Status,
            Companion::Market(..) => CompanionKind::Market,
            Companion::Outfitting(..) => CompanionKind::Outfitting,
            Companion::Shipyard(..) => CompanionKind::Shipyard,
            Companion::NavRoute(..) => CompanionKind::NavRoute,
            Companion::ModulesInfo(..) => CompanionKind::ModulesInfo,
            Companion::Cargo(..) => CompanionKind::Cargo,
            Companion::ShipLocker(..) => CompanionKind::ShipLocker,
            Companion::Backpack(..) => CompanionKind::Backpack,
            Companion::FCMaterials(..) => CompanionKind::FCMaterials,
        }
    }

    /// Parses the text of a companion file. Returns `None` for empty/partial writes.
    pub fn parse(kind: CompanionKind, text: &str) -> Option<Companion> {
        let text = text.trim_start_matches('\u{feff}').trim();
        if text.is_empty() {
            return None;
        }
        let raw: Arc<str> = Arc::from(text);
        Some(match kind {
            CompanionKind::Status => Companion::Status(Arc::new(serde_json::from_str(text).ok()?)),
            CompanionKind::Market => Companion::Market(Arc::new(serde_json::from_str(text).ok()?), raw),
            CompanionKind::Outfitting => Companion::Outfitting(Arc::new(serde_json::from_str(text).ok()?), raw),
            CompanionKind::Shipyard => Companion::Shipyard(Arc::new(serde_json::from_str(text).ok()?), raw),
            CompanionKind::NavRoute => Companion::NavRoute(Arc::new(serde_json::from_str(text).ok()?), raw),
            CompanionKind::ModulesInfo => Companion::ModulesInfo(Arc::new(serde_json::from_str(text).ok()?), raw),
            CompanionKind::Cargo => Companion::Cargo(Arc::new(serde_json::from_str(text).ok()?), raw),
            CompanionKind::ShipLocker => Companion::ShipLocker(Arc::new(serde_json::from_str(text).ok()?), raw),
            CompanionKind::Backpack => Companion::Backpack(Arc::new(serde_json::from_str(text).ok()?), raw),
            CompanionKind::FCMaterials => Companion::FCMaterials(Arc::new(serde_json::from_str(text).ok()?), raw),
        })
    }

    pub fn timestamp(&self) -> Timestamp {
        match self {
            Companion::Status(s) => s.timestamp,
            Companion::Market(m, _) => m.timestamp,
            Companion::Outfitting(m, _) => m.timestamp,
            Companion::Shipyard(m, _) => m.timestamp,
            Companion::NavRoute(m, _) => m.timestamp,
            Companion::ModulesInfo(m, _) => m.timestamp,
            Companion::Cargo(m, _) => m.timestamp,
            Companion::ShipLocker(m, _) => m.timestamp,
            Companion::Backpack(m, _) => m.timestamp,
            Companion::FCMaterials(m, _) => m.timestamp,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_flags() {
        let s = r#"{ "timestamp":"2026-10-12T09:07:40Z", "event":"Status", "Flags":151060493, "Flags2":0, "Pips":[4,8,0], "FireGroup":0, "GuiFocus":0, "Fuel":{ "FuelMain":64.000000, "FuelReservoir":1.140000 }, "Cargo":185.000000, "LegalState":"Clean", "Balance":3889591046, "Destination":{ "System":5068732507561, "Body":1, "Name":"TCG BARBAROS KFK-L3V" } }"#;
        let Companion::Status(st) = Companion::parse(CompanionKind::Status, s).unwrap() else { panic!() };
        assert!(st.has(flags::DOCKED));
        assert!(st.has(flags::IN_MAIN_SHIP));
        assert!(!st.has(flags::SUPERCRUISE));
        assert_eq!(st.balance, Some(3_889_591_046));
        assert_eq!(st.destination.as_ref().unwrap().name, "TCG BARBAROS KFK-L3V");
        assert_eq!(st.fuel.unwrap().main, 64.0);
    }

    #[test]
    fn commodity_symbols() {
        assert_eq!(commodity_symbol("$superconductors_name;"), "superconductors");
        assert_eq!(commodity_symbol("metaalloys"), "metaalloys");
        assert_eq!(commodity_symbol("Gold"), "gold");
    }

    #[test]
    fn partial_write_is_none() {
        assert!(Companion::parse(CompanionKind::Status, "").is_none());
        assert!(Companion::parse(CompanionKind::Status, "{ \"timestamp\":\"2026").is_none());
    }
}
