//! Static catalogue of Elite Dangerous actions: friendly name, context (which vehicle / mode the
//! action belongs to) and category (the section of the in-game Controls menu).
//!
//! The table follows the element order of the game's own 4.2 `.binds` files, which mirrors the
//! Controls menu. Names follow the in-game labels where known; groupings were cross-checked
//! against EDRefCard's `bindingsData.py` (MIT licence, see the crate README).
//!
//! Actions missing from the table still work: [`action_info`] derives a name by splitting the
//! identifier into words and guesses the context from prefixes/suffixes (`Buggy` → SRV,
//! `Humanoid` → on foot, `ExplorationFSS` → scanners, …).

use std::borrow::Cow;
use std::sync::LazyLock;

use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};

use crate::presets::PresetCategory;
use crate::util::humanize;

/// The vehicle or game mode an action belongs to (top-level grouping of the reference card).
///
/// Variants are declared in reference-card order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Context {
    Ship,
    /// Ship-launched fighter orders (issued from the mothership's cockpit).
    Fighter,
    /// Full Spectrum System Scanner and Detailed Surface Scanner.
    Scanners,
    /// Crewing someone else's ship.
    Multicrew,
    Srv,
    OnFoot,
    /// UI panel navigation (`UI_*`, tab and page cycling).
    Interface,
    /// Galaxy and system map camera.
    GalaxyMap,
    /// Global actions: game menu, push-to-talk, HMD reset, headlook, Galnet audio.
    General,
    /// Camera suite (vanity cameras) and free camera.
    Camera,
    HoloMe,
    /// Livery / store preview.
    Store,
    /// Settlement placement camera (colonisation).
    Colonisation,
}

impl Context {
    /// All contexts in reference-card order.
    pub const ALL: [Context; 13] = [
        Self::Ship,
        Self::Fighter,
        Self::Scanners,
        Self::Multicrew,
        Self::Srv,
        Self::OnFoot,
        Self::Interface,
        Self::GalaxyMap,
        Self::General,
        Self::Camera,
        Self::HoloMe,
        Self::Store,
        Self::Colonisation,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Ship => "Ship",
            Self::Fighter => "Fighter",
            Self::Scanners => "Scanners",
            Self::Multicrew => "Multicrew",
            Self::Srv => "SRV",
            Self::OnFoot => "On foot",
            Self::Interface => "Interface",
            Self::GalaxyMap => "Galaxy map",
            Self::General => "General",
            Self::Camera => "Camera",
            Self::HoloMe => "Holo-Me",
            Self::Store => "Store",
            Self::Colonisation => "Colonisation",
        }
    }

    /// Which of the four per-category presets (`StartPreset.4.start`) supplies this context's
    /// bindings. Best effort for the general-purpose contexts.
    pub fn preset_category(self) -> PresetCategory {
        match self {
            Self::Ship | Self::Fighter | Self::Scanners | Self::Multicrew => PresetCategory::Ship,
            Self::Srv => PresetCategory::Srv,
            Self::OnFoot => PresetCategory::OnFoot,
            Self::Interface
            | Self::GalaxyMap
            | Self::General
            | Self::Camera
            | Self::HoloMe
            | Self::Store
            | Self::Colonisation => PresetCategory::General,
        }
    }
}

/// Section within a context, following the in-game Controls menu. Declared in card order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Category {
    MouseControls,
    FlightRotation,
    FlightThrust,
    AlternateFlight,
    FlightThrottle,
    LandingOverrides,
    FlightMiscellaneous,
    Targeting,
    Weapons,
    Cooling,
    Miscellaneous,
    ModeSwitches,
    FighterOrders,
    Fss,
    Dss,
    Multicrew,
    Driving,
    DrivingThrottle,
    DrivingTargeting,
    DrivingTurret,
    DrivingMiscellaneous,
    DrivingModeSwitches,
    OnFootMovement,
    OnFootCombat,
    OnFootTools,
    OnFootInteraction,
    ItemWheel,
    OnFootSocial,
    OnFootModeSwitches,
    Interface,
    GalaxyMap,
    General,
    Headlook,
    GalnetAudio,
    CameraSuite,
    FreeCamera,
    HoloMe,
    Store,
    Colonisation,
    Other,
}

impl Category {
    pub fn label(self) -> &'static str {
        match self {
            Self::MouseControls => "Mouse controls",
            Self::FlightRotation => "Flight rotation",
            Self::FlightThrust => "Flight thrust",
            Self::AlternateFlight => "Alternate flight controls",
            Self::FlightThrottle => "Flight throttle",
            Self::LandingOverrides => "Flight landing overrides",
            Self::FlightMiscellaneous => "Flight miscellaneous",
            Self::Targeting => "Targeting",
            Self::Weapons => "Weapons",
            Self::Cooling => "Cooling",
            Self::Miscellaneous => "Miscellaneous",
            Self::ModeSwitches => "Mode switches",
            Self::FighterOrders => "Fighter orders",
            Self::Fss => "Full Spectrum System Scanner",
            Self::Dss => "Detailed Surface Scanner",
            Self::Multicrew => "Multicrew",
            Self::Driving => "Driving",
            Self::DrivingThrottle => "Driving throttle",
            Self::DrivingTargeting => "Driving targeting",
            Self::DrivingTurret => "Driving turret",
            Self::DrivingMiscellaneous => "Driving miscellaneous",
            Self::DrivingModeSwitches => "Driving mode switches",
            Self::OnFootMovement => "On foot movement",
            Self::OnFootCombat => "On foot combat",
            Self::OnFootTools => "On foot tools & items",
            Self::OnFootInteraction => "On foot interaction",
            Self::ItemWheel => "Item wheel",
            Self::OnFootSocial => "On foot social",
            Self::OnFootModeSwitches => "On foot mode switches",
            Self::Interface => "Interface",
            Self::GalaxyMap => "Galaxy map",
            Self::General => "General",
            Self::Headlook => "Headlook",
            Self::GalnetAudio => "Galnet audio",
            Self::CameraSuite => "Camera suite",
            Self::FreeCamera => "Free camera",
            Self::HoloMe => "Holo-Me",
            Self::Store => "Store",
            Self::Colonisation => "Settlement placement",
            Self::Other => "Other",
        }
    }
}

/// The game mode in which an action is live. Two actions can only conflict when they are live
/// at the same time; see [`crate::analysis`] for the rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Scope {
    /// Live everywhere (game menu, push-to-talk, HMD reset, Galnet audio).
    General,
    /// UI panel / menu navigation; only live while a panel or menu has focus.
    Interface,
    /// Ship cockpit in normal flight (including fighter orders).
    Ship,
    /// Landing overrides — replace flight controls while the landing gear is down.
    ShipLanding,
    /// Alternate flight axes — replace the normal axes while toggled on.
    ShipAlternate,
    /// Headlook — replaces whatever shares its inputs while headlook is active.
    Headlook,
    Fss,
    Dss,
    GalaxyMap,
    Multicrew,
    Srv,
    /// SRV turret mode.
    SrvTurret,
    OnFoot,
    /// While the on-foot item / emote wheel is open.
    ItemWheel,
    CameraSuite,
    FreeCamera,
    HoloMe,
    Store,
    Colonisation,
}

impl Scope {
    pub fn label(self) -> &'static str {
        match self {
            Self::General => "Everywhere",
            Self::Interface => "UI panels",
            Self::Ship => "Ship",
            Self::ShipLanding => "Ship (landing gear down)",
            Self::ShipAlternate => "Ship (alternate flight controls)",
            Self::Headlook => "Headlook",
            Self::Fss => "FSS",
            Self::Dss => "DSS",
            Self::GalaxyMap => "Galaxy / system map",
            Self::Multicrew => "Multicrew",
            Self::Srv => "SRV",
            Self::SrvTurret => "SRV turret",
            Self::OnFoot => "On foot",
            Self::ItemWheel => "On-foot item wheel",
            Self::CameraSuite => "Camera suite",
            Self::FreeCamera => "Free camera",
            Self::HoloMe => "Holo-Me",
            Self::Store => "Store",
            Self::Colonisation => "Settlement placement",
        }
    }
}

