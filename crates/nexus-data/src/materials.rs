//! Engineering materials (Raw / Manufactured / Encoded) and Odyssey micro-resources.
//!
//! Sources (compiled in, parsed lazily on first use):
//! * EDCD/FDevIDs `material.csv` and `microresources.csv` (ids, grades, trader categories, names).
//! * The ten 2022–24 Thargoid-war materials FDevIDs lacks, taken from EDDiscovery `MCMRType.cs`
//!   (Apache-2.0, © EDDiscovery development team).
//!
//! Symbols are the lower-case journal names (`"disruptedwakeechoes"`, `"tg_abrasion02"`).

use std::sync::LazyLock;

use rustc_hash::FxHashMap;

use crate::commodities::{name_key, symbol as normalise};

const MATERIAL_CSV: &str = include_str!("../../../data/fdevids/material.csv");
const MICRO_CSV: &str = include_str!("../../../data/fdevids/microresources.csv");

/// Storage cap per material by grade (G1..G5); the same for Raw, Manufactured and Encoded.
pub const CAPS: [u32; 5] = [300, 250, 200, 150, 100];

/// Which material trader (and which journal `Materials` array) a material belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MaterialKind {
    Raw,
    Manufactured,
    Encoded,
}

impl MaterialKind {
    pub const ALL: [MaterialKind; 3] = [MaterialKind::Raw, MaterialKind::Manufactured, MaterialKind::Encoded];

    pub fn label(self) -> &'static str {
        match self {
            MaterialKind::Raw => "Raw",
            MaterialKind::Manufactured => "Manufactured",
            MaterialKind::Encoded => "Encoded",
        }
    }

    /// Parses journal spellings: `"Raw"`, `"raw"` (MaterialTrade.TraderType), `"Manufactured"`, `"Encoded"`,
    /// `"$MICRORESOURCE_CATEGORY_Encoded;"`-style wrappers are tolerated too.
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim().trim_start_matches('$').trim_end_matches(';');
        let s = s.rsplit('_').next().unwrap_or(s);
        if s.eq_ignore_ascii_case("raw") {
            Some(MaterialKind::Raw)
        } else if s.eq_ignore_ascii_case("manufactured") {
            Some(MaterialKind::Manufactured)
        } else if s.eq_ignore_ascii_case("encoded") {
            Some(MaterialKind::Encoded)
        } else {
            None
        }
    }

    /// Highest grade that exists for this kind (Raw has no grade 5).
    pub fn max_grade(self) -> u8 {
        match self {
            MaterialKind::Raw => 4,
            _ => 5,
        }
    }
}

impl std::fmt::Display for MaterialKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

/// One engineering material.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Material {
    /// FDevIDs id (0 for the few materials FDevIDs does not list).
    pub id: u64,
    /// Lower-case journal symbol, e.g. `"disruptedwakeechoes"`.
    pub symbol: String,
    /// English display name, e.g. `"Atypical Disrupted Wake Echoes"`.
    pub name: String,
    pub kind: MaterialKind,
    /// Material-trader category: `"Category 1"`…`"Category 7"` for Raw, family names such as
    /// `"Chemical"`, `"Mechanical Components"`, `"Wake Scans"` for Manufactured/Encoded, and
    /// `"Guardian"` / `"Thargoid"` for the materials traders do not accept.
    pub category: String,
    /// 1 (very common) ..= 5 (very rare).
    pub grade: u8,
    /// Whether a material trader accepts/sells it (false for Guardian and Thargoid materials).
    pub tradeable: bool,
}

impl Material {
    /// Storage cap for this material.
    pub fn cap(&self) -> u32 {
        cap(self.grade)
    }

    /// Raw trader category number 1..=7 (None for Manufactured/Encoded).
    pub fn raw_category(&self) -> Option<u8> {
        if self.kind != MaterialKind::Raw {
            return None;
        }
        self.category.strip_prefix("Category ")?.parse().ok()
    }

    /// "Very Common" … "Very Rare".
    pub fn grade_name(&self) -> &'static str {
        grade_name(self.grade)
    }
}

/// Storage cap by grade: 300 / 250 / 200 / 150 / 100 (0 for an invalid grade).
pub fn cap(grade: u8) -> u32 {
    match grade {
        1..=5 => CAPS[grade as usize - 1],
        _ => 0,
    }
}

/// In-game rarity wording for a grade.
pub fn grade_name(grade: u8) -> &'static str {
    match grade {
        1 => "Very Common",
        2 => "Common",
        3 => "Standard",
        4 => "Rare",
        5 => "Very Rare",
        _ => "Unknown",
    }
}

