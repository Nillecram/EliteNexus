//! Physical inputs (keys, buttons, hat directions, axes), the devices they live on, and
//! human-readable labels for both.

use std::borrow::Cow;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::util::humanize;

/// One bound input: a key, button, hat direction or axis on a device, optionally combined with
/// modifier keys.
///
/// Unbound slots (`Device="{NoDevice}"` or an empty `Key`) are never represented as an `Input`;
/// they show up as `None` in [`ActionBinding`](crate::ActionBinding).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Input {
    /// Device id exactly as the game writes it: `Keyboard`, `Mouse`, `GamePad`, `SaitekX52Pro`,
    /// … or an 8-digit `VVVVPPPP` USB vendor/product id (e.g. `231D0200`) for devices the game
    /// has no name for.
    pub device: String,
    /// The `DeviceIndex` attribute that tells identical devices apart (0 for the first one).
    pub device_index: u32,
    /// Key name: `Key_A`, `Mouse_2`, `Joy_5`, `Joy_POV1Up`, `Joy_XAxis`, `Pos_Mouse_ZAxis`, ….
    /// A lower-case `pos_`/`neg_` prefix is normalised to `Pos_`/`Neg_` while parsing.
    pub key: String,
    /// Keys that must be held together with [`key`](Self::key), as `(device, key)` pairs in file
    /// order.
    pub modifiers: Vec<(String, String)>,
    /// Long-press binding (`<Hold Value="1"/>`).
    pub hold: bool,
}

impl Input {
    /// An unmodified, non-hold input on the first device with that id.
    pub fn new(device: impl Into<String>, key: impl Into<String>) -> Self {
        Self {
            device: device.into(),
            device_index: 0,
            key: key.into(),
            modifiers: Vec::new(),
            hold: false,
        }
    }

    /// Adds a modifier key (builder style).
    #[must_use]
    pub fn with_modifier(mut self, device: impl Into<String>, key: impl Into<String>) -> Self {
        self.modifiers.push((device.into(), key.into()));
        self
    }

    /// Marks the input as a long-press binding (builder style).
    #[must_use]
    pub fn held(mut self) -> Self {
        self.hold = true;
        self
    }

    /// What kind of device the input is on.
    pub fn device_kind(&self) -> DeviceKind {
        DeviceKind::of(&self.device, &self.key)
    }

    /// True for a full analogue axis (`Joy_XAxis`, `Mouse_YAxis`, `GamePad_LStickX`, …); false for
    /// buttons and for the digital halves of an axis (`Pos_Joy_ZAxis`).
    pub fn is_analogue(&self) -> bool {
        if split_direction(&self.key).0.is_some() {
            return false;
        }
        let key = self.key.as_str();
        key.ends_with("Axis")
            || key.contains("Stick")
            || key.ends_with("Trigger")
            || key.ends_with("TriggerLeft")
            || key.ends_with("TriggerRight")
    }

    /// Label of the key alone, without device or modifiers: `A`, `Num 7`, `Btn 5`, `Hat 1 Up`,
    /// `RZ Axis`, `Wheel Up`.
    pub fn key_label(&self) -> String {
        key_label(&self.key)
    }

    /// Full human-readable form, e.g. `LShift + A`, `Mouse 2`, `Wheel Up`, `X52 Pro Btn 5`,
    /// `X52 Pro Btn 3 + Btn 5` (modifier on the same stick), `Pad LB (hold)`.
    ///
    /// Keyboard and mouse inputs carry no device prefix; every other device is prefixed with its
    /// short name ([`device_short_name`]) and, for the 2nd+ identical device, `#n`.
    pub fn display(&self) -> String {
        let mut out = String::with_capacity(16);
        let mut prev_device: Option<&str> = None;
        let modifiers = self
            .modifiers
            .iter()
            .map(|(d, k)| (d.as_str(), 0, k.as_str()));
        let main = std::iter::once((self.device.as_str(), self.device_index, self.key.as_str()));
        for (i, (device, index, key)) in modifiers.chain(main).enumerate() {
            if i > 0 {
                out.push_str(" + ");
            }
            if needs_device_prefix(device) && prev_device != Some(device) {
                out.push_str(&device_short_name(device));
                if index > 0 {
                    out.push_str(" #");
                    out.push_str(&(index + 1).to_string());
                }
                out.push(' ');
            }
            out.push_str(&key_label(key));
            prev_device = Some(device);
        }
        if self.hold {
            out.push_str(" (hold)");
        }
        out
    }
}

