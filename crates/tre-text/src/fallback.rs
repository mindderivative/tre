//! Font fallback cascade via real `fontconfig` system discovery
//! (PLAN_PHASE4_STEP4_1.md task 2), Linux-only this step -- Windows
//! (DirectWrite)/macOS (Core Text) system font discovery is deferred,
//! matching Phase 1's platform-gating precedent.

use std::path::PathBuf;

use fontconfig::Fontconfig;
use rustybuzz::Face;
use skrifa::{FontRef, MetadataProvider};

use crate::shape::{shape_run, ShapedRun, TextRun};
use crate::TextError;

/// The generic family names this cascade queries, in priority order --
/// `fontconfig` itself resolves each to whatever the system's actual
/// configuration maps it to; this project makes no assumption about which
/// concrete font file that turns out to be on any given machine.
const CASCADE_FAMILIES: [&str; 3] = ["sans-serif", "Noto Sans", "emoji"];

/// An ordered list of font file paths to try in turn: primary UI sans,
/// then a broad-coverage fallback, then a color emoji font.
#[derive(Debug, Clone)]
pub struct FontCascade {
    pub entries: Vec<PathBuf>,
}

impl FontCascade {
    /// # Errors
    ///
    /// [`TextError::FontDiscoveryUnavailable`] if Fontconfig itself can't
    /// be initialized, or if none of [`CASCADE_FAMILIES`] resolves to any
    /// installed font at all.
    pub fn discover() -> Result<Self, TextError> {
        let fc = Fontconfig::new().ok_or(TextError::FontDiscoveryUnavailable)?;
        let mut entries = Vec::new();
        for family in CASCADE_FAMILIES {
            if let Some(font) = fc.find(family, None) {
                if !entries.contains(&font.path) {
                    entries.push(font.path);
                }
            }
        }
        if entries.is_empty() {
            return Err(TextError::FontDiscoveryUnavailable);
        }
        Ok(Self { entries })
    }
}

/// True if every non-whitespace character of `text` maps to a real glyph
/// (not `.notdef`) in `font_bytes`.
///
/// # Errors
///
/// [`TextError::InvalidFontForOutlines`] if `font_bytes` isn't a font
/// `skrifa` can parse at all (reusing that variant rather than adding a
/// third "invalid font" case for what is, underneath, the same failure
/// mode as outline extraction hitting unparseable font data).
pub fn covers(font_bytes: &[u8], text: &str) -> Result<bool, TextError> {
    let font = FontRef::new(font_bytes).map_err(|_| TextError::InvalidFontForOutlines)?;
    let charmap = font.charmap();
    Ok(text
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .all(|ch| charmap.map(ch).is_some()))
}

/// Picks the first `font_candidates` entry that fully covers `run_text`'s
/// characters, falling back to index `0` (the primary font) if none does
/// -- a run degrading to `.notdef` tofu glyphs in the primary font is
/// still a defined, visible outcome, not a panic or a silently dropped
/// run.
///
/// # Errors
///
/// Propagates [`covers`]'s error if any candidate's bytes aren't a font
/// `skrifa` can parse.
pub fn resolve_font_index(font_candidates: &[&[u8]], run_text: &str) -> Result<usize, TextError> {
    for (index, bytes) in font_candidates.iter().enumerate() {
        if covers(bytes, run_text)? {
            return Ok(index);
        }
    }
    Ok(0)
}

/// Resolves and shapes one already-segmented [`TextRun`] against whichever
/// `font_candidates` entry actually covers it, returning both which index
/// was chosen and the resulting [`ShapedRun`] -- real fallback behavior,
/// not a cosmetic pass-through that always picks index `0`.
///
/// # Errors
///
/// Propagates [`resolve_font_index`]'s error, or
/// [`TextError::InvalidFontForShaping`] if the resolved candidate's bytes
/// aren't a font `rustybuzz` can parse.
pub fn resolve_run(
    font_candidates: &[&[u8]],
    text: &str,
    run: &TextRun,
) -> Result<(usize, ShapedRun), TextError> {
    let run_text = &text[run.text_range.clone()];
    let font_index = resolve_font_index(font_candidates, run_text)?;
    let face =
        Face::from_slice(font_candidates[font_index], 0).ok_or(TextError::InvalidFontForShaping)?;
    Ok((font_index, shape_run(&face, text, run)))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reads a real, installed font's bytes by family name via
    /// `fontconfig` -- panics with a clear message (not a silent skip) if
    /// the family isn't installed, so a CI environment missing an
    /// expected font package fails loudly rather than passing vacuously.
    fn read_family(family: &str) -> Vec<u8> {
        let fc = Fontconfig::new().expect("fontconfig must be available to run this test");
        let font = fc
            .find(family, None)
            .unwrap_or_else(|| panic!("font family {family:?} must be installed to run this test"));
        std::fs::read(&font.path)
            .unwrap_or_else(|e| panic!("failed to read {}: {e}", font.path.display()))
    }

    #[test]
    fn covers_reports_true_for_a_glyph_a_font_actually_has() {
        let dejavu = read_family("DejaVu Sans");
        assert!(covers(&dejavu, "hello").unwrap());
    }

    // DejaVu Sans's own bundled Unicode coverage is unexpectedly broad --
    // it turns out to include the classic "Emoticons" block (U+1F600-
    // U+1F61F, confirmed via `fc-query`'s charset dump), so a naive
    // "surely a plain-text font lacks this emoji" assumption using e.g.
    // U+1F600 (the grinning face) is simply false on this real, installed
    // font and would have made this test assert something untrue. U+1F9E0
    // (the "brain" emoji, a newer Unicode block DejaVu Sans's charset
    // dump has zero coverage of at all) is independently confirmed absent
    // from DejaVu Sans and present in Noto Color Emoji the same way.
    const UNCOVERED_BY_DEJAVU_SANS: char = '\u{1F9E0}';

    #[test]
    fn covers_reports_false_for_a_color_emoji_a_plain_text_font_lacks() {
        let dejavu = read_family("DejaVu Sans");
        assert!(!covers(&dejavu, &UNCOVERED_BY_DEJAVU_SANS.to_string()).unwrap());
    }

    #[test]
    fn resolve_font_index_falls_through_to_the_font_that_actually_covers_the_run() {
        let dejavu = read_family("DejaVu Sans");
        let emoji = read_family("Noto Color Emoji");
        let candidates: [&[u8]; 2] = [&dejavu, &emoji];

        assert_eq!(
            resolve_font_index(&candidates, "hello").unwrap(),
            0,
            "plain Latin text is covered by the primary font already"
        );
        assert_eq!(
            resolve_font_index(&candidates, &UNCOVERED_BY_DEJAVU_SANS.to_string()).unwrap(),
            1,
            "an emoji codepoint the primary font lacks must resolve to the emoji fallback"
        );
    }
}
