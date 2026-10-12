//! Ship jump parameters derived from a journal `Loadout` (or a SLEF build), for the galaxy
//! (exact) plotter and for picking a neutron-plotter range.
//!
//! Mirrors Spansh's own `parseSLEF()`: FSD constants by module symbol, engineering modifiers
//! `FSDOptimalMass` / `MaxFuelPerJump` replace the base values, `base_mass = UnladenMass +
//! FuelCapacity.Reserve`, the Guardian FSD booster becomes `range_boost`.
//!
//! FSD constants are game facts as listed in EDDiscovery's module table (Apache-2.0,
//! `EliteDangerousCore/EliteDangerous/FrontierData/Items/ModuleList.cs`); the SCO Mk II row uses
//! Spansh's own override (`fsd_mapping`, fuel power 2.5025, supercharge ×6).

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// Constants of one Frame Shift Drive module.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct FsdSpec {
    /// Module size 2–8.
    pub size: u8,
    /// `A`–`E`.
    pub rating: char,
    /// Supercruise Overcharge (SCO) drive.
    pub sco: bool,
    /// Optimal mass, t (after engineering when built from a loadout).
    pub optimal_mass: f64,
    /// Maximum fuel per jump, t (after engineering when built from a loadout).
    pub max_fuel_per_jump: f64,
    /// Linear constant / 1000 (Spansh `fuel_multiplier`, e.g. 0.012 for an A rating).
    pub fuel_multiplier: f64,
    /// Power constant (Spansh `fuel_power`, 2.0 for size 2 … 2.9 for size 8).
    pub fuel_power: f64,
    /// Neutron boost multiplier: 4, or 6 for the SCO Mk II.
    pub supercharge_multiplier: u8,
}

/// Rows: size 2–7. Columns: module `class1`…`class5` = rating E, D, C, B, A.
const CLASSIC_OPT_MASS: [[f64; 5]; 6] = [
    [48.0, 54.0, 60.0, 75.0, 90.0],
    [80.0, 90.0, 100.0, 125.0, 150.0],
    [280.0, 315.0, 350.0, 438.0, 525.0],
    [560.0, 630.0, 700.0, 875.0, 1050.0],
    [960.0, 1080.0, 1200.0, 1500.0, 1800.0],
    [1440.0, 1620.0, 1800.0, 2250.0, 2700.0],
];
const CLASSIC_MAX_FUEL: [[f64; 5]; 6] = [
    [0.6, 0.6, 0.6, 0.8, 0.9],
    [1.2, 1.2, 1.2, 1.5, 1.8],
    [2.0, 2.0, 2.0, 2.5, 3.0],
    [3.3, 3.3, 3.3, 4.1, 5.0],
    [5.3, 5.3, 5.3, 6.6, 8.0],
    [8.5, 8.5, 8.5, 10.6, 12.8],
];
const CLASSIC_LINEAR: [f64; 5] = [0.011, 0.010, 0.008, 0.010, 0.012];

/// Rows: size 2–8 (SCO). Columns as above.
const SCO_OPT_MASS: [[f64; 5]; 7] = [
    [60.0, 90.0, 90.0, 90.0, 100.0],
    [100.0, 150.0, 150.0, 150.0, 167.0],
    [350.0, 525.0, 525.0, 525.0, 585.0],
    [700.0, 1050.0, 1050.0, 1050.0, 1175.0],
    [1200.0, 1800.0, 1800.0, 1800.0, 2000.0],
    [1800.0, 2700.0, 2700.0, 2700.0, 3000.0],
    [2800.0, 4200.0, 4200.0, 4200.0, 4670.0],
];
const SCO_MAX_FUEL: [[f64; 5]; 7] = [
    [0.6, 0.9, 0.9, 0.9, 1.0],
    [1.2, 1.8, 1.8, 1.8, 1.9],
    [2.0, 3.0, 3.0, 3.0, 3.2],
    [3.3, 5.0, 5.0, 5.0, 5.2],
    [5.3, 8.0, 8.0, 8.0, 8.3],
    [8.5, 12.8, 12.8, 12.8, 13.1],
    [13.6, 20.4, 20.4, 20.4, 20.7],
];
const SCO_LINEAR: [f64; 5] = [0.008, 0.012, 0.012, 0.012, 0.013];

/// Power constant by size 2–8.
const FUEL_POWER: [f64; 7] = [2.0, 2.15, 2.3, 2.45, 2.6, 2.75, 2.9];