impl fmt::Display for Input {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.display())
    }
}

/// Broad device family, used to group and colour-code bindings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum DeviceKind {
    Keyboard,
    Mouse,
    /// Joysticks, throttles, pedals, button boxes, virtual joysticks.
    Joystick,
    Gamepad,
    /// VR motion controllers.
    Vr,
    /// Head trackers and anything else.
    Other,
}

impl DeviceKind {
    /// Classifies a device id; `key` is used as a hint for devices the game names by VID/PID.
    pub fn of(device: &str, key: &str) -> Self {
        match device {
            "Keyboard" => return Self::Keyboard,
            "Mouse" => return Self::Mouse,
            "GamePad"
            | "XB360 Pad"
            | "DualShock4"
            | "Playstation3Controller"
            | "GioteckPS3WiredController"
            | "Logitech710WirelessGamepad"
            | "SaitekP2500RumbleForce"
            | "SaitekP2600RumbleForce"
            | "XterminatorDualControl" => return Self::Gamepad,
            "OculusTouch" => return Self::Vr,
            "EDTracker" => return Self::Other,
            _ => {}
        }
        let (_, base) = split_direction(key);
        if base.starts_with("Pad_") || base.starts_with("GamePad_") {
            Self::Gamepad
        } else if base.starts_with("Key_") {
            Self::Keyboard
        } else {
            Self::Joystick
        }
    }

    /// Display label (`Keyboard`, `Joystick / HOTAS`, …).
    pub fn label(self) -> &'static str {
        match self {
            Self::Keyboard => "Keyboard",
            Self::Mouse => "Mouse",
            Self::Joystick => "Joystick / HOTAS",
            Self::Gamepad => "Gamepad",
            Self::Vr => "VR controller",
            Self::Other => "Other",
        }
    }
}