/// One catalogue entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ActionDef {
    /// Element name in the `.binds` file.
    pub id: &'static str,
    /// Friendly name (in-game label where known), without the context (`Primary Fire` exists for
    /// ship, SRV, multicrew and on foot).
    pub name: &'static str,
    pub context: Context,
    pub category: Category,
    /// Present in older presets but no longer written by the current game; ignored by the
    /// conflict check.
    pub legacy: bool,
}

impl ActionDef {
    /// The mode in which this action is live.
    pub fn scope(&self) -> Scope {
        scope_of(self.id, self.context, self.category)
    }

    const fn legacy(self) -> Self {
        Self {
            legacy: true,
            ..self
        }
    }
}

/// Catalogue data for one action id, known or guessed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ActionInfo {
    pub name: Cow<'static, str>,
    pub context: Context,
    pub category: Category,
    pub scope: Scope,
    /// False when the id is not in the catalogue and everything above was guessed.
    pub known: bool,
    pub legacy: bool,
}

/// Looks up an action id in the catalogue.
pub fn lookup(id: &str) -> Option<&'static ActionDef> {
    static INDEX: LazyLock<FxHashMap<&'static str, &'static ActionDef>> =
        LazyLock::new(|| ACTIONS.iter().map(|d| (d.id, d)).collect());
    INDEX.get(id).copied()
}

/// Every catalogued action, in Controls-menu order.
pub fn all_actions() -> &'static [ActionDef] {
    ACTIONS
}

/// Friendly name, context, category and scope for any action id — catalogued or not.
pub fn action_info(id: &str) -> ActionInfo {
    if let Some(def) = lookup(id) {
        return ActionInfo {
            name: Cow::Borrowed(def.name),
            context: def.context,
            category: def.category,
            scope: def.scope(),
            known: true,
            legacy: def.legacy,
        };
    }
    let (context, category) = guess_context(id);
    ActionInfo {
        name: Cow::Owned(fallback_name(id)),
        context,
        category,
        scope: scope_of(id, context, category),
        known: false,
        legacy: false,
    }
}

/// Position of an action in the catalogue (unknown actions sort last).
pub fn catalogue_order(id: &str) -> usize {
    static ORDER: LazyLock<FxHashMap<&'static str, usize>> =
        LazyLock::new(|| ACTIONS.iter().enumerate().map(|(i, d)| (d.id, i)).collect());
    ORDER.get(id).copied().unwrap_or(usize::MAX)
}

fn scope_of(id: &str, context: Context, category: Category) -> Scope {
    match category {
        Category::LandingOverrides => return Scope::ShipLanding,
        // The toggle itself is pressed in normal flight.
        Category::AlternateFlight if id != "UseAlternateFlightValuesToggle" => {
            return Scope::ShipAlternate;
        }
        Category::Headlook => return Scope::Headlook,
        Category::Fss => return Scope::Fss,
        Category::Dss => return Scope::Dss,
        Category::DrivingTurret => return Scope::SrvTurret,
        Category::ItemWheel => return Scope::ItemWheel,
        Category::CameraSuite => return Scope::CameraSuite,
        Category::FreeCamera => return Scope::FreeCamera,
        _ => {}
    }
    match context {
        Context::Ship | Context::Fighter => Scope::Ship,
        Context::Scanners => Scope::Fss,
        Context::Multicrew => Scope::Multicrew,
        Context::Srv => Scope::Srv,
        Context::OnFoot => Scope::OnFoot,
        Context::Interface => Scope::Interface,
        Context::GalaxyMap => Scope::GalaxyMap,
        Context::General => Scope::General,
        Context::Camera => Scope::CameraSuite,
        Context::HoloMe => Scope::HoloMe,
        Context::Store => Scope::Store,
        Context::Colonisation => Scope::Colonisation,
    }
}

/// Guesses context and category of an action the catalogue does not know.
fn guess_context(id: &str) -> (Context, Category) {
    let has = |s: &str| id.contains(s);
    if id.ends_with("_Landing") {
        (Context::Ship, Category::LandingOverrides)
    } else if id.ends_with("_Buggy") || id.starts_with("Buggy") || has("SRV") {
        (Context::Srv, Category::Other)
    } else if id.starts_with("Humanoid") || id.ends_with("_Humanoid") {
        (Context::OnFoot, Category::Other)
    } else if id.starts_with("ExplorationFSS") || id.starts_with("FSS") {
        (Context::Scanners, Category::Fss)
    } else if id.starts_with("ExplorationSAA") || id.starts_with("SAA") {
        (Context::Scanners, Category::Dss)
    } else if id.starts_with("MultiCrew") {
        (Context::Multicrew, Category::Multicrew)
    } else if id.starts_with("CommanderCreator") {
        (Context::HoloMe, Category::HoloMe)
    } else if id.starts_with("Store") {
        (Context::Store, Category::Store)
    } else if id.starts_with("UI_") {
        (Context::Interface, Category::Interface)
    } else if id.starts_with("GalnetAudio") {
        (Context::General, Category::GalnetAudio)
    } else if id.starts_with("HeadLook") {
        (Context::General, Category::Headlook)
    } else if id.starts_with("Cam") || has("GalaxyMapHome") {
        (Context::GalaxyMap, Category::GalaxyMap)
    } else if id.starts_with("Order") {
        (Context::Fighter, Category::FighterOrders)
    } else if has("Placement") || has("Settlement") || has("Colonisation") {
        (Context::Colonisation, Category::Colonisation)
    } else if has("FreeCam") || has("Camera") {
        (Context::Camera, Category::FreeCamera)
    } else if id.ends_with("Alternate") {
        (Context::Ship, Category::AlternateFlight)
    } else {
        (Context::Ship, Category::Other)
    }
}

/// Friendly name for an uncatalogued id: words, minus the `Humanoid` / `Button` / `Buggy` noise
/// that the context already conveys.
fn fallback_name(id: &str) -> String {
    let mut name = humanize(id);
    for suffix in [" Button", " Buggy", " Humanoid"] {
        if let Some(stripped) = name.strip_suffix(suffix)
            && !stripped.is_empty()
        {
            let len = stripped.len();
            name.truncate(len);
        }
    }
    if let Some(stripped) = name.strip_prefix("Humanoid ")
        && !stripped.is_empty()
    {
        name = stripped.to_owned();
    }
    if let Some(stripped) = name.strip_suffix(" Landing") {
        name = format!("{stripped} (Landing)");
    }
    name
}