/// Guardian FSD booster range bonus by size 1–5, ly.
const GUARDIAN_BOOST: [f64; 5] = [4.0, 6.0, 7.75, 9.25, 10.5];

const RATINGS: [char; 5] = ['E', 'D', 'C', 'B', 'A'];

/// FSD constants for a module symbol (`int_hyperdrive_size5_class5`,
/// `int_hyperdrive_overcharge_size6_class5`, …), case-insensitive. `None` for anything else.
pub fn fsd_spec(item: &str) -> Option<FsdSpec> {
    let item = item.trim().to_ascii_lowercase();
    let rest = item.strip_prefix("int_hyperdrive_")?;
    let (sco, rest) = match rest.strip_prefix("overcharge_") {
        Some(r) => (true, r),
        None => (false, rest),
    };
    let (size, rest) = rest.strip_prefix("size")?.split_once("_class")?;
    let size: usize = size.parse().ok()?;
    let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
    let class: usize = rest[..digits].parse().ok()?;
    let suffix = &rest[digits..];
    if !(1..=5).contains(&class) {
        return None;
    }
    let col = class - 1;

    if suffix.contains("overchargebooster_mkii") {
        // FSD (SCO) Mk II: Spansh's fsd_mapping values.
        return (sco && size == 8 && class == 5).then_some(FsdSpec {
            size: 8,
            rating: 'A',
            sco: true,
            optimal_mass: 4670.0,
            max_fuel_per_jump: 6.8,
            fuel_multiplier: 0.011,
            fuel_power: 2.5025,
            supercharge_multiplier: 6,
        });
    }
    if !(suffix.is_empty() || suffix == "_free") {
        return None;
    }
    let row = size.checked_sub(2)?;
    let (opt, fuel, linear) = if sco {
        (
            SCO_OPT_MASS.get(row)?[col],
            SCO_MAX_FUEL.get(row)?[col],
            SCO_LINEAR[col],
        )
    } else {
        (
            CLASSIC_OPT_MASS.get(row)?[col],
            CLASSIC_MAX_FUEL.get(row)?[col],
            CLASSIC_LINEAR[col],
        )
    };
    Some(FsdSpec {
        size: u8::try_from(size).ok()?,
        rating: RATINGS[col],
        sco,
        optimal_mass: opt,
        max_fuel_per_jump: fuel,
        fuel_multiplier: linear,
        fuel_power: *FUEL_POWER.get(row)?,
        supercharge_multiplier: 4,
    })
}

/// Range bonus of a Guardian FSD booster symbol (`int_guardianfsdbooster_size5` → 10.5 ly).
pub fn guardian_booster_range(item: &str) -> Option<f64> {
    let item = item.trim().to_ascii_lowercase();
    let size: usize = item
        .strip_prefix("int_guardianfsdbooster_size")?
        .parse()
        .ok()?;
    GUARDIAN_BOOST.get(size.checked_sub(1)?).copied()
}

/// Why a loadout could not be used.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum LoadoutError {
    #[error("not valid JSON: {0}")]
    Json(String),
    #[error("expected a Loadout event, got {0:?}")]
    NotLoadout(String),
    #[error("no Frame Shift Drive fitted")]
    NoFsd,
    #[error("unknown Frame Shift Drive {0:?}; set the FSD constants on the request manually")]
    UnknownFsd(String),
    #[error("{0} missing or invalid")]
    Missing(&'static str),
}

/// Everything the plotters need to know about a ship.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShipProfile {
    /// Ship symbol, lower-case (`anaconda`, `krait_light`).
    pub ship: String,
    pub ship_name: Option<String>,
    /// FSD module symbol, lower-case.
    pub fsd_item: String,
    /// FSD constants with engineering applied.
    pub fsd: FsdSpec,
    /// `FSDOptimalMass` / `MaxFuelPerJump` came from engineering modifiers.
    pub fsd_engineered: bool,
    /// Guardian FSD booster bonus, ly.
    pub range_boost: Option<f64>,
    /// Main tank, t (`FuelCapacity.Main`).
    pub tank_size: f64,
    /// Reservoir, t (`FuelCapacity.Reserve`).
    pub reservoir_size: f64,
    /// Hull + modules without fuel and cargo, t.
    pub unladen_mass: f64,
    pub cargo_capacity: u32,
    /// The game's own figure ("zero cargo, just enough fuel for 1 jump").
    pub max_jump_range: Option<f64>,
    /// SLEF JSON (`[{"header":…,"data":<Loadout>}]`), sent to the galaxy plotter as `ship_build`.
    pub slef: Option<String>,
}

