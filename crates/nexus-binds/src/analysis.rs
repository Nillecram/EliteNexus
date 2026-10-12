//! Binding analysis: conflicts, unbound essentials, modifier pitfalls and per-device statistics.
//!
//! # Conflict rules
//!
//! Two actions conflict when the **same input** — device, device index, key (including the
//! `Pos_`/`Neg_` half of an axis), the *set* of modifiers, and the hold flag — is bound to both
//! **and both can be live at the same time**:
//!
//! 1. Every action has a [`Scope`], the game mode in which it is live (from the
//!    [catalogue](crate::catalogue)). Only actions with the same scope are compared. This is what
//!    lets UI navigation share WASD with flight (UI keys are only live while a panel has focus),
//!    galaxy-map, FSS, DSS, camera, Holo-Me, store and settlement-placement controls reuse flight
//!    keys, and the SRV and on-foot presets reuse ship keys.
//! 2. Mode overlays get their own scope because they *replace* the controls they share inputs
//!    with while active: landing overrides (gear down), alternate flight axes (toggled), headlook,
//!    the SRV turret and the on-foot item wheel.
//! 3. [`Scope::General`] actions (game menu, push-to-talk, HMD reset, Galnet audio) are live
//!    everywhere, so they are also compared with ship, SRV, on-foot and multicrew actions. Two
//!    general actions sharing an input are reported once, under "Everywhere".
//! 4. Pairs that Frontier's own default presets deliberately put on one input are accepted:
//!    SRV steering vs. roll (ground vs. air), SRV secondary fire vs. target ahead, subsystem
//!    cycling vs. the colonisation module trigger, on-foot interact / reload / tool mode /
//!    mission help / conflict-zone menu (context dependent), crouch vs. secondary interact, item
//!    wheel vs. emote wheel (both hold), and the multicrew utility-fire / UI-cycle pairs.
//! 5. Legacy actions the current game no longer reads are ignored, as is an action sharing an
//!    input with itself (primary and secondary identical).
//!
//! `A` and `LShift + A` are different inputs, as are a tap and a hold of the same key. A key
//! that is used as a modifier *and* bound on its own in the same scope is listed separately as a
//! [`ModifierNote`]. That is informational, not an error — Frontier's own presets do it a lot,
//! especially for gamepads — but it is the first place to look when one press seems to trigger
//! two things.
//!
//! # Essential bindings
//!
//! A curated list of [requirements](REQUIREMENTS) per context, each satisfied by any one of
//! several alternatives (e.g. *Frame Shift Drive*: `HyperSuperCombination`, or both `Supercruise`
//! and `Hyperspace`; *Pitch*: the pitch axis, both pitch buttons, or a mouse axis set to pitch).
//! [`Importance::Essential`] ones block normal play; [`Importance::Recommended`] ones are merely
//! very useful.

use std::cmp::Ordering;

use rustc_hash::{FxHashMap, FxHashSet};
use serde::Serialize;

use crate::bindings::{ActionBinding, Bindings, Slot};
use crate::catalogue::{Context, Scope, action_info, catalogue_order, lookup};
use crate::input::{DeviceKind, Input, device_name, key_label};
use crate::util::natural_cmp;

/// Result of [`analyze`].
#[derive(Debug, Clone, Default, Serialize)]
pub struct Report {
    /// Inputs bound to several actions that are live at the same time.
    pub conflicts: Vec<Conflict>,
    /// Unsatisfied requirements, essentials first.
    pub missing: Vec<MissingBinding>,
    /// Keys used both as a modifier and on their own in the same scope (informational).
    pub modifier_notes: Vec<ModifierNote>,
    /// Usage per physical device, keyboard and mouse first.
    pub devices: Vec<DeviceSummary>,
    pub totals: Totals,
}

impl Report {
    /// Unsatisfied requirements of [`Importance::Essential`].
    pub fn missing_essentials(&self) -> impl Iterator<Item = &MissingBinding> + '_ {
        self.missing
            .iter()
            .filter(|m| m.importance == Importance::Essential)
    }

    /// `(action, slot)` pairs that take part in a conflict.
    pub fn conflicted_slots(&self) -> FxHashSet<(&str, Slot)> {
        self.conflicts
            .iter()
            .flat_map(|c| c.actions.iter().map(|a| (a.action.as_str(), a.slot)))
            .collect()
    }

    /// True when the input in `slot` of `action` takes part in a conflict.
    pub fn is_conflicted(&self, action: &str, slot: Slot) -> bool {
        self.conflicts.iter().any(|c| {
            c.actions
                .iter()
                .any(|a| a.action == action && a.slot == slot)
        })
    }
}

/// One input bound to several simultaneously live actions.
#[derive(Debug, Clone, Serialize)]
pub struct Conflict {
    /// The shared input (as written for the first action).
    pub input: Input,
    /// The mode in which the actions collide.
    pub scope: Scope,
    /// Two or more actions, in catalogue order.
    pub actions: Vec<ConflictAction>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConflictAction {
    pub action: String,
    pub name: String,
    pub context: Context,
    /// Which slot of the action holds the shared input.
    pub slot: Slot,
}

/// How much an unbound requirement matters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub enum Importance {
    /// Needed for normal play (flying, jumping, docking, landing, basic UI and on-foot use).
    Essential,
    /// Commonly needed; the game is playable without it.
    Recommended,
}

impl Importance {
    pub fn label(self) -> &'static str {
        match self {
            Self::Essential => "Essential",
            Self::Recommended => "Recommended",
        }
    }
}

/// An unsatisfied requirement.
#[derive(Debug, Clone, Serialize)]
pub struct MissingBinding {
    /// Stable id such as `ship.fsd`.
    pub id: &'static str,
    pub label: &'static str,
    pub context: Context,
    pub importance: Importance,
    /// Human-readable ways to satisfy it, e.g. `Toggle Frame Shift Drive` or
    /// `Supercruise + Hyperspace Jump`.
    pub options: Vec<String>,
    /// Every action id that appears in an option.
    pub actions: Vec<&'static str>,
}