/// Thargoid-war materials that FDevIDs `material.csv` does not list (EDD `MCMRType.cs`).
const EXTRA_MATERIALS: &[(&str, &str, MaterialKind, u8)] = &[
    ("tg_abrasion03", "Hardened Surface Fragments", MaterialKind::Manufactured, 1),
    ("unknowncorechip", "Tactical Core Chip", MaterialKind::Manufactured, 2),
    ("tg_abrasion02", "Phasing Membrane Residue", MaterialKind::Manufactured, 3),
    ("tg_causticgeneratorparts", "Corrosive Mechanisms", MaterialKind::Manufactured, 3),
    ("tg_causticshard", "Caustic Shard", MaterialKind::Manufactured, 3),
    ("tg_causticcrystal", "Caustic Crystal", MaterialKind::Manufactured, 4),
    ("tg_abrasion01", "Heat Exposure Specimen", MaterialKind::Manufactured, 5),
    ("tg_interdictiondata", "Thargoid Interdiction Telemetry", MaterialKind::Encoded, 3),
    ("tg_shutdowndata", "Massive Energy Surge Analytics", MaterialKind::Encoded, 3),
    ("tg_structuraldata02", "Thargoid Capital Ship Data", MaterialKind::Encoded, 3),
];

struct MaterialTable {
    list: Vec<Material>,
    by_symbol: FxHashMap<String, usize>,
    by_name: FxHashMap<String, usize>,
    by_id: FxHashMap<u64, usize>,
}

static MATERIALS: LazyLock<MaterialTable> = LazyLock::new(load_materials);

fn untradeable_family(symbol: &str) -> &'static str {
    if symbol.starts_with("guardian") || symbol.starts_with("ancient") { "Guardian" } else { "Thargoid" }
}

fn load_materials() -> MaterialTable {
    let mut list = Vec::with_capacity(160);
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .trim(csv::Trim::All)
        .from_reader(MATERIAL_CSV.as_bytes());
    for rec in rdr.records().flatten() {
        let (Some(id), Some(sym), Some(grade), Some(kind), Some(cat), Some(name)) =
            (rec.get(0), rec.get(1), rec.get(2), rec.get(3), rec.get(4), rec.get(5))
        else {
            continue;
        };
        let Some(kind) = MaterialKind::parse(kind) else { continue };
        let symbol = sym.to_ascii_lowercase();
        let tradeable = !cat.eq_ignore_ascii_case("none") && !cat.is_empty();
        let category = if !tradeable {
            untradeable_family(&symbol).to_string()
        } else if kind == MaterialKind::Raw {
            format!("Category {cat}")
        } else {
            cat.to_string()
        };
        list.push(Material {
            id: id.parse().unwrap_or(0),
            symbol,
            name: name.to_string(),
            kind,
            category,
            grade: grade.parse().unwrap_or(0),
            tradeable,
        });
    }
    for &(sym, name, kind, grade) in EXTRA_MATERIALS {
        if list.iter().any(|m| m.symbol == sym) {
            continue;
        }
        list.push(Material {
            id: 0,
            symbol: sym.to_string(),
            name: name.to_string(),
            kind,
            category: untradeable_family(sym).to_string(),
            grade,
            tradeable: false,
        });
    }
    // Stable, useful order: kind, tradeable categories first, category, grade.
    list.sort_by(|a, b| {
        (a.kind, !a.tradeable, &a.category, a.grade, &a.name).cmp(&(
            b.kind,
            !b.tradeable,
            &b.category,
            b.grade,
            &b.name,
        ))
    });
    let mut by_symbol = FxHashMap::default();
    let mut by_name = FxHashMap::default();
    let mut by_id = FxHashMap::default();
    for (i, m) in list.iter().enumerate() {
        by_symbol.insert(m.symbol.clone(), i);
        by_name.entry(name_key(&m.name)).or_insert(i);
        if m.id != 0 {
            by_id.insert(m.id, i);
        }
    }
    MaterialTable { list, by_symbol, by_name, by_id }
}

/// Every known material (FDevIDs + EDD extras), sorted by kind, category and grade.
pub fn all() -> &'static [Material] {
    &MATERIALS.list
}

/// Looks a material up by journal symbol (`"iron"`, `"$iron_name;"`, `"Iron"`) or English name
/// (`"Atypical Disrupted Wake Echoes"`), case-insensitively.
pub fn material(symbol_or_name: &str) -> Option<&'static Material> {
    let t = &*MATERIALS;
    t.by_symbol
        .get(&normalise(symbol_or_name))
        .or_else(|| t.by_name.get(&name_key(symbol_or_name)))
        .map(|&i| &t.list[i])
}

/// Looks a material up by its FDevIDs id.
pub fn material_by_id(id: u64) -> Option<&'static Material> {
    let t = &*MATERIALS;
    t.by_id.get(&id).map(|&i| &t.list[i])
}

