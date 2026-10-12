//! Station services as a bitmask, plus material-trader / technology-broker type inference.
//!
//! The bit assignments are stored in the database: **only ever append new bits**.

use serde::{Deserialize, Serialize};
use std::ops::{BitOr, BitOrAssign};

/// Bitmask of station services. Built from journal `StationServices` names (`blackmarket`,
/// `facilitator`, `techBroker`, ...) or Spansh service names (`Black Market`,
/// `Interstellar Factors Contact`, `Technology Broker`, ...).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct Services(pub u64);

macro_rules! services {
    ($( $(#[$m:meta])* $name:ident = $bit:expr, $display:expr; )*) => {
        impl Services {
            $( $(#[$m])* pub const $name: Services = Services(1 << $bit); )*

            /// Every known service with its display name, in bit order.
            pub const ALL: &'static [(Services, &'static str)] = &[ $( (Services::$name, $display), )* ];
        }
    };
}

services! {
    /// Commodity market (`commodities` / `Market`).
    MARKET = 0, "Market";
    SHIPYARD = 1, "Shipyard";
    OUTFITTING = 2, "Outfitting";
    REFUEL = 3, "Refuel";
    REPAIR = 4, "Repair";
    /// Rearm / restock.
    REARM = 5, "Restock";
    BLACK_MARKET = 6, "Black Market";
    /// `facilitator` / "Interstellar Factors Contact".
    INTERSTELLAR_FACTORS = 7, "Interstellar Factors";
    MATERIAL_TRADER = 8, "Material Trader";
    TECH_BROKER = 9, "Technology Broker";
    /// `exploration` / "Universal Cartographics".
    UNIVERSAL_CARTOGRAPHICS = 10, "Universal Cartographics";
    VISTA_GENOMICS = 11, "Vista Genomics";
    MISSIONS = 12, "Missions";
    CREW_LOUNGE = 13, "Crew Lounge";
    SEARCH_RESCUE = 14, "Search and Rescue";
    POWERPLAY = 15, "Powerplay";
    BARTENDER = 16, "Bartender";
    PIONEER_SUPPLIES = 17, "Pioneer Supplies";
    APEX_INTERSTELLAR = 18, "Apex Interstellar";
    FRONTLINE_SOLUTIONS = 19, "Frontline Solutions";
    /// `voucherredemption` / "Redemption Office".
    REDEMPTION_OFFICE = 20, "Redemption Office";
    SOCIAL_SPACE = 21, "Social Space";
    SHOP = 22, "Shop";
    LIVERY = 23, "Livery";
    CARRIER_MANAGEMENT = 24, "Fleet Carrier Management";
    CARRIER_FUEL = 25, "Fleet Carrier Fuel";
    CARRIER_VENDOR = 26, "Fleet Carrier Vendor";
    CARRIER_ADMINISTRATION = 27, "Fleet Carrier Administration";
    COLONISATION = 28, "System Colonisation";
    CONSTRUCTION_SERVICES = 29, "Construction Services";
    CONTACTS = 30, "Contacts";
    DOCK = 31, "Dock";
    AUTODOCK = 32, "Autodock";
    SQUADRON_BANK = 33, "Squadron Bank";
    REFINERY_CONTACT = 34, "Refinery Contact";
    WORKSHOP = 35, "Workshop";
    TUNING = 36, "Tuning";
    FLIGHT_CONTROLLER = 37, "Flight Controller";
    STATION_OPERATIONS = 38, "Station Operations";
    STATION_MENU = 39, "Station Menu";
    ON_DOCK_MISSION = 40, "On Dock Mission";
    MISSIONS_GENERATED = 41, "Missions Generated";
    ENGINEER = 42, "Engineer";
    MODULE_PACKS = 43, "Module Packs";
}

impl Services {
    pub const NONE: Services = Services(0);

    pub fn bits(self) -> u64 {
        self.0
    }

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// True if every service in `other` is present.
    pub fn contains(self, other: Services) -> bool {
        self.0 & other.0 == other.0
    }

    pub fn insert(&mut self, other: Services) {
        self.0 |= other.0;
    }

    /// One journal or Spansh service name -> its bit (`None` for names we don't track).
    pub fn from_name(name: &str) -> Option<Services> {
        let s = match crate::names::key(name).as_str() {
            "commodities" | "market" => Services::MARKET,
            "shipyard" => Services::SHIPYARD,
            "outfitting" => Services::OUTFITTING,
            "refuel" => Services::REFUEL,
            "repair" => Services::REPAIR,
            "rearm" | "restock" => Services::REARM,
            "blackmarket" => Services::BLACK_MARKET,
            "facilitator" | "interstellarfactorscontact" | "interstellarfactors" => {
                Services::INTERSTELLAR_FACTORS
            }
            "materialtrader" => Services::MATERIAL_TRADER,
            "techbroker" | "technologybroker" => Services::TECH_BROKER,
            "exploration" | "universalcartographics" => Services::UNIVERSAL_CARTOGRAPHICS,
            "vistagenomics" => Services::VISTA_GENOMICS,
            "missions" => Services::MISSIONS,
            "crewlounge" => Services::CREW_LOUNGE,
            "searchrescue" | "searchandrescue" => Services::SEARCH_RESCUE,
            "powerplay" => Services::POWERPLAY,
            "bartender" => Services::BARTENDER,
            "pioneersupplies" => Services::PIONEER_SUPPLIES,
            "apexinterstellar" => Services::APEX_INTERSTELLAR,
            "frontlinesolutions" => Services::FRONTLINE_SOLUTIONS,
            "voucherredemption" | "redemptionoffice" => Services::REDEMPTION_OFFICE,
            "socialspace" => Services::SOCIAL_SPACE,
            "shop" => Services::SHOP,
            "livery" => Services::LIVERY,
            "carriermanagement" | "fleetcarriermanagement" => Services::CARRIER_MANAGEMENT,
            "carrierfuel" | "fleetcarrierfuel" => Services::CARRIER_FUEL,
            "carriervendor" | "fleetcarriervendor" => Services::CARRIER_VENDOR,
            "carrieradministration" | "fleetcarrieradministration" => {
                Services::CARRIER_ADMINISTRATION
            }
            "registeringcolonisation" | "systemcolonisation" | "colonisation" => {
                Services::COLONISATION
            }
            "constructionservices" | "colonisationcontribution" => Services::CONSTRUCTION_SERVICES,
            "contacts" => Services::CONTACTS,
            "dock" => Services::DOCK,
            "autodock" => Services::AUTODOCK,
            "squadronbank" => Services::SQUADRON_BANK,
            "refinerycontact" | "refinery" => Services::REFINERY_CONTACT,
            "workshop" => Services::WORKSHOP,
            "tuning" => Services::TUNING,
            "flightcontroller" => Services::FLIGHT_CONTROLLER,
            "stationoperations" => Services::STATION_OPERATIONS,
            "stationmenu" => Services::STATION_MENU,
            "ondockmission" => Services::ON_DOCK_MISSION,
            "missionsgenerated" => Services::MISSIONS_GENERATED,
            "engineer" => Services::ENGINEER,
            "modulepacks" => Services::MODULE_PACKS,
            _ => return None,
        };
        Some(s)
    }

    /// Bitmask from a list of journal or Spansh names; unknown names are ignored.
    pub fn from_names<I, S>(names: I) -> Services
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut out = Services::NONE;
        for n in names {
            match Services::from_name(n.as_ref()) {
                Some(s) => out.insert(s),
                None => tracing::trace!(service = n.as_ref(), "unknown station service"),
            }
        }
        out
    }

    /// Display names of the services present, in bit order.
    pub fn names(self) -> Vec<&'static str> {
        Services::ALL
            .iter()
            .filter(|(s, _)| self.contains(*s))
            .map(|(_, n)| *n)
            .collect()
    }
}

