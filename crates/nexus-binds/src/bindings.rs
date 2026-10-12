//! The `.binds` data model and its parser.
//!
//! A bindings file is a flat list of elements under `<Root PresetName=".." MajorVersion="4"
//! MinorVersion="2">`. Each element is either
//!
//! * a **button action** — `<Primary>` and `<Secondary>` slots, optionally `<ToggleOn>`:
//!   ```xml
//!   <LandingGearToggle>
//!     <Primary Device="Keyboard" Key="Key_L" />
//!     <Secondary Device="{NoDevice}" Key="" />
//!   </LandingGearToggle>
//!   ```
//!   A slot may carry `<Modifier Device=".." Key=".."/>` children (key combinations) and a
//!   `<Hold Value="1"/>` child (long press);
//! * an **axis action** — `<Binding>`, `<Inverted Value="0|1"/>`, `<Deadzone Value="0.0"/>`;
//! * a **setting** — `<MouseXMode Value="Bindings_MouseYaw"/>`, or text content such as
//!   `<KeyboardLayout>en-US</KeyboardLayout>`.
//!
//! Slots with `Device="{NoDevice}"` or an empty `Key` are unbound. Some of the game's own
//! presets write the key attribute as lower-case `key=""`, so attribute names are matched
//! case-insensitively.

use std::borrow::Cow;
use std::collections::BTreeSet;
use std::fmt;
use std::path::Path;

use roxmltree::{Document, Node};
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};

use crate::catalogue::{ActionInfo, action_info};
use crate::error::BindsError;
use crate::input::Input;

/// Device id the game writes for an empty slot.
pub const NO_DEVICE: &str = "{NoDevice}";

/// `MajorVersion.MinorVersion` of a bindings file (`4.2` for current Odyssey files).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

/// Whether an action is driven by buttons (`Primary`/`Secondary`) or by an analogue axis
/// (`Binding`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ActionKind {
    Button,
    Axis,
}

/// One of the three places an input can be bound to an action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Slot {
    Primary,
    Secondary,
    Axis,
}

impl Slot {
    pub fn label(self) -> &'static str {
        match self {
            Self::Primary => "primary",
            Self::Secondary => "secondary",
            Self::Axis => "axis",
        }
    }
}

/// An analogue axis assignment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AxisBinding {
    pub input: Input,
    pub inverted: bool,
    /// Dead zone as a fraction of the axis range (0.0 – 1.0).
    pub deadzone: f32,
}

/// One action element of a bindings file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActionBinding {
    /// Element name, e.g. `HyperSuperCombination`. See [`crate::catalogue`] for friendly names.
    pub action: String,
    pub kind: ActionKind,
    pub primary: Option<Input>,
    pub secondary: Option<Input>,
    /// Only ever set for [`ActionKind::Axis`] actions.
    pub axis: Option<AxisBinding>,
    /// `<ToggleOn Value=".."/>`: whether the button toggles (`Some(true)`) or must be held
    /// (`Some(false)`); `None` when the action has no such option.
    pub toggle_on: Option<bool>,
    /// Any other child elements (name, `Value` attribute or text) — none exist in current files,
    /// kept so nothing the game adds later is silently dropped.
    pub settings: Vec<(String, String)>,
}

impl ActionBinding {
    /// An action with nothing bound.
    pub fn unbound(action: impl Into<String>, kind: ActionKind) -> Self {
        Self {
            action: action.into(),
            kind,
            primary: None,
            secondary: None,
            axis: None,
            toggle_on: None,
            settings: Vec::new(),
        }
    }

    /// True when at least one slot has an input.
    pub fn is_bound(&self) -> bool {
        self.primary.is_some() || self.secondary.is_some() || self.axis.is_some()
    }

    /// The input in `slot`, if bound.
    pub fn input(&self, slot: Slot) -> Option<&Input> {
        match slot {
            Slot::Primary => self.primary.as_ref(),
            Slot::Secondary => self.secondary.as_ref(),
            Slot::Axis => self.axis.as_ref().map(|a| &a.input),
        }
    }

    /// All bound inputs with their slot, in primary / secondary / axis order.
    pub fn inputs(&self) -> impl Iterator<Item = (Slot, &Input)> + '_ {
        [Slot::Primary, Slot::Secondary, Slot::Axis]
            .into_iter()
            .filter_map(|s| self.input(s).map(|i| (s, i)))
    }

    /// Catalogue information (friendly name, context, category) for this action.
    pub fn info(&self) -> ActionInfo {
        action_info(&self.action)
    }
}

