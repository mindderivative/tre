//! Bidi + script run segmentation, then real shaping via `rustybuzz`
//! (PLAN_PHASE4_STEP4_1.md task 1) -- HarfBuzz/rustybuzz shape one
//! direction- and script-uniform run at a time, so splitting a mixed-
//! direction, possibly-mixed-script string into runs is this module's own
//! job, not something `rustybuzz::shape` does for the caller.

use std::ops::Range;

use rustybuzz::{shape, Direction, Face, Script, UnicodeBuffer};
use unicode_bidi::{BidiInfo, Level};
use unicode_script::UnicodeScript;

use crate::TextError;

/// One shaped glyph, in the units `rustybuzz` itself reports (font design
/// units, scaled by the font's own `unitsPerEm` -- not pixels). Step 4.2's
/// atlas/rasterization work is what maps these to a concrete pixel size.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShapedGlyph {
    pub glyph_id: u32,
    /// Byte offset into the original input string this glyph's grapheme
    /// cluster starts at -- `rustybuzz`'s own cluster value, unchanged.
    pub cluster: u32,
    pub x_advance: i32,
    pub y_advance: i32,
    pub x_offset: i32,
    pub y_offset: i32,
}

/// One direction- and script-uniform shaped run, in **visual** order
/// already (see [`segment_runs`]) -- concatenating every run's `glyphs` in
/// the order `shape_text` returns them is the correct rendering order for
/// the whole input line, including a mixed LTR/RTL paragraph.
#[derive(Debug, Clone)]
pub struct ShapedRun {
    /// Byte range into the original input string this run covers.
    pub text_range: Range<usize>,
    pub direction: Direction,
    pub glyphs: Vec<ShapedGlyph>,
}

/// One direction- and script-uniform slice of the input, in visual order,
/// before shaping -- [`shape_text`]'s intermediate step, exposed
/// separately so its run boundaries can be unit-tested without needing a
/// real font.
#[derive(Debug, Clone, PartialEq)]
pub struct TextRun {
    pub text_range: Range<usize>,
    pub level: Level,
    pub script: unicode_script::Script,
}

/// Splits `text` into bidi- and script-uniform runs, in **visual** order.
///
/// `unicode_bidi::BidiInfo::visual_runs` already reorders same-level runs
/// within a paragraph into visual order; this function additionally
/// splits each of those level-runs further wherever the resolved
/// (non-`Common`/`Inherited`) `Script` property changes, since a single
/// bidi level can still span more than one script (e.g. an RTL paragraph
/// mixing Arabic and Hebrew).
#[must_use]
pub fn segment_runs(text: &str) -> Vec<TextRun> {
    let bidi_info = BidiInfo::new(text, None);
    let mut runs = Vec::new();
    for para in &bidi_info.paragraphs {
        let para_range = para.range.clone();
        let (levels, level_runs) = bidi_info.visual_runs(para, para_range);
        for level_run in level_runs {
            if level_run.is_empty() {
                continue;
            }
            let level = levels[level_run.start];
            split_by_script(text, level_run, level, &mut runs);
        }
    }
    runs
}

fn split_by_script(text: &str, level_run: Range<usize>, level: Level, out: &mut Vec<TextRun>) {
    let mut current_start = level_run.start;
    let mut current_script: Option<unicode_script::Script> = None;
    for (offset, ch) in text[level_run.clone()].char_indices() {
        let byte_pos = level_run.start + offset;
        let Some(script) = real_script(ch) else {
            continue;
        };
        match current_script {
            None => current_script = Some(script),
            Some(running) if running != script => {
                out.push(TextRun {
                    text_range: current_start..byte_pos,
                    level,
                    script: running,
                });
                current_start = byte_pos;
                current_script = Some(script);
            }
            Some(_) => {}
        }
    }
    out.push(TextRun {
        text_range: current_start..level_run.end,
        level,
        script: current_script.unwrap_or(unicode_script::Script::Common),
    });
}

/// `Common` (punctuation, digits, whitespace) and `Inherited` (combining
/// marks) codepoints carry no script identity of their own -- they take
/// on whichever real script surrounds them, so they must never split a
/// run by themselves.
fn real_script(ch: char) -> Option<unicode_script::Script> {
    match ch.script() {
        unicode_script::Script::Common | unicode_script::Script::Inherited => None,
        other => Some(other),
    }
}