const fn def(
    id: &'static str,
    name: &'static str,
    context: Context,
    category: Category,
) -> ActionDef {
    ActionDef {
        id,
        name,
        context,
        category,
        legacy: false,
    }
}
const fn ship(id: &'static str, name: &'static str, category: Category) -> ActionDef {
    def(id, name, Context::Ship, category)
}
const fn general(id: &'static str, name: &'static str, category: Category) -> ActionDef {
    def(id, name, Context::General, category)
}
const fn ui(id: &'static str, name: &'static str) -> ActionDef {
    def(id, name, Context::Interface, Category::Interface)
}
const fn galmap(id: &'static str, name: &'static str) -> ActionDef {
    def(id, name, Context::GalaxyMap, Category::GalaxyMap)
}
const fn srv(id: &'static str, name: &'static str, category: Category) -> ActionDef {
    def(id, name, Context::Srv, category)
}
const fn crew(id: &'static str, name: &'static str) -> ActionDef {
    def(id, name, Context::Multicrew, Category::Multicrew)
}
const fn fighter(id: &'static str, name: &'static str) -> ActionDef {
    def(id, name, Context::Fighter, Category::FighterOrders)
}
const fn camera(id: &'static str, name: &'static str, category: Category) -> ActionDef {
    def(id, name, Context::Camera, category)
}
const fn holo(id: &'static str, name: &'static str) -> ActionDef {
    def(id, name, Context::HoloMe, Category::HoloMe)
}
const fn fss(id: &'static str, name: &'static str) -> ActionDef {
    def(id, name, Context::Scanners, Category::Fss)
}
const fn dss(id: &'static str, name: &'static str) -> ActionDef {
    def(id, name, Context::Scanners, Category::Dss)
}
const fn foot(id: &'static str, name: &'static str, category: Category) -> ActionDef {
    def(id, name, Context::OnFoot, category)
}
const fn store(id: &'static str, name: &'static str) -> ActionDef {
    def(id, name, Context::Store, Category::Store)
}
const fn colony(id: &'static str, name: &'static str) -> ActionDef {
    def(id, name, Context::Colonisation, Category::Colonisation)
}

use Category as C;

