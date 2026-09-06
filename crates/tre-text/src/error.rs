use std::fmt;

/// Errors this crate reports via `Result` rather than panicking, matching
/// every other crate in this workspace (`SvgError`, `EngineError`).
#[derive(Debug)]
pub enum TextError {
    /// `rustybuzz::Face::from_slice` rejected the font bytes -- not a
    /// valid/parseable OpenType/TrueType font.
    InvalidFontForShaping,
    /// `skrifa::FontRef::new` rejected the font bytes, or the requested
    /// glyph has no entry in the font's outline table at all (e.g. a
    /// bitmap-only font, or a glyph ID past the font's own glyph count).
    InvalidFontForOutlines,
    /// `skrifa`'s outline-drawing call itself failed for a glyph that did
    /// resolve to an outline entry (a malformed `glyf`/CFF table).
    OutlineDrawFailed,
    /// `fontconfig::Fontconfig::new()` returned `None` (Fontconfig itself
    /// unavailable), or none of the requested cascade families resolved to
    /// any installed font at all.
    FontDiscoveryUnavailable,
}

impl fmt::Display for TextError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFontForShaping => write!(f, "font data is not valid for shaping"),
            Self::InvalidFontForOutlines => {
                write!(f, "font data or glyph is not valid for outline extraction")
            }
            Self::OutlineDrawFailed => write!(f, "failed to draw glyph outline"),
            Self::FontDiscoveryUnavailable => {
                write!(f, "no fonts could be discovered via fontconfig")
            }
        }
    }
}

impl std::error::Error for TextError {}
