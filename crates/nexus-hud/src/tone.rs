//! Glare and exposure: the game's tone-mapping values in `<HDRNode_Reference>`, which players
//! change to tame Odyssey's glare and its brightness adaptation (the auto-exposure that darkens
//! everything else when the cockpit lights fill the view). Frontier documents none of them; what
//! each does is what players report, and the curve maths below.

use serde::{Deserialize, Serialize};

use crate::error::HudError;
use crate::override_file::{read_value, with_value};

pub const NODE: &str = "HDRNode_Reference";
const GLARE: &str = "GlareCompensation";
const PERCENTILES: &str = "Percentiles";
const ACES: [&str; 3] = ["ACES_C", "ACES_D", "ACES_E"];
/// The two ACES coefficients nobody changes (the game ships the standard fit's values).
const ACES_A: f32 = 2.51;
const ACES_B: f32 = 0.03;

/// Tone-mapping values set in the override file; `None` = the game's own value.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Tone {
    /// `GlareCompensation`: lower values reduce the peak brightness of glare from lights.
    pub glare: Option<f32>,
    /// `Percentiles`: which parts of the picture's brightness the auto-exposure aims at.
    pub percentiles: Option<[f32; 3]>,
    /// `ACES_C`, `ACES_D`, `ACES_E`: the shape of the tone curve (contrast in darks and mid-tones).
    pub aces: Option<[f32; 3]>,
}

/// The values the game ships with (Odyssey 4.4, `GraphicsConfiguration.xml`), used when the
/// installed game's own file can't be read.
pub const GAME_4_4: Defaults = Defaults { glare: 1.3333, percentiles: [0.7, 0.8, 0.9], aces: [2.4543, 0.295, 0.42] };

/// The standard ACES filmic curve fit (Krzysztof Narkowicz), which the game's own C, D and E
/// depart from.
pub const STANDARD_ACES: [f32; 3] = [2.43, 0.59, 0.14];

/// The game's own tone-mapping values.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Defaults {
    pub glare: f32,
    pub percentiles: [f32; 3],
    pub aces: [f32; 3],
}

impl Defaults {
    /// Reads them from the game's `GraphicsConfiguration.xml`; values it lacks keep the 4.4 ones.
    pub fn from_base(text: &str) -> Result<Defaults, HudError> {
        let t = read(text)?;
        Ok(Defaults { glare: t.glare.unwrap_or(GAME_4_4.glare), percentiles: t.percentiles.unwrap_or(GAME_4_4.percentiles), aces: t.aces.unwrap_or(GAME_4_4.aces) })
    }
}

impl Tone {
    pub fn is_default(&self) -> bool {
        *self == Tone::default()
    }

    /// The values the game will use: these, or the game's own.
    pub fn effective(&self, d: &Defaults) -> Defaults {
        Defaults { glare: self.glare.unwrap_or(d.glare), percentiles: self.percentiles.unwrap_or(d.percentiles), aces: self.aces.unwrap_or(d.aces) }
    }

    /// Drops values equal to the game's own, so the file only holds real changes.
    pub fn normalized(&self, d: &Defaults) -> Tone {
        let near = |a: &[f32], b: &[f32]| a.iter().zip(b).all(|(x, y)| (x - y).abs() < 5e-5);
        Tone {
            glare: self.glare.filter(|g| !near(&[*g], &[d.glare])),
            percentiles: self.percentiles.filter(|p| !near(p, &d.percentiles)),
            aces: self.aces.filter(|a| !near(a, &d.aces)),
        }
    }
}

/// A named set of values players have used.
#[derive(Debug, Clone, Copy)]
pub struct TonePreset {
    pub name: &'static str,
    pub note: &'static str,
    pub tone: Tone,
}