/// A key used as a modifier by some actions and bound on its own to others in the same scope.
#[derive(Debug, Clone, Serialize)]
pub struct ModifierNote {
    pub scope: Scope,
    pub device: String,
    pub key: String,
    /// Display label of the key, e.g. `LCtrl`.
    pub label: String,
    /// Actions whose binding uses the key as a modifier.
    pub as_modifier: Vec<String>,
    /// Actions bound to the key without modifiers.
    pub alone: Vec<String>,
}

/// Usage statistics for one physical device.
#[derive(Debug, Clone, Serialize)]
pub struct DeviceSummary {
    pub device: String,
    pub device_index: u32,
    /// Display name, with `#n` for the 2nd+ identical device.
    pub name: String,
    pub kind: DeviceKind,
    /// Bound primary/secondary slots whose main key is on this device.
    pub buttons: usize,
    /// Bound axis slots on this device.
    pub axes: usize,
    /// Times a key of this device is used as a modifier.
    pub modifier_uses: usize,
    /// Distinct keys of this device in use (main keys and modifiers).
    pub inputs: usize,
    /// Distinct actions with a main input on this device.
    pub actions: usize,
    /// Conflicts whose shared input is on this device.
    pub conflicts: usize,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Totals {
    /// Action elements in the file.
    pub actions: usize,
    /// Actions with at least one input.
    pub bound_actions: usize,
    /// Bound slots (primary + secondary + axis).
    pub bindings: usize,
    /// Action ids the catalogue does not know (names and contexts were guessed).
    pub unknown_actions: Vec<String>,
}

/// Analyses a set of bindings. See the [module docs](self) for the rules.
pub fn analyze(bindings: &Bindings) -> Report {
    let records = collect_records(bindings);
    let conflicts = find_conflicts(&records);
    let modifier_notes = find_modifier_notes(&records);
    let missing = find_missing(bindings);
    let devices = summarize_devices(&records, &conflicts);
    let totals = Totals {
        actions: bindings.actions.len(),
        bound_actions: bindings.bound_actions().count(),
        bindings: records.len(),
        unknown_actions: bindings
            .actions
            .iter()
            .filter(|a| lookup(&a.action).is_none())
            .map(|a| a.action.clone())
            .collect(),
    };
    Report {
        conflicts,
        missing,
        modifier_notes,
        devices,
        totals,
    }
}

/// One bound slot.
struct Record<'a> {
    action: &'a ActionBinding,
    slot: Slot,
    input: &'a Input,
    scope: Scope,
    legacy: bool,
    order: usize,
}

fn collect_records(bindings: &Bindings) -> Vec<Record<'_>> {
    let mut out = Vec::with_capacity(bindings.actions.len());
    for action in &bindings.actions {
        if !action.is_bound() {
            continue;
        }
        let info = action_info(&action.action);
        let order = catalogue_order(&action.action);
        for (slot, input) in action.inputs() {
            out.push(Record {
                action,
                slot,
                input,
                scope: info.scope,
                legacy: info.legacy,
                order,
            });
        }
    }
    out
}

/// Identity of an input for conflict purposes (modifier order does not matter).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct InputKey<'a> {
    device: &'a str,
    index: u32,
    key: &'a str,
    modifiers: Vec<(&'a str, &'a str)>,
    hold: bool,
}

impl<'a> InputKey<'a> {
    fn of(input: &'a Input) -> Self {
        let mut modifiers: Vec<(&str, &str)> = input
            .modifiers
            .iter()
            .map(|(d, k)| (d.as_str(), k.as_str()))
            .collect();
        modifiers.sort_unstable();
        modifiers.dedup();
        Self {
            device: &input.device,
            index: input.device_index,
            key: &input.key,
            modifiers,
            hold: input.hold,
        }
    }
}

/// Scopes whose actions are compared with general (live-everywhere) actions.
const VEHICLE_SCOPES: [Scope; 4] = [Scope::Ship, Scope::Srv, Scope::OnFoot, Scope::Multicrew];

/// The scopes an action of `scope` is checked in.
fn domains(scope: Scope) -> impl Iterator<Item = Scope> {
    let general = scope == Scope::General;
    std::iter::once(scope).chain(VEHICLE_SCOPES.into_iter().filter(move |_| general))
}

/// Groups of actions that Frontier's own presets bind to one input on purpose; any two members
/// of a group may share an input.
const SHARED_BY_DESIGN: &[&[&str]] = &[
    &["SteeringAxis", "BuggyRollAxisRaw"],
    &["SteerLeftButton", "BuggyRollLeftButton"],
    &["SteerRightButton", "BuggyRollRightButton"],
    &["BuggySecondaryFireButton", "SelectTarget_Buggy"],
    &["CycleNextSubsystem", "TriggerColonisationModule"],
    &[
        "HumanoidPrimaryInteractButton",
        "HumanoidReloadButton",
        "HumanoidToggleToolModeButton",
        "HumanoidToggleMissionHelpPanelButton",
        "HumanoidConflictContextualUIButton",
    ],
    &["HumanoidCrouchButton", "HumanoidSecondaryInteractButton"],
    &["HumanoidItemWheelButton", "HumanoidEmoteWheelButton"],
    &[
        "MultiCrewPrimaryUtilityFire",
        "MultiCrewCockpitUICycleForward",
    ],
    &[
        "MultiCrewSecondaryUtilityFire",
        "MultiCrewCockpitUICycleBackward",
    ],
];

/// True when `a` and `b` may share an input by design.
pub fn shared_by_design(a: &str, b: &str) -> bool {
    SHARED_BY_DESIGN
        .iter()
        .any(|group| group.contains(&a) && group.contains(&b))
}

