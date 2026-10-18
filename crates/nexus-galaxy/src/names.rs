//! Normalisation of the game's symbol strings into the stable display names stored in the database.
//!
//! The journal (and therefore EDDN, which strips every `*_Localised` key) uses symbols such as
//! `$economy_HighTech;`, `$government_PrisonColony;` or `$SYSTEM_SECURITY_high;`, while Spansh dumps
//! use display names (`High Tech`, `Prison Colony`, `High`). Everything is normalised to the display
//! form so that both sources compare equal.

/// Strips the `$` prefix and `;` suffix of a game symbol.
pub(crate) fn clean_symbol(raw: &str) -> &str {
    let s = raw.trim();
    let s = s.strip_prefix('$').unwrap_or(s);
    s.strip_suffix(';').unwrap_or(s)
}

/// `HighTech` -> `High Tech`, `CivilLiberty` -> `Civil Liberty`; display strings pass through unchanged.
pub(crate) fn split_camel(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 4);
    let mut prev_lower = false;
    for (i, ch) in s.chars().enumerate() {
        if ch.is_uppercase() && prev_lower {
            out.push(' ');
        }
        if i == 0 {
            out.extend(ch.to_uppercase());
        } else {
            out.push(ch);
        }
        prev_lower = ch.is_lowercase() || ch.is_ascii_digit();
    }
    out
}

/// The part of `$prefix_Value;` after the first `_` (`economy_HighTech` -> `HighTech`).
fn after_prefix(raw: &str) -> &str {
    let s = clean_symbol(raw);
    match s.split_once('_') {
        Some((_, rest)) if raw.trim_start().starts_with('$') => rest,
        _ => s,
    }
}

fn non_empty(s: &str) -> Option<&str> {
    let t = s.trim();
    (!t.is_empty()).then_some(t)
}

/// Economy name: `$economy_Agri;` -> `Agriculture`, `$economy_HighTech;` -> `High Tech`, `Industrial` -> `Industrial`.
pub(crate) fn economy(raw: &str) -> Option<String> {
    let raw = non_empty(raw)?;
    let v = after_prefix(raw);
    let out = match v.to_ascii_lowercase().as_str() {
        "agri" | "agriculture" => "Agriculture".to_string(),
        "carrier" | "private enterprise" => "Private Enterprise".to_string(),
        "engineer" | "engineering" => "Engineering".to_string(),
        "undefined" | "none" | "" => "None".to_string(),
        "hightech" | "high tech" => "High Tech".to_string(),
        _ => split_camel(v),
    };
    Some(out)
}

/// Government name: `$government_PrisonColony;` -> `Prison Colony`, `$government_Carrier;` -> `Private Ownership`.
pub(crate) fn government(raw: &str) -> Option<String> {
    let raw = non_empty(raw)?;
    let v = after_prefix(raw);
    let out = match v.to_ascii_lowercase().as_str() {
        "carrier" | "private ownership" => "Private Ownership".to_string(),
        "prisoncolony" | "prison colony" => "Prison Colony".to_string(),
        "" => "None".to_string(),
        _ => split_camel(v),
    };
    Some(out)
}

/// Security: `$SYSTEM_SECURITY_high;` -> `High`, `$GAlAXY_MAP_INFO_state_anarchy;` -> `Anarchy`.
pub(crate) fn security(raw: &str) -> Option<String> {
    let raw = non_empty(raw)?;
    if raw.starts_with('$') {
        let l = raw.to_ascii_lowercase();
        let out = if l.contains("anarchy") {
            "Anarchy"
        } else if l.contains("lawless") {
            "Lawless"
        } else if l.contains("_high") {
            "High"
        } else if l.contains("_medium") {
            "Medium"
        } else if l.contains("_low") {
            "Low"
        } else {
            return Some(split_camel(after_prefix(raw)));
        };
        return Some(out.to_string());
    }
    Some(split_camel(raw))
}

/// Allegiance: `PilotsFederation` -> `Pilots Federation`; empty -> `None` (unpopulated / unknown).
pub(crate) fn allegiance(raw: &str) -> Option<String> {
    let raw = non_empty(raw)?;
    Some(split_camel(clean_symbol(raw)))
}

/// Faction / BGS state: `CivilLiberty` -> `Civil Liberty`.
pub(crate) fn state(raw: &str) -> Option<String> {
    let raw = non_empty(raw)?;
    Some(split_camel(clean_symbol(raw)))
}

/// Faction happiness: `$Faction_HappinessBand2;` -> `Happy`.
pub(crate) fn happiness(raw: &str) -> Option<String> {
    let raw = non_empty(raw)?;
    let l = raw.to_ascii_lowercase();
    let out = if l.contains("band1") {
        "Elated"
    } else if l.contains("band2") {
        "Happy"
    } else if l.contains("band3") {
        "Discontented"
    } else if l.contains("band4") {
        "Unhappy"
    } else if l.contains("band5") {
        "Despondent"
    } else {
        return Some(split_camel(after_prefix(raw)));
    };
    Some(out.to_string())
}

/// Kept in `nexus-data`, where the commander model uses it too.
pub use nexus_data::names::clean_station_name;

/// Lower-case alphanumerics only: `Black Market` and `blackmarket` both become `blackmarket`.
pub(crate) fn key(raw: &str) -> String {
    raw.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn economies() {
        assert_eq!(economy("$economy_HighTech;").as_deref(), Some("High Tech"));
        assert_eq!(economy("$economy_Agri;").as_deref(), Some("Agriculture"));
        assert_eq!(
            economy("$economy_Carrier;").as_deref(),
            Some("Private Enterprise")
        );
        assert_eq!(economy("$economy_None;").as_deref(), Some("None"));
        assert_eq!(economy("Refinery").as_deref(), Some("Refinery"));
        assert_eq!(economy("High Tech").as_deref(), Some("High Tech"));
        assert_eq!(economy(""), None);
    }

    #[test]
    fn governments_and_security() {
        assert_eq!(
            government("$government_PrisonColony;").as_deref(),
            Some("Prison Colony")
        );
        assert_eq!(
            government("$government_Democracy;").as_deref(),
            Some("Democracy")
        );
        assert_eq!(
            government("$government_Carrier;").as_deref(),
            Some("Private Ownership")
        );
        assert_eq!(
            government("Private Ownership").as_deref(),
            Some("Private Ownership")
        );
        assert_eq!(security("$SYSTEM_SECURITY_high;").as_deref(), Some("High"));
        assert_eq!(
            security("$GAlAXY_MAP_INFO_state_anarchy;").as_deref(),
            Some("Anarchy")
        );
        assert_eq!(security("Medium").as_deref(), Some("Medium"));
    }

    #[test]
    fn misc() {
        assert_eq!(
            allegiance("PilotsFederation").as_deref(),
            Some("Pilots Federation")
        );
        assert_eq!(
            allegiance("Pilots Federation").as_deref(),
            Some("Pilots Federation")
        );
        assert_eq!(allegiance(""), None);
        assert_eq!(state("CivilLiberty").as_deref(), Some("Civil Liberty"));
        assert_eq!(state("Civil Liberty").as_deref(), Some("Civil Liberty"));
        assert_eq!(
            happiness("$Faction_HappinessBand2;").as_deref(),
            Some("Happy")
        );
        assert_eq!(
            key("Interstellar Factors Contact"),
            "interstellarfactorscontact"
        );
    }
}
