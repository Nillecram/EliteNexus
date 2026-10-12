//! Commodities (trade goods) keyed by lower-case journal symbol.
//!
//! Source: EDCD/FDevIDs `commodity.csv` + `rare_commodity.csv`, compiled in and parsed lazily.
//! `id` is the Market.json / EDDN commodity id.

use std::sync::LazyLock;

use rustc_hash::FxHashMap;

const COMMODITY_CSV: &str = include_str!("../../../data/fdevids/commodity.csv");
const RARE_CSV: &str = include_str!("../../../data/fdevids/rare_commodity.csv");

/// One commodity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Commodity {
    /// FDevIDs / Market.json id.
    pub id: u64,
    /// Lower-case journal symbol, e.g. `"gold"`, `"lowtemperaturediamond"`.
    pub symbol: String,
    /// English display name, e.g. `"Low Temperature Diamonds"`.
    pub name: String,
    /// Market category, e.g. `"Minerals"`, `"Legal Drugs"`, `"Salvage"`.
    pub category: String,
    /// Rare commodity (sold at one station only).
    pub rare: bool,
    /// For rares: the MarketID of the station that sells it.
    pub market_id: Option<u64>,
}

struct Table {
    list: Vec<Commodity>,
    by_symbol: FxHashMap<String, usize>,
    by_name: FxHashMap<String, usize>,
    by_id: FxHashMap<u64, usize>,
}

static TABLE: LazyLock<Table> = LazyLock::new(load);

fn load() -> Table {
    let mut list = Vec::with_capacity(420);
    let reader = |text: &'static str| {
        csv::ReaderBuilder::new().has_headers(true).flexible(true).trim(csv::Trim::All).from_reader(text.as_bytes())
    };
    for rec in reader(COMMODITY_CSV).records().flatten() {
        let (Some(id), Some(sym), Some(cat), Some(name)) = (rec.get(0), rec.get(1), rec.get(2), rec.get(3)) else {
            continue;
        };
        list.push(Commodity {
            id: id.parse().unwrap_or(0),
            symbol: sym.to_ascii_lowercase(),
            name: name.to_string(),
            category: cat.to_string(),
            rare: false,
            market_id: None,
        });
    }
    for rec in reader(RARE_CSV).records().flatten() {
        let (Some(id), Some(sym), Some(market), Some(cat), Some(name)) =
            (rec.get(0), rec.get(1), rec.get(2), rec.get(3), rec.get(4))
        else {
            continue;
        };
        list.push(Commodity {
            id: id.parse().unwrap_or(0),
            symbol: sym.to_ascii_lowercase(),
            name: name.to_string(),
            category: cat.to_string(),
            rare: true,
            market_id: market.parse().ok(),
        });
    }
    let mut by_symbol = FxHashMap::default();
    let mut by_name = FxHashMap::default();
    let mut by_id = FxHashMap::default();
    for (i, c) in list.iter().enumerate() {
        by_symbol.entry(c.symbol.clone()).or_insert(i);
        by_name.entry(name_key(&c.name)).or_insert(i);
        by_id.insert(c.id, i);
    }
    Table { list, by_symbol, by_name, by_id }
}

/// Normalises a journal symbol: `"$gold_name;"` → `"gold"`, `"Gold"` → `"gold"`, `"gold"` → `"gold"`.
pub fn symbol(raw: &str) -> String {
    let s = raw.trim();
    let s = s.strip_prefix('$').unwrap_or(s);
    let s = s.strip_suffix(';').unwrap_or(s);
    let s = strip_name_suffix(s);
    s.to_ascii_lowercase()
}

/// `"gold_name"` → `"gold"` (ASCII case-insensitive).
fn strip_name_suffix(s: &str) -> &str {
    match s.len().checked_sub(5).and_then(|i| s.get(i..).map(|t| (i, t))) {
        Some((i, t)) if i > 0 && t.eq_ignore_ascii_case("_name") => &s[..i],
        _ => s,
    }
}