pub const PRESETS: &[TonePreset] = &[
    TonePreset { name: "Game default", note: "The values the game ships with.", tone: Tone { glare: None, percentiles: None, aces: None } },
    TonePreset {
        name: "Less glare",
        note: "Glare compensation down to 1.0. CMDR Morbad, Frontier forums, Feb 2022: the brightness adaptation can't be switched off, but turning this down reduces the peak brightness of glare from lights.",
        tone: Tone { glare: Some(1.0), percentiles: None, aces: None },
    },
    TonePreset {
        name: "Morbad's Odyssey tweak",
        note: "Posted by CMDR Morbad on the Frontier forums in May 2021 for commanders who found Odyssey too bright and washed out: less glare, different auto-exposure targets and the standard ACES curve.",
        tone: Tone { glare: Some(1.0), percentiles: Some([0.6, 0.54, 0.9]), aces: Some(STANDARD_ACES) },
    },
];

/// The tone-mapping values an override file sets.
pub fn read(text: &str) -> Result<Tone, HudError> {
    let num = |name: &'static str| -> Result<Option<f32>, HudError> {
        match read_value(text, &[NODE, name])? {
            None => Ok(None),
            Some(v) => v.parse::<f32>().ok().filter(|x| x.is_finite()).map(Some).ok_or(HudError::BadValue { name, text: v }),
        }
    };
    let percentiles = match read_value(text, &[NODE, PERCENTILES])? {
        None => None,
        Some(v) => Some(crate::matrix::HudMatrix::parse_row(&v).ok_or(HudError::BadRow { row: PERCENTILES, text: v })?),
    };
    let [c, d, e] = [num(ACES[0])?, num(ACES[1])?, num(ACES[2])?];
    // The curve only means something as a whole: a partial set mixes with the game's values.
    let aces = match (c, d, e) {
        (None, None, None) => None,
        (c, d, e) => Some([c.unwrap_or(GAME_4_4.aces[0]), d.unwrap_or(GAME_4_4.aces[1]), e.unwrap_or(GAME_4_4.aces[2])]),
    };
    Ok(Tone { glare: num(GLARE)?, percentiles, aces })
}

/// `text` with these tone-mapping values written (and the ones left `None` removed).
pub fn write(text: Option<&str>, tone: &Tone) -> Result<String, HudError> {
    let mut out = text.unwrap_or("").to_string();
    let mut set = |name: &str, value: Option<String>| -> Result<(), HudError> {
        out = with_value(Some(&out), &[NODE, name], value.as_deref())?;
        Ok(())
    };
    set(GLARE, tone.glare.map(|g| number(g, 4)))?;
    set(PERCENTILES, tone.percentiles.map(|p| p.iter().map(|v| number(*v, 3)).collect::<Vec<_>>().join(",")))?;
    for (i, name) in ACES.iter().enumerate() {
        set(name, tone.aces.map(|a| number(a[i], 4)))?;
    }
    // Nothing set and no file before: stay without one.
    if text.is_none() && out.is_empty() {
        return Ok(String::new());
    }
    Ok(out)
}

fn number(v: f32, decimals: usize) -> String {
    let s = format!("{v:.decimals$}");
    let s = s.trim_end_matches('0');
    let s = s.strip_suffix('.').map(|s| format!("{s}.0")).unwrap_or_else(|| s.to_string());
    if s == "-0.0" { "0.0".into() } else { s }
}

/// The ACES tone curve with coefficients C, D and E: scene brightness (1 = a bright surface)
/// to screen brightness (0–1).
pub fn aces_curve(x: f32, cde: [f32; 3]) -> f32 {
    let [c, d, e] = cde;
    ((x * (ACES_A * x + ACES_B)) / (x * (c * x + d) + e)).clamp(0.0, 1.0)
}

