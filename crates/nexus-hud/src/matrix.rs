//! The HUD colour matrix and the maths to build and preview one.
//!
//! The game recolours its interface with a 3×3 matrix. Each row says where one channel of the
//! standard colour goes: `<MatrixRed>` holds how much of the standard red ends up in the red,
//! green and blue of what is shown, `<MatrixGreen>` and `<MatrixBlue>` the same for green and blue.
//! So `out = r·MatrixRed + g·MatrixGreen + b·MatrixBlue`, clamped to 0–1.

use serde::{Deserialize, Serialize};

pub type Rgb = [f32; 3];

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct HudMatrix {
    pub red: Rgb,
    pub green: Rgb,
    pub blue: Rgb,
}

impl Default for HudMatrix {
    fn default() -> Self {
        Self::IDENTITY
    }
}

/// Rec. 709 luma weights (what the SVG/CSS colour filters use).
const LUMA: Rgb = [0.2126, 0.7152, 0.0722];

impl HudMatrix {
    /// The standard orange HUD: every channel stays where it is.
    pub const IDENTITY: HudMatrix = HudMatrix { red: [1.0, 0.0, 0.0], green: [0.0, 1.0, 0.0], blue: [0.0, 0.0, 1.0] };

    pub const ROW_NAMES: [&'static str; 3] = ["MatrixRed", "MatrixGreen", "MatrixBlue"];

    pub fn rows(&self) -> [Rgb; 3] {
        [self.red, self.green, self.blue]
    }

    pub fn from_rows(rows: [Rgb; 3]) -> Self {
        HudMatrix { red: rows[0], green: rows[1], blue: rows[2] }
    }

    pub fn row_mut(&mut self, i: usize) -> &mut Rgb {
        match i {
            0 => &mut self.red,
            1 => &mut self.green,
            _ => &mut self.blue,
        }
    }

    /// What a standard HUD colour (0–1 per channel) looks like through this matrix.
    pub fn apply(&self, c: Rgb) -> Rgb {
        let mut out = [0.0; 3];
        for (o, slot) in out.iter_mut().enumerate() {
            *slot = (c[0] * self.red[o] + c[1] * self.green[o] + c[2] * self.blue[o]).clamp(0.0, 1.0);
        }
        out
    }

    /// What an on-screen colour (sRGB, 0–1) looks like through this matrix, mixed in linear light
    /// as the HUD editors do (Arkku's through the browser, EDHM-UI's explicitly). Whether the game
    /// mixes in linear or gamma space isn't documented, so previews are an approximation.
    pub fn apply_srgb(&self, c: Rgb) -> Rgb {
        self.apply(c.map(to_linear)).map(to_srgb)
    }

    pub fn is_identity(&self) -> bool {
        self.approx_eq(&Self::IDENTITY)
    }

    /// Equal to within the precision written to the file.
    pub fn approx_eq(&self, other: &HudMatrix) -> bool {
        self.rows().iter().flatten().zip(other.rows().iter().flatten()).all(|(a, b)| (a - b).abs() < 5e-4)
    }

    /// This matrix followed by `then` (apply `self` first).
    pub fn then(&self, then: &HudMatrix) -> HudMatrix {
        HudMatrix::from_rows(self.rows().map(|r| mul_row(r, then)))
    }

    /// Every output scaled by `k`: below 1 dims the HUD, above 1 brightens it (until it clips).
    pub fn scaled(&self, k: f32) -> HudMatrix {
        HudMatrix::from_rows(self.rows().map(|r| r.map(|v| v * k)))
    }

    /// Rotates hues around the grey axis by `degrees`, keeping brightness (the CSS `hue-rotate`
    /// filter). The standard orange turns yellow, green, cyan, blue, purple, pink as the angle grows.
    pub fn hue_rotation(degrees: f32) -> HudMatrix {
        let (s, c) = degrees.to_radians().sin_cos();
        // Output-major (out_r = a[0][0]·r + a[0][1]·g + a[0][2]·b), as published for hue-rotate.
        let a = [
            [0.213 + c * 0.787 - s * 0.213, 0.715 - c * 0.715 - s * 0.715, 0.072 - c * 0.072 + s * 0.928],
            [0.213 - c * 0.213 + s * 0.143, 0.715 + c * 0.285 + s * 0.140, 0.072 - c * 0.072 - s * 0.283],
            [0.213 - c * 0.213 - s * 0.787, 0.715 - c * 0.715 + s * 0.715, 0.072 + c * 0.928 + s * 0.072],
        ];
        transpose(a)
    }

    /// Scales saturation: 0 is grey, 1 unchanged, above 1 more vivid.
    pub fn saturation(s: f32) -> HudMatrix {
        let mut a = [[0.0; 3]; 3];
        for (o, row) in a.iter_mut().enumerate() {
            for (i, v) in row.iter_mut().enumerate() {
                *v = LUMA[i] * (1.0 - s) + if i == o { s } else { 0.0 };
            }
        }
        transpose(a)
    }

    /// The simple controls: rotate the hue, set the saturation, then the brightness.
    pub fn from_adjust(adjust: &Adjust) -> HudMatrix {
        HudMatrix::hue_rotation(adjust.hue).then(&HudMatrix::saturation(adjust.saturation)).scaled(adjust.brightness)
    }

    /// Rows as the game writes them: `1, 0.25, 0`.
    pub fn format_row(row: Rgb) -> String {
        row.iter().map(|v| format_number(*v)).collect::<Vec<_>>().join(", ")
    }

    /// Parses a row such as `1, 0.25, 0` (any spacing). `None` unless exactly three numbers.
    pub fn parse_row(text: &str) -> Option<Rgb> {
        let mut it = text.split(',').map(|p| p.trim().parse::<f32>().ok().filter(|v| v.is_finite()));
        let row = [it.next()??, it.next()??, it.next()??];
        it.next().is_none().then_some(row)
    }

    /// The three `<Matrix…>` lines, the way commanders share themes on forums and in HUD editors.
    pub fn to_snippet(&self) -> String {
        Self::ROW_NAMES.iter().zip(self.rows()).map(|(n, r)| format!("<{n}>{}</{n}>", Self::format_row(r))).collect::<Vec<_>>().join("\n")
    }

    /// Reads a shared theme: any text holding `<MatrixRed>`, `<MatrixGreen>` and/or `<MatrixBlue>`
    /// rows (a forum post, a whole override file). Rows left out keep their standard values;
    /// `None` when there is no usable row or one of them isn't three numbers.
    pub fn from_snippet(text: &str) -> Option<HudMatrix> {
        let mut m = HudMatrix::IDENTITY;
        let mut any = false;
        for (i, name) in Self::ROW_NAMES.iter().enumerate() {
            let open = format!("<{name}>");
            let Some(start) = text.find(&open).map(|s| s + open.len()) else { continue };
            let end = text[start..].find(&format!("</{name}>"))? + start;
            *m.row_mut(i) = Self::parse_row(&text[start..end])?;
            any = true;
        }
        any.then_some(m)
    }
}

/// The three simple controls behind [`HudMatrix::from_adjust`].
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Adjust {
    /// Degrees, 0–360.
    pub hue: f32,
    /// 0 = grey, 1 = as the game draws it.
    pub saturation: f32,
    /// 1 = as the game draws it, lower is dimmer.
    pub brightness: f32,
}

impl Default for Adjust {
    fn default() -> Self {
        Adjust { hue: 0.0, saturation: 1.0, brightness: 1.0 }
    }
}

pub(crate) fn to_linear(c: f32) -> f32 {
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

fn to_srgb(c: f32) -> f32 {
    if c <= 0.003_130_8 { c * 12.92 } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 }
}

fn mul_row(r: Rgb, m: &HudMatrix) -> Rgb {
    let mut out = [0.0; 3];
    for (o, slot) in out.iter_mut().enumerate() {
        *slot = r[0] * m.red[o] + r[1] * m.green[o] + r[2] * m.blue[o];
    }
    out
}

fn transpose(a: [[f32; 3]; 3]) -> HudMatrix {
    HudMatrix { red: [a[0][0], a[1][0], a[2][0]], green: [a[0][1], a[1][1], a[2][1]], blue: [a[0][2], a[1][2], a[2][2]] }
}

/// Up to three decimals, trailing zeros dropped (`1`, `0.25`, `-0.3`).
fn format_number(v: f32) -> String {
    let s = format!("{:.3}", v);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    match s {
        "-0" | "" => "0".to_string(),
        s => s.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: Rgb, b: Rgb) -> bool {
        a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-3)
    }