fn clash(a: &Record<'_>, b: &Record<'_>, domain: Scope) -> bool {
    if a.action.action == b.action.action {
        return false;
    }
    // Two general actions are only reported in the general domain.
    if a.scope == Scope::General && b.scope == Scope::General && domain != Scope::General {
        return false;
    }
    !shared_by_design(&a.action.action, &b.action.action)
}

fn find_conflicts(records: &[Record<'_>]) -> Vec<Conflict> {
    let mut groups: FxHashMap<(Scope, InputKey<'_>), Vec<usize>> = FxHashMap::default();
    for (i, r) in records.iter().enumerate() {
        if r.legacy {
            continue;
        }
        let key = InputKey::of(r.input);
        for domain in domains(r.scope) {
            groups.entry((domain, key.clone())).or_default().push(i);
        }
    }

    let mut conflicts = Vec::new();
    for ((domain, _), members) in groups {
        if members.len() < 2 {
            continue;
        }
        let mut involved = vec![false; members.len()];
        for (x, &i) in members.iter().enumerate() {
            for (y, &j) in members.iter().enumerate().skip(x + 1) {
                if clash(&records[i], &records[j], domain) {
                    involved[x] = true;
                    involved[y] = true;
                }
            }
        }
        let mut actions: Vec<&Record<'_>> = members
            .iter()
            .zip(&involved)
            .filter(|(_, inv)| **inv)
            .map(|(&i, _)| &records[i])
            .collect();
        if actions.len() < 2 {
            continue;
        }
        actions.sort_by(|a, b| {
            a.order
                .cmp(&b.order)
                .then_with(|| a.action.action.cmp(&b.action.action))
        });
        actions.dedup_by(|a, b| a.action.action == b.action.action);
        conflicts.push(Conflict {
            input: actions[0].input.clone(),
            scope: domain,
            actions: actions
                .iter()
                .map(|r| {
                    let info = action_info(&r.action.action);
                    ConflictAction {
                        action: r.action.action.clone(),
                        name: info.name.into_owned(),
                        context: info.context,
                        slot: r.slot,
                    }
                })
                .collect(),
        });
    }
    conflicts.sort_by(|a, b| {
        a.scope
            .cmp(&b.scope)
            .then_with(|| natural_cmp(&a.input.display(), &b.input.display()))
            .then_with(|| a.input.device.cmp(&b.input.device))
            .then_with(|| a.actions[0].action.cmp(&b.actions[0].action))
    });
    conflicts
}

fn find_modifier_notes(records: &[Record<'_>]) -> Vec<ModifierNote> {
    type Key<'a> = (Scope, &'a str, &'a str);
    let mut as_modifier: FxHashMap<Key<'_>, Vec<usize>> = FxHashMap::default();
    let mut alone: FxHashMap<Key<'_>, Vec<usize>> = FxHashMap::default();
    for (i, r) in records.iter().enumerate() {
        if r.legacy {
            continue;
        }
        for domain in domains(r.scope) {
            if r.input.modifiers.is_empty() {
                alone
                    .entry((domain, r.input.device.as_str(), r.input.key.as_str()))
                    .or_default()
                    .push(i);
            }
            for (device, key) in &r.input.modifiers {
                as_modifier
                    .entry((domain, device.as_str(), key.as_str()))
                    .or_default()
                    .push(i);
            }
        }
    }

    let names = |idx: &[usize]| -> Vec<String> {
        let mut v: Vec<&Record<'_>> = idx.iter().map(|&i| &records[i]).collect();
        v.sort_by(|a, b| {
            a.order
                .cmp(&b.order)
                .then_with(|| a.action.action.cmp(&b.action.action))
        });
        v.dedup_by(|a, b| a.action.action == b.action.action);
        v.iter().map(|r| r.action.action.clone()).collect()
    };
    let mut notes: Vec<ModifierNote> = as_modifier
        .iter()
        .filter_map(|(&(domain, device, key), users)| {
            let solo = alone.get(&(domain, device, key))?;
            // In vehicle domains, a note made only of general actions belongs to the general domain.
            let all_general = users
                .iter()
                .chain(solo)
                .all(|&i| records[i].scope == Scope::General);
            if all_general && domain != Scope::General {
                return None;
            }
            Some(ModifierNote {
                scope: domain,
                device: device.to_owned(),
                key: key.to_owned(),
                label: key_label(key),
                as_modifier: names(users),
                alone: names(solo),
            })
        })
        .collect();
    notes.sort_by(|a, b| {
        a.scope
            .cmp(&b.scope)
            .then_with(|| natural_cmp(&a.label, &b.label))
    });
    notes
}

fn summarize_devices(records: &[Record<'_>], conflicts: &[Conflict]) -> Vec<DeviceSummary> {
    #[derive(Default)]
    struct Acc<'a> {
        buttons: usize,
        axes: usize,
        modifier_uses: usize,
        inputs: FxHashSet<&'a str>,
        actions: FxHashSet<&'a str>,
        conflicts: usize,
        kind_hint: &'a str,
    }
    let mut acc: FxHashMap<(&str, u32), Acc<'_>> = FxHashMap::default();
    for r in records {
        let a = acc
            .entry((r.input.device.as_str(), r.input.device_index))
            .or_default();
        match r.slot {
            Slot::Axis => a.axes += 1,
            Slot::Primary | Slot::Secondary => a.buttons += 1,
        }
        a.inputs.insert(r.input.key.as_str());
        a.actions.insert(r.action.action.as_str());
        a.kind_hint = r.input.key.as_str();
        for (device, key) in &r.input.modifiers {
            let m = acc.entry((device.as_str(), 0)).or_default();
            m.modifier_uses += 1;
            m.inputs.insert(key.as_str());
            if m.kind_hint.is_empty() {
                m.kind_hint = key.as_str();
            }
        }
    }
    for c in conflicts {
        if let Some(a) = acc.get_mut(&(c.input.device.as_str(), c.input.device_index)) {
            a.conflicts += 1;
        }
    }
    let mut out: Vec<DeviceSummary> = acc
        .into_iter()
        .map(|((device, index), a)| {
            let mut name = device_name(device).into_owned();
            if index > 0 {
                name.push_str(&format!(" #{}", index + 1));
            }
            DeviceSummary {
                device: device.to_owned(),
                device_index: index,
                name,
                kind: DeviceKind::of(device, a.kind_hint),
                buttons: a.buttons,
                axes: a.axes,
                modifier_uses: a.modifier_uses,
                inputs: a.inputs.len(),
                actions: a.actions.len(),
                conflicts: a.conflicts,
            }
        })
        .collect();
    out.sort_by(|a, b| {
        a.kind
            .cmp(&b.kind)
            .then_with(|| natural_cmp(&a.name, &b.name))
            .then(a.device_index.cmp(&b.device_index))
    });
    out
}

/// One way of satisfying a [`Requirement`].
#[derive(Debug, Clone, Copy)]
pub enum Alternative {
    /// All of these actions are bound.
    All(&'static [&'static str]),
    /// One of these mouse-mode settings (e.g. `MouseXMode`) has a value containing `contains`
    /// (e.g. `Pitch` matches `Bindings_MousePitch` and `Bindings_MousePitchInverted`); an empty
    /// `contains` accepts any non-empty value.
    Mouse {
        settings: &'static [&'static str],
        contains: &'static str,
    },
}

/// A curated "this should be bound" rule.
#[derive(Debug, Clone, Copy)]
pub struct Requirement {
    pub id: &'static str,
    pub label: &'static str,
    pub context: Context,
    pub importance: Importance,
    /// Satisfied when any alternative holds.
    pub alternatives: &'static [Alternative],
}

const fn req(
    id: &'static str,
    label: &'static str,
    context: Context,
    importance: Importance,
    alternatives: &'static [Alternative],
) -> Requirement {
    Requirement {
        id,
        label,
        context,
        importance,
        alternatives,
    }
}

use Alternative::{All, Mouse};
use Importance::{Essential, Recommended};

const SHIP_MOUSE: &[&str] = &["MouseXMode", "MouseYMode"];
const FOOT_MOUSE: &[&str] = &["MouseHumanoidXMode", "MouseHumanoidYMode"];

/// The curated requirement list checked by [`analyze`].
#[rustfmt::skip]
pub const REQUIREMENTS: &[Requirement] = &[
    // Ship flight
    req("ship.pitch", "Pitch", Context::Ship, Essential, &[
        All(&["PitchAxisRaw"]), All(&["PitchUpButton", "PitchDownButton"]), Mouse { settings: SHIP_MOUSE, contains: "Pitch" },
    ]),
    req("ship.roll", "Roll", Context::Ship, Essential, &[
        All(&["RollAxisRaw"]), All(&["RollLeftButton", "RollRightButton"]), Mouse { settings: SHIP_MOUSE, contains: "Roll" },
    ]),
    req("ship.yaw", "Yaw", Context::Ship, Essential, &[
        All(&["YawAxisRaw"]), All(&["YawLeftButton", "YawRightButton"]), Mouse { settings: SHIP_MOUSE, contains: "Yaw" },
    ]),
    req("ship.throttle", "Throttle", Context::Ship, Essential, &[
        All(&["ThrottleAxis"]), All(&["ForwardKey", "BackwardKey"]), All(&["SetSpeedZero", "SetSpeed100"]), All(&["AheadThrust"]),
    ]),
    req("ship.vertical_thrust", "Vertical thrust (up / down)", Context::Ship, Essential, &[
        All(&["VerticalThrustRaw"]), All(&["UpThrustButton", "DownThrustButton"]),
    ]),
    req("ship.lateral_thrust", "Lateral thrust (left / right)", Context::Ship, Recommended, &[
        All(&["LateralThrustRaw"]), All(&["LeftThrustButton", "RightThrustButton"]),
    ]),
    req("ship.boost", "Engine boost", Context::Ship, Essential, &[All(&["UseBoostJuice"])]),
    req("ship.fsd", "Frame Shift Drive (jump / supercruise)", Context::Ship, Essential, &[
        All(&["HyperSuperCombination"]), All(&["Supercruise", "Hyperspace"]),
    ]),
    req("ship.next_route_system", "Target next system in route", Context::Ship, Essential, &[All(&["TargetNextRouteSystem"])]),
    req("ship.landing_gear", "Landing gear", Context::Ship, Essential, &[All(&["LandingGearToggle"])]),
    req("ship.cargo_scoop", "Cargo scoop", Context::Ship, Essential, &[All(&["ToggleCargoScoop"])]),
    req("ship.hardpoints", "Deploy hardpoints", Context::Ship, Essential, &[All(&["DeployHardpointToggle"])]),
    req("ship.primary_fire", "Primary fire", Context::Ship, Essential, &[All(&["PrimaryFire"])]),
    req("ship.secondary_fire", "Secondary fire", Context::Ship, Essential, &[All(&["SecondaryFire"])]),
    req("ship.target_ahead", "Select target ahead", Context::Ship, Essential, &[All(&["SelectTarget"])]),
    req("ship.left_panel", "Left (navigation) panel", Context::Ship, Essential, &[All(&["FocusLeftPanel"]), All(&["UIFocus"])]),
    req("ship.right_panel", "Right (systems) panel", Context::Ship, Essential, &[All(&["FocusRightPanel"]), All(&["UIFocus"])]),
    req("ship.comms_panel", "Comms panel", Context::Ship, Recommended, &[All(&["FocusCommsPanel"]), All(&["UIFocus"])]),
    req("ship.role_panel", "Role panel", Context::Ship, Recommended, &[All(&["FocusRadarPanel"]), All(&["UIFocus"])]),
    req("ship.galaxy_map", "Galaxy map", Context::Ship, Essential, &[All(&["GalaxyMapOpen"])]),
    req("ship.system_map", "System map", Context::Ship, Essential, &[All(&["SystemMapOpen"])]),
    req("ship.fss", "Enter FSS (Full Spectrum System Scanner)", Context::Ship, Essential, &[All(&["ExplorationFSSEnter"])]),
    req("ship.hud_mode", "Switch HUD mode (combat / analysis)", Context::Ship, Essential, &[All(&["PlayerHUDModeToggle"])]),
    req("ship.flight_assist", "Flight assist toggle", Context::Ship, Recommended, &[All(&["ToggleFlightAssist"])]),
    req("ship.power", "Power distribution (SYS / ENG / WEP / RST)", Context::Ship, Recommended, &[
        All(&["IncreaseSystemsPower", "IncreaseEnginesPower", "IncreaseWeaponsPower", "ResetPowerDistribution"]),
    ]),
    req("ship.heat_sink", "Deploy heat sink", Context::Ship, Recommended, &[All(&["DeployHeatSink"])]),
    req("ship.next_target", "Cycle targets", Context::Ship, Recommended, &[All(&["CycleNextTarget"]), All(&["CyclePreviousTarget"])]),
    req("ship.hostile_target", "Target hostiles", Context::Ship, Recommended, &[
        All(&["SelectHighestThreat"]), All(&["CycleNextHostileTarget"]),
    ]),
    req("ship.subsystem", "Cycle subsystems", Context::Ship, Recommended, &[All(&["CycleNextSubsystem"]), All(&["CyclePreviousSubsystem"])]),
    req("ship.fire_group", "Cycle fire groups", Context::Ship, Recommended, &[All(&["CycleFireGroupNext"]), All(&["CycleFireGroupPrevious"])]),
    req("ship.lights", "Ship lights", Context::Ship, Recommended, &[All(&["ShipSpotLightToggle"])]),
    req("ship.night_vision", "Night vision", Context::Ship, Recommended, &[All(&["NightVisionToggle"])]),
    // Scanners
    req("fss.leave", "Leave FSS", Context::Scanners, Essential, &[All(&["ExplorationFSSQuit"])]),
    req("fss.zoom", "FSS zoom (scan a signal)", Context::Scanners, Recommended, &[All(&["ExplorationFSSZoomIn", "ExplorationFSSZoomOut"])]),
    req("fss.tuning", "FSS frequency tuning", Context::Scanners, Recommended, &[
        All(&["ExplorationFSSRadioTuningX_Raw"]),
        All(&["ExplorationFSSRadioTuningAbsoluteX"]),
        All(&["ExplorationFSSRadioTuningX_Increase", "ExplorationFSSRadioTuningX_Decrease"]),
    ]),
    req("fss.honk", "FSS discovery scan", Context::Scanners, Recommended, &[All(&["ExplorationFSSDiscoveryScan"])]),
    req("dss.exit", "Exit DSS", Context::Scanners, Recommended, &[All(&["ExplorationSAAExitThirdPerson"])]),
    // Interface
    req("ui.up_down", "UI up / down", Context::Interface, Essential, &[All(&["UI_Up", "UI_Down"])]),
    req("ui.left_right", "UI left / right", Context::Interface, Essential, &[All(&["UI_Left", "UI_Right"])]),
    req("ui.select", "UI select", Context::Interface, Essential, &[All(&["UI_Select"])]),
    req("ui.back", "UI back", Context::Interface, Essential, &[All(&["UI_Back"])]),
    req("ui.tabs", "Next / previous panel tab", Context::Interface, Essential, &[All(&["CycleNextPanel", "CyclePreviousPanel"])]),
    req("ui.pages", "Next / previous page", Context::Interface, Recommended, &[All(&["CycleNextPage", "CyclePreviousPage"])]),
    // General
    req("general.menu", "Game menu", Context::General, Recommended, &[All(&["Pause"])]),
    // Galaxy map
    req("galmap.zoom", "Galaxy map zoom", Context::GalaxyMap, Recommended, &[All(&["CamZoomAxis"]), All(&["CamZoomIn", "CamZoomOut"])]),
    // SRV
    req("srv.steering", "SRV steering", Context::Srv, Essential, &[
        All(&["SteeringAxis"]), All(&["SteerLeftButton", "SteerRightButton"]), Mouse { settings: &["MouseBuggySteeringXMode"], contains: "" },
    ]),
    req("srv.speed", "SRV speed", Context::Srv, Essential, &[
        All(&["DriveSpeedAxis"]),
        All(&["IncreaseSpeedButtonMax", "DecreaseSpeedButtonMax"]),
        All(&["IncreaseSpeedButtonPartial", "DecreaseSpeedButtonPartial"]),
    ]),
    req("srv.recall_ship", "Recall / dismiss ship", Context::Srv, Essential, &[All(&["RecallDismissShip"])]),
    req("srv.right_panel", "SRV right (systems) panel", Context::Srv, Essential, &[All(&["FocusRightPanel_Buggy"]), All(&["UIFocus_Buggy"])]),
    req("srv.left_panel", "SRV left (navigation) panel", Context::Srv, Recommended, &[All(&["FocusLeftPanel_Buggy"]), All(&["UIFocus_Buggy"])]),
    req("srv.handbrake", "SRV handbrake", Context::Srv, Recommended, &[All(&["AutoBreakBuggyButton"])]),
    req("srv.thrusters", "SRV vertical thrusters", Context::Srv, Recommended, &[All(&["VerticalThrustersButton"])]),
    req("srv.primary_fire", "SRV primary fire", Context::Srv, Recommended, &[All(&["BuggyPrimaryFireButton"])]),
    req("srv.turret", "SRV turret mode", Context::Srv, Recommended, &[All(&["ToggleBuggyTurretButton"])]),
    req("srv.cargo_scoop", "SRV cargo scoop", Context::Srv, Recommended, &[All(&["ToggleCargoScoop_Buggy"])]),
    req("srv.headlights", "SRV headlights", Context::Srv, Recommended, &[All(&["HeadlightsBuggyButton"])]),
    // On foot
    req("foot.move", "Move forward / backward", Context::OnFoot, Essential, &[
        All(&["HumanoidForwardAxis"]), All(&["HumanoidForwardButton", "HumanoidBackwardButton"]),
    ]),
    req("foot.strafe", "Strafe", Context::OnFoot, Essential, &[
        All(&["HumanoidStrafeAxis"]), All(&["HumanoidStrafeLeftButton", "HumanoidStrafeRightButton"]),
    ]),
    req("foot.look_horizontal", "Look left / right", Context::OnFoot, Essential, &[
        All(&["HumanoidRotateAxis"]),
        All(&["HumanoidRotateLeftButton", "HumanoidRotateRightButton"]),
        Mouse { settings: FOOT_MOUSE, contains: "Yaw" },
    ]),
    req("foot.look_vertical", "Look up / down", Context::OnFoot, Essential, &[
        All(&["HumanoidPitchAxis"]),
        All(&["HumanoidPitchUpButton", "HumanoidPitchDownButton"]),
        Mouse { settings: FOOT_MOUSE, contains: "Pitch" },
    ]),
    req("foot.interact", "Interact", Context::OnFoot, Essential, &[All(&["HumanoidPrimaryInteractButton"])]),
    req("foot.primary_fire", "Primary fire / use tool", Context::OnFoot, Essential, &[All(&["HumanoidPrimaryFireButton"])]),
    req("foot.jump", "Jump", Context::OnFoot, Recommended, &[All(&["HumanoidJumpButton"])]),
    req("foot.sprint", "Sprint", Context::OnFoot, Recommended, &[All(&["HumanoidSprintButton"])]),
    req("foot.crouch", "Crouch", Context::OnFoot, Recommended, &[All(&["HumanoidCrouchButton"])]),
    req("foot.item_wheel", "Item wheel", Context::OnFoot, Recommended, &[All(&["HumanoidItemWheelButton"])]),
    req("foot.reload", "Reload", Context::OnFoot, Recommended, &[All(&["HumanoidReloadButton"])]),
    req("foot.weapons", "Weapon selection", Context::OnFoot, Recommended, &[
        All(&["HumanoidSelectPrimaryWeaponButton"]), All(&["HumanoidSelectNextWeaponButton"]), All(&["HumanoidSwitchWeapon"]),
    ]),
    req("foot.flashlight", "Flashlight", Context::OnFoot, Recommended, &[All(&["HumanoidToggleFlashlightButton"])]),
    req("foot.medkit", "Use medkit", Context::OnFoot, Recommended, &[All(&["HumanoidHealthPack"])]),
    req("foot.energy_cell", "Use energy cell", Context::OnFoot, Recommended, &[All(&["HumanoidBattery"])]),
];

impl Requirement {
    /// True when any alternative holds for `bindings`.
    pub fn is_satisfied(&self, bindings: &Bindings) -> bool {
        self.alternatives.iter().any(|alt| match *alt {
            All(actions) => actions.iter().all(|a| bindings.is_bound(a)),
            Mouse { settings, contains } => settings.iter().any(|s| {
                bindings
                    .setting(s)
                    .is_some_and(|v| !v.is_empty() && (contains.is_empty() || v.contains(contains)))
            }),
        })
    }

    fn options(&self) -> Vec<String> {
        self.alternatives
            .iter()
            .map(|alt| match *alt {
                All(actions) => actions
                    .iter()
                    .map(|a| action_info(a).name.into_owned())
                    .collect::<Vec<_>>()
                    .join(" + "),
                Mouse { contains: "", .. } => "a mouse axis".to_owned(),
                Mouse { contains, .. } => {
                    format!("a mouse axis set to {}", contains.to_lowercase())
                }
            })
            .collect()
    }

    fn actions(&self) -> Vec<&'static str> {
        let mut out: Vec<&'static str> = Vec::new();
        for alt in self.alternatives {
            if let All(actions) = alt {
                for a in *actions {
                    if !out.contains(a) {
                        out.push(a);
                    }
                }
            }
        }
        out
    }
}

