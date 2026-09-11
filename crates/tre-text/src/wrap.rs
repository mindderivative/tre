//! Real line-breaking (Phase 15 Steps 15.1/15.5) -- pure logic over
//! already-shaped glyph data, engine-side so it's testable without a
//! real font or a Python binding, mirroring `caret.rs`'s own "testable
//! in isolation" precedent. Reuses `caret_positions`'s exact pen-advance
//! formula (`scale = px_size / units_per_em`, `x_advance * scale`) so a
//! wrapped line's own measured width and its later rendered width never
//! diverge.
//!
//! Break *opportunities* -- where a line is allowed or required to end
//! -- come from `unicode-linebreak`'s real implementation of [UAX
//! #14][uax14] (Step 15.5; Step 15.1's own v1 hand-rolled these from
//! whitespace/`\n` alone). This covers real hyphen/punctuation/CJK-
//! ideograph break points, not just spaces -- the same "use the real,
//! established algorithm, don't hand-roll UAX rule tables" precedent
//! `caret.rs`'s own bidi/script handling already follows via
//! `unicode-bidi`/`unicode-script`.
//!
//! [uax14]: https://www.unicode.org/reports/tr14/
//!
//! **Real, disclosed v1 scope**: LTR/single-script text only (matching
//! `caret.rs`'s own established scope -- glyph `cluster` byte offsets
//! are only monotonic within one run for LTR text); a single
//! unbreakable token wider than `max_width` on its own still gets its
//! own line rather than being split further (no hyphenation of an
//! already-unbroken word); left-aligned only.
//!
//! **Trailing-whitespace convention** (matches how a real word processor
//! renders a wrap point, not a byte-loss bug): whitespace immediately
//! before any line break -- whether an explicit `\n` or an automatic
//! wrap -- is consumed into the line being closed as an unrendered
//! *tail*: its bytes are still covered by that line's own `byte_range`
//! (so every byte offset still maps to exactly one line, and a caret
//! can still be placed there), but it contributes no width and no
//! glyphs to that line, and the next line starts clean with no leading
//! space. Leading whitespace at the very start of the whole input, or
//! right after an explicit `\n`, is real content and IS rendered.

use std::ops::Range;

use unicode_linebreak::{linebreaks, BreakOpportunity};

use crate::shape::ShapedRun;

/// One real visual line produced by [`wrap_lines`]. `byte_range` covers
/// every byte of the original input this line is responsible for --
/// including any trailing whitespace/newline consumed at a line break
/// (see the module doc comment) -- so every `byte_range` across one
/// `wrap_lines` call's whole result is contiguous and gapless, letting
/// a caller find "the line containing byte offset N" unambiguously.
#[derive(Debug, Clone, PartialEq)]
pub struct WrappedLine {
    pub byte_range: Range<usize>,
    /// Index range into the flat, concatenated glyph list (every
    /// `run.glyphs` in `runs`, back to back, in order) this line
    /// actually renders -- excludes any `\n` glyph and any trailing
    /// whitespace glyphs.
    pub start_glyph: usize,
    pub end_glyph: usize,
    pub width: f32,
}

struct FlatGlyph {
    cluster: usize,
    advance: f32,
}

/// A flat, cumulative view over every glyph across `runs`, in order --
/// this module's own working representation, avoiding tracking which of
/// possibly-several `runs` a given glyph index falls in.
fn flatten_glyphs(runs: &[ShapedRun], px_size: f32, units_per_em: u16) -> Vec<FlatGlyph> {
    let scale = px_size / f32::from(units_per_em);
    runs.iter()
        .flat_map(|run| &run.glyphs)
        .map(|g| FlatGlyph {
            cluster: g.cluster as usize,
            advance: g.x_advance as f32 * scale,
        })
        .collect()
}