impl ShipProfile {
    /// From a journal `Loadout` line (as stored by the model) or a SLEF array.
    pub fn from_loadout_json(json: &str) -> Result<Self, LoadoutError> {
        let value: Value =
            serde_json::from_str(json).map_err(|e| LoadoutError::Json(e.to_string()))?;
        Self::from_loadout(&value)
    }

    /// From a parsed `Loadout` object, or a SLEF array / object (`{"header":…,"data":{…}}`).
    pub fn from_loadout(value: &Value) -> Result<Self, LoadoutError> {
        let loadout = match value {
            Value::Array(items) => items
                .first()
                .and_then(|i| i.get("data"))
                .ok_or(LoadoutError::Missing("SLEF data"))?,
            Value::Object(m) if m.contains_key("data") && m.contains_key("header") => &m["data"],
            other => other,
        };
        let obj = loadout
            .as_object()
            .ok_or(LoadoutError::Missing("Loadout object"))?;
        if let Some(event) = obj.get("event").and_then(Value::as_str)
            && event != "Loadout"
        {
            return Err(LoadoutError::NotLoadout(event.to_owned()));
        }
        let modules = obj
            .get("Modules")
            .and_then(Value::as_array)
            .ok_or(LoadoutError::Missing("Modules"))?;
        let str_field =
            |m: &Value, key: &str| m.get(key).and_then(Value::as_str).map(str::to_owned);

        let fsd_module = modules
            .iter()
            .find(|m| {
                str_field(m, "Slot").is_some_and(|s| s.eq_ignore_ascii_case("FrameShiftDrive"))
            })
            .ok_or(LoadoutError::NoFsd)?;
        let fsd_item = str_field(fsd_module, "Item")
            .ok_or(LoadoutError::Missing("FSD Item"))?
            .to_ascii_lowercase();
        let mut fsd =
            fsd_spec(&fsd_item).ok_or_else(|| LoadoutError::UnknownFsd(fsd_item.clone()))?;

        let mut fsd_engineered = false;
        if let Some(modifiers) = fsd_module
            .pointer("/Engineering/Modifiers")
            .and_then(Value::as_array)
        {
            for modifier in modifiers {
                let Some(value) = modifier
                    .get("Value")
                    .and_then(Value::as_f64)
                    .filter(|v| v.is_finite() && *v > 0.0)
                else {
                    continue;
                };
                let label = str_field(modifier, "Label").unwrap_or_default();
                if label.eq_ignore_ascii_case("FSDOptimalMass") {
                    fsd.optimal_mass = value;
                    fsd_engineered = true;
                } else if label.eq_ignore_ascii_case("MaxFuelPerJump") {
                    fsd.max_fuel_per_jump = value;
                    fsd_engineered = true;
                }
            }
        }

        // Only one booster fits; take the largest if a malformed loadout lists several.
        let range_boost = modules
            .iter()
            .filter_map(|m| m.get("Item").and_then(Value::as_str))
            .filter_map(guardian_booster_range)
            .reduce(f64::max);

        let fuel = obj.get("FuelCapacity");
        let tank_size = fuel
            .and_then(|f| f.get("Main"))
            .and_then(Value::as_f64)
            .filter(|t| t.is_finite() && *t > 0.0)
            .ok_or(LoadoutError::Missing("FuelCapacity.Main"))?;
        let reservoir_size = fuel
            .and_then(|f| f.get("Reserve"))
            .and_then(Value::as_f64)
            .filter(|t| t.is_finite() && *t >= 0.0)
            .unwrap_or(0.0);
        let unladen_mass = obj
            .get("UnladenMass")
            .and_then(Value::as_f64)
            .filter(|m| m.is_finite() && *m > 0.0)
            .ok_or(LoadoutError::Missing("UnladenMass"))?;

        Ok(ShipProfile {
            ship: str_field(loadout, "Ship")
                .unwrap_or_default()
                .to_ascii_lowercase(),
            ship_name: str_field(loadout, "ShipName").filter(|n| !n.trim().is_empty()),
            fsd_item,
            fsd,
            fsd_engineered,
            range_boost,
            tank_size,
            reservoir_size,
            unladen_mass,
            cargo_capacity: obj
                .get("CargoCapacity")
                .and_then(Value::as_u64)
                .and_then(|c| u32::try_from(c).ok())
                .unwrap_or(0),
            max_jump_range: obj
                .get("MaxJumpRange")
                .and_then(Value::as_f64)
                .filter(|r| r.is_finite() && *r > 0.0),
            slef: serde_json::to_string(&json!([{
                "header": {"appName": "EliteNexus", "appVersion": env!("CARGO_PKG_VERSION")},
                "data": loadout,
            }]))
            .ok(),
        })
    }