#[rustfmt::skip]
static ACTIONS: &[ActionDef] = &[
    // ---- Ship: mouse -------------------------------------------------------------------------
    ship("MouseReset", "Reset Mouse", C::MouseControls),
    ship("BlockMouseDecay", "Block Mouse Decay", C::MouseControls),
    // ---- Ship: flight rotation ---------------------------------------------------------------
    ship("YawAxisRaw", "Yaw Axis", C::FlightRotation),
    ship("YawLeftButton", "Yaw Left", C::FlightRotation),
    ship("YawRightButton", "Yaw Right", C::FlightRotation),
    ship("YawToRollButton", "Yaw Into Roll", C::FlightRotation),
    ship("RollAxisRaw", "Roll Axis", C::FlightRotation),
    ship("RollLeftButton", "Roll Left", C::FlightRotation),
    ship("RollRightButton", "Roll Right", C::FlightRotation),
    ship("PitchAxisRaw", "Pitch Axis", C::FlightRotation),
    ship("PitchUpButton", "Pitch Up", C::FlightRotation),
    ship("PitchDownButton", "Pitch Down", C::FlightRotation),
    // ---- Ship: flight thrust -----------------------------------------------------------------
    ship("LateralThrustRaw", "Lateral Thrust Axis", C::FlightThrust),
    ship("LeftThrustButton", "Thrust Left", C::FlightThrust),
    ship("RightThrustButton", "Thrust Right", C::FlightThrust),
    ship("VerticalThrustRaw", "Vertical Thrust Axis", C::FlightThrust),
    ship("UpThrustButton", "Thrust Up", C::FlightThrust),
    ship("DownThrustButton", "Thrust Down", C::FlightThrust),
    ship("AheadThrust", "Forward Thrust Axis", C::FlightThrust),
    ship("ForwardThrustButton", "Thrust Forward", C::FlightThrust),
    ship("BackwardThrustButton", "Thrust Backward", C::FlightThrust),
    // ---- Ship: alternate flight controls ------------------------------------------------------
    ship("UseAlternateFlightValuesToggle", "Alternate Flight Controls", C::AlternateFlight),
    ship("YawAxisAlternate", "Yaw Axis (Alternate)", C::AlternateFlight),
    ship("RollAxisAlternate", "Roll Axis (Alternate)", C::AlternateFlight),
    ship("PitchAxisAlternate", "Pitch Axis (Alternate)", C::AlternateFlight),
    ship("LateralThrustAlternate", "Lateral Thrust Axis (Alternate)", C::AlternateFlight),
    ship("VerticalThrustAlternate", "Vertical Thrust Axis (Alternate)", C::AlternateFlight),
    // ---- Ship: flight throttle ----------------------------------------------------------------
    ship("ThrottleAxis", "Throttle Axis", C::FlightThrottle),
    ship("ToggleReverseThrottleInput", "Toggle Reverse Throttle", C::FlightThrottle),
    ship("ForwardKey", "Increase Throttle", C::FlightThrottle),
    ship("BackwardKey", "Decrease Throttle", C::FlightThrottle),
    ship("SetSpeedMinus100", "Set Speed to -100%", C::FlightThrottle),
    ship("SetSpeedMinus75", "Set Speed to -75%", C::FlightThrottle),
    ship("SetSpeedMinus50", "Set Speed to -50%", C::FlightThrottle),
    ship("SetSpeedMinus25", "Set Speed to -25%", C::FlightThrottle),
    ship("SetSpeedZero", "Set Speed to 0%", C::FlightThrottle),
    ship("SetSpeed25", "Set Speed to 25%", C::FlightThrottle),
    ship("SetSpeed50", "Set Speed to 50%", C::FlightThrottle),
    ship("SetSpeed75", "Set Speed to 75%", C::FlightThrottle),
    ship("SetSpeed100", "Set Speed to 100%", C::FlightThrottle),
    // ---- Ship: landing overrides ----------------------------------------------------------------
    ship("YawAxis_Landing", "Yaw Axis (Landing)", C::LandingOverrides),
    ship("YawLeftButton_Landing", "Yaw Left (Landing)", C::LandingOverrides),
    ship("YawRightButton_Landing", "Yaw Right (Landing)", C::LandingOverrides),
    ship("PitchAxis_Landing", "Pitch Axis (Landing)", C::LandingOverrides),
    ship("PitchUpButton_Landing", "Pitch Up (Landing)", C::LandingOverrides),
    ship("PitchDownButton_Landing", "Pitch Down (Landing)", C::LandingOverrides),
    ship("RollAxis_Landing", "Roll Axis (Landing)", C::LandingOverrides),
    ship("RollLeftButton_Landing", "Roll Left (Landing)", C::LandingOverrides),
    ship("RollRightButton_Landing", "Roll Right (Landing)", C::LandingOverrides),
    ship("LateralThrust_Landing", "Lateral Thrust Axis (Landing)", C::LandingOverrides),
    ship("LeftThrustButton_Landing", "Thrust Left (Landing)", C::LandingOverrides),
    ship("RightThrustButton_Landing", "Thrust Right (Landing)", C::LandingOverrides),
    ship("VerticalThrust_Landing", "Vertical Thrust Axis (Landing)", C::LandingOverrides),
    ship("UpThrustButton_Landing", "Thrust Up (Landing)", C::LandingOverrides),
    ship("DownThrustButton_Landing", "Thrust Down (Landing)", C::LandingOverrides),
    ship("AheadThrust_Landing", "Forward Thrust Axis (Landing)", C::LandingOverrides),
    ship("ForwardThrustButton_Landing", "Thrust Forward (Landing)", C::LandingOverrides),
    ship("BackwardThrustButton_Landing", "Thrust Backward (Landing)", C::LandingOverrides),
    // ---- Ship: flight miscellaneous -----------------------------------------------------------
    ship("ToggleFlightAssist", "Toggle Flight Assist", C::FlightMiscellaneous),
    ship("UseBoostJuice", "Engine Boost", C::FlightMiscellaneous),
    ship("HyperSuperCombination", "Toggle Frame Shift Drive", C::FlightMiscellaneous),
    ship("Supercruise", "Supercruise", C::FlightMiscellaneous),
    ship("Hyperspace", "Hyperspace Jump", C::FlightMiscellaneous),
    ship("DisableRotationCorrectToggle", "Disable Rotational Correction", C::FlightMiscellaneous),
    ship("OrbitLinesToggle", "Orbit Lines", C::FlightMiscellaneous),
    // ---- Ship: targeting -----------------------------------------------------------------------
    ship("SelectTarget", "Select Target Ahead", C::Targeting),
    ship("CycleNextTarget", "Cycle Next Target", C::Targeting),
    ship("CyclePreviousTarget", "Cycle Previous Target", C::Targeting),
    ship("SelectHighestThreat", "Select Highest Threat", C::Targeting),
    ship("CycleNextHostileTarget", "Cycle Next Hostile Target", C::Targeting),
    ship("CyclePreviousHostileTarget", "Cycle Previous Hostile Target", C::Targeting),
    ship("TargetWingman0", "Select Wingman 1", C::Targeting),
    ship("TargetWingman1", "Select Wingman 2", C::Targeting),
    ship("TargetWingman2", "Select Wingman 3", C::Targeting),
    ship("SelectTargetsTarget", "Select Wingman's Target", C::Targeting),
    ship("WingNavLock", "Wingman Nav-Lock", C::Targeting),
    ship("CycleNextSubsystem", "Cycle Next Subsystem", C::Targeting),
    ship("CyclePreviousSubsystem", "Cycle Previous Subsystem", C::Targeting),
    ship("TargetNextRouteSystem", "Target Next System in Route", C::Targeting),
    // ---- Ship: weapons ---------------------------------------------------------------------------
    ship("PrimaryFire", "Primary Fire", C::Weapons),
    ship("SecondaryFire", "Secondary Fire", C::Weapons),
    ship("CycleFireGroupNext", "Cycle Next Fire Group", C::Weapons),
    ship("CycleFireGroupPrevious", "Cycle Previous Fire Group", C::Weapons),
    ship("DeployHardpointToggle", "Deploy Hardpoints", C::Weapons),
    // ---- Ship: cooling ---------------------------------------------------------------------------
    ship("ToggleButtonUpInput", "Silent Running", C::Cooling),
    ship("DeployHeatSink", "Deploy Heat Sink", C::Cooling),
    // ---- Ship: miscellaneous ---------------------------------------------------------------------
    ship("ShipSpotLightToggle", "Ship Lights", C::Miscellaneous),
    ship("RadarRangeAxis", "Sensor Range Axis", C::Miscellaneous),
    ship("RadarIncreaseRange", "Increase Sensor Range", C::Miscellaneous),
    ship("RadarDecreaseRange", "Decrease Sensor Range", C::Miscellaneous),
    ship("IncreaseEnginesPower", "Divert Power to Engines", C::Miscellaneous),
    ship("IncreaseWeaponsPower", "Divert Power to Weapons", C::Miscellaneous),
    ship("IncreaseSystemsPower", "Divert Power to Systems", C::Miscellaneous),
    ship("ResetPowerDistribution", "Balance Power Distribution", C::Miscellaneous),
    general("HMDReset", "Reset HMD Orientation", C::General),
    ship("ToggleCargoScoop", "Cargo Scoop", C::Miscellaneous),
    ship("EjectAllCargo", "Jettison All Cargo", C::Miscellaneous),
    ship("LandingGearToggle", "Landing Gear", C::Miscellaneous),
    general("MicrophoneMute", "Voice Comms Mute / Push to Talk", C::General),
    ship("UseShieldCell", "Use Shield Cell", C::Miscellaneous),
    ship("FireChaffLauncher", "Fire Chaff Launcher", C::Miscellaneous),
    ship("TriggerFieldNeutraliser", "Shutdown Field Neutraliser", C::Miscellaneous),
    ship("ChargeECM", "Charge ECM", C::Miscellaneous),
    ship("WeaponColourToggle", "Cycle Weapon Colour", C::Miscellaneous),
    ship("EngineColourToggle", "Cycle Engine Colour", C::Miscellaneous),
    ship("NightVisionToggle", "Night Vision", C::Miscellaneous),
    ship("TriggerColonisationModule", "Trigger Colonisation Module", C::Miscellaneous),
    ship("PhotoCameraToggle", "Camera Suite", C::Miscellaneous),
    // ---- Ship: mode switches ---------------------------------------------------------------------
    ship("UIFocus", "UI Focus", C::ModeSwitches),
    ship("FocusLeftPanel", "Left Panel (Navigation)", C::ModeSwitches),
    ship("FocusCommsPanel", "Comms Panel", C::ModeSwitches),
    ship("QuickCommsPanel", "Quick Comms", C::ModeSwitches),
    ship("FocusRadarPanel", "Role Panel", C::ModeSwitches),
    ship("FocusRightPanel", "Right Panel (Systems)", C::ModeSwitches),
    ship("GalaxyMapOpen", "Open Galaxy Map", C::ModeSwitches),
    ship("SystemMapOpen", "Open System Map", C::ModeSwitches),
    ship("ShowPGScoreSummaryInput", "Show CQC Scoreboard", C::ModeSwitches),
    ship("HeadLookToggle", "Headlook Toggle", C::ModeSwitches),
    general("Pause", "Game Menu", C::General),
    general("FriendsMenu", "Friends Menu", C::General),
    ship("OpenCodexGoToDiscovery", "Open Codex", C::ModeSwitches),
    ship("PlayerHUDModeToggle", "Switch HUD Mode (Combat / Analysis)", C::ModeSwitches),
    ship("ExplorationFSSEnter", "Enter FSS Mode", C::ModeSwitches),
    // ---- Interface -----------------------------------------------------------------------------
    ui("UI_Up", "UI Up"),
    ui("UI_Down", "UI Down"),
    ui("UI_Left", "UI Left"),
    ui("UI_Right", "UI Right"),
    ui("UI_Select", "UI Select"),
    ui("UI_Back", "UI Back"),
    ui("UI_Toggle", "UI Toggle"),
    ui("CycleNextPanel", "Next Panel Tab"),
    ui("CyclePreviousPanel", "Previous Panel Tab"),
    ui("CycleNextPage", "Next Page"),
    ui("CyclePreviousPage", "Previous Page"),
    // ---- General: headlook -----------------------------------------------------------------------
    general("HeadLookReset", "Reset Headlook", C::Headlook),
    general("HeadLookPitchUp", "Look Up", C::Headlook),
    general("HeadLookPitchDown", "Look Down", C::Headlook),
    general("HeadLookPitchAxisRaw", "Look Up / Down Axis", C::Headlook),
    general("HeadLookYawLeft", "Look Left", C::Headlook),
    general("HeadLookYawRight", "Look Right", C::Headlook),
    general("HeadLookYawAxis", "Look Left / Right Axis", C::Headlook),
    // ---- Galaxy map ------------------------------------------------------------------------------
    galmap("CamPitchAxis", "Camera Pitch Axis"),
    galmap("CamPitchUp", "Camera Pitch Up"),
    galmap("CamPitchDown", "Camera Pitch Down"),
    galmap("CamYawAxis", "Camera Yaw Axis"),
    galmap("CamYawLeft", "Camera Yaw Left"),
    galmap("CamYawRight", "Camera Yaw Right"),
    galmap("CamTranslateYAxis", "Move Forward / Backward Axis"),
    galmap("CamTranslateForward", "Move Forward"),
    galmap("CamTranslateBackward", "Move Backward"),
    galmap("CamTranslateXAxis", "Move Left / Right Axis"),
    galmap("CamTranslateLeft", "Move Left"),
    galmap("CamTranslateRight", "Move Right"),
    galmap("CamTranslateZAxis", "Move Up / Down Axis"),
    galmap("CamTranslateUp", "Move Up"),
    galmap("CamTranslateDown", "Move Down"),
    galmap("CamZoomAxis", "Zoom Axis"),
    galmap("CamZoomIn", "Zoom In"),
    galmap("CamZoomOut", "Zoom Out"),
    galmap("CamTranslateZHold", "Vertical Movement Hold"),
    galmap("GalaxyMapHome", "Return to Current System"),
    // ---- SRV -------------------------------------------------------------------------------------
    srv("ToggleDriveAssist", "Drive Assist", C::Driving),
    srv("SteeringAxis", "Steering Axis", C::Driving),
    srv("SteerLeftButton", "Steer Left", C::Driving),
    srv("SteerRightButton", "Steer Right", C::Driving),
    srv("BuggyRollAxisRaw", "Roll Axis", C::Driving),
    srv("BuggyRollLeftButton", "Roll Left", C::Driving),
    srv("BuggyRollRightButton", "Roll Right", C::Driving),
    srv("BuggyPitchAxis", "Pitch Axis", C::Driving),
    srv("BuggyPitchUpButton", "Pitch Up", C::Driving),
    srv("BuggyPitchDownButton", "Pitch Down", C::Driving),
    srv("VerticalThrustersButton", "Vertical Thrusters (Jump)", C::Driving),
    srv("BuggyPrimaryFireButton", "Primary Fire", C::DrivingTargeting),
    srv("BuggySecondaryFireButton", "Secondary Fire", C::DrivingTargeting),
    srv("AutoBreakBuggyButton", "Handbrake", C::Driving),
    srv("HeadlightsBuggyButton", "Headlights", C::DrivingMiscellaneous),
    srv("ToggleBuggyTurretButton", "Turret Mode", C::DrivingTargeting),
    srv("BuggyCycleFireGroupNext", "Cycle Next Fire Group", C::DrivingTargeting),
    srv("BuggyCycleFireGroupPrevious", "Cycle Previous Fire Group", C::DrivingTargeting),
    srv("SelectTarget_Buggy", "Select Target Ahead", C::DrivingTargeting),
    srv("BuggyTurretYawAxisRaw", "Turret Yaw Axis", C::DrivingTurret),
    srv("BuggyTurretYawLeftButton", "Turret Yaw Left", C::DrivingTurret),
    srv("BuggyTurretYawRightButton", "Turret Yaw Right", C::DrivingTurret),
    srv("BuggyTurretPitchAxisRaw", "Turret Pitch Axis", C::DrivingTurret),
    srv("BuggyTurretPitchUpButton", "Turret Pitch Up", C::DrivingTurret),
    srv("BuggyTurretPitchDownButton", "Turret Pitch Down", C::DrivingTurret),
    srv("DriveSpeedAxis", "Drive Speed Axis", C::DrivingThrottle),
    srv("BuggyToggleReverseThrottleInput", "Toggle Reverse", C::DrivingThrottle),
    srv("IncreaseSpeedButtonMax", "Increase Speed (Max)", C::DrivingThrottle),
    srv("DecreaseSpeedButtonMax", "Decrease Speed (Max)", C::DrivingThrottle),
    srv("IncreaseSpeedButtonPartial", "Increase Speed (Partial)", C::DrivingThrottle),
    srv("DecreaseSpeedButtonPartial", "Decrease Speed (Partial)", C::DrivingThrottle),
    srv("IncreaseEnginesPower_Buggy", "Divert Power to Engines", C::DrivingMiscellaneous),
    srv("IncreaseWeaponsPower_Buggy", "Divert Power to Weapons", C::DrivingMiscellaneous),
    srv("IncreaseSystemsPower_Buggy", "Divert Power to Systems", C::DrivingMiscellaneous),
    srv("ResetPowerDistribution_Buggy", "Balance Power Distribution", C::DrivingMiscellaneous),
    srv("ToggleCargoScoop_Buggy", "Cargo Scoop", C::DrivingMiscellaneous),
    srv("EjectAllCargo_Buggy", "Jettison All Cargo", C::DrivingMiscellaneous),
    srv("RecallDismissShip", "Recall / Dismiss Ship", C::DrivingMiscellaneous),
    srv("PhotoCameraToggle_Buggy", "Camera Suite", C::DrivingMiscellaneous),
    srv("UIFocus_Buggy", "UI Focus", C::DrivingModeSwitches),
    srv("FocusLeftPanel_Buggy", "Left Panel (Navigation)", C::DrivingModeSwitches),
    srv("FocusCommsPanel_Buggy", "Comms Panel", C::DrivingModeSwitches),
    srv("QuickCommsPanel_Buggy", "Quick Comms", C::DrivingModeSwitches),
    srv("FocusRadarPanel_Buggy", "Role Panel", C::DrivingModeSwitches),
    srv("FocusRightPanel_Buggy", "Right Panel (Systems)", C::DrivingModeSwitches),
    srv("GalaxyMapOpen_Buggy", "Open Galaxy Map", C::DrivingModeSwitches),
    srv("SystemMapOpen_Buggy", "Open System Map", C::DrivingModeSwitches),
    srv("OpenCodexGoToDiscovery_Buggy", "Open Codex", C::DrivingModeSwitches),
    srv("PlayerHUDModeToggle_Buggy", "Switch HUD Mode", C::DrivingModeSwitches),
    srv("HeadLookToggle_Buggy", "Headlook Toggle", C::DrivingModeSwitches),
    // ---- Multicrew -------------------------------------------------------------------------------
    crew("MultiCrewToggleMode", "Switch Multicrew Mode"),
    crew("MultiCrewPrimaryFire", "Primary Fire"),
    crew("MultiCrewSecondaryFire", "Secondary Fire"),
    crew("MultiCrewPrimaryUtilityFire", "Primary Utility Fire"),
    crew("MultiCrewSecondaryUtilityFire", "Secondary Utility Fire"),
    crew("MultiCrewThirdPersonYawAxisRaw", "Third Person Yaw Axis"),
    crew("MultiCrewThirdPersonYawLeftButton", "Third Person Yaw Left"),
    crew("MultiCrewThirdPersonYawRightButton", "Third Person Yaw Right"),
    crew("MultiCrewThirdPersonPitchAxisRaw", "Third Person Pitch Axis"),
    crew("MultiCrewThirdPersonPitchUpButton", "Third Person Pitch Up"),
    crew("MultiCrewThirdPersonPitchDownButton", "Third Person Pitch Down"),
    crew("MultiCrewThirdPersonFovAxisRaw", "Third Person Field of View Axis"),
    crew("MultiCrewThirdPersonFovOutButton", "Third Person Field of View Out"),
    crew("MultiCrewThirdPersonFovInButton", "Third Person Field of View In"),
    crew("MultiCrewCockpitUICycleForward", "Cockpit UI Cycle Forward"),
    crew("MultiCrewCockpitUICycleBackward", "Cockpit UI Cycle Backward"),
    // ---- Fighter orders ---------------------------------------------------------------------------
    fighter("OrderRequestDock", "Request Dock"),
    fighter("OrderDefensiveBehaviour", "Defend"),
    fighter("OrderAggressiveBehaviour", "Engage at Will"),
    fighter("OrderFocusTarget", "Attack Target"),
    fighter("OrderHoldFire", "Hold Fire"),
    fighter("OrderHoldPosition", "Hold Position"),
    fighter("OrderFollow", "Follow"),
    fighter("OpenOrders", "Open Fighter Orders"),
    // ---- Camera suite / free camera -----------------------------------------------------------------
    camera("VanityCameraScrollLeft", "Previous Camera", C::CameraSuite),
    camera("VanityCameraScrollRight", "Next Camera", C::CameraSuite),
    camera("ToggleFreeCam", "Free Camera", C::CameraSuite),
    camera("VanityCameraOne", "Camera 1: Cockpit Front", C::CameraSuite),
    camera("VanityCameraTwo", "Camera 2: Cockpit Back", C::CameraSuite),
    camera("VanityCameraThree", "Camera 3: Commander 1", C::CameraSuite),
    camera("VanityCameraFour", "Camera 4: Commander 2", C::CameraSuite),
    camera("VanityCameraFive", "Camera 5: Co-pilot 1", C::CameraSuite),
    camera("VanityCameraSix", "Camera 6: Co-pilot 2", C::CameraSuite),
    camera("VanityCameraSeven", "Camera 7: Front", C::CameraSuite),
    camera("VanityCameraEight", "Camera 8: Back", C::CameraSuite),
    camera("VanityCameraNine", "Camera 9: Low", C::CameraSuite),
    camera("VanityCameraTen", "Camera 10", C::CameraSuite),
    camera("FreeCamToggleHUD", "Toggle HUD", C::FreeCamera),
    camera("FreeCamSpeedInc", "Increase Speed", C::FreeCamera),
    camera("FreeCamSpeedDec", "Decrease Speed", C::FreeCamera),
    camera("MoveFreeCamY", "Move Forward / Backward Axis", C::FreeCamera),
    camera("ToggleReverseThrottleInputFreeCam", "Toggle Reverse", C::FreeCamera),
    camera("MoveFreeCamForward", "Move Forward", C::FreeCamera),
    camera("MoveFreeCamBackwards", "Move Backward", C::FreeCamera),
    camera("MoveFreeCamX", "Move Left / Right Axis", C::FreeCamera),
    camera("MoveFreeCamRight", "Move Right", C::FreeCamera),
    camera("MoveFreeCamLeft", "Move Left", C::FreeCamera),
    camera("MoveFreeCamZ", "Move Up / Down Axis", C::FreeCamera),
    camera("MoveFreeCamUpAxis", "Move Up Axis", C::FreeCamera),
    camera("MoveFreeCamDownAxis", "Move Down Axis", C::FreeCamera),
    camera("MoveFreeCamUp", "Move Up", C::FreeCamera),
    camera("MoveFreeCamDown", "Move Down", C::FreeCamera),
    camera("PitchCamera", "Pitch Axis", C::FreeCamera),
    camera("PitchCameraUp", "Pitch Up", C::FreeCamera),
    camera("PitchCameraDown", "Pitch Down", C::FreeCamera),
    camera("YawCamera", "Yaw Axis", C::FreeCamera),
    camera("YawCameraLeft", "Yaw Left", C::FreeCamera),
    camera("YawCameraRight", "Yaw Right", C::FreeCamera),
    camera("RollCamera", "Roll Axis", C::FreeCamera),
    camera("RollCameraLeft", "Roll Left", C::FreeCamera),
    camera("RollCameraRight", "Roll Right", C::FreeCamera),
    camera("ToggleRotationLock", "Rotation Lock", C::FreeCamera),
    camera("FixCameraRelativeToggle", "Lock to Vehicle", C::FreeCamera),
    camera("FixCameraWorldToggle", "Lock to World", C::FreeCamera),
    camera("QuitCamera", "Exit Camera", C::FreeCamera),
    camera("ToggleAdvanceMode", "Advanced Mode", C::FreeCamera),
    camera("FreeCamZoomIn", "Zoom In", C::FreeCamera),
    camera("FreeCamZoomOut", "Zoom Out", C::FreeCamera),
    camera("FStopDec", "Decrease F-Stop", C::FreeCamera),
    camera("FStopInc", "Increase F-Stop", C::FreeCamera),
    // ---- Holo-Me -----------------------------------------------------------------------------------
    holo("CommanderCreator_Undo", "Undo"),
    holo("CommanderCreator_Redo", "Redo"),
    holo("CommanderCreator_Rotation_MouseToggle", "Toggle Mouse Rotation"),
    holo("CommanderCreator_Rotation", "Rotate Axis"),
    // ---- General: Galnet audio ----------------------------------------------------------------------
    general("GalnetAudio_Play_Pause", "Galnet Audio Play / Pause", C::GalnetAudio),
    general("GalnetAudio_SkipForward", "Galnet Audio Skip Forward", C::GalnetAudio),
    general("GalnetAudio_SkipBackward", "Galnet Audio Skip Backward", C::GalnetAudio),
    general("GalnetAudio_ClearQueue", "Galnet Audio Clear Queue", C::GalnetAudio),
    // ---- Scanners: FSS -------------------------------------------------------------------------------
    fss("ExplorationFSSCameraPitch", "FSS Pitch Axis"),
    fss("ExplorationFSSCameraPitchIncreaseButton", "FSS Pitch Up"),
    fss("ExplorationFSSCameraPitchDecreaseButton", "FSS Pitch Down"),
    fss("ExplorationFSSCameraYaw", "FSS Yaw Axis"),
    fss("ExplorationFSSCameraYawIncreaseButton", "FSS Yaw Right"),
    fss("ExplorationFSSCameraYawDecreaseButton", "FSS Yaw Left"),
    fss("ExplorationFSSZoomIn", "FSS Zoom In"),
    fss("ExplorationFSSZoomOut", "FSS Zoom Out"),
    fss("ExplorationFSSMiniZoomIn", "FSS Step Zoom In"),
    fss("ExplorationFSSMiniZoomOut", "FSS Step Zoom Out"),
    fss("ExplorationFSSRadioTuningX_Raw", "FSS Tuning Axis"),
    fss("ExplorationFSSRadioTuningX_Increase", "FSS Tune Right"),
    fss("ExplorationFSSRadioTuningX_Decrease", "FSS Tune Left"),
    fss("ExplorationFSSRadioTuningAbsoluteX", "FSS Absolute Tuning Axis"),
    fss("ExplorationFSSDiscoveryScan", "Discovery Scan (Honk)"),
    fss("ExplorationFSSQuit", "Leave FSS"),
    fss("ExplorationFSSTarget", "FSS Target Signal"),
    fss("ExplorationFSSShowHelp", "FSS Help"),
    // ---- Scanners: DSS -------------------------------------------------------------------------------
    dss("ExplorationSAAChangeScannedAreaViewToggle", "Toggle Planet Front / Back View"),
    dss("ExplorationSAAExitThirdPerson", "Exit DSS"),
    dss("ExplorationSAANextGenus", "Next Genus"),
    dss("ExplorationSAAPreviousGenus", "Previous Genus"),
    dss("SAAThirdPersonYawAxisRaw", "DSS Yaw Axis"),
    dss("SAAThirdPersonYawLeftButton", "DSS Yaw Left"),
    dss("SAAThirdPersonYawRightButton", "DSS Yaw Right"),
    dss("SAAThirdPersonPitchAxisRaw", "DSS Pitch Axis"),
    dss("SAAThirdPersonPitchUpButton", "DSS Pitch Up"),
    dss("SAAThirdPersonPitchDownButton", "DSS Pitch Down"),
    dss("SAAThirdPersonFovAxisRaw", "DSS Field of View Axis"),
    dss("SAAThirdPersonFovOutButton", "DSS Field of View Out"),
    dss("SAAThirdPersonFovInButton", "DSS Field of View In"),
    // ---- On foot -------------------------------------------------------------------------------------
    foot("HumanoidForwardAxis", "Move Forward / Backward Axis", C::OnFootMovement),
    foot("HumanoidForwardButton", "Move Forward", C::OnFootMovement),
    foot("HumanoidBackwardButton", "Move Backward", C::OnFootMovement),
    foot("HumanoidStrafeAxis", "Strafe Axis", C::OnFootMovement),
    foot("HumanoidStrafeLeftButton", "Strafe Left", C::OnFootMovement),
    foot("HumanoidStrafeRightButton", "Strafe Right", C::OnFootMovement),
    foot("HumanoidRotateAxis", "Look Left / Right Axis", C::OnFootMovement),
    foot("HumanoidRotateLeftButton", "Turn Left", C::OnFootMovement),
    foot("HumanoidRotateRightButton", "Turn Right", C::OnFootMovement),
    foot("HumanoidPitchAxis", "Look Up / Down Axis", C::OnFootMovement),
    foot("HumanoidPitchUpButton", "Look Up", C::OnFootMovement),
    foot("HumanoidPitchDownButton", "Look Down", C::OnFootMovement),
    foot("HumanoidSprintButton", "Sprint", C::OnFootMovement),
    foot("HumanoidWalkButton", "Walk", C::OnFootMovement),
    foot("HumanoidCrouchButton", "Crouch", C::OnFootMovement),
    foot("HumanoidJumpButton", "Jump", C::OnFootMovement),
    foot("HumanoidPrimaryInteractButton", "Interact", C::OnFootInteraction),
    foot("HumanoidSecondaryInteractButton", "Secondary Interact", C::OnFootInteraction),
    foot("HumanoidItemWheelButton", "Item Wheel", C::OnFootTools),
    foot("HumanoidEmoteWheelButton", "Emote Wheel", C::OnFootSocial),
    foot("HumanoidUtilityWheelCycleMode", "Cycle Wheel Mode", C::ItemWheel),
    foot("HumanoidItemWheelButton_XAxis", "Wheel Left / Right Axis", C::ItemWheel),
    foot("HumanoidItemWheelButton_XLeft", "Wheel Left", C::ItemWheel),
    foot("HumanoidItemWheelButton_XRight", "Wheel Right", C::ItemWheel),
    foot("HumanoidItemWheelButton_YAxis", "Wheel Up / Down Axis", C::ItemWheel),
    foot("HumanoidItemWheelButton_YUp", "Wheel Up", C::ItemWheel),
    foot("HumanoidItemWheelButton_YDown", "Wheel Down", C::ItemWheel),
    foot("HumanoidPrimaryFireButton", "Primary Fire", C::OnFootCombat),
    foot("HumanoidZoomButton", "Zoom / Aim", C::OnFootCombat),
    foot("HumanoidThrowGrenadeButton", "Throw Grenade", C::OnFootCombat),
    foot("HumanoidMeleeButton", "Melee", C::OnFootCombat),
    foot("HumanoidReloadButton", "Reload", C::OnFootCombat),
    foot("HumanoidSwitchWeapon", "Switch Weapon", C::OnFootCombat),
    foot("HumanoidSelectPrimaryWeaponButton", "Select Primary Weapon", C::OnFootCombat),
    foot("HumanoidSelectSecondaryWeaponButton", "Select Secondary Weapon", C::OnFootCombat),
    foot("HumanoidSelectUtilityWeaponButton", "Select Utility Weapon", C::OnFootCombat),
    foot("HumanoidSelectNextWeaponButton", "Next Weapon", C::OnFootCombat),
    foot("HumanoidSelectPreviousWeaponButton", "Previous Weapon", C::OnFootCombat),
    foot("HumanoidHideWeaponButton", "Holster Weapon", C::OnFootCombat),
    foot("HumanoidSelectNextGrenadeTypeButton", "Next Grenade Type", C::OnFootCombat),
    foot("HumanoidSelectPreviousGrenadeTypeButton", "Previous Grenade Type", C::OnFootCombat),
    foot("HumanoidToggleFlashlightButton", "Flashlight", C::OnFootTools),
    foot("HumanoidToggleNightVisionButton", "Night Vision", C::OnFootTools),
    foot("HumanoidToggleShieldsButton", "Toggle Shields", C::OnFootTools),
    foot("HumanoidClearAuthorityLevel", "Clear Authority Level", C::OnFootTools),
    foot("HumanoidHealthPack", "Use Medkit", C::OnFootTools),
    foot("HumanoidBattery", "Use Energy Cell", C::OnFootTools),
    foot("HumanoidSelectFragGrenade", "Select Frag Grenade", C::OnFootCombat),
    foot("HumanoidSelectEMPGrenade", "Select Shield Disruptor Grenade", C::OnFootCombat),
    foot("HumanoidSelectShieldGrenade", "Select Shield Projector Grenade", C::OnFootCombat),
    foot("HumanoidSwitchToRechargeTool", "Switch to Energylink", C::OnFootTools),
    foot("HumanoidSwitchToCompAnalyser", "Switch to Profile Analyser", C::OnFootTools),
    foot("HumanoidSwitchToSuitTool", "Switch to Suit Tool", C::OnFootTools),
    foot("HumanoidToggleToolModeButton", "Toggle Tool Mode", C::OnFootTools),
    foot("HumanoidToggleMissionHelpPanelButton", "Toggle Mission Help Panel", C::OnFootInteraction),
    foot("HumanoidPing", "Ping", C::OnFootSocial),
    foot("GalaxyMapOpen_Humanoid", "Open Galaxy Map", C::OnFootModeSwitches),
    foot("SystemMapOpen_Humanoid", "Open System Map", C::OnFootModeSwitches),
    foot("FocusCommsPanel_Humanoid", "Comms Panel", C::OnFootModeSwitches),
    foot("QuickCommsPanel_Humanoid", "Quick Comms", C::OnFootModeSwitches),
    foot("HumanoidOpenAccessPanelButton", "Access Panel", C::OnFootModeSwitches),
    foot("HumanoidConflictContextualUIButton", "Conflict Zone Context Menu", C::OnFootInteraction),
    foot("PhotoCameraToggle_Humanoid", "Camera Suite", C::OnFootModeSwitches),
    // ---- Store ---------------------------------------------------------------------------------------
    store("StoreEnableRotation", "Enable Rotation"),
    store("StorePitchCamera", "Pitch Axis"),
    store("StoreYawCamera", "Yaw Axis"),
    store("StoreCamZoomIn", "Zoom In"),
    store("StoreCamZoomOut", "Zoom Out"),
    store("StoreToggle", "Toggle Preview"),
    // ---- On foot: emotes -----------------------------------------------------------------------------
    foot("HumanoidEmoteSlot1", "Emote 1", C::OnFootSocial),
    foot("HumanoidEmoteSlot2", "Emote 2", C::OnFootSocial),
    foot("HumanoidEmoteSlot3", "Emote 3", C::OnFootSocial),
    foot("HumanoidEmoteSlot4", "Emote 4", C::OnFootSocial),
    foot("HumanoidEmoteSlot5", "Emote 5", C::OnFootSocial),
    foot("HumanoidEmoteSlot6", "Emote 6", C::OnFootSocial),
    foot("HumanoidEmoteSlot7", "Emote 7", C::OnFootSocial),
    foot("HumanoidEmoteSlot8", "Emote 8", C::OnFootSocial),
    // ---- Colonisation: settlement placement ------------------------------------------------------------
    colony("PlaceSettlement", "Place Settlement"),
    colony("ChangeConstructionOption", "Change Construction Option"),
    colony("RotateSettlement", "Rotate Axis"),
    colony("RotateSettlementLeft", "Rotate Left"),
    colony("RotateSettlementRight", "Rotate Right"),
    colony("ExitSettlementPlacementCamera", "Exit Placement Camera"),
    colony("MovePlacementCamY", "Camera Forward / Backward Axis"),
    colony("MovePlacementCamForward", "Camera Forward"),
    colony("MovePlacementCamBackwards", "Camera Backward"),
    colony("MovePlacementCamX", "Camera Left / Right Axis"),
    colony("MovePlacementCamRight", "Camera Right"),
    colony("MovePlacementCamLeft", "Camera Left"),
    colony("MovePlacementCamZ", "Camera Up / Down Axis"),
    colony("MovePlacementCamUp", "Camera Up"),
    colony("MovePlacementCamDown", "Camera Down"),
    colony("MovePlacementCamUpAxis", "Camera Up Axis"),
    colony("MovePlacementCamDownAxis", "Camera Down Axis"),
    colony("PitchPlacementCamera", "Camera Pitch Axis"),
    colony("PitchPlacementCameraUp", "Camera Pitch Up"),
    colony("PitchPlacementCameraDown", "Camera Pitch Down"),
    colony("YawPlacementCamera", "Camera Yaw Axis"),
    colony("YawPlacementCameraRight", "Camera Yaw Right"),
    colony("YawPlacementCameraLeft", "Camera Yaw Left"),
    colony("PlacementCamSpeedInc", "Increase Camera Speed"),
    colony("PlacementCamSpeedDec", "Decrease Camera Speed"),
    // ---- Legacy: present in older presets, no longer written by the game -----------------------------
    ship("ToggleVanityCamera", "Vanity Camera", C::Miscellaneous).legacy(),
    srv("ToggleVanityCamera_Buggy", "Vanity Camera", C::DrivingMiscellaneous).legacy(),
    camera("FocusDistanceInc", "Increase Focus Distance", C::FreeCamera).legacy(),
    camera("FocusDistanceDec", "Decrease Focus Distance", C::FreeCamera).legacy(),
    store("StorePitchCameraUp", "Pitch Up").legacy(),
    store("StorePitchCameraDown", "Pitch Down").legacy(),
    store("StoreYawCameraLeft", "Yaw Left").legacy(),
    store("StoreYawCameraRight", "Yaw Right").legacy(),
    general("OculusReset", "Reset Oculus Orientation", C::General).legacy(),
    foot("HumanoidActivateSuitAbilityButton", "Activate Suit Ability", C::OnFootTools).legacy(),
    foot("HumanoidConflictContextualUI", "Conflict Zone Context Menu", C::OnFootInteraction).legacy(),
    foot("HumanoidPulseScanButton", "Pulse Scan", C::OnFootTools).legacy(),
    foot("HumanoidToggleJumpAssistModuleButton", "Jump Assist Module", C::OnFootTools).legacy(),
    foot("HumanoidToggleShieldBoosterModuleButton", "Shield Booster Module", C::OnFootTools).legacy(),
    foot("HumanoidToggleWeaponBoosterModuleButton", "Weapon Booster Module", C::OnFootTools).legacy(),
    foot("HumanoidIncreaseEnginesPowerButton", "Divert Power to Engines", C::OnFootTools).legacy(),
    foot("HumanoidIncreaseSystemsPowerButton", "Divert Power to Systems", C::OnFootTools).legacy(),
    foot("HumanoidIncreaseWeaponsPowerButton", "Divert Power to Weapons", C::OnFootTools).legacy(),
    foot("HumanoidResetPowerDistributionButton", "Balance Power Distribution", C::OnFootTools).legacy(),
];