    #[test]
    fn rows_say_where_each_channel_goes() {
        // The game's own RedToBlueTest preset: red becomes blue.
        let m = HudMatrix { red: [0.0, 0.0, 1.0], green: [0.0, 1.0, 0.0], blue: [1.0, 0.0, 0.0] };
        assert!(close(m.apply([1.0, 0.0, 0.0]), [0.0, 0.0, 1.0]));
        // Orange (red + some green) turns azure.
        assert!(close(m.apply([1.0, 0.5, 0.0]), [0.0, 0.5, 1.0]));
        // Only red feeds the output green here.
        let m = HudMatrix { red: [0.0, 1.0, 0.0], green: [0.0, 0.0, 0.0], blue: [0.0, 0.0, 0.0] };
        assert!(close(m.apply([1.0, 1.0, 1.0]), [0.0, 1.0, 0.0]));
    }

    #[test]
    fn identity_and_neutral_adjustments() {
        assert!(HudMatrix::IDENTITY.is_identity());
        assert!(HudMatrix::from_adjust(&Adjust::default()).is_identity());
        assert!(HudMatrix::hue_rotation(360.0).is_identity());
        // Grey stays grey under any hue rotation.
        assert!(close(HudMatrix::hue_rotation(123.0).apply([0.5, 0.5, 0.5]), [0.5, 0.5, 0.5]));
        // Brightness dims everything evenly.
        assert!(close(HudMatrix::IDENTITY.scaled(0.5).apply([1.0, 0.5, 0.0]), [0.5, 0.25, 0.0]));
        // Saturation 0 is grey.
        let g = HudMatrix::saturation(0.0).apply([1.0, 0.5, 0.0]);
        assert!((g[0] - g[1]).abs() < 1e-4 && (g[1] - g[2]).abs() < 1e-4);
    }