impl BitOr for Services {
    type Output = Services;
    fn bitor(self, rhs: Services) -> Services {
        Services(self.0 | rhs.0)
    }
}

impl BitOrAssign for Services {
    fn bitor_assign(&mut self, rhs: Services) {
        self.0 |= rhs.0;
    }
}

/// Material trader type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MaterialTrader {
    Raw = 1,
    Manufactured = 2,
    Encoded = 3,
}

impl MaterialTrader {
    pub(crate) fn code(self) -> i64 {
        self as i64
    }

    pub(crate) fn from_code(code: i64) -> Option<MaterialTrader> {
        match code {
            1 => Some(MaterialTrader::Raw),
            2 => Some(MaterialTrader::Manufactured),
            3 => Some(MaterialTrader::Encoded),
            _ => None,
        }
    }

    fn from_economy(economy: &str) -> Option<MaterialTrader> {
        match crate::names::key(economy).as_str() {
            "hightech" | "military" => Some(MaterialTrader::Encoded),
            "extraction" | "refinery" => Some(MaterialTrader::Raw),
            "industrial" => Some(MaterialTrader::Manufactured),
            _ => None,
        }
    }

    /// Trader type from the station's economies (FDevIDs "How to determine MatTrader and Broker type"):
    /// High Tech / Military -> Encoded, Extraction / Refinery -> Raw, Industrial -> Manufactured.
    /// The primary economy decides; the secondary is only used when the primary doesn't match.
    pub fn infer(primary: Option<&str>, secondary: Option<&str>) -> Option<MaterialTrader> {
        primary
            .and_then(MaterialTrader::from_economy)
            .or_else(|| secondary.and_then(MaterialTrader::from_economy))
    }

