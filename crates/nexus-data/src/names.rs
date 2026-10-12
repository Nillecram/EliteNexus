//! Helpers for the game's symbol strings (`$economy_Extraction;`, `$government_Prison;`, …).

/// Prefer the localised string; otherwise derive something readable from the symbol.
pub fn localised_or(raw: &str, localised: Option<&str>) -> String {
    match localised {
        Some(l) if !l.is_empty() => l.to_string(),
        _ => humanize_symbol(raw),
    }
}

/// `$economy_HighTech;` -> `High Tech`, `$SYSTEM_SECURITY_low;` -> `Low`, `metaalloys` -> `Metaalloys`.
pub fn humanize_symbol(raw: &str) -> String {
    let s = raw.trim_start_matches('$').trim_end_matches(';');
    let s = s.strip_suffix("_name").or_else(|| s.strip_suffix("_Name")).unwrap_or(s);
    let last = s.rsplit('_').next().unwrap_or(s);
    split_camel(last)
}

/// `HighTech` -> `High Tech`; first letter upper-cased.
pub fn split_camel(s: &str) -> String {
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

/// Thousands-separated credits, e.g. `12 345 678 Cr` -> "12,345,678".
pub fn group_digits(v: i64) -> String {
    let neg = v < 0;
    let digits = v.unsigned_abs().to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3 + 1);
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    if neg { format!("-{out}") } else { out }
}

/// Compact credits: 3.89 B, 12.3 M, 450 k.
pub fn compact_credits(v: i64) -> String {
    let a = v.unsigned_abs() as f64;
    let sign = if v < 0 { "-" } else { "" };
    if a >= 1e9 {
        format!("{sign}{:.2} B", a / 1e9)
    } else if a >= 1e6 {
        format!("{sign}{:.1} M", a / 1e6)
    } else if a >= 1e4 {
        format!("{sign}{:.0} k", a / 1e3)
    } else {
        format!("{sign}{a:.0}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn humanizes() {
        assert_eq!(humanize_symbol("$economy_HighTech;"), "High Tech");
        assert_eq!(humanize_symbol("$government_Prison;"), "Prison");
        assert_eq!(localised_or("$economy_Agri;", Some("Agriculture")), "Agriculture");
        assert_eq!(group_digits(3_889_591_046), "3,889,591,046");
        assert_eq!(group_digits(-1200), "-1,200");
        assert_eq!(compact_credits(3_889_591_046), "3.89 B");
    }
}