#[cfg(test)]
mod tests {
    use super::*;
    use rustc_hash::FxHashSet;

    #[test]
    fn ids_are_unique_and_names_present() {
        let mut seen = FxHashSet::default();
        for d in ACTIONS {
            assert!(seen.insert(d.id), "duplicate catalogue id {}", d.id);
            assert!(!d.name.is_empty(), "{} has no name", d.id);
        }
        assert!(ACTIONS.len() > 400);
    }

    #[test]
    fn categories_match_their_context() {
        for d in ACTIONS {
            let ok = match d.category {
                C::Driving
                | C::DrivingThrottle
                | C::DrivingTargeting
                | C::DrivingTurret
                | C::DrivingMiscellaneous
                | C::DrivingModeSwitches => d.context == Context::Srv,
                C::OnFootMovement
                | C::OnFootCombat
                | C::OnFootTools
                | C::OnFootInteraction
                | C::ItemWheel
                | C::OnFootSocial
                | C::OnFootModeSwitches => d.context == Context::OnFoot,
                C::Fss | C::Dss => d.context == Context::Scanners,
                C::CameraSuite | C::FreeCamera => d.context == Context::Camera,
                C::Interface => d.context == Context::Interface,
                C::GalaxyMap => d.context == Context::GalaxyMap,
                C::Headlook | C::GalnetAudio | C::General => d.context == Context::General,
                _ => true,
            };
            assert!(
                ok,
                "{} has category {:?} outside its context {:?}",
                d.id, d.category, d.context
            );
            // Context-specific actions are named after their element suffix.
            if d.id.ends_with("_Buggy") {
                assert_eq!(d.context, Context::Srv, "{}", d.id);
            }
            if d.id.ends_with("_Humanoid") || d.id.starts_with("Humanoid") {
                assert_eq!(d.context, Context::OnFoot, "{}", d.id);
            }
        }
    }

