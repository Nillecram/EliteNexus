//! Station kinds and landing-pad sizes.

use serde::{Deserialize, Serialize};

/// Largest landing pad a station offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Pad {
    Small = 1,
    Medium = 2,
    Large = 3,
}

impl Pad {
    pub fn as_char(self) -> char {
        match self {
            Pad::Small => 'S',
            Pad::Medium => 'M',
            Pad::Large => 'L',
        }
    }

    /// `'S'`, `'M'`, `'L'` (case-insensitive).
    pub fn from_char(c: char) -> Option<Pad> {
        match c.to_ascii_uppercase() {
            'S' => Some(Pad::Small),
            'M' => Some(Pad::Medium),
            'L' => Some(Pad::Large),
            _ => None,
        }
    }

    /// Database code: 1 = S, 2 = M, 3 = L (0 = unknown).
    pub(crate) fn code(self) -> i64 {
        self as i64
    }

    pub(crate) fn from_code(code: i64) -> Option<Pad> {
        match code {
            1 => Some(Pad::Small),
            2 => Some(Pad::Medium),
            3 => Some(Pad::Large),
            _ => None,
        }
    }

    /// Largest pad from journal/Spansh landing pad counts; `None` if all counts are zero.
    pub fn from_counts(small: u32, medium: u32, large: u32) -> Option<Pad> {
        if large > 0 {
            Some(Pad::Large)
        } else if medium > 0 {
            Some(Pad::Medium)
        } else if small > 0 {
            Some(Pad::Small)
        } else {
            None
        }
    }
}

/// Normalised station type. Accepts journal (`Coriolis`, `CraterOutpost`, `FleetCarrier`, ...)
/// and Spansh (`Coriolis Starport`, `Planetary Outpost`, `Drake-Class Carrier`, ...) spellings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StationKind {
    Coriolis,
    Orbis,
    Ocellus,
    Dodec,
    Outpost,
    AsteroidBase,
    MegaShip,
    FleetCarrier,
    PlanetaryPort,
    PlanetaryOutpost,
    Settlement,
    SurfaceStation,
    SpaceConstructionDepot,
    PlanetaryConstructionDepot,
    DockablePlanetStation,
    Other,
}

impl StationKind {
    /// `None` for an empty string; unknown non-empty strings map to [`StationKind::Other`].
    pub fn parse(raw: &str) -> Option<StationKind> {
        let k = crate::names::key(raw);
        if k.is_empty() {
            return None;
        }
        Some(match k.as_str() {
            "coriolis" | "coriolisstarport" => StationKind::Coriolis,
            "orbis" | "orbisstarport" => StationKind::Orbis,
            "ocellus" | "ocellusstarport" | "bernal" => StationKind::Ocellus,
            "dodec" | "dodecstarport" | "dodecstation" => StationKind::Dodec,
            "outpost" => StationKind::Outpost,
            "asteroidbase" => StationKind::AsteroidBase,
            "megaship" | "megashipcivilian" => StationKind::MegaShip,
            "craterport" | "planetaryport" => StationKind::PlanetaryPort,
            "crateroutpost" | "planetaryoutpost" => StationKind::PlanetaryOutpost,
            "onfootsettlement" | "settlement" | "surfacesettlement" | "odysseysettlement" => {
                StationKind::Settlement
            }
            "surfacestation" => StationKind::SurfaceStation,
            "spaceconstructiondepot" => StationKind::SpaceConstructionDepot,
            "planetaryconstructiondepot" => StationKind::PlanetaryConstructionDepot,
            "dockableplanetstation" => StationKind::DockablePlanetStation,
            other if other.contains("carrier") => StationKind::FleetCarrier,
            _ => StationKind::Other,
        })
    }