    /// Spansh `base_mass`: unladen mass plus the reservoir.
    pub fn base_mass(&self) -> f64 {
        self.unladen_mass + self.reservoir_size
    }

    /// Jump range in ly with `fuel` t in the main tank and `cargo` t aboard (standard FSD formula
    /// plus the Guardian booster; an estimate — Spansh does its own maths).
    pub fn jump_range(&self, fuel: f64, cargo: f64) -> f64 {
        let fuel = fuel.max(0.0);
        let mass = self.base_mass() + fuel + cargo.max(0.0);
        let fuel_for_jump = fuel.min(self.fsd.max_fuel_per_jump);
        if mass <= 0.0
            || fuel_for_jump <= 0.0
            || self.fsd.fuel_multiplier <= 0.0
            || self.fsd.fuel_power <= 0.0
        {
            return 0.0;
        }
        (fuel_for_jump / self.fsd.fuel_multiplier).powf(1.0 / self.fsd.fuel_power)
            * self.fsd.optimal_mass
            / mass
            + self.range_boost.unwrap_or(0.0)
    }

    /// Range with a full main tank and `cargo` t aboard — the neutron plotter's "range".
    pub fn laden_range(&self, cargo: f64) -> f64 {
        self.jump_range(self.tank_size, cargo)
    }

