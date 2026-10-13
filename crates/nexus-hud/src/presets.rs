//! Community HUD colour themes, credited to the commanders who made them. Most come from the
//! Frontier forums' "NO2O" list of themes that keep portraits and friend/foe colours usable.

use crate::matrix::HudMatrix;

#[derive(Debug, Clone, Copy)]
pub struct Preset {
    pub name: &'static str,
    /// Who made it and what to expect, in a sentence.
    pub note: &'static str,
    pub matrix: HudMatrix,
}

const fn m(red: [f32; 3], green: [f32; 3], blue: [f32; 3]) -> HudMatrix {
    HudMatrix { red, green, blue }
}

pub const STANDARD: Preset = Preset { name: "Standard orange", note: "The game's own colours.", matrix: HudMatrix::IDENTITY };

pub const ALL: &[Preset] = &[
    STANDARD,
    Preset {
        name: "Mocha",
        note: "By CMDR Uncertain: coffee instead of orange, a softer HUD. Reported to work well in Odyssey.",
        matrix: m([0.7, 0.08, -0.08], [0.0, 1.0, 1.0], [0.15, -0.15, 1.0]),
    },
    Preset {
        name: "Lemon",
        note: "By CMDR Mordu: pale yellow HUD, hostiles yellow and friendlies cornflower blue, so friend and foe differ in blue and yellow rather than red and green.",
        matrix: m([0.39, 0.47, 0.21], [0.08, 0.14, 1.0], [1.0, -0.03, -1.0]),
    },
    Preset { name: "Empire blue", note: "By CMDR Diomedian.", matrix: m([0.0, 0.15, 1.0], [0.0, 1.0, 0.85], [1.0, 0.0, 0.0]) },
    Preset { name: "Federation", note: "By CMDR Diomedian.", matrix: m([1.0, 0.15, 0.0], [0.0, 1.0, 0.85], [0.0, 0.0, 1.0]) },
    Preset { name: "Alliance green", note: "By CMDR Diomedian.", matrix: m([0.0, 0.3, 0.0], [0.0, 1.0, 0.85], [1.0, 0.0, 0.0]) },
    Preset { name: "Royal purple", note: "By CMDR Alex Traut.", matrix: m([0.3, 0.0, 1.0], [0.25, 1.0, 0.25], [1.0, 0.0, 0.0]) },
    Preset { name: "Blue and red", note: "By Arkku, author of the HUD colour editor: a blue HUD that keeps friendlies green.", matrix: m([0.2, 0.05, 2.0], [-0.15, 2.0, -0.5], [1.75, -1.0, -0.3]) },
    Preset { name: "VR yellow", note: "Picked by Prima Games as the easiest to read in VR.", matrix: m([1.0, 0.97, 0.0], [1.0, 0.0, 0.0], [1.0, 0.0, 0.0]) },
    Preset { name: "Grey", note: "The game's own unused DesaturateTest preset: no colour at all.", matrix: m([0.33, 0.33, 0.33], [0.33, 0.33, 0.33], [0.33, 0.33, 0.33]) },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_unique_and_only_standard_is_identity() {
        let mut names: Vec<_> = ALL.iter().map(|p| p.name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), ALL.len());
        assert_eq!(ALL.iter().filter(|p| p.matrix.is_identity()).count(), 1);
    }
}