/// Whether the symbol names a known engineering material.
pub fn is_material(symbol: &str) -> bool {
    material(symbol).is_some()
}

/// Display name for a material symbol; unknown symbols are prettified.
pub fn display_name(symbol: &str) -> String {
    material(symbol).map(|m| m.name.clone()).unwrap_or_else(|| crate::commodities::prettify(symbol))
}

/// All materials of one trader category (e.g. `(Encoded, "Wake Scans")`), ordered G1→G5.
pub fn category_members(kind: MaterialKind, category: &str) -> Vec<&'static Material> {
    let mut v: Vec<&'static Material> =
        all().iter().filter(|m| m.kind == kind && m.category.eq_ignore_ascii_case(category)).collect();
    v.sort_by_key(|m| m.grade);
    v
}

/// The distinct trader categories of a kind, in table order (tradeable ones only).
pub fn categories(kind: MaterialKind) -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    for m in all().iter().filter(|m| m.kind == kind && m.tradeable) {
        if !out.contains(&m.category.as_str()) {
            out.push(m.category.as_str());
        }
    }
    out
}

// ---------------------------------------------------------------------------------------------
// Odyssey micro-resources
// ---------------------------------------------------------------------------------------------

/// Ship-locker category of an Odyssey micro-resource.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MicroResourceKind {
    /// "Goods" in game.
    Item,
    /// "Assets" in game.
    Component,
    Data,
    Consumable,
}

impl MicroResourceKind {
    pub const ALL: [MicroResourceKind; 4] =
        [MicroResourceKind::Item, MicroResourceKind::Component, MicroResourceKind::Data, MicroResourceKind::Consumable];

    /// Accepts `"Item"`, `"Goods"`, `"Component"`, `"Assets"`, `"Data"`, `"Consumable"` and the
    /// `"$MICRORESOURCE_CATEGORY_Data;"` journal form, case-insensitively.
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim().trim_start_matches('$').trim_end_matches(';');
        let s = s.rsplit('_').next().unwrap_or(s).to_ascii_lowercase();
        match s.as_str() {
            "item" | "items" | "goods" => Some(MicroResourceKind::Item),
            "component" | "components" | "assets" => Some(MicroResourceKind::Component),
            "data" => Some(MicroResourceKind::Data),
            "consumable" | "consumables" => Some(MicroResourceKind::Consumable),
            _ => None,
        }
    }

    /// The in-game locker tab name.
    pub fn label(self) -> &'static str {
        match self {
            MicroResourceKind::Item => "Goods",
            MicroResourceKind::Component => "Assets",
            MicroResourceKind::Data => "Data",
            MicroResourceKind::Consumable => "Consumables",
        }
    }
}

/// One Odyssey micro-resource (goods, assets, data, consumables).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MicroResource {
    pub id: u64,
    /// Lower-case journal symbol, e.g. `"opticallens"`.
    pub symbol: String,
    pub name: String,
    pub kind: MicroResourceKind,
}

struct MicroTable {
    list: Vec<MicroResource>,
    by_symbol: FxHashMap<String, usize>,
    by_name: FxHashMap<String, usize>,
}

static MICRO: LazyLock<MicroTable> = LazyLock::new(|| {
    let mut list = Vec::with_capacity(200);
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .trim(csv::Trim::All)
        .from_reader(MICRO_CSV.as_bytes());
    for rec in rdr.records().flatten() {
        let (Some(id), Some(sym), Some(kind), Some(name)) = (rec.get(0), rec.get(1), rec.get(2), rec.get(3)) else {
            continue;
        };
        let Some(kind) = MicroResourceKind::parse(kind) else { continue };
        list.push(MicroResource {
            id: id.parse().unwrap_or(0),
            symbol: sym.to_ascii_lowercase(),
            name: name.to_string(),
            kind,
        });
    }
    list.sort_by(|a, b| (a.kind, &a.name).cmp(&(b.kind, &b.name)));
    let mut by_symbol = FxHashMap::default();
    let mut by_name = FxHashMap::default();
    for (i, m) in list.iter().enumerate() {
        by_symbol.insert(m.symbol.clone(), i);
        by_name.entry(name_key(&m.name)).or_insert(i);
    }
    MicroTable { list, by_symbol, by_name }
});

/// Every Odyssey micro-resource from FDevIDs, sorted by kind and name.
pub fn micro_resources() -> &'static [MicroResource] {
    &MICRO.list
}

/// Looks a micro-resource up by symbol (`"opticallens"`, `"$opticallens_name;"`) or English name.
pub fn micro_resource(symbol_or_name: &str) -> Option<&'static MicroResource> {
    let t = &*MICRO;
    t.by_symbol
        .get(&normalise(symbol_or_name))
        .or_else(|| t.by_name.get(&name_key(symbol_or_name)))
        .map(|&i| &t.list[i])
}