    /// Best case: no cargo, just enough fuel for one jump.
    pub fn max_range(&self) -> f64 {
        self.jump_range(self.fsd.max_fuel_per_jump, 0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classic_and_sco_tables() {
        let a5 = fsd_spec("int_hyperdrive_size5_class5").expect("5A");
        assert_eq!((a5.size, a5.rating, a5.sco), (5, 'A', false));
        assert_eq!(
            (
                a5.optimal_mass,
                a5.max_fuel_per_jump,
                a5.fuel_multiplier,
                a5.fuel_power
            ),
            (1050.0, 5.0, 0.012, 2.45)
        );
        let e2 = fsd_spec("Int_Hyperdrive_Size2_Class1_free").expect("starter FSD");
        assert_eq!(
            (
                e2.optimal_mass,
                e2.max_fuel_per_jump,
                e2.fuel_multiplier,
                e2.fuel_power
            ),
            (48.0, 0.6, 0.011, 2.0)
        );
        let sco = fsd_spec("int_hyperdrive_overcharge_size6_class5").expect("6A SCO");
        assert_eq!(
            (
                sco.optimal_mass,
                sco.max_fuel_per_jump,
                sco.fuel_multiplier,
                sco.fuel_power
            ),
            (2000.0, 8.3, 0.013, 2.6)
        );
        let sco8 = fsd_spec("int_hyperdrive_overcharge_size8_class1").expect("8E SCO");
        assert_eq!(
            (
                sco8.optimal_mass,
                sco8.max_fuel_per_jump,
                sco8.fuel_multiplier
            ),
            (2800.0, 13.6, 0.008)
        );
        let mk2 = fsd_spec("int_hyperdrive_overcharge_size8_class5_overchargebooster_mkii")
            .expect("Mk II");
        assert_eq!(
            (
                mk2.max_fuel_per_jump,
                mk2.fuel_power,
                mk2.supercharge_multiplier
            ),
            (6.8, 2.5025, 6)
        );
        assert_eq!(fsd_spec("int_hyperdrive_size8_class5"), None); // listed but not a real module
        assert_eq!(fsd_spec("int_hyperdrive_size5_class6"), None);
        assert_eq!(fsd_spec("int_fuelscoop_size5_class5"), None);
        assert_eq!(
            guardian_booster_range("int_guardianfsdbooster_size5"),
            Some(10.5)
        );
        assert_eq!(guardian_booster_range("int_guardianfsdbooster_size0"), None);
    }

    const LOADOUT: &str = r#"{ "timestamp":"2026-10-01T12:00:00Z", "event":"Loadout", "Ship":"krait_light", "ShipID":7, "ShipName":"Wayfarer", "ShipIdent":"KP-01", "HullValue":40000000, "ModulesValue":60000000, "HullHealth":1.0, "UnladenMass":398.5, "CargoCapacity":16, "MaxJumpRange":71.2, "FuelCapacity":{ "Main":32.0, "Reserve":0.63 }, "Rebuy":5000000,
        "Modules":[
            { "Slot":"FrameShiftDrive", "Item":"int_hyperdrive_overcharge_size5_class5", "On":true, "Priority":0, "Health":1.0,
              "Engineering":{ "Engineer":"Felicity Farseer", "EngineerID":300100, "BlueprintID":128673694, "BlueprintName":"FSD_LongRange", "Level":5, "Quality":1.0,
                "ExperimentalEffect":"special_fsd_heavy", "ExperimentalEffect_Localised":"Mass Manager",
                "Modifiers":[ { "Label":"Mass", "Value":26.0, "OriginalValue":20.0, "LessIsGood":1 },
                              { "Label":"FSDOptimalMass", "Value":2117.5, "OriginalValue":1175.0, "LessIsGood":0 } ] } },
            { "Slot":"Slot03_Size3", "Item":"int_guardianfsdbooster_size3", "On":true, "Priority":0, "Health":1.0 },
            { "Slot":"FuelTank", "Item":"int_fueltank_size5_class3", "On":true, "Priority":1, "Health":1.0 }
        ] }"#;

    #[test]
    fn profile_from_loadout() {
        let p = ShipProfile::from_loadout_json(LOADOUT).expect("profile");
        assert_eq!(p.ship, "krait_light");
        assert_eq!(p.ship_name.as_deref(), Some("Wayfarer"));
        assert_eq!(p.fsd_item, "int_hyperdrive_overcharge_size5_class5");
        assert!(p.fsd.sco && p.fsd_engineered);
        assert_eq!(p.fsd.optimal_mass, 2117.5);
        assert_eq!(p.fsd.max_fuel_per_jump, 5.2);
        assert_eq!(p.fsd.fuel_multiplier, 0.013);
        assert_eq!(p.range_boost, Some(7.75));
        assert_eq!(
            (p.tank_size, p.reservoir_size, p.cargo_capacity),
            (32.0, 0.63, 16)
        );
        assert!((p.base_mass() - 399.13).abs() < 1e-9);
        // (5.2/0.013)^(1/2.45) * 2117.5 / (399.13 + 5.2) + 7.75
        let expected = (5.2f64 / 0.013).powf(1.0 / 2.45) * 2117.5 / (399.13 + 5.2) + 7.75;
        assert!((p.max_range() - expected).abs() < 1e-9);
        assert!(p.laden_range(16.0) < p.max_range());
        let slef: Value =
            serde_json::from_str(p.slef.as_deref().expect("slef")).expect("slef json");
        assert_eq!(slef[0]["data"]["Ship"], "krait_light");
        assert_eq!(slef[0]["header"]["appName"], "EliteNexus");

        // The SLEF form round-trips.
        let again = ShipProfile::from_loadout(&slef).expect("from SLEF");
        assert_eq!(again.fsd, p.fsd);
    }

    #[test]
    fn loadout_errors() {
        assert!(matches!(
            ShipProfile::from_loadout_json("{"),
            Err(LoadoutError::Json(_))
        ));
        assert_eq!(
            ShipProfile::from_loadout_json(r#"{"event":"Cargo","Modules":[]}"#),
            Err(LoadoutError::NotLoadout("Cargo".into()))
        );
        assert_eq!(
            ShipProfile::from_loadout_json(r#"{"event":"Loadout","Modules":[]}"#),
            Err(LoadoutError::NoFsd)
        );
        assert_eq!(
            ShipProfile::from_loadout_json(
                r#"{"event":"Loadout","Modules":[{"Slot":"FrameShiftDrive","Item":"int_hyperdrive_size9_class9"}]}"#
            ),
            Err(LoadoutError::UnknownFsd(
                "int_hyperdrive_size9_class9".into()
            ))
        );
        assert_eq!(
            ShipProfile::from_loadout_json(
                r#"{"event":"Loadout","Modules":[{"Slot":"FrameShiftDrive","Item":"int_hyperdrive_size5_class5"}]}"#
            ),
            Err(LoadoutError::Missing("FuelCapacity.Main"))
        );
    }
}