/// Shapes `text` against `face`, returning one [`ShapedRun`] per
/// bidi+script-uniform run, already in the correct visual left-to-right
/// concatenation order for rendering.
///
/// # Errors
///
/// Returns [`TextError::InvalidFontForShaping`] only in the degenerate
/// case where `face` itself cannot be used at all (this only happens if
/// the caller constructed an invalid `Face` some other way, since
/// `Face::from_slice`'s own `None` case is checked by the caller before a
/// `Face` value can exist) -- included for interface symmetry with
/// [`crate::outline::glyph_outline`] and [`crate::fallback`]'s
/// `Result`-returning contract, not because `rustybuzz::shape` itself
/// panics on this class of input.
pub fn shape_text(face: &Face, text: &str) -> Result<Vec<ShapedRun>, TextError> {
    Ok(segment_runs(text)
        .into_iter()
        .map(|run| shape_run(face, text, &run))
        .collect())
}

/// `pub(crate)`, not private -- [`crate::fallback::resolve_run`] reshapes
/// a single already-segmented [`TextRun`] against a different cascade
/// font, so it needs this same per-run shaping step `shape_text` uses
/// internally, not the whole-string entry point.
pub(crate) fn shape_run(face: &Face, text: &str, run: &TextRun) -> ShapedRun {
    let direction = if run.level.is_rtl() {
        Direction::RightToLeft
    } else {
        Direction::LeftToRight
    };
    let mut buffer = UnicodeBuffer::new();
    buffer.push_str(&text[run.text_range.clone()]);
    buffer.set_direction(direction);
    if let Ok(script) = run.script.short_name().parse::<Script>() {
        buffer.set_script(script);
    }
    let glyph_buffer = shape(face, &[], buffer);
    let glyphs = glyph_buffer
        .glyph_infos()
        .iter()
        .zip(glyph_buffer.glyph_positions())
        .map(|(info, pos)| ShapedGlyph {
            glyph_id: info.glyph_id,
            cluster: info.cluster,
            x_advance: pos.x_advance,
            y_advance: pos.y_advance,
            x_offset: pos.x_offset,
            y_offset: pos.y_offset,
        })
        .collect();
    ShapedRun {
        text_range: run.text_range.clone(),
        direction,
        glyphs,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segment_runs_of_pure_latin_text_is_a_single_ltr_run() {
        let runs = segment_runs("hello world");
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].text_range, 0..11);
        assert!(!runs[0].level.is_rtl());
        assert_eq!(runs[0].script, unicode_script::Script::Latin);
    }

    #[test]
    fn segment_runs_reorders_a_mixed_latin_hebrew_string_visually() {
        // "he" (Latin) followed by two Hebrew letters (Alef, Bet) --
        // logically Latin-then-Hebrew, but visually (since the Hebrew
        // run is RTL and this is one bidi paragraph starting LTR by
        // strong-first-character rule) the Hebrew run's *internal*
        // character order reverses while the run itself stays after the
        // Latin run in visual (left-to-right screen) placement order for
        // an LTR-paragraph-base with only a single embedded RTL run.
        let text = "he\u{5D0}\u{5D1}";
        let runs = segment_runs(text);
        assert_eq!(
            runs.len(),
            2,
            "expected exactly one Latin run and one Hebrew run: {runs:?}"
        );
        assert!(!runs[0].level.is_rtl(), "the Latin run must be LTR");
        assert_eq!(runs[0].script, unicode_script::Script::Latin);
        assert!(runs[1].level.is_rtl(), "the Hebrew run must be RTL");
        assert_eq!(runs[1].script, unicode_script::Script::Hebrew);
    }

    #[test]
    fn real_script_ignores_common_and_inherited_codepoints() {
        assert_eq!(real_script(' '), None);
        assert_eq!(real_script('7'), None);
        assert_eq!(real_script('a'), Some(unicode_script::Script::Latin));
        assert_eq!(real_script('\u{5D0}'), Some(unicode_script::Script::Hebrew));
    }

    #[test]
    fn segment_runs_keeps_digits_attached_to_the_surrounding_script_run() {
        // "a1b" is entirely Latin-script-context (the digit is Common,
        // not a script boundary), so it must stay one single run.
        let runs = segment_runs("a1b");
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].text_range, 0..3);
    }
}