/// A parsed bindings file (or several merged per preset category, see
/// [`PresetDirectory::load_effective`](crate::PresetDirectory::load_effective)).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Bindings {
    /// `PresetName` attribute of `<Root>` (for merged bindings: the preset names joined by ` + `).
    pub preset_name: String,
    /// `MajorVersion`/`MinorVersion` of `<Root>`; absent in the game's built-in presets.
    pub version: Option<Version>,
    /// `<KeyboardLayout>` (e.g. `en-US`).
    pub keyboard_layout: Option<String>,
    /// Every action element in file order, bound or not.
    pub actions: Vec<ActionBinding>,
    /// Plain settings (`MouseXMode`, `ThrottleRange`, …) in file order; values are the `Value`
    /// attribute or the element text.
    pub settings: Vec<(String, String)>,
    /// Every device id used by a bound input or modifier.
    pub devices: BTreeSet<String>,
    /// For merged bindings: which preset each category was taken from. Empty for a single file.
    pub sources: Vec<(crate::PresetCategory, String)>,
}

impl Bindings {
    /// Reads and parses a `.binds` file. UTF-8 (with or without BOM) and UTF-16 are accepted.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, BindsError> {
        let path = path.as_ref();
        let bytes = std::fs::read(path).map_err(|e| BindsError::io(path, e))?;
        Self::parse(&decode_text(&bytes))
    }

    /// Parses the XML text of a `.binds` file.
    pub fn parse(xml: &str) -> Result<Self, BindsError> {
        let xml = xml.strip_prefix('\u{feff}').unwrap_or(xml);
        let doc = Document::parse(xml)?;
        let root = doc.root_element();
        if root.tag_name().name() != "Root" {
            return Err(BindsError::NotBindings(root.tag_name().name().to_owned()));
        }

        let version = attr(root, "MajorVersion").and_then(|major| {
            let major = major.trim().parse().ok()?;
            let minor = attr(root, "MinorVersion")
                .and_then(|m| m.trim().parse().ok())
                .unwrap_or(0);
            Some(Version { major, minor })
        });

        let mut actions: Vec<ActionBinding> = Vec::with_capacity(512);
        let mut positions: FxHashMap<&str, usize> = FxHashMap::default();
        let mut settings = Vec::with_capacity(128);
        let mut keyboard_layout = None;
        for node in root.children().filter(Node::is_element) {
            let name = node.tag_name().name();
            let is_action = node.children().any(|c| {
                c.is_element() && matches!(c.tag_name().name(), "Primary" | "Secondary" | "Binding")
            });
            if is_action {
                // Some of the game's own presets repeat an element; like settings, the last one
                // wins (keeping the position of the first).
                let action = parse_action(node);
                match positions.get(name) {
                    Some(&i) => actions[i] = action,
                    None => {
                        positions.insert(name, actions.len());
                        actions.push(action);
                    }
                }
            } else {
                let value = value_or_text(node);
                if name == "KeyboardLayout" && !value.is_empty() {
                    keyboard_layout = Some(value.clone());
                }
                settings.push((name.to_owned(), value));
            }
        }

        let mut bindings = Self {
            preset_name: attr(root, "PresetName").unwrap_or_default().to_owned(),
            version,
            keyboard_layout,
            actions,
            settings,
            devices: BTreeSet::new(),
            sources: Vec::new(),
        };
        bindings.refresh_devices();
        Ok(bindings)
    }

    /// The action element named `id`, if the file has one.
    pub fn action(&self, id: &str) -> Option<&ActionBinding> {
        self.actions.iter().find(|a| a.action == id)
    }

    /// True when the action exists and has at least one input.
    pub fn is_bound(&self, id: &str) -> bool {
        self.action(id).is_some_and(ActionBinding::is_bound)
    }

    /// Actions with at least one input, in file order.
    pub fn bound_actions(&self) -> impl Iterator<Item = &ActionBinding> + '_ {
        self.actions.iter().filter(|a| a.is_bound())
    }

    /// Value of a setting such as `MouseXMode` (the last occurrence wins, as some files repeat
    /// settings).
    pub fn setting(&self, name: &str) -> Option<&str> {
        self.settings
            .iter()
            .rev()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }

    /// Number of bound slots (primary, secondary and axis) over all actions.
    pub fn binding_count(&self) -> usize {
        self.actions.iter().map(|a| a.inputs().count()).sum()
    }

    /// Recomputes [`devices`](Self::devices) from the bound inputs.
    pub fn refresh_devices(&mut self) {
        let mut devices = BTreeSet::new();
        for action in &self.actions {
            for (_, input) in action.inputs() {
                devices.insert(input.device.clone());
                for (device, _) in &input.modifiers {
                    devices.insert(device.clone());
                }
            }
        }
        self.devices = devices;
    }
}