    #[test]
    fn known_lookups() {
        let fsd = action_info("HyperSuperCombination");
        assert!(fsd.known);
        assert_eq!(fsd.name, "Toggle Frame Shift Drive");
        assert_eq!(fsd.context, Context::Ship);
        assert_eq!(fsd.category, Category::FlightMiscellaneous);
        assert_eq!(fsd.scope, Scope::Ship);
        assert_eq!(action_info("UI_Select").scope, Scope::Interface);
        assert_eq!(
            action_info("YawLeftButton_Landing").scope,
            Scope::ShipLanding
        );
        assert_eq!(action_info("YawAxisAlternate").scope, Scope::ShipAlternate);
        assert_eq!(
            action_info("UseAlternateFlightValuesToggle").scope,
            Scope::Ship
        );
        assert_eq!(action_info("HeadLookPitchUp").scope, Scope::Headlook);
        assert_eq!(
            action_info("BuggyTurretYawLeftButton").scope,
            Scope::SrvTurret
        );
        assert_eq!(
            action_info("HumanoidItemWheelButton_XLeft").scope,
            Scope::ItemWheel
        );
        assert_eq!(action_info("OrderFollow").scope, Scope::Ship);
        assert_eq!(action_info("Pause").scope, Scope::General);
        assert_eq!(action_info("ExplorationFSSQuit").scope, Scope::Fss);
        assert_eq!(action_info("ExplorationFSSEnter").scope, Scope::Ship);
        assert!(action_info("ToggleVanityCamera").legacy);
        assert!(catalogue_order("YawAxisRaw") < catalogue_order("UI_Select"));
        assert_eq!(catalogue_order("Unknown"), usize::MAX);
    }