/// `(device id, full name, short name)` for every device in the game's `DeviceMappings.xml`.
const KNOWN_DEVICES: &[(&str, &str, &str)] = &[
    ("Keyboard", "Keyboard", "Keyboard"),
    ("Mouse", "Mouse", "Mouse"),
    ("GamePad", "Gamepad", "Pad"),
    ("XB360 Pad", "Xbox 360 Controller", "Pad"),
    ("DualShock4", "DualShock 4", "DS4"),
    ("Playstation3Controller", "PlayStation 3 Controller", "PS3"),
    (
        "GioteckPS3WiredController",
        "Gioteck PS3 Wired Controller",
        "PS3",
    ),
    (
        "Logitech710WirelessGamepad",
        "Logitech F710 Gamepad",
        "F710",
    ),
    ("OculusTouch", "Oculus Touch", "Touch"),
    ("SaitekX52", "Saitek X52", "X52"),
    ("SaitekX52Pro", "Saitek X52 Pro", "X52 Pro"),
    ("SaitekX55Joystick", "Saitek X55 Stick", "X55 Stick"),
    ("SaitekX55Throttle", "Saitek X55 Throttle", "X55 Throttle"),
    ("SaitekX56Joystick", "Saitek X56 Stick", "X56 Stick"),
    ("SaitekX56Throttle", "Saitek X56 Throttle", "X56 Throttle"),
    ("SaitekX45", "Saitek X45", "X45"),
    ("SaitekAV8R03", "Saitek AV8R", "AV8R"),
    ("SaitekFLY5", "Saitek F.L.Y. 5", "FLY 5"),
    ("SaitekST290Pro", "Saitek ST290 Pro", "ST290"),
    (
        "SaitekP2500RumbleForce",
        "Saitek P2500 Rumble Force",
        "P2500",
    ),
    (
        "SaitekP2600RumbleForce",
        "Saitek P2600 Rumble Force",
        "P2600",
    ),
    (
        "SaitekCyborgEvoWireless",
        "Saitek Cyborg Evo Wireless",
        "Cyborg Evo",
    ),
    (
        "SaitekProFlightRudderPedals",
        "Saitek Pro Flight Rudder Pedals",
        "Pedals",
    ),
    (
        "SaitekProFlightCombatRudderPedals",
        "Saitek Pro Flight Combat Rudder Pedals",
        "Pedals",
    ),
    ("LogitechG940Joystick", "Logitech G940 Stick", "G940 Stick"),
    (
        "LogitechG940Throttle",
        "Logitech G940 Throttle",
        "G940 Throttle",
    ),
    ("LogitechG940Pedals", "Logitech G940 Pedals", "G940 Pedals"),
    ("LogitechExtreme3DPro", "Logitech Extreme 3D Pro", "3D Pro"),
    (
        "LogitechWingManStrikeForce3D",
        "Logitech WingMan Strike Force 3D",
        "WingMan",
    ),
    (
        "ThrustMasterWarthogJoystick",
        "Thrustmaster Warthog Stick",
        "Warthog Stick",
    ),
    (
        "ThrustMasterWarthogThrottle",
        "Thrustmaster Warthog Throttle",
        "Warthog Throttle",
    ),
    (
        "ThrustMasterWarthogCombined",
        "Thrustmaster Warthog (combined)",
        "Warthog",
    ),
    (
        "ThrustMasterTFlightHOTASX",
        "Thrustmaster T.Flight HOTAS X",
        "HOTAS X",
    ),
    (
        "ThrustMasterHOTAS4",
        "Thrustmaster T.Flight HOTAS 4",
        "HOTAS 4",
    ),
    (
        "TFlightHotasOne",
        "Thrustmaster T.Flight HOTAS One",
        "HOTAS One",
    ),
    ("T16000M", "Thrustmaster T.16000M Stick", "T16000M"),
    ("T16000MTHROTTLE", "Thrustmaster TWCS Throttle", "TWCS"),
    ("T-Rudder", "Thrustmaster T-Rudder", "T-Rudder"),
    ("TMwTARGET", "Thrustmaster TARGET (virtual)", "TARGET"),
    ("BlackWidow", "Speedlink Black Widow", "Black Widow"),
    ("EDTracker", "EDTracker", "EDTracker"),
    ("vJoy", "vJoy", "vJoy"),
    ("CHProThrottle1", "CH Pro Throttle", "CH Throttle"),
    ("CHProThrottle2", "CH Pro Throttle", "CH Throttle"),
    ("CHFighterStick", "CH Fighterstick", "CH Fighterstick"),
    ("CHCombatStick", "CH Combatstick", "CH Combatstick"),
    ("CHProPedals", "CH Pro Pedals", "CH Pedals"),
    ("TrustPredator", "Trust Predator", "Predator"),
    (
        "XterminatorDualControl",
        "Xterminator Dual Control",
        "Xterminator",
    ),
    (
        "SideWinderForceFeedback2",
        "Microsoft SideWinder Force Feedback 2",
        "SideWinder",
    ),
    ("ArduinoLeonardo", "Arduino Leonardo", "Leonardo"),
    (
        "SlawFlightControlRudder",
        "Slaw Flight Control Rudder",
        "Slaw Rudder",
    ),
    ("VPCPanel", "Virpil Control Panel", "VPC Panel"),
    ("VPCThrottle", "Virpil Throttle", "VPC Throttle"),
    ("GenericJoystick", "Joystick", "Joy"),
];

/// USB vendor ids of common flight-sim hardware, for devices the game names by `VVVVPPPP`.
const VENDORS: &[(&str, &str)] = &[
    ("231D", "VKB"),
    ("3344", "Virpil"),
    ("044F", "Thrustmaster"),
    ("06A3", "Saitek"),
    ("0738", "Mad Catz"),
    ("046D", "Logitech"),
    ("068E", "CH Products"),
    ("045E", "Microsoft"),
    ("054C", "Sony"),
    ("4098", "WinWing"),
    ("10F5", "Turtle Beach"),
    ("1DD2", "Leo Bodnar"),
    ("16C0", "Teensy"),
    ("2341", "Arduino"),
    ("1B4F", "SparkFun"),
    ("2E8A", "Raspberry Pi"),
    ("1209", "Open hardware"),
    ("0483", "STMicro"),
    ("03EB", "Atmel"),
    ("1234", "vJoy"),
    ("28DE", "Valve"),
    ("2833", "Oculus"),
    ("0EB7", "Fanatec"),
    ("346E", "Moza"),
    ("0079", "DragonRise"),
];

fn known_device(device: &str) -> Option<&'static (&'static str, &'static str, &'static str)> {
    KNOWN_DEVICES.iter().find(|(id, _, _)| *id == device)
}

