//! The colours the game draws its interface in before the matrix is applied, so a preview can
//! show what a matrix does to each kind of element. Frontier publishes no palette: these are the
//! reference colours other tools use (COVAS:NEXT's HUD orange and highlight blue) and the radar's
//! hostile red and allied green.

use crate::dimmer::Protect;
use crate::matrix::Rgb;

/// One of the game's standard interface colours.
#[derive(Debug, Clone, Copy)]
pub struct Swatch {
    pub name: &'static str,
    /// Where the commander sees it.
    pub used_for: &'static str,
    pub rgb: Rgb,
}

const fn rgb8(r: u8, g: u8, b: u8) -> Rgb {
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0]
}

/// Most of the HUD: panels, scanner, reticle, text.
pub const ORANGE: Swatch = Swatch { name: "HUD orange", used_for: "Panels, scanner, reticle, most text", rgb: rgb8(255, 117, 0) };
/// Hostile and wanted contacts, warnings.
pub const RED: Swatch = Swatch { name: "Hostile red", used_for: "Hostile and wanted contacts, warnings", rgb: rgb8(255, 32, 16) };
/// Friendly contacts and allied factions.
pub const GREEN: Swatch = Swatch { name: "Friendly green", used_for: "Friendly contacts, allies", rgb: rgb8(56, 232, 40) };
/// Shields, the throttle's blue zone, other highlights.
pub const BLUE: Swatch = Swatch { name: "Highlight blue", used_for: "Shields, the throttle's blue zone, wingmen", rgb: rgb8(105, 217, 218) };
/// Highlights and selected text.
pub const WHITE: Swatch = Swatch { name: "White", used_for: "Highlights, selected entries", rgb: rgb8(255, 255, 255) };

pub const ALL: [Swatch; 5] = [ORANGE, RED, GREEN, BLUE, WHITE];

/// Distance between two colours, 0 (same) to about 1.7 (black to white).
fn distance(a: Rgb, b: Rgb) -> f32 {
    a.iter().zip(b).map(|(x, y)| (x - y) * (x - y)).sum::<f32>().sqrt()
}

/// Relative luminance of an sRGB colour, 0–1 (the standard orange is about 0.34).
fn luminance(c: Rgb) -> f32 {
    let l = c.map(crate::matrix::to_linear);
    0.2126 * l[0] + 0.7152 * l[1] + 0.0722 * l[2]
}

/// Whether a standard colour (linear light, before the matrix) counts as the HUD's orange, by the
/// same rule as the shader code Elite Nexus installs (`dimmer::HUD_REPLACE`, same comparisons in the
/// same order): red is the largest channel, and green and blue are within `p`'s shares of it. The
/// HUD's orange (green about 7 % of red in the game's own data) and its darker shades pass; hostile
/// red (too little green), friendly green, shield blue, white and black don't.
pub fn is_hud_orange(c: Rgb, p: &Protect) -> bool {
    let [r, g, b] = c;
    g.max(b) < r && g <= r * p.max_gr && g >= r * p.min_gr && b <= r * p.max_br
}