/// Lookup key for English names: lower-case ASCII alphanumerics only (`"Meta-Alloys"` → `"metaalloys"`).
pub fn name_key(name: &str) -> String {
    name.chars().filter(char::is_ascii_alphanumeric).map(|c| c.to_ascii_lowercase()).collect()
}

/// Readable fallback for an unknown symbol: `"$ThargoidTissueSampleType9b_name;"` → `"Thargoid Tissue Sample Type9b"`.
pub fn prettify(raw: &str) -> String {
    let s = raw.trim();
    let s = s.strip_prefix('$').unwrap_or(s);
    let s = s.strip_suffix(';').unwrap_or(s);
    let s = strip_name_suffix(s);
    s.split(['_', ' ']).filter(|w| !w.is_empty()).map(crate::names::split_camel).collect::<Vec<_>>().join(" ")
}

/// All commodities (regular first, then rares) in FDevIDs order.
pub fn all() -> &'static [Commodity] {
    &TABLE.list
}

/// Looks a commodity up by journal symbol in any of its spellings (`"$gold_name;"`, `"Gold"`, `"gold"`)
/// or by English name (`"Low Temperature Diamonds"`).
pub fn commodity(symbol_or_journal_name: &str) -> Option<&'static Commodity> {
    let t = &*TABLE;
    t.by_symbol
        .get(&symbol(symbol_or_journal_name))
        .or_else(|| t.by_name.get(&name_key(symbol_or_journal_name)))
        .map(|&i| &t.list[i])
}

/// Looks a commodity up by its Market.json / FDevIDs id.
pub fn commodity_by_id(id: u64) -> Option<&'static Commodity> {
    let t = &*TABLE;
    t.by_id.get(&id).map(|&i| &t.list[i])
}

/// Display name for a commodity symbol; unknown symbols are prettified.
pub fn display_name(symbol: &str) -> String {
    commodity(symbol).map(|c| c.name.clone()).unwrap_or_else(|| prettify(symbol))
}

/// Distinct market categories, sorted.
pub fn categories() -> Vec<&'static str> {
    let mut v: Vec<&'static str> = all().iter().map(|c| c.category.as_str()).collect();
    v.sort_unstable();
    v.dedup();
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalises_symbols() {
        assert_eq!(symbol("$gold_name;"), "gold");
        assert_eq!(symbol("$Gold_Name;"), "gold");
        assert_eq!(symbol("Gold"), "gold");
        assert_eq!(symbol("lowtemperaturediamond"), "lowtemperaturediamond");
        assert_eq!(name_key("Meta-Alloys"), "metaalloys");
    }

    #[test]
    fn lookups() {
        for s in ["$gold_name;", "Gold", "gold"] {
            let c = commodity(s).expect(s);
            assert_eq!((c.id, c.name.as_str(), c.category.as_str(), c.rare), (128049154, "Gold", "Metals", false));
        }
        let ltd = commodity("$LowTemperatureDiamond_name;").unwrap();
        assert_eq!(ltd.name, "Low Temperature Diamonds");
        assert_eq!(commodity("Low Temperature Diamonds").map(|c| c.symbol.as_str()), Some("lowtemperaturediamond"));
        assert_eq!(commodity("drones").map(|c| c.name.as_str()), Some("Limpets"));
        // Rhino-update (2026) minerals are present.
        assert_eq!(commodity("helium3").map(|c| c.name.as_str()), Some("Helium-3"));
        let lavian = commodity("lavianbrandy").unwrap();
        assert!(lavian.rare);
        assert_eq!(lavian.market_id, Some(128106744));
        assert_eq!(commodity_by_id(128049154).map(|c| c.symbol.as_str()), Some("gold"));
        assert!(commodity("definitely_not_a_commodity").is_none());
        assert_eq!(all().len(), 270 + 142);
    }

    #[test]
    fn display_names() {
        assert_eq!(display_name("$painite_name;"), "Painite");
        assert_eq!(display_name("opal"), "Void Opal");
        assert_eq!(display_name("$SomeNewThing_name;"), "Some New Thing");
        assert_eq!(prettify("$ThargoidTissueSampleType9b_name;"), "Thargoid Tissue Sample Type9b");
        assert!(categories().contains(&"Minerals"));
    }
}