/// Splits an 8-hex-digit `VVVVPPPP` device id into vendor and product id.
fn vid_pid(device: &str) -> Option<(&str, &str)> {
    (device.len() == 8 && device.bytes().all(|b| b.is_ascii_hexdigit())).then(|| device.split_at(4))
}

fn vendor_name(vid: &str) -> Option<&'static str> {
    VENDORS
        .iter()
        .find(|(v, _)| v.eq_ignore_ascii_case(vid))
        .map(|(_, name)| *name)
}

/// Full display name of a device id: `Saitek X52 Pro`, `VKB device (231D:0200)`, ….
pub fn device_name(device: &str) -> Cow<'static, str> {
    if let Some((_, name, _)) = known_device(device) {
        return Cow::Borrowed(name);
    }
    if let Some((vid, pid)) = vid_pid(device) {
        let (vid, pid) = (vid.to_ascii_uppercase(), pid.to_ascii_uppercase());
        return Cow::Owned(match vendor_name(&vid) {
            Some(vendor) => format!("{vendor} device ({vid}:{pid})"),
            None => format!("USB device {vid}:{pid}"),
        });
    }
    Cow::Owned(humanize(device))
}

/// Short device label used as a prefix in [`Input::display`]: `X52 Pro`, `Pad`, `VKB 0200`, ….
pub fn device_short_name(device: &str) -> Cow<'static, str> {
    if let Some((_, _, short)) = known_device(device) {
        return Cow::Borrowed(short);
    }
    if let Some((vid, pid)) = vid_pid(device) {
        let (vid, pid) = (vid.to_ascii_uppercase(), pid.to_ascii_uppercase());
        return Cow::Owned(match vendor_name(&vid) {
            Some(vendor) => format!("{vendor} {pid}"),
            None => format!("{vid}:{pid}"),
        });
    }
    Cow::Owned(humanize(device))
}

fn needs_device_prefix(device: &str) -> bool {
    !matches!(device, "Keyboard" | "Mouse")
}

/// Splits a `Pos_`/`Neg_` prefix (the digital halves of an axis) off a key name.
pub(crate) fn split_direction(key: &str) -> (Option<char>, &str) {
    if let Some(rest) = key.strip_prefix("Pos_") {
        (Some('+'), rest)
    } else if let Some(rest) = key.strip_prefix("Neg_") {
        (Some('-'), rest)
    } else {
        (None, key)
    }
}

/// Human-readable label for a key name, independent of the device it is on.
pub fn key_label(key: &str) -> String {
    let (direction, base) = split_direction(key);
    let (label, direction_used) = match base.split_once('_') {
        Some(("Key", rest)) => (keyboard_label(rest), false),
        Some(("Mouse", rest)) => mouse_label(rest, direction),
        Some(("Joy", rest)) => (joystick_label(rest), false),
        Some(("Pad" | "GamePad", rest)) => (gamepad_label(rest), false),
        // The device prefix already says "Touch".
        Some(("OculusTouch", rest)) => (humanize(rest), false),
        _ => (humanize(base), false),
    };
    match direction {
        Some(sign) if !direction_used => format!("{label} {sign}"),
        _ => label,
    }
}

fn keyboard_label(name: &str) -> String {
    if let Some(num) = name.strip_prefix("Numpad_") {
        let sym = match num {
            "Multiply" => "*",
            "Subtract" => "-",
            "Add" => "+",
            "Divide" => "/",
            "Decimal" => ".",
            "Equals" => "=",
            "Comma" => ",",
            "Enter" => "Enter",
            other => return format!("Num {}", humanize(other)),
        };
        return format!("Num {sym}");
    }
    let label = match name {
        "LeftShift" => "LShift",
        "RightShift" => "RShift",
        "LeftControl" => "LCtrl",
        "RightControl" => "RCtrl",
        "LeftAlt" => "LAlt",
        "RightAlt" => "RAlt",
        "LeftWin" => "LWin",
        "RightWin" => "RWin",
        "Apps" => "Menu",
        "Escape" => "Esc",
        "CapsLock" => "Caps Lock",
        "NumLock" => "Num Lock",
        "ScrollLock" => "Scroll Lock",
        "Minus" => "-",
        "Equals" => "=",
        "LeftBracket" => "[",
        "RightBracket" => "]",
        "SemiColon" => ";",
        "Apostrophe" => "'",
        "Grave" => "`",
        "BackSlash" => "\\",
        "Comma" => ",",
        "Period" => ".",
        "Slash" => "/",
        "Colon" => ":",
        "AT" => "@",
        "Underline" => "_",
        "UpArrow" => "Up Arrow",
        "DownArrow" => "Down Arrow",
        "LeftArrow" => "Left Arrow",
        "RightArrow" => "Right Arrow",
        "PageUp" => "Page Up",
        "PageDown" => "Page Down",
        "SYSRQ" => "Print Screen",
        "OEM_102" => "OEM 102 (\\|)",
        "PrevTrack" => "Previous Track",
        "NextTrack" => "Next Track",
        "PlayPause" => "Play/Pause",
        "MediaStop" => "Media Stop",
        "VolumeDown" => "Volume Down",
        "VolumeUp" => "Volume Up",
        "GreenModifier" => "Green Modifier",
        "OrangeModifier" => "Orange Modifier",
        // Letters, digits and function keys (`A`, `7`, `F12`) are already readable.
        short if short.len() <= 3 && short.bytes().all(|b| b.is_ascii_alphanumeric()) => short,
        other => return humanize(other),
    };
    label.to_owned()
}