    #[test]
    fn unknown_actions_get_guessed_context_and_name() {
        let srv = action_info("FrobnicateWidget_Buggy");
        assert!(!srv.known);
        assert_eq!(srv.context, Context::Srv);
        assert_eq!(srv.name, "Frobnicate Widget");
        let foot = action_info("HumanoidDanceButton");
        assert_eq!(foot.context, Context::OnFoot);
        assert_eq!(foot.name, "Dance");
        assert_eq!(foot.scope, Scope::OnFoot);
        let fss = action_info("ExplorationFSSWobble");
        assert_eq!((fss.context, fss.scope), (Context::Scanners, Scope::Fss));
        let landing = action_info("StrafeButton_Landing");
        assert_eq!(landing.scope, Scope::ShipLanding);
        assert_eq!(landing.name, "Strafe Button (Landing)");
        let ship = action_info("DoSomethingNew");
        assert_eq!(
            (ship.context, ship.category),
            (Context::Ship, Category::Other)
        );
        assert_eq!(action_info("UI_Wiggle").context, Context::Interface);
        assert_eq!(action_info("CamOrbit").context, Context::GalaxyMap);
    }

    #[test]
    fn preset_categories() {
        assert_eq!(Context::Ship.preset_category(), PresetCategory::Ship);
        assert_eq!(Context::Scanners.preset_category(), PresetCategory::Ship);
        assert_eq!(Context::Srv.preset_category(), PresetCategory::Srv);
        assert_eq!(Context::OnFoot.preset_category(), PresetCategory::OnFoot);
        assert_eq!(
            Context::Interface.preset_category(),
            PresetCategory::General
        );
    }
}