/// How much brighter (above 1) or darker one curve draws a dark tone, a mid-tone and a bright
/// tone than another: what a curve change looks like, in numbers.
pub fn curve_ratios(cde: [f32; 3], against: [f32; 3]) -> [f32; 3] {
    [0.03, 0.18, 1.0].map(|x| aces_curve(x, cde) / aces_curve(x, against).max(1e-6))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_the_override_file() {
        let morbad = PRESETS[2].tone;
        let text = write(None, &morbad).unwrap();
        assert!(text.contains("<GlareCompensation>1.0</GlareCompensation>"), "{text}");
        assert!(text.contains("<Percentiles>0.6,0.54,0.9</Percentiles>"), "{text}");
        assert!(text.contains("<ACES_D>0.59</ACES_D>"), "{text}");
        assert_eq!(read(&text).unwrap(), morbad);
        // Back to the game's values: the section goes, the file stays valid.
        let back = write(Some(&text), &Tone::default()).unwrap();
        assert_eq!(read(&back).unwrap(), Tone::default());
        assert!(!back.contains(NODE), "{back}");
        assert_eq!(write(None, &Tone::default()).unwrap(), "");
    }

    #[test]
    fn reads_the_post_as_written() {
        // CMDR Morbad's file, as attached to the forum post.
        let post = "<?xml version=\"1.0\" encoding=\"UTF-8\" ?>\n<GraphicsConfig>\n\t<HDRNode_Reference>\n\t\t<Percentiles>0.6,0.540000,0.9000</Percentiles>\n\t\t<GlareCompensation>1.0</GlareCompensation>\n\t\t<ACES_C>2.43</ACES_C>\n\t\t<ACES_D>0.59</ACES_D>\n\t\t<ACES_E>0.14</ACES_E>\n\t</HDRNode_Reference>\n</GraphicsConfig>\n";
        assert_eq!(read(post).unwrap(), PRESETS[2].tone);
        // Writing it again tidies the numbers and keeps the meaning.
        let again = write(Some(post), &PRESETS[2].tone).unwrap();
        assert!(again.contains("<Percentiles>0.6,0.54,0.9</Percentiles>"), "{again}");
        assert_eq!(read(&again).unwrap(), PRESETS[2].tone);
        assert!(matches!(read("<GraphicsConfig><HDRNode_Reference><GlareCompensation>x</GlareCompensation></HDRNode_Reference></GraphicsConfig>"), Err(HudError::BadValue { name: "GlareCompensation", .. })));
        assert!(matches!(read("<GraphicsConfig><HDRNode_Reference><Percentiles>1,2</Percentiles></HDRNode_Reference></GraphicsConfig>"), Err(HudError::BadRow { row: "Percentiles", .. })));
    }

    #[test]
    fn defaults_come_from_the_games_file() {
        let base = "<GraphicsConfig><HDRNode><GlareCompensation>9</GlareCompensation></HDRNode><HDRNode_Reference><GlareCompensation>1.3333</GlareCompensation><Percentiles>0.7,0.8,0.9</Percentiles><ACES_A>2.51</ACES_A><ACES_C>2.4543</ACES_C><ACES_D>0.295</ACES_D><ACES_E>0.42</ACES_E></HDRNode_Reference></GraphicsConfig>";
        assert_eq!(Defaults::from_base(base).unwrap(), GAME_4_4);
        let t = Tone { glare: Some(1.3333), percentiles: Some([0.6, 0.54, 0.9]), aces: Some(GAME_4_4.aces) };
        assert_eq!(t.normalized(&GAME_4_4), Tone { percentiles: Some([0.6, 0.54, 0.9]), ..Tone::default() });
        assert_eq!(Tone::default().effective(&GAME_4_4), GAME_4_4);
    }

    #[test]
    fn the_standard_curve_lifts_darks_and_mid_tones() {
        // Same white point, about 2.5x brighter darks and 1.7x brighter mid-tones than the game's.
        let [dark, mid, bright] = curve_ratios(STANDARD_ACES, GAME_4_4.aces);
        assert!(dark > 2.0 && dark < 3.0, "{dark}");
        assert!(mid > 1.5 && mid < 1.9, "{mid}");
        assert!((bright - 1.0).abs() < 0.02, "{bright}");
        assert!((aces_curve(0.0, GAME_4_4.aces)).abs() < 1e-6);
        assert!(aces_curve(100.0, GAME_4_4.aces) <= 1.0);
    }
}