    #[test]
    fn previews_mix_in_linear_light() {
        let orange = [1.0, 117.0 / 255.0, 0.0];
        assert!(close(HudMatrix::IDENTITY.apply_srgb(orange), orange));
        // Half the light is not half the sRGB value.
        let half = HudMatrix::IDENTITY.scaled(0.5).apply_srgb([1.0, 1.0, 1.0]);
        assert!((half[0] - 0.735).abs() < 0.01, "{half:?}");
    }

    #[test]
    fn composition_applies_left_first() {
        let swap = HudMatrix { red: [0.0, 0.0, 1.0], green: [0.0, 1.0, 0.0], blue: [1.0, 0.0, 0.0] };
        let half = HudMatrix::IDENTITY.scaled(0.5);
        let c = [1.0, 0.25, 0.0];
        assert!(close(swap.then(&half).apply(c), half.apply(swap.apply(c))));
    }

    #[test]
    fn rows_round_trip() {
        assert_eq!(HudMatrix::parse_row(" 1, 0.25,0.5 "), Some([1.0, 0.25, 0.5]));
        assert_eq!(HudMatrix::parse_row("\t\t1, 0, 0 "), Some([1.0, 0.0, 0.0]));
        assert_eq!(HudMatrix::parse_row("-0.3, 1.5, 0"), Some([-0.3, 1.5, 0.0]));
        assert_eq!(HudMatrix::parse_row("1, 0"), None);
        assert_eq!(HudMatrix::parse_row("1, 0, 0, 0"), None);
        assert_eq!(HudMatrix::parse_row("1, x, 0"), None);
        assert_eq!(HudMatrix::parse_row("NaN, 0, 0"), None);
        assert_eq!(HudMatrix::format_row([1.0, 0.25, -0.3]), "1, 0.25, -0.3");
        assert_eq!(HudMatrix::format_row([0.33333, -0.0001, 2.0]), "0.333, 0, 2");
    }

    #[test]
    fn shared_snippets() {
        let m = HudMatrix { red: [0.0, 0.39, 1.0], green: [0.0, 1.0, 0.0], blue: [1.0, 0.0, 0.0] };
        assert_eq!(HudMatrix::from_snippet(&m.to_snippet()), Some(m));
        // As posted: indented, with the surrounding tags and odd spacing.
        let post = "Try this!\n<GUIColour>\n  <Default>\n    <LocalisationName>Standard</LocalisationName>\n    <MatrixRed>  0, 0.39, 1 </MatrixRed>\n    <MatrixGreen> 0, 1, 0 </MatrixGreen>\n    <MatrixBlue> 1, 0, 0 </MatrixBlue>\n  </Default>\n</GUIColour>";
        assert_eq!(HudMatrix::from_snippet(post), Some(m));
        // One row: the others stay standard.
        assert_eq!(HudMatrix::from_snippet("<MatrixRed>0, 0, 1</MatrixRed>"), Some(HudMatrix { red: [0.0, 0.0, 1.0], ..HudMatrix::IDENTITY }));
        assert_eq!(HudMatrix::from_snippet("no theme here"), None);
        assert_eq!(HudMatrix::from_snippet("<MatrixRed>0, 0</MatrixRed>"), None);
        assert_eq!(HudMatrix::from_snippet("<MatrixRed>0, 0, 1"), None);
    }
}