/// What a matrix costs in play, in words: colours the commander relies on that it makes hard to
/// tell apart or to read. With `protect` on, hostile red, friendly green and shield blue keep their
/// colours, so the warnings about them only appear when the theme brings the HUD close to them.
pub fn concerns(m: &crate::HudMatrix, protect: Option<&Protect>) -> Vec<&'static str> {
    let [o, r, g, b] = [ORANGE, RED, GREEN, BLUE].map(|s| m.apply_srgb_protected(s.rgb, protect));
    let mut out = Vec::new();
    // Alike when close, or both colourless (then only their brightness differs).
    let chroma = |c: Rgb| c.iter().copied().fold(f32::MIN, f32::max) - c.iter().copied().fold(f32::MAX, f32::min);
    if distance(r, g) < 0.3 || (chroma(r) < 0.15 && chroma(g) < 0.15) {
        out.push("Hostile and friendly contacts look alike: in a fight you may not tell them apart on the scanner.");
    }
    if distance(r, o) < 0.2 {
        out.push("Hostile contacts look like the rest of the HUD, so they no longer stand out.");
    }
    if distance(b, o) < 0.15 {
        out.push("Shields are drawn in the same colour as the rest of the HUD.");
    }
    if luminance(o) < 0.05 {
        out.push("The HUD will be very dark: hard to read against bright stars and planets.");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::HudMatrix;

    #[test]
    fn the_standard_hud_has_no_concerns() {
        assert!(concerns(&HudMatrix::IDENTITY, None).is_empty());
        assert!(concerns(&HudMatrix::IDENTITY, Some(&Protect { on: true, ..Protect::default() })).is_empty());
    }

    #[test]
    fn flags_matrices_that_hide_things() {
        // Everything from the red channel: hostile red and the orange HUD merge.
        let red_only = HudMatrix { red: [1.0, 0.0, 0.0], green: [0.0, 0.0, 0.0], blue: [0.0, 0.0, 0.0] };
        assert!(!concerns(&red_only, None).is_empty());
        // Grey: red and green merge.
        let grey = HudMatrix::saturation(0.0);
        assert!(concerns(&grey, None).iter().any(|c| c.starts_with("Hostile and friendly")));
        assert!(concerns(&HudMatrix::IDENTITY.scaled(0.1), None).iter().any(|c| c.contains("very dark")));
    }

    #[test]
    fn the_rule_themes_only_the_hud_orange() {
        let p = Protect { on: true, ..Protect::default() };
        // Measured in the game (linear light): HUD orange and dark orange.
        assert!(is_hud_orange([1.0, 0.068, 0.025], &p));
        assert!(is_hud_orange([0.234, 0.044, 0.025], &p));
        // The reference orange (#FF7500): green 0.178 of red in linear light.
        assert!(is_hud_orange(ORANGE.rgb.map(crate::matrix::to_linear), &p));
        for kept in [[1.0, 1.0, 1.0], [0.0, 1.0, 0.0], [0.0, 0.6, 1.0], [1.0, 0.01, 0.01], [0.0, 0.0, 0.0]] {
            assert!(!is_hud_orange(kept, &p), "{kept:?}");
        }
        // The reference hostile red, friendly green, highlight blue and white.
        for s in [RED, GREEN, BLUE, WHITE] {
            assert!(!is_hud_orange(s.rgb.map(crate::matrix::to_linear), &p), "{}", s.name);
        }
    }

    #[test]
    fn protection_keeps_friend_and_foe_and_the_warnings_follow() {
        let p = Protect { on: true, ..Protect::default() };
        // The game's own red-to-blue test matrix turns the orange HUD blue, hostile red blue as well.
        let swap = HudMatrix { red: [0.0, 0.0, 1.0], green: [0.0, 1.0, 0.0], blue: [1.0, 0.0, 0.0] };
        assert_ne!(swap.apply_srgb_protected(RED.rgb, None), RED.rgb);
        assert_eq!(swap.apply_srgb_protected(RED.rgb, Some(&p)), RED.rgb);
        assert_eq!(swap.apply_srgb_protected(GREEN.rgb, Some(&p)), GREEN.rgb);
        assert_eq!(swap.apply_srgb_protected(BLUE.rgb, Some(&p)), BLUE.rgb);
        assert_eq!(swap.apply_srgb_protected(ORANGE.rgb, Some(&p)), swap.apply_srgb(ORANGE.rgb));
        // Switched off, it themes everything again.
        assert_eq!(swap.apply_srgb_protected(RED.rgb, Some(&Protect::default())), swap.apply_srgb(RED.rgb));
        // Grey merges friend and foe; with the protection they keep their colours.
        let grey = HudMatrix::saturation(0.0);
        assert!(concerns(&grey, Some(&p)).iter().all(|c| !c.starts_with("Hostile and friendly")));
        // A red theme still makes the HUD look like hostile contacts, protection or not.
        let red = HudMatrix { red: [1.0, 0.0, 0.0], green: [0.0, 0.1, 0.0], blue: [0.0, 0.0, 0.0] };
        assert!(concerns(&red, Some(&p)).iter().any(|c| c.starts_with("Hostile contacts look like")));
    }
}