    pub fn display(self) -> &'static str {
        match self {
            MaterialTrader::Raw => "Raw",
            MaterialTrader::Manufactured => "Manufactured",
            MaterialTrader::Encoded => "Encoded",
        }
    }
}

/// Technology broker type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TechBroker {
    Human = 1,
    Guardian = 2,
}

impl TechBroker {
    pub(crate) fn code(self) -> i64 {
        self as i64
    }

    pub(crate) fn from_code(code: i64) -> Option<TechBroker> {
        match code {
            1 => Some(TechBroker::Human),
            2 => Some(TechBroker::Guardian),
            _ => None,
        }
    }

    /// High Tech primary -> Guardian, Industrial primary -> Human, otherwise High Tech secondary ->
    /// Guardian, otherwise Human (the community default).
    pub fn infer(primary: Option<&str>, secondary: Option<&str>) -> TechBroker {
        let p = primary.map(crate::names::key).unwrap_or_default();
        let s = secondary.map(crate::names::key).unwrap_or_default();
        if p == "hightech" {
            TechBroker::Guardian
        } else if p == "industrial" {
            TechBroker::Human
        } else if s == "hightech" {
            TechBroker::Guardian
        } else {
            TechBroker::Human
        }
    }

    pub fn display(self) -> &'static str {
        match self {
            TechBroker::Human => "Human",
            TechBroker::Guardian => "Guardian",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn journal_and_spansh_names_agree() {
        let journal = Services::from_names([
            "dock",
            "autodock",
            "blackmarket",
            "commodities",
            "contacts",
            "exploration",
            "missions",
            "outfitting",
            "crewlounge",
            "rearm",
            "refuel",
            "repair",
            "shipyard",
            "tuning",
            "engineer",
            "missionsgenerated",
            "facilitator",
            "flightcontroller",
            "stationoperations",
            "powerplay",
            "searchrescue",
            "materialtrader",
            "stationMenu",
            "shop",
            "livery",
            "socialspace",
            "bartender",
            "vistagenomics",
            "pioneersupplies",
            "apexinterstellar",
            "frontlinesolutions",
            "techBroker",
            "voucherredemption",
            "somethingnew",
        ]);
        let spansh = Services::from_names([
            "Dock",
            "Autodock",
            "Black Market",
            "Market",
            "Contacts",
            "Universal Cartographics",
            "Missions",
            "Outfitting",
            "Crew Lounge",
            "Restock",
            "Refuel",
            "Repair",
            "Shipyard",
            "Tuning",
            "Engineer",
            "Missions Generated",
            "Interstellar Factors Contact",
            "Flight Controller",
            "Station Operations",
            "Powerplay",
            "Search and Rescue",
            "Material Trader",
            "Station Menu",
            "Shop",
            "Livery",
            "Social Space",
            "Bartender",
            "Vista Genomics",
            "Pioneer Supplies",
            "Apex Interstellar",
            "Frontline Solutions",
            "Technology Broker",
            "Redemption Office",
        ]);
        assert_eq!(journal, spansh);
        assert!(journal.contains(Services::INTERSTELLAR_FACTORS | Services::MATERIAL_TRADER));
        assert!(!journal.contains(Services::SQUADRON_BANK));
        assert!(journal.names().contains(&"Universal Cartographics"));
    }

    #[test]
    fn trader_and_broker_types() {
        assert_eq!(
            MaterialTrader::infer(Some("High Tech"), None),
            Some(MaterialTrader::Encoded)
        );
        assert_eq!(
            MaterialTrader::infer(Some("Military"), Some("Industrial")),
            Some(MaterialTrader::Encoded)
        );
        assert_eq!(
            MaterialTrader::infer(Some("Refinery"), None),
            Some(MaterialTrader::Raw)
        );
        assert_eq!(
            MaterialTrader::infer(Some("Extraction"), None),
            Some(MaterialTrader::Raw)
        );
        assert_eq!(
            MaterialTrader::infer(Some("Industrial"), None),
            Some(MaterialTrader::Manufactured)
        );
        assert_eq!(
            MaterialTrader::infer(Some("Tourism"), Some("Industrial")),
            Some(MaterialTrader::Manufactured)
        );
        assert_eq!(MaterialTrader::infer(Some("Tourism"), None), None);
        assert_eq!(
            TechBroker::infer(Some("High Tech"), None),
            TechBroker::Guardian
        );
        assert_eq!(
            TechBroker::infer(Some("Industrial"), Some("High Tech")),
            TechBroker::Human
        );
        assert_eq!(
            TechBroker::infer(Some("Tourism"), Some("High Tech")),
            TechBroker::Guardian
        );
        assert_eq!(TechBroker::infer(Some("Tourism"), None), TechBroker::Human);
    }
}
