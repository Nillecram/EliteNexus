//! Error type for loading and parsing bindings.

use std::path::PathBuf;

/// Everything that can go wrong while reading a `.binds` / `.start` file.
#[derive(Debug, thiserror::Error)]
pub enum BindsError {
    /// The file could not be read.
    #[error("cannot read {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// The text is not well-formed XML.
    #[error("malformed bindings XML: {0}")]
    Xml(#[from] roxmltree::Error),
    /// Well-formed XML whose root element is not `<Root>`.
    #[error("not an Elite Dangerous bindings file (root element is <{0}>, expected <Root>)")]
    NotBindings(String),
    /// A preset named by `StartPreset.*.start` has no `.binds` file in any searched folder.
    #[error("preset {0:?} not found (built-in presets live in the game's ControlSchemes folder)")]
    PresetNotFound(String),
    /// The bindings folder contains no `.binds` file at all.
    #[error("no .binds files in {0}")]
    NoPresets(PathBuf),
}

impl BindsError {
    pub(crate) fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }
}