/// Decodes file bytes: UTF-16 (by BOM) or UTF-8 (lossy, so a stray byte cannot fail the load).
fn decode_text(bytes: &[u8]) -> Cow<'_, str> {
    fn utf16(rest: &[u8], from: fn([u8; 2]) -> u16) -> Cow<'static, str> {
        let units: Vec<u16> = rest
            .as_chunks::<2>()
            .0
            .iter()
            .map(|&pair| from(pair))
            .collect();
        Cow::Owned(String::from_utf16_lossy(&units))
    }
    if let Some(rest) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        utf16(rest, u16::from_le_bytes)
    } else if let Some(rest) = bytes.strip_prefix(&[0xFE, 0xFF]) {
        utf16(rest, u16::from_be_bytes)
    } else {
        String::from_utf8_lossy(bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes))
    }
}

/// Attribute lookup that falls back to a case-insensitive match (`key=""` in some presets).
fn attr<'a>(node: Node<'a, '_>, name: &str) -> Option<&'a str> {
    node.attribute(name).or_else(|| {
        node.attributes()
            .find(|a| a.name().eq_ignore_ascii_case(name))
            .map(|a| a.value())
    })
}

fn value_or_text(node: Node<'_, '_>) -> String {
    attr(node, "Value")
        .or_else(|| node.text())
        .map(str::trim)
        .unwrap_or_default()
        .to_owned()
}

fn flag(node: Node<'_, '_>) -> bool {
    attr(node, "Value").is_some_and(|v| v.trim() == "1")
}

fn parse_action(node: Node<'_, '_>) -> ActionBinding {
    let mut action = ActionBinding::unbound(node.tag_name().name(), ActionKind::Button);
    let mut axis_input = None;
    let mut inverted = false;
    let mut deadzone = 0.0;
    for child in node.children().filter(Node::is_element) {
        match child.tag_name().name() {
            "Primary" => action.primary = parse_input(child),
            "Secondary" => action.secondary = parse_input(child),
            "Binding" => {
                action.kind = ActionKind::Axis;
                axis_input = parse_input(child);
            }
            "Inverted" => inverted = flag(child),
            "Deadzone" => {
                deadzone = attr(child, "Value")
                    .and_then(|v| v.trim().parse::<f32>().ok())
                    .filter(|d| d.is_finite())
                    .unwrap_or(0.0);
            }
            "ToggleOn" => action.toggle_on = Some(flag(child)),
            other => action
                .settings
                .push((other.to_owned(), value_or_text(child))),
        }
    }
    if action.kind == ActionKind::Axis {
        action.axis = axis_input.map(|input| AxisBinding {
            input,
            inverted,
            deadzone,
        });
    }
    action
}

/// Parses a `<Primary>`, `<Secondary>` or `<Binding>` slot; `None` when unbound.
fn parse_input(node: Node<'_, '_>) -> Option<Input> {
    let device = attr(node, "Device").map(str::trim).unwrap_or_default();
    let key = attr(node, "Key").map(str::trim).unwrap_or_default();
    if device.is_empty() || device == NO_DEVICE || key.is_empty() {
        return None;
    }
    let device_index = attr(node, "DeviceIndex")
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(0);
    let mut input = Input::new(device, normalize_key(key));
    input.device_index = device_index;
    for child in node.children().filter(Node::is_element) {
        match child.tag_name().name() {
            "Modifier" => {
                let device = attr(child, "Device").map(str::trim).unwrap_or_default();
                let key = attr(child, "Key").map(str::trim).unwrap_or_default();
                if !device.is_empty() && device != NO_DEVICE && !key.is_empty() {
                    input
                        .modifiers
                        .push((device.to_owned(), normalize_key(key)));
                }
            }
            "Hold" => input.hold = attr(child, "Value").is_none_or(|v| v.trim() != "0"),
            _ => {}
        }
    }
    Some(input)
}