/// Display name for a micro-resource symbol; unknown symbols are prettified.
pub fn micro_resource_name(symbol: &str) -> String {
    micro_resource(symbol).map(|m| m.name.clone()).unwrap_or_else(|| crate::commodities::prettify(symbol))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_and_lookup() {
        // 137 FDevIDs rows + 10 EDD extras.
        assert_eq!(all().len(), 147);
        let m = material("$DisruptedWakeEchoes_name;").expect("wake echoes");
        assert_eq!(m.name, "Atypical Disrupted Wake Echoes");
        assert_eq!((m.kind, m.grade, m.category.as_str()), (MaterialKind::Encoded, 1, "Wake Scans"));
        assert_eq!(material("Atypical Disrupted Wake Echoes").map(|m| m.symbol.as_str()), Some("disruptedwakeechoes"));
        assert!(material("nonsense").is_none());
        assert_eq!(material_by_id(128672322).map(|m| m.symbol.as_str()), Some("carbon"));
        // Trailing whitespace in the FDevIDs name is trimmed.
        assert_eq!(display_name("shielddensityreports"), "Untypical Shield Scans");
    }

    #[test]
    fn grades_and_categories() {
        let iron = material("Iron").unwrap();
        assert_eq!((iron.kind, iron.grade, iron.raw_category()), (MaterialKind::Raw, 1, Some(4)));
        assert_eq!(iron.category, "Category 4");
        let pi = material("pharmaceuticalisolators").unwrap();
        assert_eq!((pi.kind, pi.grade, pi.category.as_str()), (MaterialKind::Manufactured, 5, "Chemical"));
        let fpc = material("fedproprietarycomposites").unwrap();
        assert_eq!((fpc.grade, fpc.category.as_str(), fpc.name.as_str()), (4, "Composite", "Proprietary Composites"));
        let mc = material("mechanicalcomponents").unwrap();
        assert_eq!((mc.grade, mc.category.as_str()), (3, "Mechanical Components"));
        let dwe = material("dataminedwake").unwrap();
        assert_eq!((dwe.grade, dwe.category.as_str()), (5, "Wake Scans"));
        // Not tradeable.
        let sf = material("unknownenergysource").unwrap();
        assert_eq!(
            (sf.name.as_str(), sf.grade, sf.tradeable, sf.category.as_str()),
            ("Sensor Fragment", 5, false, "Thargoid")
        );
        let gpc = material("guardian_powercell").unwrap();
        assert_eq!((gpc.tradeable, gpc.category.as_str()), (false, "Guardian"));
        let tcc = material("unknowncorechip").unwrap();
        assert_eq!((tcc.grade, tcc.tradeable), (2, false));
        // Every tradeable category has exactly one material per grade.
        for kind in MaterialKind::ALL {
            let cats = categories(kind);
            assert_eq!(
                cats.len(),
                if kind == MaterialKind::Raw {
                    7
                } else if kind == MaterialKind::Encoded {
                    6
                } else {
                    10
                }
            );
            for c in cats {
                let grades: Vec<u8> = category_members(kind, c).iter().map(|m| m.grade).collect();
                let expect: Vec<u8> = (1..=kind.max_grade()).collect();
                assert_eq!(grades, expect, "{kind} {c}");
            }
        }
    }

    #[test]
    fn caps() {
        assert_eq!([cap(1), cap(2), cap(3), cap(4), cap(5)], [300, 250, 200, 150, 100]);
        assert_eq!(cap(0), 0);
        assert_eq!(material("polonium").unwrap().cap(), 150);
    }

    #[test]
    fn kinds_parse() {
        assert_eq!(MaterialKind::parse("raw"), Some(MaterialKind::Raw));
        assert_eq!(MaterialKind::parse("Manufactured"), Some(MaterialKind::Manufactured));
        assert_eq!(MaterialKind::parse("encoded"), Some(MaterialKind::Encoded));
        assert_eq!(MicroResourceKind::parse("$MICRORESOURCE_CATEGORY_Data;"), Some(MicroResourceKind::Data));
        assert_eq!(MicroResourceKind::parse("Goods"), Some(MicroResourceKind::Item));
        assert_eq!(MicroResourceKind::parse("Component"), Some(MicroResourceKind::Component));
    }

    #[test]
    fn micro_resources_load() {
        assert_eq!(micro_resources().len(), 196);
        let ol = micro_resource("opticallens").unwrap();
        assert_eq!((ol.name.as_str(), ol.kind), ("Optical Lens", MicroResourceKind::Component));
        assert_eq!(micro_resource("Manufacturing Instructions").map(|m| m.kind), Some(MicroResourceKind::Data));
        assert_eq!(micro_resource_name("healthpack"), "Medkit");
    }
}
