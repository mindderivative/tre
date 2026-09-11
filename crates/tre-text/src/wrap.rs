//! Real word-wrap line-breaking (Phase 15 Step 15.1) -- pure logic over
//! already-shaped glyph data, engine-side so it's testable without a
//! real font or a Python binding, mirroring `caret.rs`'s own "testable
//! in isolation" precedent. Reuses `caret_positions`'s exact pen-advance
//! formula (`scale = px_size / units_per_em`, `x_advance * scale`) so a
//! wrapped line's own measured width and its later rendered width never
//! diverge.
//!
//! **Real, disclosed v1 scope**: LTR/single-script text only (matching
//! `caret.rs`'s own established scope -- glyph `cluster` byte offsets
//! are only monotonic within one run for LTR text); whitespace-boundary
//! word-wrap only, not full UAX #14 line-breaking (no hyphenation, no
//! punctuation/CJK-ideograph break opportunities); a single word wider
//! than `max_width` on its own still gets its own line rather than
//! being split further; `\r` is treated as ordinary whitespace, not a
//! second line-break character (a caller normalizing `\r\n` to `\n`
//! first is a real, reasonable expectation left to the caller).
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TokenKind {
    Word,
    Whitespace,
    Newline,
}

struct Token {
    byte_range: Range<usize>,
    kind: TokenKind,
}

/// Splits `text` into maximal word/whitespace runs, with every `\n` its
/// own separate one-byte token (never merged with adjacent whitespace,
/// since it carries a distinct "forced break" meaning).
fn tokenize(text: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut current_start = 0usize;
    let mut current_kind: Option<TokenKind> = None;
    for (i, ch) in text.char_indices() {
        if ch == '\n' {
            if let Some(kind) = current_kind.take() {
                tokens.push(Token {
                    byte_range: current_start..i,
                    kind,
                });
            }
            tokens.push(Token {
                byte_range: i..i + ch.len_utf8(),
                kind: TokenKind::Newline,
            });
            current_start = i + ch.len_utf8();
            continue;
        }
        let kind = if ch.is_whitespace() {
            TokenKind::Whitespace
        } else {
            TokenKind::Word
        };
        match current_kind {
            None => {
                current_kind = Some(kind);
                current_start = i;
            }
            Some(running) if running != kind => {
                tokens.push(Token {
                    byte_range: current_start..i,
                    kind: running,
                });
                current_kind = Some(kind);
                current_start = i;
            }
            Some(_) => {}
        }
    }
    if let Some(kind) = current_kind {
        tokens.push(Token {
            byte_range: current_start..text.len(),
            kind,
        });
    }
    tokens
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

/// Splits `text` into real visual lines: always breaks on `\n`, and
/// (when `max_width` is `Some`) greedily word-wraps within each such
/// segment to fit -- see the module doc comment for the exact algorithm
/// and its real, disclosed scope limits. `None` degrades cleanly to
/// hard-wrap-only, identical to passing `Some(f32::INFINITY)`.
///
/// Always returns at least one line, even for a fully empty `text` --
/// matching `caret_positions`'s own "always at least one real stop"
/// convention.
#[must_use]
pub fn wrap_lines(
    text: &str,
    runs: &[ShapedRun],
    px_size: f32,
    units_per_em: u16,
    max_width: Option<f32>,
) -> Vec<WrappedLine> {
    let max_width = max_width.unwrap_or(f32::INFINITY);
    let glyphs = flatten_glyphs(runs, px_size, units_per_em);
    let tokens = tokenize(text);

    let mut lines = Vec::new();

    let mut line_start_byte = 0usize;
    let mut line_start_glyph = 0usize;
    let mut content_end_byte = 0usize;
    let mut content_end_glyph = 0usize;
    let mut line_width = 0.0_f32;
    let mut line_has_word = false;
    let mut pending_ws: Option<Range<usize>> = None;

    for token in &tokens {
        match token.kind {
            TokenKind::Word => {
                let (after_ws_glyph, ws_width) = match &pending_ws {
                    Some(ws) => measure(&glyphs, content_end_glyph, ws.end),
                    None => (content_end_glyph, 0.0),
                };
                let (after_word_glyph, word_width) =
                    measure(&glyphs, after_ws_glyph, token.byte_range.end);
                let candidate_width = line_width + ws_width + word_width;

                if !line_has_word || candidate_width <= max_width {
                    line_width = candidate_width;
                    content_end_byte = token.byte_range.end;
                    content_end_glyph = after_word_glyph;
                    line_has_word = true;
                    pending_ws = None;
                } else {
                    let tail_end = pending_ws.as_ref().map_or(content_end_byte, |ws| ws.end);
                    lines.push(WrappedLine {
                        byte_range: line_start_byte..tail_end,
                        start_glyph: line_start_glyph,
                        end_glyph: content_end_glyph,
                        width: line_width,
                    });
                    line_start_byte = tail_end;
                    line_start_glyph = after_ws_glyph;
                    content_end_byte = token.byte_range.end;
                    content_end_glyph = after_word_glyph;
                    line_width = word_width;
                    line_has_word = true;
                    pending_ws = None;
                }
            }
            TokenKind::Whitespace => {
                pending_ws = Some(token.byte_range.clone());
            }
            TokenKind::Newline => {
                let tail_end = token.byte_range.end;
                lines.push(WrappedLine {
                    byte_range: line_start_byte..tail_end,
                    start_glyph: line_start_glyph,
                    end_glyph: content_end_glyph,
                    width: line_width,
                });
                let (skip_end_glyph, _) = measure(&glyphs, content_end_glyph, tail_end);
                line_start_byte = tail_end;
                line_start_glyph = skip_end_glyph;
                content_end_byte = tail_end;
                content_end_glyph = skip_end_glyph;
                line_width = 0.0;
                line_has_word = false;
                pending_ws = None;
            }
        }
    }

    lines.push(WrappedLine {
        byte_range: line_start_byte..text.len(),
        start_glyph: line_start_glyph,
        end_glyph: content_end_glyph,
        width: line_width,
    });

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
}
