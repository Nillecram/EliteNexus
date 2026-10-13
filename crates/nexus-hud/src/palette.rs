//! The colours the game draws its interface in before the matrix is applied, so a preview can
//! show what a matrix does to each kind of element. Frontier publishes no palette: these are the
//! reference colours other tools use (COVAS:NEXT's HUD orange and highlight blue) and the radar's
//! hostile red and allied green.

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

/// What a matrix costs in play, in words: colours the commander relies on that it makes hard to
/// tell apart or to read.
pub fn concerns(m: &crate::HudMatrix) -> Vec<&'static str> {
    let [o, r, g, b] = [ORANGE, RED, GREEN, BLUE].map(|s| m.apply_srgb(s.rgb));
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
        assert!(concerns(&HudMatrix::IDENTITY).is_empty());
    }

    #[test]
    fn flags_matrices_that_hide_things() {
        // Everything from the red channel: hostile red and the orange HUD merge.
        let red_only = HudMatrix { red: [1.0, 0.0, 0.0], green: [0.0, 0.0, 0.0], blue: [0.0, 0.0, 0.0] };
        assert!(!concerns(&red_only).is_empty());
        // Grey: red and green merge.
        let grey = HudMatrix::saturation(0.0);
        assert!(concerns(&grey).iter().any(|c| c.starts_with("Hostile and friendly")));
        assert!(concerns(&HudMatrix::IDENTITY.scaled(0.1)).iter().any(|c| c.contains("very dark")));
    }
}
