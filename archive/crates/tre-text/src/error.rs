/// Errors this crate reports via `Result` rather than panicking, matching
/// every other crate in this workspace (`SvgError`, `EngineError`).
#[derive(Debug, thiserror::Error)]
pub enum TextError {
    /// `rustybuzz::Face::from_slice` rejected the font bytes -- not a
    /// valid/parseable OpenType/TrueType font.
    #[error("font data is not valid for shaping")]
    InvalidFontForShaping,
    /// `skrifa::FontRef::new` rejected the font bytes, or the requested
    /// glyph has no entry in the font's outline table at all (e.g. a
    /// bitmap-only font, or a glyph ID past the font's own glyph count).
    #[error("font data or glyph is not valid for outline extraction")]
    InvalidFontForOutlines,
    /// `skrifa`'s outline-drawing call itself failed for a glyph that did
    /// resolve to an outline entry (a malformed `glyf`/CFF table).
    #[error("failed to draw glyph outline")]
    OutlineDrawFailed,
    /// `fontconfig::Fontconfig::new()` returned `None` (Fontconfig itself
    /// unavailable), or none of the requested cascade families resolved to
    /// any installed font at all.
    #[error("no fonts could be discovered via fontconfig")]
    FontDiscoveryUnavailable,
}