/// Normalises the `pos_`/`neg_` prefix case some presets use.
fn normalize_key(key: &str) -> String {
    let prefix = key.get(..4);
    match prefix {
        Some(p) if p.eq_ignore_ascii_case("pos_") && p != "Pos_" => format!("Pos_{}", &key[4..]),
        Some(p) if p.eq_ignore_ascii_case("neg_") && p != "Neg_" => format!("Neg_{}", &key[4..]),
        _ => key.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"<?xml version="1.0" encoding="UTF-8" ?>
<Root PresetName="Custom" MajorVersion="4" MinorVersion="2">
	<KeyboardLayout>en-US</KeyboardLayout>
	<MouseXMode Value="Bindings_MouseRoll" />
	<MouseGUI Value="0" />
	<YawAxisRaw>
		<Binding Device="SaitekX52Pro" Key="Joy_RZAxis" />
		<Inverted Value="1" />
		<Deadzone Value="0.05000000" />
	</YawAxisRaw>
	<RollAxisRaw>
		<Binding Device="{NoDevice}" Key="" />
		<Inverted Value="0" />
		<Deadzone Value="0.00000000" />
	</RollAxisRaw>
	<YawLeftButton>
		<Primary Device="Keyboard" Key="Key_A" />
		<Secondary Device="{NoDevice}" Key="" />
	</YawLeftButton>
	<PhotoCameraToggle>
		<Primary Device="Keyboard" Key="Key_End">
			<Modifier Device="Keyboard" Key="Key_LeftControl" />
			<Modifier Device="Keyboard" Key="Key_LeftAlt" />
		</Primary>
		<Secondary Device="231D0200" DeviceIndex="1" Key="Joy_POV1Up" />
	</PhotoCameraToggle>
	<HumanoidItemWheelButton>
		<Primary Device="Keyboard" Key="Key_LeftAlt">
			<Hold Value="1" />
		</Primary>
		<Secondary Device="Mouse" key="neg_Mouse_ZAxis" />
		<ToggleOn Value="1" />
	</HumanoidItemWheelButton>
	<BuggyToggleReverseThrottleInput>
		<Primary Device="Keyboard" Key="" />
		<Secondary Device="{NoDevice}" Key="" />
		<ToggleOn />
	</BuggyToggleReverseThrottleInput>
	<MouseGUI Value="1" />
</Root>
"#;

    #[test]
    fn parses_header_settings_and_layout() {
        let b = Bindings::parse(SAMPLE).unwrap();
        assert_eq!(b.preset_name, "Custom");
        assert_eq!(b.version, Some(Version { major: 4, minor: 2 }));
        assert_eq!(b.version.unwrap().to_string(), "4.2");
        assert_eq!(b.keyboard_layout.as_deref(), Some("en-US"));
        assert_eq!(b.setting("MouseXMode"), Some("Bindings_MouseRoll"));
        // Repeated settings: the last occurrence wins.
        assert_eq!(b.setting("MouseGUI"), Some("1"));
        assert_eq!(b.setting("Nope"), None);
        assert_eq!(b.actions.len(), 6);
    }

    #[test]
    fn parses_axes_buttons_modifiers_and_hold() {
        let b = Bindings::parse(SAMPLE).unwrap();

        let yaw = b.action("YawAxisRaw").unwrap();
        assert_eq!(yaw.kind, ActionKind::Axis);
        let axis = yaw.axis.as_ref().unwrap();
        assert_eq!(axis.input.device, "SaitekX52Pro");
        assert_eq!(axis.input.key, "Joy_RZAxis");
        assert!(axis.inverted);
        assert!((axis.deadzone - 0.05).abs() < 1e-6);

        let roll = b.action("RollAxisRaw").unwrap();
        assert_eq!(roll.kind, ActionKind::Axis);
        assert!(!roll.is_bound());

        let yaw_left = b.action("YawLeftButton").unwrap();
        assert_eq!(yaw_left.kind, ActionKind::Button);
        assert_eq!(yaw_left.primary, Some(Input::new("Keyboard", "Key_A")));
        assert_eq!(yaw_left.secondary, None);
        assert_eq!(yaw_left.toggle_on, None);

        let cam = b.action("PhotoCameraToggle").unwrap();
        let primary = cam.primary.as_ref().unwrap();
        assert_eq!(
            primary.modifiers,
            vec![
                ("Keyboard".to_owned(), "Key_LeftControl".to_owned()),
                ("Keyboard".to_owned(), "Key_LeftAlt".to_owned())
            ]
        );
        assert_eq!(primary.display(), "LCtrl + LAlt + End");
        let secondary = cam.secondary.as_ref().unwrap();
        assert_eq!(secondary.device_index, 1);
        assert_eq!(secondary.display(), "VKB 0200 #2 Hat 1 Up");

        let wheel = b.action("HumanoidItemWheelButton").unwrap();
        assert!(wheel.primary.as_ref().unwrap().hold);
        assert_eq!(wheel.toggle_on, Some(true));
        // Lower-case `key` attribute and `neg_` prefix are both normalised.
        assert_eq!(wheel.secondary.as_ref().unwrap().key, "Neg_Mouse_ZAxis");
        assert_eq!(wheel.secondary.as_ref().unwrap().display(), "Wheel Down");

        // A device with an empty key is unbound; `<ToggleOn />` without a value reads as false.
        let reverse = b.action("BuggyToggleReverseThrottleInput").unwrap();
        assert!(!reverse.is_bound());
        assert_eq!(reverse.toggle_on, Some(false));
    }

    #[test]
    fn collects_devices_and_counts() {
        let b = Bindings::parse(SAMPLE).unwrap();
        let devices: Vec<&str> = b.devices.iter().map(String::as_str).collect();
        assert_eq!(devices, ["231D0200", "Keyboard", "Mouse", "SaitekX52Pro"]);
        assert_eq!(b.binding_count(), 6);
        assert_eq!(b.bound_actions().count(), 4);
        assert!(b.is_bound("YawLeftButton"));
        assert!(!b.is_bound("RollAxisRaw"));
        assert!(!b.is_bound("NotInFile"));
        let slots: Vec<Slot> = b
            .action("PhotoCameraToggle")
            .unwrap()
            .inputs()
            .map(|(s, _)| s)
            .collect();
        assert_eq!(slots, [Slot::Primary, Slot::Secondary]);
    }

    #[test]
    fn repeated_action_elements_keep_the_last() {
        let xml = r#"<Root PresetName="Dup">
            <CamZoomIn><Primary Device="Keyboard" Key="Key_Z"/><Secondary Device="{NoDevice}" Key=""/></CamZoomIn>
            <CamZoomOut><Primary Device="Keyboard" Key="Key_X"/><Secondary Device="{NoDevice}" Key=""/></CamZoomOut>
            <CamZoomIn><Primary Device="Keyboard" Key="Key_Q"/><Secondary Device="{NoDevice}" Key=""/></CamZoomIn>
        </Root>"#;
        let b = Bindings::parse(xml).unwrap();
        let ids: Vec<&str> = b.actions.iter().map(|a| a.action.as_str()).collect();
        assert_eq!(ids, ["CamZoomIn", "CamZoomOut"]);
        assert_eq!(
            b.action("CamZoomIn").unwrap().primary.as_ref().unwrap().key,
            "Key_Q"
        );
    }

    #[test]
    fn builtin_preset_without_version_parses() {
        let xml = r#"<Root PresetName="KeyboardMouseOnly" SortOrder="0">
            <UI_Select><Primary Device="Keyboard" Key="Key_Space"/><Secondary Device="{NoDevice}" key=""/></UI_Select>
        </Root>"#;
        let b = Bindings::parse(xml).unwrap();
        assert_eq!(b.version, None);
        assert_eq!(b.keyboard_layout, None);
        assert!(b.is_bound("UI_Select"));
    }

    #[test]
    fn rejects_malformed_and_foreign_xml() {
        assert!(matches!(
            Bindings::parse("<Root><Oops></Root>"),
            Err(BindsError::Xml(_))
        ));
        assert!(
            matches!(Bindings::parse("<Other/>"), Err(BindsError::NotBindings(name)) if name == "Other")
        );
        assert!(matches!(Bindings::parse(""), Err(BindsError::Xml(_))));
    }

    #[test]
    fn decodes_bom_and_utf16() {
        let with_bom = format!("\u{feff}{SAMPLE}");
        assert_eq!(Bindings::parse(&with_bom).unwrap().preset_name, "Custom");

        let mut utf16 = vec![0xFF, 0xFE];
        for unit in SAMPLE.encode_utf16() {
            utf16.extend_from_slice(&unit.to_le_bytes());
        }
        let text = decode_text(&utf16);
        assert_eq!(Bindings::parse(&text).unwrap().actions.len(), 6);

        let mut utf8_bom = vec![0xEF, 0xBB, 0xBF];
        utf8_bom.extend_from_slice(SAMPLE.as_bytes());
        assert_eq!(
            Bindings::parse(&decode_text(&utf8_bom))
                .unwrap()
                .preset_name,
            "Custom"
        );
    }

    #[test]
    fn load_reports_missing_file() {
        let err = Bindings::load("definitely/not/here.binds").unwrap_err();
        assert!(matches!(err, BindsError::Io { .. }));
        assert!(err.to_string().contains("here.binds"));
    }

    #[test]
    fn key_normalisation() {
        assert_eq!(normalize_key("neg_Joy_XAxis"), "Neg_Joy_XAxis");
        assert_eq!(normalize_key("POS_Joy_XAxis"), "Pos_Joy_XAxis");
        assert_eq!(normalize_key("Pos_Joy_XAxis"), "Pos_Joy_XAxis");
        assert_eq!(normalize_key("Key_A"), "Key_A");
        assert_eq!(normalize_key("Ü"), "Ü");
    }
}
