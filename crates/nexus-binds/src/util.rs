//! Small text helpers shared by the parser, the catalogue and the HTML renderer.

use std::borrow::Cow;
use std::cmp::Ordering;

/// Turns an identifier into words: `HumanoidToggleFlashlightButton` → `Humanoid Toggle Flashlight
/// Button`, `UI_Focus` → `UI Focus`, `FSSEnter` → `FSS Enter`, `POV1Up` → `POV 1 Up`.
///
/// Underscores become spaces, acronyms stay together and the first letter is upper-cased.
pub fn humanize(id: &str) -> String {
    let chars: Vec<char> = id.chars().collect();
    let mut out = String::with_capacity(id.len() + 8);
    for (i, &c) in chars.iter().enumerate() {
        if c == '_' || c.is_whitespace() {
            if !out.is_empty() && !out.ends_with(' ') {
                out.push(' ');
            }
            continue;
        }
        if i > 0 && !out.is_empty() && !out.ends_with(' ') {
            let prev = chars[i - 1];
            let next = chars.get(i + 1).copied();
            let boundary = (c.is_uppercase() && (prev.is_lowercase() || prev.is_ascii_digit()))
                || (c.is_uppercase()
                    && prev.is_uppercase()
                    && next.is_some_and(char::is_lowercase))
                || (c.is_ascii_digit() && prev.is_alphabetic())
                || (c.is_alphabetic() && prev.is_ascii_digit());
            if boundary {
                out.push(' ');
            }
        }
        if out.is_empty() {
            out.extend(c.to_uppercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// Compares strings so that embedded numbers sort numerically (`Btn 2` < `Btn 10`), ignoring ASCII
/// case for the text parts.
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    let (mut a, mut b) = (a.as_bytes(), b.as_bytes());
    loop {
        match (a.first(), b.first()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let (na, ra) = split_digits(a);
                let (nb, rb) = split_digits(b);
                // Compare by magnitude first (length after stripping leading zeros), then digits.
                let ta = trim_zeros(na);
                let tb = trim_zeros(nb);
                let ord = ta
                    .len()
                    .cmp(&tb.len())
                    .then_with(|| ta.cmp(tb))
                    .then_with(|| na.len().cmp(&nb.len()));
                if ord != Ordering::Equal {
                    return ord;
                }
                a = ra;
                b = rb;
            }
            (Some(x), Some(y)) => {
                let ord = x.to_ascii_lowercase().cmp(&y.to_ascii_lowercase());
                if ord != Ordering::Equal {
                    return ord;
                }
                a = &a[1..];
                b = &b[1..];
            }
        }
    }
}

fn split_digits(s: &[u8]) -> (&[u8], &[u8]) {
    let n = s.iter().take_while(|c| c.is_ascii_digit()).count();
    s.split_at(n)
}

fn trim_zeros(s: &[u8]) -> &[u8] {
    let n = s.iter().take_while(|&&c| c == b'0').count();
    &s[n.min(s.len().saturating_sub(1))..]
}

/// Escapes text for safe inclusion in HTML element content and double-quoted attributes.
pub fn escape_html(s: &str) -> Cow<'_, str> {
    if !s.contains(['&', '<', '>', '"', '\'']) {
        return Cow::Borrowed(s);
    }
    let mut out = String::with_capacity(s.len() + 16);
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    Cow::Owned(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn humanize_splits_camel_case_acronyms_and_digits() {
        assert_eq!(
            humanize("HumanoidToggleFlashlightButton"),
            "Humanoid Toggle Flashlight Button"
        );
        assert_eq!(humanize("UIFocus"), "UI Focus");
        assert_eq!(humanize("ExplorationFSSEnter"), "Exploration FSS Enter");
        assert_eq!(humanize("TargetWingman0"), "Target Wingman 0");
        assert_eq!(humanize("POV1Up"), "POV 1 Up");
        assert_eq!(
            humanize("GalnetAudio_Play_Pause"),
            "Galnet Audio Play Pause"
        );
        assert_eq!(humanize("yawRotateHeadlook"), "Yaw Rotate Headlook");
        assert_eq!(
            humanize("HumanoidSelectEMPGrenade"),
            "Humanoid Select EMP Grenade"
        );
        assert_eq!(humanize(""), "");
    }

    #[test]
    fn natural_order_sorts_numbers_by_value() {
        let mut v = vec!["Btn 10", "Btn 2", "btn 1", "Hat 1 Up", "Btn 02"];
        v.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(v, ["btn 1", "Btn 2", "Btn 02", "Btn 10", "Hat 1 Up"]);
        assert_eq!(natural_cmp("A", "A"), Ordering::Equal);
        assert_eq!(natural_cmp("Z Axis", "Z"), Ordering::Greater);
    }

    #[test]
    fn escapes_markup() {
        assert_eq!(escape_html("plain"), "plain");
        assert!(matches!(escape_html("plain"), Cow::Borrowed(_)));
        assert_eq!(
            escape_html("<a href=\"x\">'&'</a>"),
            "&lt;a href=&quot;x&quot;&gt;&#39;&amp;&#39;&lt;/a&gt;"
        );
    }
}