/// Scans `glyphs` forward from `from` while each glyph's `cluster` is
/// still less than `byte_end`, returning the resulting index and the
/// total advance consumed. `glyphs` must be in non-decreasing `cluster`
/// order (true for LTR text, this module's own real scope).
fn measure(glyphs: &[FlatGlyph], from: usize, byte_end: usize) -> (usize, f32) {
    let mut i = from;
    let mut width = 0.0_f32;
    while i < glyphs.len() && glyphs[i].cluster < byte_end {
        width += glyphs[i].advance;
        i += 1;
    }
    (i, width)
}

/// The real byte offset right after the last non-whitespace character
/// in `text[start..end]`, or `start` itself if that whole range is
/// whitespace -- used to exclude a line's own trailing whitespace from
/// its measured width/glyph range without losing those bytes from its
/// `byte_range` (see the module doc comment's "trailing-whitespace
/// convention").
fn trimmed_content_end(text: &str, start: usize, end: usize) -> usize {
    start + text[start..end].trim_end().len()
}

/// Splits `text` into real visual lines: `\n` always hard-breaks, and
/// (when `max_width` is `Some`) real UAX #14 break opportunities
/// within each such segment are greedily filled to fit -- see the
/// module doc comment for the exact algorithm and its real, disclosed
/// scope limits. `None` degrades cleanly to hard-wrap-only, identical
/// to passing `Some(f32::INFINITY)`.
///
/// Always returns at least one line, even for a fully empty `text` --
/// matching `caret_positions`'s own "always at least one real stop"
/// convention. A `text` that ends with a real `\n` gets one additional
/// real, empty trailing line (matching every real text editor's own
/// "pressing Enter at the end opens a new, real, currently-empty line
/// the caret can sit on" behavior) -- `unicode-linebreak` itself
/// reports only a single break at that shared position (the `\n`'s own
/// mandatory break coincides exactly with its "end of text" break, so
/// only one is ever emitted there), which on its own would otherwise
/// silently collapse that real, expected extra line.
#[must_use]
pub fn wrap_lines(
    text: &str,
    runs: &[ShapedRun],
    px_size: f32,
    units_per_em: u16,
    max_width: Option<f32>,
) -> Vec<WrappedLine> {
    // `unicode_linebreak::linebreaks` yields nothing at all for an
    // empty string (its "always at least one final break" guarantee
    // only holds for non-empty input, confirmed against the real
    // crate) -- handled directly here rather than relying on the main
    // loop to produce it.
    if text.is_empty() {
        return vec![WrappedLine {
            byte_range: 0..0,
            start_glyph: 0,
            end_glyph: 0,
            width: 0.0,
        }];
    }

    let max_width = max_width.unwrap_or(f32::INFINITY);
    let glyphs = flatten_glyphs(runs, px_size, units_per_em);

    let mut lines = Vec::new();
    let mut line_start_byte = 0usize;
    let mut line_start_glyph = 0usize;
    // The most recent break point accepted as a real candidate for
    // ending the CURRENT line: (byte offset to close at, glyph index
    // right after its own trimmed content, that content's own width).
    // `None` means the current line has no accepted candidate yet.
    let mut candidate: Option<(usize, usize, f32)> = None;

    for (break_byte, kind) in linebreaks(text) {
        // Repeatedly close the line at the last accepted candidate
        // while this break point's own content (measured from the
        // CURRENT line start) doesn't fit -- almost always at most one
        // real closure per break point, but a real, general fix, not
        // limited to that common case.
        loop {
            let trimmed_end = trimmed_content_end(text, line_start_byte, break_byte);
            let (end_glyph, width) = measure(&glyphs, line_start_glyph, trimmed_end);
            if candidate.is_none() || width <= max_width {
                candidate = Some((break_byte, end_glyph, width));
                break;
            }
            let (prev_break_byte, prev_end_glyph, prev_width) = candidate.take().unwrap();
            let (skip_glyph, _) = measure(&glyphs, prev_end_glyph, prev_break_byte);
            lines.push(WrappedLine {
                byte_range: line_start_byte..prev_break_byte,
                start_glyph: line_start_glyph,
                end_glyph: prev_end_glyph,
                width: prev_width,
            });
            line_start_byte = prev_break_byte;
            line_start_glyph = skip_glyph;
        }

        if kind == BreakOpportunity::Mandatory {
            let (final_break_byte, final_end_glyph, final_width) = candidate.take().unwrap();
            let (skip_glyph, _) = measure(&glyphs, final_end_glyph, final_break_byte);
            lines.push(WrappedLine {
                byte_range: line_start_byte..final_break_byte,
                start_glyph: line_start_glyph,
                end_glyph: final_end_glyph,
                width: final_width,
            });
            line_start_byte = final_break_byte;
            line_start_glyph = skip_glyph;
        }
    }

    if text.ends_with('\n') {
        lines.push(WrappedLine {
            byte_range: text.len()..text.len(),
            start_glyph: glyphs.len(),
            end_glyph: glyphs.len(),
            width: 0.0,
        });
    }

    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustybuzz::Direction;

    fn glyph(cluster: u32, x_advance: i32) -> crate::shape::ShapedGlyph {
        crate::shape::ShapedGlyph {
            glyph_id: 0,
            cluster,
            x_advance,
            y_advance: 0,
            x_offset: 0,
            y_offset: 0,
        }
    }

    fn run(text_range: Range<usize>, glyphs: Vec<crate::shape::ShapedGlyph>) -> ShapedRun {
        ShapedRun {
            text_range,
            direction: Direction::LeftToRight,
            glyphs,
        }
    }

    /// Builds synthetic glyphs for `text` where every real (non-`\n`)
    /// character is exactly `advance` font units wide, and every `\n`
    /// produces NO glyph at all -- one real, concrete answer to this
    /// module's own disclosed "does `\n` shape to a real glyph?"
    /// question, used here to prove the algorithm is correct under that
    /// assumption; the opposite assumption (a nonzero-width `\n` glyph)
    /// is covered by a dedicated test below.
    fn synthetic_glyphs(text: &str, advance: i32) -> Vec<crate::shape::ShapedGlyph> {
        text.char_indices()
            .filter(|(_, ch)| *ch != '\n')
            .map(|(i, _)| glyph(u32::try_from(i).unwrap(), advance))
            .collect()
    }

    fn assert_contiguous_and_complete(lines: &[WrappedLine], text: &str) {
        assert_eq!(
            lines[0].byte_range.start, 0,
            "the first line must start at byte 0"
        );
        for pair in lines.windows(2) {
            assert_eq!(
                pair[0].byte_range.end, pair[1].byte_range.start,
                "line byte_ranges must be contiguous with no gap or overlap: {lines:?}"
            );
        }
        assert_eq!(
            lines.last().unwrap().byte_range.end,
            text.len(),
            "the last line must reach the end of the input"
        );
    }

    #[test]
    fn no_max_width_wraps_only_on_explicit_newlines() {
        let text = "hello world\nsecond line";
        let glyphs = synthetic_glyphs(text, 10);
        let runs = vec![run(0..text.len(), glyphs)];
        let lines = wrap_lines(text, &runs, 10.0, 10, None);
        assert_eq!(lines.len(), 2, "exactly one break, at the \\n: {lines:?}");
        assert_eq!(&text[lines[0].byte_range.clone()], "hello world\n");
        assert_eq!(&text[lines[1].byte_range.clone()], "second line");
        assert_contiguous_and_complete(&lines, text);
    }

    #[test]
    fn a_single_word_wider_than_max_width_is_not_split() {
        let text = "supercalifragilisticexpialidocious";
        let glyphs = synthetic_glyphs(text, 10);
        let runs = vec![run(0..text.len(), glyphs)];
        // Max width far smaller than the word's own real width (350 units).
        let lines = wrap_lines(text, &runs, 10.0, 10, Some(50.0));
        assert_eq!(
            lines.len(),
            1,
            "an unbreakable word must stay on one line: {lines:?}"
        );
        assert_eq!(&text[lines[0].byte_range.clone()], text);
        assert_contiguous_and_complete(&lines, text);
    }

    #[test]
    fn several_short_words_wrap_at_exactly_the_right_boundary() {
        // 4 words, each 4 real chars wide (advance 10/unit -> 40 wide),
        // separated by single spaces (also 10 wide). max_width=90 fits
        // exactly two words + the space between them (40+10+40=90);
        // three would need 140.
        let text = "aaaa bbbb cccc dddd";
        let glyphs = synthetic_glyphs(text, 10);
        let runs = vec![run(0..text.len(), glyphs)];
        let lines = wrap_lines(text, &runs, 10.0, 10, Some(90.0));
        let rendered: Vec<&str> = lines.iter().map(|l| &text[l.byte_range.clone()]).collect();
        assert_eq!(
            lines.iter().map(|l| l.width).collect::<Vec<_>>(),
            vec![90.0, 90.0],
            "each full line of two words should measure exactly 90.0 wide: {rendered:?}"
        );
        assert_eq!(
            lines.len(),
            2,
            "four words at two-per-line should make exactly 2 lines: {rendered:?}"
        );
        assert_contiguous_and_complete(&lines, text);
    }

    #[test]
    fn trailing_whitespace_before_a_wrap_is_excluded_from_width_but_not_dropped() {
        // "aaaa " (5 bytes, with a trailing space) then "bbbb" (4 bytes)
        // -- with max_width smaller than both words combined, "bbbb"
        // must wrap to its own line, and the space must not count
        // toward either line's rendered width.
        let text = "aaaa bbbb";
        let glyphs = synthetic_glyphs(text, 10);
        let runs = vec![run(0..text.len(), glyphs)];
        let lines = wrap_lines(text, &runs, 10.0, 10, Some(45.0));
        assert_eq!(lines.len(), 2, "{lines:?}");
        assert_eq!(
            lines[0].width, 40.0,
            "the trailing space must not count toward line 0's width"
        );
        assert_eq!(lines[1].width, 40.0);
        assert_eq!(
            &text[lines[0].byte_range.clone()],
            "aaaa ",
            "line 0 still owns the space's bytes"
        );
        assert_eq!(&text[lines[1].byte_range.clone()], "bbbb");
        assert_contiguous_and_complete(&lines, text);
    }

    #[test]
    fn empty_text_produces_exactly_one_empty_line() {
        let lines = wrap_lines("", &[], 10.0, 10, Some(100.0));
        assert_eq!(
            lines,
            vec![WrappedLine {
                byte_range: 0..0,
                start_glyph: 0,
                end_glyph: 0,
                width: 0.0
            }]
        );
    }

    #[test]
    fn a_trailing_newline_produces_a_real_final_empty_line() {
        let text = "hello\n";
        let glyphs = synthetic_glyphs(text, 10);
        let runs = vec![run(0..text.len(), glyphs)];
        let lines = wrap_lines(text, &runs, 10.0, 10, None);
        assert_eq!(lines.len(), 2, "{lines:?}");
        assert_eq!(&text[lines[0].byte_range.clone()], "hello\n");
        assert_eq!(
            lines[1].byte_range,
            6..6,
            "a real, empty final line after the trailing \\n"
        );
        assert_eq!(lines[1].width, 0.0);
    }

    #[test]
    fn two_consecutive_newlines_produce_three_real_lines() {
        // Real editor expectation: pressing Enter twice from an empty
        // document produces 3 real lines (two empty ones the newlines
        // themselves close, plus the real trailing one the caret now
        // sits on) -- proves the "ends_with('\\n')" fix composes
        // correctly with multiple real, interior mandatory breaks, not
        // just a single trailing one.
        let text = "\n\n";
        let lines = wrap_lines(text, &[], 10.0, 10, None);
        assert_eq!(lines.len(), 3, "{lines:?}");
        assert_eq!(lines[0].byte_range, 0..1);
        assert_eq!(lines[1].byte_range, 1..2);
        assert_eq!(lines[2].byte_range, 2..2);
    }

    #[test]
    fn a_newline_glyph_with_real_nonzero_advance_is_still_excluded_from_any_line() {
        // The opposite assumption from `synthetic_glyphs`: here `\n`
        // itself produces a real glyph with a nonzero advance (some
        // fonts do map U+000A to a real, if invisible, glyph). Proves
        // wrap_lines excludes it from both the closing line's width AND
        // the next line's width regardless of which assumption holds.
        let text = "hi\nbye";
        let glyphs = vec![
            glyph(0, 10),  // h
            glyph(1, 10),  // i
            glyph(2, 999), // \n -- a real, large, wrong-if-counted advance
            glyph(3, 10),  // b
            glyph(4, 10),  // y
            glyph(5, 10),  // e
        ];
        let runs = vec![run(0..text.len(), glyphs)];
        let lines = wrap_lines(text, &runs, 10.0, 10, None);
        assert_eq!(lines.len(), 2, "{lines:?}");
        assert_eq!(
            lines[0].width, 20.0,
            "the \\n's own 999-wide advance must never count"
        );
        assert_eq!(lines[1].width, 30.0);
        assert_contiguous_and_complete(&lines, text);
    }

    #[test]
    fn leading_whitespace_at_the_very_start_is_real_content_and_is_rendered() {
        let text = " hi";
        let glyphs = synthetic_glyphs(text, 10);
        let runs = vec![run(0..text.len(), glyphs)];
        let lines = wrap_lines(text, &runs, 10.0, 10, None);
        assert_eq!(lines.len(), 1);
        assert_eq!(
            lines[0].width, 30.0,
            "leading whitespace at doc start is real, typed content -- it must be measured, \
             unlike whitespace consumed at an automatic wrap point"
        );
    }

    #[test]
    fn real_uax14_breaks_after_a_hyphen_not_just_whitespace() {
        // "well-known" has a real UAX #14 break opportunity right
        // after the hyphen -- something Step 15.1's own whitespace-
        // only v1 could never do. 10 real chars, no spaces at all.
        let text = "well-known";
        let glyphs = synthetic_glyphs(text, 10);
        let runs = vec![run(0..text.len(), glyphs)];
        // "well-" is 5 chars = 50 wide; "known" is 5 chars = 50 wide.
        // A width that fits "well-" but not "well-known" (100) proves
        // the break is real and usable, not just tolerated.
        let lines = wrap_lines(text, &runs, 10.0, 10, Some(60.0));
        assert_eq!(lines.len(), 2, "{lines:?}");
        assert_eq!(
            &text[lines[0].byte_range.clone()],
            "well-",
            "breaks right after the hyphen"
        );
        assert_eq!(&text[lines[1].byte_range.clone()], "known");
        assert_contiguous_and_complete(&lines, text);
    }

    #[test]
    fn real_uax14_forbids_a_break_between_a_word_and_its_own_trailing_punctuation() {
        // "hello!" -- UAX #14 forbids breaking between a word and an
        // immediately-following exclamation mark (real LB13-class
        // behavior), so this stays one unbreakable unit even under a
        // width that would otherwise wrap after "hello".
        let text = "hello! world";
        let glyphs = synthetic_glyphs(text, 10);
        let runs = vec![run(0..text.len(), glyphs)];
        let lines = wrap_lines(text, &runs, 10.0, 10, Some(50.0));
        assert_eq!(lines.len(), 2, "{lines:?}");
        assert_eq!(
            &text[lines[0].byte_range.clone()],
            "hello! ",
            "the '!' must stay attached to 'hello', not start its own line"
        );
        assert_eq!(&text[lines[1].byte_range.clone()], "world");
    }
}