fn find_missing(bindings: &Bindings) -> Vec<MissingBinding> {
    let mut missing: Vec<MissingBinding> = REQUIREMENTS
        .iter()
        .filter(|r| !r.is_satisfied(bindings))
        .map(|r| MissingBinding {
            id: r.id,
            label: r.label,
            context: r.context,
            importance: r.importance,
            options: r.options(),
            actions: r.actions(),
        })
        .collect();
    missing.sort_by(|a, b| match a.importance.cmp(&b.importance) {
        Ordering::Equal => a.context.cmp(&b.context),
        other => other,
    });
    missing
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slot(device: &str, key: &str) -> String {
        format!(
            r#"<Primary Device="{device}" Key="{key}" /><Secondary Device="{{NoDevice}}" Key="" />"#
        )
    }

    fn button(action: &str, device: &str, key: &str) -> String {
        format!("<{action}>{}</{action}>", slot(device, key))
    }

    fn doc(body: &str) -> Bindings {
        Bindings::parse(&format!(
            r#"<Root PresetName="Test" MajorVersion="4" MinorVersion="2">{body}</Root>"#
        ))
        .unwrap()
    }

    fn conflict_sets(report: &Report) -> Vec<(Scope, Vec<String>)> {
        report
            .conflicts
            .iter()
            .map(|c| {
                (
                    c.scope,
                    c.actions.iter().map(|a| a.action.clone()).collect(),
                )
            })
            .collect()
    }

    #[test]
    fn same_scope_same_input_conflicts() {
        let b = doc(&[
            button("PrimaryFire", "Mouse", "Mouse_1"),
            button("SelectTarget", "Mouse", "Mouse_1"),
        ]
        .concat());
        let r = analyze(&b);
        assert_eq!(
            conflict_sets(&r),
            [(
                Scope::Ship,
                vec!["SelectTarget".to_owned(), "PrimaryFire".to_owned()]
            )]
        );
        assert!(r.is_conflicted("PrimaryFire", Slot::Primary));
        assert!(!r.is_conflicted("PrimaryFire", Slot::Secondary));
        assert_eq!(r.conflicts[0].input.display(), "Mouse 1");
        assert!(
            r.conflicted_slots()
                .contains(&("SelectTarget", Slot::Primary))
        );
    }

    #[test]
    fn different_scopes_do_not_conflict() {
        let b = doc(&[
            button("ForwardKey", "Keyboard", "Key_W"),
            button("UI_Up", "Keyboard", "Key_W"),
            button("CamTranslateForward", "Keyboard", "Key_W"),
            button("HumanoidForwardButton", "Keyboard", "Key_W"),
            button("BuggyPitchUpButton", "Keyboard", "Key_W"),
            button("YawLeftButton_Landing", "Keyboard", "Key_W"),
            button("HeadLookPitchUp", "Keyboard", "Key_W"),
        ]
        .concat());
        assert!(analyze(&b).conflicts.is_empty());
    }

    #[test]
    fn general_actions_clash_with_vehicles_once() {
        let b = doc(&[
            button("MicrophoneMute", "Keyboard", "Key_T"),
            button("SelectTarget", "Keyboard", "Key_T"),
            button("HumanoidToggleFlashlightButton", "Keyboard", "Key_T"),
            button("UI_Select", "Keyboard", "Key_T"),
        ]
        .concat());
        let sets = conflict_sets(&analyze(&b));
        assert_eq!(sets.len(), 2);
        assert!(sets.contains(&(
            Scope::Ship,
            vec!["SelectTarget".into(), "MicrophoneMute".into()]
        )));
        assert!(sets.contains(&(
            Scope::OnFoot,
            vec![
                "MicrophoneMute".into(),
                "HumanoidToggleFlashlightButton".into()
            ]
        )));

        let two_general = doc(&[
            button("Pause", "Keyboard", "Key_P"),
            button("FriendsMenu", "Keyboard", "Key_P"),
        ]
        .concat());
        assert_eq!(
            conflict_sets(&analyze(&two_general)),
            [(Scope::General, vec!["Pause".into(), "FriendsMenu".into()])]
        );
    }

    #[test]
    fn modifiers_hold_and_device_index_distinguish_inputs() {
        let b = doc(&format!(
            "{}<SelectTarget><Primary Device=\"Keyboard\" Key=\"Key_T\"><Modifier Device=\"Keyboard\" Key=\"Key_LeftShift\"/></Primary>{}</SelectTarget>\
             <CycleNextTarget><Primary Device=\"Keyboard\" Key=\"Key_T\"><Hold Value=\"1\"/></Primary></CycleNextTarget>\
             <DeployHeatSink><Primary Device=\"T16000M\" DeviceIndex=\"1\" Key=\"Joy_1\"/></DeployHeatSink>\
             <ChargeECM><Primary Device=\"T16000M\" Key=\"Joy_1\"/></ChargeECM>",
            button("PrimaryFire", "Keyboard", "Key_T"),
            r#"<Secondary Device="{NoDevice}" Key="" />"#,
        ));
        assert!(analyze(&b).conflicts.is_empty());
    }

    #[test]
    fn modifier_order_is_irrelevant() {
        let combo = |action: &str, m1: &str, m2: &str| {
            format!(
                r#"<{action}><Primary Device="Keyboard" Key="Key_End"><Modifier Device="Keyboard" Key="{m1}"/><Modifier Device="Keyboard" Key="{m2}"/></Primary></{action}>"#
            )
        };
        let b = doc(&[
            combo("PhotoCameraToggle", "Key_LeftControl", "Key_LeftAlt"),
            combo("OrbitLinesToggle", "Key_LeftAlt", "Key_LeftControl"),
        ]
        .concat());
        let r = analyze(&b);
        assert_eq!(r.conflicts.len(), 1);
        // The input is reported as written for the first action in catalogue order.
        assert_eq!(r.conflicts[0].actions[0].action, "OrbitLinesToggle");
        assert_eq!(r.conflicts[0].input.display(), "LAlt + LCtrl + End");
    }

    #[test]
    fn shared_by_design_and_legacy_are_accepted() {
        let b = doc(&[
            button("SteerLeftButton", "Keyboard", "Key_A"),
            button("BuggyRollLeftButton", "Keyboard", "Key_A"),
            button("CycleNextSubsystem", "Keyboard", "Key_Y"),
            button("TriggerColonisationModule", "Keyboard", "Key_Y"),
            button("HumanoidPrimaryInteractButton", "Keyboard", "Key_E"),
            button("HumanoidToggleMissionHelpPanelButton", "Keyboard", "Key_E"),
            button("HumanoidConflictContextualUIButton", "Keyboard", "Key_E"),
            button("FreeCamZoomIn", "Keyboard", "Key_UpArrow"),
            button("FocusDistanceInc", "Keyboard", "Key_UpArrow"),
        ]
        .concat());
        assert!(analyze(&b).conflicts.is_empty());
        assert!(shared_by_design("SteeringAxis", "BuggyRollAxisRaw"));
        assert!(!shared_by_design("SteeringAxis", "PrimaryFire"));

        // A third, unrelated action on the same key is still reported, together with the pair.
        let b = doc(&[
            button("HumanoidPrimaryInteractButton", "Keyboard", "Key_E"),
            button("HumanoidReloadButton", "Keyboard", "Key_E"),
            button("HumanoidMeleeButton", "Keyboard", "Key_E"),
        ]
        .concat());
        let sets = conflict_sets(&analyze(&b));
        assert_eq!(sets.len(), 1);
        assert_eq!(sets[0].1.len(), 3);
    }

    #[test]
    fn axis_conflicts_are_detected() {
        let axis = |action: &str, key: &str| {
            format!(
                r#"<{action}><Binding Device="SaitekX52Pro" Key="{key}"/><Inverted Value="0"/><Deadzone Value="0.0"/></{action}>"#
            )
        };
        let b = doc(&[
            axis("YawAxisRaw", "Joy_XAxis"),
            axis("RollAxisRaw", "Joy_XAxis"),
            axis("YawAxisAlternate", "Joy_XAxis"),
        ]
        .concat());
        let r = analyze(&b);
        assert_eq!(
            conflict_sets(&r),
            [(Scope::Ship, vec!["YawAxisRaw".into(), "RollAxisRaw".into()])]
        );
        assert_eq!(r.conflicts[0].actions[0].slot, Slot::Axis);
        assert_eq!(r.devices[0].conflicts, 1);
    }

    #[test]
    fn same_action_twice_is_not_a_conflict() {
        let b = doc(
            r#"<PrimaryFire><Primary Device="Mouse" Key="Mouse_1"/><Secondary Device="Mouse" Key="Mouse_1"/></PrimaryFire>"#,
        );
        assert!(analyze(&b).conflicts.is_empty());
    }

    #[test]
    fn modifier_used_alone_is_noted() {
        let b = doc(&format!(
            r#"{}<PhotoCameraToggle_Humanoid><Primary Device="Keyboard" Key="Key_Space"><Modifier Device="Keyboard" Key="Key_LeftControl"/></Primary></PhotoCameraToggle_Humanoid>
               <PhotoCameraToggle><Primary Device="Keyboard" Key="Key_End"><Modifier Device="Keyboard" Key="Key_LeftControl"/></Primary></PhotoCameraToggle>"#,
            button("HumanoidCrouchButton", "Keyboard", "Key_LeftControl")
        ));
        let r = analyze(&b);
        assert_eq!(r.modifier_notes.len(), 1);
        let note = &r.modifier_notes[0];
        assert_eq!((note.scope, note.label.as_str()), (Scope::OnFoot, "LCtrl"));
        assert_eq!(note.as_modifier, ["PhotoCameraToggle_Humanoid"]);
        assert_eq!(note.alone, ["HumanoidCrouchButton"]);
    }

    #[test]
    fn missing_requirements_and_alternatives() {
        let empty = doc("");
        let r = analyze(&empty);
        assert_eq!(r.missing.len(), REQUIREMENTS.len());
        assert!(r.missing_essentials().count() > 20);
        assert_eq!(r.missing[0].importance, Importance::Essential);
        let fsd = r.missing.iter().find(|m| m.id == "ship.fsd").unwrap();
        assert_eq!(
            fsd.options,
            ["Toggle Frame Shift Drive", "Supercruise + Hyperspace Jump"]
        );
        assert_eq!(
            fsd.actions,
            ["HyperSuperCombination", "Supercruise", "Hyperspace"]
        );

        // Either alternative satisfies the requirement; mouse modes count for rotation.
        let b = doc(&[
            button("Supercruise", "Keyboard", "Key_J"),
            button("Hyperspace", "Keyboard", "Key_K"),
            r#"<MouseYMode Value="Bindings_MousePitchInverted"/>"#.to_owned(),
            button("YawLeftButton", "Keyboard", "Key_A"),
        ]
        .concat());
        let r = analyze(&b);
        let missing: Vec<&str> = r.missing.iter().map(|m| m.id).collect();
        assert!(!missing.contains(&"ship.fsd"));
        assert!(!missing.contains(&"ship.pitch"));
        assert!(missing.contains(&"ship.yaw"), "yaw needs both buttons");
        assert!(missing.contains(&"ship.roll"));
    }

    #[test]
    fn requirement_ids_and_actions_are_valid() {
        let mut ids = FxHashSet::default();
        for r in REQUIREMENTS {
            assert!(ids.insert(r.id), "duplicate requirement {}", r.id);
            for a in r.actions() {
                assert!(
                    lookup(a).is_some(),
                    "{} references unknown action {a}",
                    r.id
                );
            }
        }
    }

    #[test]
    fn device_summary_counts() {
        let b = doc(&format!(
            r#"{}{}<YawAxisRaw><Binding Device="SaitekX52Pro" Key="Joy_XAxis"/><Inverted Value="0"/><Deadzone Value="0"/></YawAxisRaw>
               <SelectTarget><Primary Device="SaitekX52Pro" Key="Joy_5"><Modifier Device="Keyboard" Key="Key_LeftShift"/></Primary></SelectTarget>"#,
            button("PrimaryFire", "Mouse", "Mouse_1"),
            button("UI_Up", "Keyboard", "Key_W"),
        ));
        let r = analyze(&b);
        let kinds: Vec<DeviceKind> = r.devices.iter().map(|d| d.kind).collect();
        assert_eq!(
            kinds,
            [
                DeviceKind::Keyboard,
                DeviceKind::Mouse,
                DeviceKind::Joystick
            ]
        );
        let kb = &r.devices[0];
        assert_eq!(
            (kb.buttons, kb.axes, kb.modifier_uses, kb.inputs, kb.actions),
            (1, 0, 1, 2, 1)
        );
        let stick = &r.devices[2];
        assert_eq!(stick.name, "Saitek X52 Pro");
        assert_eq!(
            (stick.buttons, stick.axes, stick.inputs, stick.actions),
            (1, 1, 2, 2)
        );
        assert_eq!(r.totals.bindings, 4);
        assert_eq!(r.totals.bound_actions, 4);
        assert!(r.totals.unknown_actions.is_empty());
    }

    #[test]
    fn unknown_actions_are_listed() {
        let b = doc(&button("SomeFutureAction", "Keyboard", "Key_Q"));
        assert_eq!(analyze(&b).totals.unknown_actions, ["SomeFutureAction"]);
    }
}
