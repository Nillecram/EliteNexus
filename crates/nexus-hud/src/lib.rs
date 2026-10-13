//! Elite Dangerous HUD colours.
//!
//! * [`matrix`]: the 3×3 colour matrix the game recolours its interface with, how it maps a
//!   colour ([`HudMatrix::apply`]) and how simple controls (hue, saturation, brightness) build one.
//! * [`override_file`]: reading and surgically writing `GraphicsConfigurationOverride.xml`, where
//!   the game looks for a commander's own colours (read at launch only).
//! * [`palette`]: the game's standard interface colours, for previews, and [`palette::concerns`]:
//!   what a matrix makes hard to see (friend and foe merging, a HUD too dark to read).
//! * [`presets`]: community themes, credited.
//! * [`install`]: the game's install folders and whether a shader mod (EDHM) is in one.
//! * [`tone`]: glare and exposure, the tone-mapping values in the same file.
//! * [`dimmer`]: the cockpit-light dimmer (3Dmigoto and a pattern for the game's box-light shader).

pub mod dimmer;
pub mod error;
pub mod install;
pub mod matrix;
pub mod override_file;
pub mod palette;
pub mod presets;
pub mod tone;

pub use error::HudError;
pub use matrix::{Adjust, HudMatrix, Rgb};
pub use override_file::{OverrideFile, default_graphics_dir, read_matrix, read_value, with_matrix, with_value};