fn mouse_label(name: &str, direction: Option<char>) -> (String, bool) {
    if !name.is_empty() && name.bytes().all(|b| b.is_ascii_digit()) {
        return (format!("Mouse {name}"), false);
    }
    match (name, direction) {
        ("ZAxis", Some('+')) => ("Wheel Up".to_owned(), true),
        ("ZAxis", Some(_)) => ("Wheel Down".to_owned(), true),
        ("ZAxis", None) => ("Mouse Wheel".to_owned(), false),
        ("XAxis", _) => ("Mouse X".to_owned(), false),
        ("YAxis", _) => ("Mouse Y".to_owned(), false),
        (other, _) => (format!("Mouse {}", humanize(other)), false),
    }
}

fn joystick_label(name: &str) -> String {
    if !name.is_empty() && name.bytes().all(|b| b.is_ascii_digit()) {
        return format!("Btn {name}");
    }
    if let Some(rest) = name.strip_prefix("POV") {
        let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
        let (number, direction) = rest.split_at(digits);
        if !number.is_empty() {
            return format!("Hat {number} {}", humanize(direction))
                .trim_end()
                .to_owned();
        }
    }
    if let Some(axis) = name.strip_suffix("Axis").filter(|a| !a.is_empty()) {
        return format!("{axis} Axis");
    }
    humanize(name)
}