    pub fn display(self) -> &'static str {
        match self {
            StationKind::Coriolis => "Coriolis Starport",
            StationKind::Orbis => "Orbis Starport",
            StationKind::Ocellus => "Ocellus Starport",
            StationKind::Dodec => "Dodec Starport",
            StationKind::Outpost => "Outpost",
            StationKind::AsteroidBase => "Asteroid Base",
            StationKind::MegaShip => "Mega Ship",
            StationKind::FleetCarrier => "Fleet Carrier",
            StationKind::PlanetaryPort => "Planetary Port",
            StationKind::PlanetaryOutpost => "Planetary Outpost",
            StationKind::Settlement => "Settlement",
            StationKind::SurfaceStation => "Surface Station",
            StationKind::SpaceConstructionDepot => "Space Construction Depot",
            StationKind::PlanetaryConstructionDepot => "Planetary Construction Depot",
            StationKind::DockablePlanetStation => "Dockable Planet Station",
            StationKind::Other => "Other",
        }
    }

    pub fn is_planetary(self) -> bool {
        matches!(
            self,
            StationKind::PlanetaryPort
                | StationKind::PlanetaryOutpost
                | StationKind::Settlement
                | StationKind::SurfaceStation
                | StationKind::PlanetaryConstructionDepot
                | StationKind::DockablePlanetStation
        )
    }

    pub fn is_carrier(self) -> bool {
        self == StationKind::FleetCarrier
    }

    /// Odyssey on-foot settlement.
    pub fn is_settlement(self) -> bool {
        self == StationKind::Settlement
    }

    /// Pad size implied by the type when no landing-pad counts are known.
    pub fn default_max_pad(self) -> Option<Pad> {
        match self {
            StationKind::Coriolis
            | StationKind::Orbis
            | StationKind::Ocellus
            | StationKind::Dodec
            | StationKind::AsteroidBase
            | StationKind::MegaShip
            | StationKind::FleetCarrier
            | StationKind::PlanetaryPort => Some(Pad::Large),
            StationKind::Outpost => Some(Pad::Medium),
            _ => None,
        }
    }

    /// Display name for the database: the canonical name, or the cleaned raw string for unknown kinds.
    pub(crate) fn display_for(raw: &str) -> Option<String> {
        let kind = StationKind::parse(raw)?;
        Some(match kind {
            StationKind::Other => crate::names::split_camel(crate::names::clean_symbol(raw)),
            k => k.display().to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_journal_and_spansh_types() {
        assert_eq!(StationKind::parse("Coriolis"), Some(StationKind::Coriolis));
        assert_eq!(
            StationKind::parse("Coriolis Starport"),
            Some(StationKind::Coriolis)
        );
        assert_eq!(StationKind::parse("Bernal"), Some(StationKind::Ocellus));
        assert_eq!(
            StationKind::parse("FleetCarrier"),
            Some(StationKind::FleetCarrier)
        );
        assert_eq!(
            StationKind::parse("Drake-Class Carrier"),
            Some(StationKind::FleetCarrier)
        );
        assert_eq!(
            StationKind::parse("CraterOutpost"),
            Some(StationKind::PlanetaryOutpost)
        );
        assert_eq!(
            StationKind::parse("Planetary Port"),
            Some(StationKind::PlanetaryPort)
        );
        assert_eq!(
            StationKind::parse("OnFootSettlement"),
            Some(StationKind::Settlement)
        );
        assert_eq!(
            StationKind::parse("Asteroid base"),
            Some(StationKind::AsteroidBase)
        );
        assert_eq!(StationKind::parse("Mega ship"), Some(StationKind::MegaShip));
        assert_eq!(
            StationKind::parse("SpaceConstructionDepot"),
            Some(StationKind::SpaceConstructionDepot)
        );
        assert_eq!(
            StationKind::parse("WeirdNewThing"),
            Some(StationKind::Other)
        );
        assert_eq!(StationKind::parse(""), None);
        assert_eq!(
            StationKind::display_for("WeirdNewThing").as_deref(),
            Some("Weird New Thing")
        );
        assert!(StationKind::PlanetaryOutpost.is_planetary());
        assert!(!StationKind::Outpost.is_planetary());
        assert_eq!(StationKind::Outpost.default_max_pad(), Some(Pad::Medium));
        assert_eq!(StationKind::Settlement.default_max_pad(), None);
    }

    #[test]
    fn pads() {
        assert_eq!(Pad::from_counts(4, 4, 8), Some(Pad::Large));
        assert_eq!(Pad::from_counts(3, 1, 0), Some(Pad::Medium));
        assert_eq!(Pad::from_counts(0, 0, 0), None);
        assert_eq!(Pad::from_char('m'), Some(Pad::Medium));
        assert!(Pad::Large > Pad::Medium);
        assert_eq!(Pad::from_code(Pad::Small.code()), Some(Pad::Small));
    }
}
