//! Commodity identity: lower-case symbols (`gold`, `lowtemperaturediamond`) plus the FDevIDs seed list.

use rustc_hash::FxHashMap;

const COMMODITY_CSV: &str = include_str!("../../../data/fdevids/commodity.csv");
const RARE_COMMODITY_CSV: &str = include_str!("../../../data/fdevids/rare_commodity.csv");

/// Normalises any commodity spelling to the lower-case symbol used as the key everywhere:
/// `$gold_name;` / `Gold` / `gold` -> `gold`, Spansh `AdvancedCatalysers` -> `advancedcatalysers`.
pub fn normalize_symbol(raw: &str) -> String {
    nexus_journal::companion::commodity_symbol(raw.trim())
}

/// One row of the seed list.
#[derive(Debug, Clone)]
pub(crate) struct SeedCommodity {
    pub fdev_id: i64,
    pub symbol: String,
    pub name: String,
    pub category: String,
    pub rare: bool,
}

/// FDevIDs `commodity.csv` + `rare_commodity.csv`, in file order (stable ids across databases).
pub(crate) fn seed() -> Vec<SeedCommodity> {
    let mut out = Vec::with_capacity(420);
    // id,symbol,category,name
    for line in COMMODITY_CSV.lines().skip(1) {
        let f: Vec<&str> = line.split(',').map(str::trim).collect();
        if f.len() < 4 || f[1].is_empty() {
            continue;
        }
        out.push(SeedCommodity {
            fdev_id: f[0].parse().unwrap_or(0),
            symbol: f[1].to_ascii_lowercase(),
            category: f[2].to_string(),
            name: f[3..].join(","),
            rare: false,
        });
    }
    // id,symbol,market_id,category,name
    for line in RARE_COMMODITY_CSV.lines().skip(1) {
        let f: Vec<&str> = line.split(',').map(str::trim).collect();
        if f.len() < 5 || f[1].is_empty() {
            continue;
        }
        out.push(SeedCommodity {
            fdev_id: f[0].parse().unwrap_or(0),
            symbol: f[1].to_ascii_lowercase(),
            category: f[3].to_string(),
            name: f[4..].join(","),
            rare: true,
        });
    }
    out
}

/// Readable fallback name for an unknown symbol (`newthing` stays `Newthing`).
pub(crate) fn fallback_name(symbol: &str) -> String {
    crate::names::split_camel(symbol)
}

/// In-memory commodity table shared by the writer (symbol -> id) and readers (id -> names).
#[derive(Debug, Default, Clone)]
pub(crate) struct CommodityCache {
    by_symbol: FxHashMap<String, i64>,
    by_name: FxHashMap<String, i64>,
    by_id: FxHashMap<i64, CommodityMeta>,
}

#[derive(Debug, Clone)]
pub(crate) struct CommodityMeta {
    pub symbol: String,
    pub name: String,
    pub category: String,
    pub rare: bool,
}

impl CommodityCache {
    pub fn insert(&mut self, id: i64, meta: CommodityMeta) {
        self.by_symbol.insert(meta.symbol.clone(), id);
        self.by_name.entry(meta.name.to_lowercase()).or_insert(id);
        self.by_id.insert(id, meta);
    }

    pub fn id_by_symbol(&self, symbol: &str) -> Option<i64> {
        self.by_symbol.get(symbol).copied()
    }

    /// Resolves user input: a symbol in any spelling, or a display name (case-insensitive).
    pub fn resolve(&self, input: &str) -> Option<i64> {
        let sym = normalize_symbol(input);
        self.by_symbol
            .get(&sym)
            .or_else(|| self.by_name.get(&input.trim().to_lowercase()))
            .or_else(|| {
                // "Low Temperature Diamonds" typed without spaces, etc.
                let k = crate::names::key(input);
                self.by_symbol.get(&k)
            })
            .copied()
    }

    pub fn meta(&self, id: i64) -> Option<&CommodityMeta> {
        self.by_id.get(&id)
    }

    pub fn iter(&self) -> impl Iterator<Item = (i64, &CommodityMeta)> {
        self.by_id.iter().map(|(id, m)| (*id, m))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seed_is_sane() {
        let s = seed();
        assert!(s.len() > 350, "{}", s.len());
        let gold = s.iter().find(|c| c.symbol == "gold").expect("gold");
        assert_eq!(gold.name, "Gold");
        assert_eq!(gold.category, "Metals");
        assert_eq!(gold.fdev_id, 128049154);
        let ltd = s
            .iter()
            .find(|c| c.symbol == "lowtemperaturediamond")
            .expect("ltd");
        assert_eq!(ltd.name, "Low Temperature Diamonds");
        assert!(s.iter().any(|c| c.rare && c.symbol == "lavianbrandy"));
    }

    #[test]
    fn symbols_normalise() {
        assert_eq!(normalize_symbol("$gold_name;"), "gold");
        assert_eq!(normalize_symbol("Gold"), "gold");
        assert_eq!(normalize_symbol("AdvancedCatalysers"), "advancedcatalysers");
        assert_eq!(
            normalize_symbol(" lowtemperaturediamond "),
            "lowtemperaturediamond"
        );
    }

    #[test]
    fn cache_resolves_names_and_symbols() {
        let mut c = CommodityCache::default();
        for (i, s) in seed().into_iter().enumerate() {
            c.insert(
                i as i64 + 1,
                CommodityMeta {
                    symbol: s.symbol,
                    name: s.name,
                    category: s.category,
                    rare: s.rare,
                },
            );
        }
        let ltd = c.resolve("lowtemperaturediamond").expect("symbol");
        assert_eq!(c.resolve("Low Temperature Diamonds"), Some(ltd));
        assert_eq!(c.resolve("$LowTemperatureDiamond_name;"), Some(ltd));
        assert_eq!(c.resolve("Void Opal"), c.resolve("opal"));
        assert_eq!(c.resolve("no such thing"), None);
    }
}