fn gamepad_label(name: &str) -> String {
    let label = match name {
        "A" | "FaceDown" => "A",
        "B" | "FaceRight" => "B",
        "X" | "FaceLeft" => "X",
        "Y" | "FaceUp" => "Y",
        "LBumper" => "LB",
        "RBumper" => "RB",
        "LTrigger" => "LT",
        "RTrigger" => "RT",
        "LThumb" => "LS Click",
        "RThumb" => "RS Click",
        "DPadUp" => "D-Pad Up",
        "DPadDown" => "D-Pad Down",
        "DPadLeft" => "D-Pad Left",
        "DPadRight" => "D-Pad Right",
        "LStickX" => "LS X",
        "LStickY" => "LS Y",
        "RStickX" => "RS X",
        "RStickY" => "RS Y",
        other => return humanize(other),
    };
    label.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyboard_labels() {
        assert_eq!(key_label("Key_A"), "A");
        assert_eq!(key_label("Key_LeftShift"), "LShift");
        assert_eq!(key_label("Key_Numpad_7"), "Num 7");
        assert_eq!(key_label("Key_Numpad_Multiply"), "Num *");
        assert_eq!(key_label("Key_Grave"), "`");
        assert_eq!(key_label("Key_F12"), "F12");
        assert_eq!(key_label("Key_UpArrow"), "Up Arrow");
        assert_eq!(key_label("Key_PageDown"), "Page Down");
        // Unknown and non-ASCII key names degrade gracefully.
        assert_eq!(key_label("Key_Ü"), "Ü");
        assert_eq!(key_label("Key_SomeNewKey"), "Some New Key");
    }

    #[test]
    fn mouse_labels() {
        assert_eq!(key_label("Mouse_2"), "Mouse 2");
        assert_eq!(key_label("Pos_Mouse_ZAxis"), "Wheel Up");
        assert_eq!(key_label("Neg_Mouse_ZAxis"), "Wheel Down");
        assert_eq!(key_label("Mouse_XAxis"), "Mouse X");
        assert_eq!(key_label("Pos_Mouse_YAxis"), "Mouse Y +");
    }

    #[test]
    fn joystick_and_gamepad_labels() {
        assert_eq!(key_label("Joy_5"), "Btn 5");
        assert_eq!(key_label("Joy_POV1Up"), "Hat 1 Up");
        assert_eq!(key_label("Joy_POV2Left"), "Hat 2 Left");
        assert_eq!(key_label("Joy_XAxis"), "X Axis");
        assert_eq!(key_label("Joy_RZAxis"), "RZ Axis");
        assert_eq!(key_label("Neg_Joy_UAxis"), "U Axis -");
        assert_eq!(key_label("Pad_LBumper"), "LB");
        assert_eq!(key_label("GamePad_FaceDown"), "A");
        assert_eq!(key_label("Pos_GamePad_RTrigger"), "RT +");
        assert_eq!(
            key_label("OculusTouch_HandTriggerLeft"),
            "Hand Trigger Left"
        );
        let touch = Input::new("OculusTouch", "Pos_OculusTouch_LStickY")
            .with_modifier("OculusTouch", "OculusTouch_A");
        assert_eq!(touch.display(), "Touch A + L Stick Y +");
    }

    #[test]
    fn display_with_modifiers_devices_and_hold() {
        let i = Input::new("Keyboard", "Key_A").with_modifier("Keyboard", "Key_LeftShift");
        assert_eq!(i.display(), "LShift + A");
        assert_eq!(Input::new("Mouse", "Mouse_2").display(), "Mouse 2");
        assert_eq!(
            Input::new("SaitekX52Pro", "Joy_5").display(),
            "X52 Pro Btn 5"
        );
        let same_stick = Input::new("SaitekX52Pro", "Joy_5").with_modifier("SaitekX52Pro", "Joy_3");
        assert_eq!(same_stick.display(), "X52 Pro Btn 3 + Btn 5");
        let mixed =
            Input::new("231D0200", "Joy_POV1Up").with_modifier("Keyboard", "Key_LeftControl");
        assert_eq!(mixed.display(), "LCtrl + VKB 0200 Hat 1 Up");
        let mut second = Input::new("T16000M", "Joy_1");
        second.device_index = 1;
        assert_eq!(second.display(), "T16000M #2 Btn 1");
        assert_eq!(
            Input::new("GamePad", "Pad_LBumper").held().display(),
            "Pad LB (hold)"
        );
        assert_eq!(
            Input::new("Keyboard", "Key_B").held().to_string(),
            "B (hold)"
        );
    }

    #[test]
    fn device_names_and_kinds() {
        assert_eq!(device_name("SaitekX52Pro"), "Saitek X52 Pro");
        assert_eq!(device_name("231d0200"), "VKB device (231D:0200)");
        assert_eq!(device_name("ABCD1234"), "USB device ABCD:1234");
        assert_eq!(device_short_name("33448194"), "Virpil 8194");
        assert_eq!(device_name("SomeNewStick"), "Some New Stick");
        assert_eq!(DeviceKind::of("Keyboard", "Key_A"), DeviceKind::Keyboard);
        assert_eq!(DeviceKind::of("Mouse", "Mouse_1"), DeviceKind::Mouse);
        assert_eq!(DeviceKind::of("GamePad", "Pad_A"), DeviceKind::Gamepad);
        assert_eq!(DeviceKind::of("045E02FF", "Pad_A"), DeviceKind::Gamepad);
        assert_eq!(DeviceKind::of("231D0200", "Joy_1"), DeviceKind::Joystick);
        assert_eq!(
            DeviceKind::of("OculusTouch", "OculusTouch_A"),
            DeviceKind::Vr
        );
    }

    #[test]
    fn analogue_detection() {
        assert!(Input::new("SaitekX52Pro", "Joy_XAxis").is_analogue());
        assert!(Input::new("GamePad", "GamePad_LStickX").is_analogue());
        assert!(!Input::new("SaitekX52Pro", "Pos_Joy_XAxis").is_analogue());
        assert!(!Input::new("Keyboard", "Key_A").is_analogue());
        assert!(!Input::new("Mouse", "Mouse_1").is_analogue());
    }
}
