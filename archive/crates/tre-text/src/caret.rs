//! Real caret hit-testing (Phase 13 Step 13.5: editable text) -- pure
//! logic over already-[`ShapedRun`]/[`ShapedGlyph`] data, engine-side so
//! it's testable without a real font or a Python binding. Mirrors
//! `tre_engine::text::flatten_text`'s own pen-advance formula exactly
//! (`scale = px_size / units_per_em`, `pen.x += glyph.x_advance *
//! scale`), confirmed by reading that function's own source, so a
//! caret computed here lands exactly where the matching glyph actually
//! renders -- not a second, potentially-diverging position formula.

use crate::shape::ShapedRun;
use crate::wrap::WrappedLine;

/// One real caret stop: the pixel x position (relative to the text's own
/// pen start, `flatten_text`'s own local origin) a caret sits at when
/// placed at `byte_offset` into the original input string.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CaretPosition {
    pub byte_offset: usize,
    pub x: f32,
}

/// Computes every real caret stop across `runs` (concatenated in the
/// order `flatten_text` itself iterates them), plus one final stop at
/// `text_len` (the end-of-text position, past the last glyph's own
/// advance) -- `text_len` is `text.len()` of the original input string,
/// passed separately since it isn't otherwise recoverable purely from
/// `runs` (a real, disclosed RTL limitation: `runs` are in **visual**
/// order per `segment_runs`'s own doc comment, not necessarily the
/// logical byte order this function assumes when placing the final
/// stop -- correct for LTR text, which is this v1's real, bounded
/// scope; full bidi-aware caret placement is separate future work).
#[must_use]
pub fn caret_positions(
    runs: &[ShapedRun],
    text_len: usize,
    px_size: f32,
    units_per_em: u16,
) -> Vec<CaretPosition> {
    let scale = px_size / f32::from(units_per_em);
    let mut positions = Vec::new();
    let mut pen_x = 0.0_f32;
    for run in runs {
        for glyph in &run.glyphs {
            positions.push(CaretPosition {
                byte_offset: glyph.cluster as usize,
                x: pen_x,
            });
            pen_x += glyph.x_advance as f32 * scale;
        }
    }
    positions.push(CaretPosition {
        byte_offset: text_len,
        x: pen_x,
    });
    positions
}

/// Finds the real caret stop in `positions` nearest to pixel position
/// `x`, returning its `byte_offset`. Linear scan -- `positions` is one
/// entry per glyph of one line of real UI text, never large enough for
/// this to matter; a binary search would need `positions` sorted by `x`,
/// which does not hold in general for bidi/RTL text even though it does
/// for this function's own real LTR-only caller today.
///
/// # Panics
/// Panics if `positions` is empty -- a real caller always has at least
/// the one end-of-text stop `caret_positions` itself always appends,
/// even for a fully empty input string.
#[must_use]
pub fn hit_test(positions: &[CaretPosition], x: f32) -> usize {
    positions
        .iter()
        .min_by(|a, b| (a.x - x).abs().total_cmp(&(b.x - x).abs()))
        .expect("hit_test: positions must not be empty")
        .byte_offset
}

/// One real 2-D caret stop for multi-line text (Phase 15 Step 15.1):
/// like [`CaretPosition`], but also carries which real visual `line`
/// (an index into the [`WrappedLine`] slice [`multiline_caret_positions`]
/// was given) this stop sits on -- `x` is relative to that line's own
/// pen start (resets to `0.0` at the start of every line), matching
/// [`crate::wrap::wrap_lines`]'s own per-line layout.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MultiLineCaretPosition {
    pub byte_offset: usize,
    pub line: usize,
    pub x: f32,
}

/// Computes every real 2-D caret stop across `lines` (as produced by
/// [`crate::wrap::wrap_lines`]) -- one stop per glyph each line
/// actually renders, plus one real end-of-line stop per line (so a
/// caret can always be placed at the very end of any line, including an
/// empty one) at `byte_offset = line.byte_range.end`. Mirrors
/// [`caret_positions`]'s own pen-advance formula exactly.
///
/// **Real, disclosed boundary ambiguity**: a byte offset sitting
/// exactly at a line boundary gets two stops with the same
/// `byte_offset` -- the end of the earlier line and the start of the
/// next -- since a real caret there can be drawn at either visual
/// position (the same ambiguity every real text editor resolves with
/// "affinity" tracking, not attempted here in v1). [`line_of`] always
/// prefers the earlier line for such a byte offset.
#[must_use]
pub fn multiline_caret_positions(
    lines: &[WrappedLine],
    runs: &[ShapedRun],
    px_size: f32,
    units_per_em: u16,
) -> Vec<MultiLineCaretPosition> {
    let scale = px_size / f32::from(units_per_em);
    let flat_glyphs: Vec<_> = runs.iter().flat_map(|run| &run.glyphs).collect();
    let mut positions = Vec::new();
    for (line_index, line) in lines.iter().enumerate() {
        let mut pen_x = 0.0_f32;
        for glyph in &flat_glyphs[line.start_glyph..line.end_glyph] {
            positions.push(MultiLineCaretPosition {
                byte_offset: glyph.cluster as usize,
                line: line_index,
                x: pen_x,
            });
            pen_x += glyph.x_advance as f32 * scale;
        }
        positions.push(MultiLineCaretPosition {
            byte_offset: line.byte_range.end,
            line: line_index,
            x: pen_x,
        });
    }
    positions
}

/// Finds the real 2-D caret stop nearest to pixel position `(x, y)` in
/// `positions`, returning its `byte_offset`. First finds the real line
/// whose own `[n * line_height, (n+1) * line_height)` span contains `y`
/// (`(y / line_height).floor()`, clamped to a valid line index -- a
/// real, found correction: an earlier version used `.round()`, which
/// finds the line whose *top* is nearest rather than the line whose
/// *span* actually contains `y`, silently misattributing roughly the
/// bottom half of every line's own real vertical extent to the line
/// below it), then finds the nearest stop by `x` **within that line
/// only** -- unlike a global nearest-any-stop search, which would be
/// wrong once a real `y` is given (a stop on a distant line can easily
/// have a closer raw `x` than the right line's own nearest stop).
///
/// # Panics
/// Panics if `positions` is empty -- matching [`hit_test`]'s own
/// contract; a real caller always has at least one real line's worth of
/// stops, even for a fully empty document.
#[must_use]
pub fn hit_test_2d(
    positions: &[MultiLineCaretPosition],
    line_height: f32,
    x: f32,
    y: f32,
) -> usize {
    let line_count = positions
        .iter()
        .map(|p| p.line)
        .max()
        .expect("hit_test_2d: positions must not be empty")
        + 1;
    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "line_count stays far below f32's exact-integer range for any real document"
    )]
    let target_line = (y / line_height)
        .floor()
        .clamp(0.0, (line_count - 1) as f32) as usize;
    positions
        .iter()
        .filter(|p| p.line == target_line)
        .min_by(|a, b| (a.x - x).abs().total_cmp(&(b.x - x).abs()))
        .expect("target_line was derived from these same positions, so at least one must match it")
        .byte_offset
}

/// The real visual line a `byte_offset` currently renders on -- used by
/// vertical (up/down) caret movement to know which line the caret is
/// currently on. Prefers the earlier line when `byte_offset` sits
/// exactly at a line boundary (see [`multiline_caret_positions`]'s own
/// doc comment on this real, disclosed ambiguity). Falls back to the
/// latest stop at or before `byte_offset` when it doesn't land exactly
/// on a real stop (a real, legitimate case -- e.g. a caret currently
/// sitting inside a line's own consumed trailing-whitespace tail, which
/// has no stop of its own).
///
/// # Panics
/// Panics if `positions` is empty (see [`hit_test_2d`]).
#[must_use]
pub fn line_of(positions: &[MultiLineCaretPosition], byte_offset: usize) -> usize {
    positions
        .iter()
        .find(|p| p.byte_offset == byte_offset)
        .or_else(|| {
            positions
                .iter()
                .filter(|p| p.byte_offset <= byte_offset)
                .max_by_key(|p| p.byte_offset)
        })
        .expect("line_of: positions must not be empty")
        .line
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shape::ShapedGlyph;
    use rustybuzz::Direction;

    fn glyph(cluster: u32, x_advance: i32) -> ShapedGlyph {
        ShapedGlyph {
            glyph_id: 0,
            cluster,
            x_advance,
            y_advance: 0,
            x_offset: 0,
            y_offset: 0,
        }
    }

    fn run(text_range: std::ops::Range<usize>, glyphs: Vec<ShapedGlyph>) -> ShapedRun {
        ShapedRun {
            text_range,
            direction: Direction::LeftToRight,
            glyphs,
        }
    }

    #[test]
    fn caret_positions_places_one_stop_per_glyph_plus_end_of_text() {
        // Three glyphs, each 10 font units wide, px_size=10, units_per_em=10 -> scale=1.0.
        let runs = vec![run(0..3, vec![glyph(0, 10), glyph(1, 10), glyph(2, 10)])];
        let positions = caret_positions(&runs, 3, 10.0, 10);
        assert_eq!(
            positions,
            vec![
                CaretPosition {
                    byte_offset: 0,
                    x: 0.0
                },
                CaretPosition {
                    byte_offset: 1,
                    x: 10.0
                },
                CaretPosition {
                    byte_offset: 2,
                    x: 20.0
                },
                CaretPosition {
                    byte_offset: 3,
                    x: 30.0
                },
            ]
        );
    }

    #[test]
    fn caret_positions_scales_by_px_size_over_units_per_em() {
        // 1 glyph, 1000 font units wide, px_size=16, units_per_em=2000 -> scale=0.008.
        let runs = vec![run(0..1, vec![glyph(0, 1000)])];
        let positions = caret_positions(&runs, 1, 16.0, 2000);
        assert!(
            (positions[1].x - 8.0).abs() <= 1e-4,
            "expected 8.0, got {}",
            positions[1].x
        );
    }

    #[test]
    fn hit_test_finds_the_nearest_stop() {
        let positions = vec![
            CaretPosition {
                byte_offset: 0,
                x: 0.0,
            },
            CaretPosition {
                byte_offset: 1,
                x: 10.0,
            },
            CaretPosition {
                byte_offset: 2,
                x: 20.0,
            },
        ];
        assert_eq!(
            hit_test(&positions, -5.0),
            0,
            "before the start snaps to the first stop"
        );
        assert_eq!(hit_test(&positions, 4.0), 0, "closer to 0.0 than 10.0");
        assert_eq!(hit_test(&positions, 6.0), 1, "closer to 10.0 than 0.0");
        assert_eq!(hit_test(&positions, 20.0), 2, "exact match");
        assert_eq!(
            hit_test(&positions, 100.0),
            2,
            "past the end snaps to the last stop"
        );
    }

    #[test]
    fn multiline_caret_positions_and_hit_test_2d_resolve_y_to_the_right_line() {
        // "hi\nbye" -- 10-unit-wide glyphs, no glyph at all for `\n`.
        let text = "hi\nbye";
        let glyphs = vec![
            glyph(0, 10),
            glyph(1, 10),
            glyph(3, 10),
            glyph(4, 10),
            glyph(5, 10),
        ];
        let runs = vec![run(0..text.len(), glyphs)];
        let lines = crate::wrap_lines(text, &runs, 10.0, 10, None);
        assert_eq!(lines.len(), 2, "{lines:?}");

        let positions = multiline_caret_positions(&lines, &runs, 10.0, 10);
        // line 0: 'h'(0), 'i'(1), end-of-line(3, x=20.0)
        // line 1: 'b'(3), 'y'(4), 'e'(5), end-of-line(6, x=30.0)
        assert_eq!(
            positions,
            vec![
                MultiLineCaretPosition {
                    byte_offset: 0,
                    line: 0,
                    x: 0.0
                },
                MultiLineCaretPosition {
                    byte_offset: 1,
                    line: 0,
                    x: 10.0
                },
                MultiLineCaretPosition {
                    byte_offset: 3,
                    line: 0,
                    x: 20.0
                },
                MultiLineCaretPosition {
                    byte_offset: 3,
                    line: 1,
                    x: 0.0
                },
                MultiLineCaretPosition {
                    byte_offset: 4,
                    line: 1,
                    x: 10.0
                },
                MultiLineCaretPosition {
                    byte_offset: 5,
                    line: 1,
                    x: 20.0
                },
                MultiLineCaretPosition {
                    byte_offset: 6,
                    line: 1,
                    x: 30.0
                },
            ]
        );

        let line_height = 15.0;
        assert_eq!(
            hit_test_2d(&positions, line_height, 5.0, 0.0),
            0,
            "y=0 targets line 0; nearest x=0.0 to a click at x=5.0"
        );
        assert_eq!(
            hit_test_2d(&positions, line_height, 5.0, 15.0),
            3,
            "y=15 (one full line_height down) targets line 1; nearest x=0.0 there is byte 3"
        );
        assert_eq!(
            hit_test_2d(&positions, line_height, 24.0, 15.0),
            5,
            "on line 1, x=24.0 is nearer to x=20.0 (byte 5) than x=30.0 (byte 6)"
        );
        assert_eq!(
            hit_test_2d(&positions, line_height, 5.0, 500.0),
            3,
            "a y far past the last line clamps to the last real line, not out of range"
        );

        // A real, found regression case: a genuine MID-line y (not
        // sitting exactly on a line boundary) must resolve to the line
        // whose own [n*line_height, (n+1)*line_height) span contains
        // it, not the line whose *top* is merely nearest -- an earlier
        // `.round()`-based version of `hit_test_2d` got this wrong for
        // any y in roughly the bottom half of a line's own real
        // vertical extent (e.g. y=10 with line_height=15 rounds to
        // line 1, even though y=10 sits well inside line 0's own real
        // [0,15) span), which every other test above happened not to
        // catch since they all used boundary-exact y values.
        assert_eq!(
            hit_test_2d(&positions, line_height, 5.0, 10.0),
            0,
            "y=10.0 sits inside line 0's own real [0.0,15.0) span, not line 1's"
        );
        assert_eq!(
            hit_test_2d(&positions, line_height, 5.0, 20.0),
            3,
            "y=20.0 sits inside line 1's own real [15.0,30.0) span"
        );
    }

    #[test]
    fn line_of_prefers_the_earlier_line_at_a_boundary_and_falls_back_inside_a_whitespace_tail() {
        // "a  b" (two spaces) wrapped at a width that fits only "a"
        // alone on line 0 -- the two-space gap becomes a real,
        // unrendered tail on line 0's own byte_range, with no caret
        // stop of its own anywhere inside it.
        let text = "a  b";
        let glyphs = vec![glyph(0, 10), glyph(1, 10), glyph(2, 10), glyph(3, 10)];
        let runs = vec![run(0..text.len(), glyphs)];
        let lines = crate::wrap_lines(text, &runs, 10.0, 10, Some(15.0));
        assert_eq!(lines.len(), 2, "{lines:?}");
        assert_eq!(&text[lines[0].byte_range.clone()], "a  ", "{lines:?}");
        assert_eq!(&text[lines[1].byte_range.clone()], "b", "{lines:?}");

        let positions = multiline_caret_positions(&lines, &runs, 10.0, 10);

        assert_eq!(line_of(&positions, 0), 0, "an exact stop on line 0");
        assert_eq!(
            line_of(&positions, 3),
            0,
            "byte 3 is a real boundary tie (end of line 0 AND start of line 1) -- \
             the earlier line must win"
        );
        assert_eq!(
            line_of(&positions, 1),
            0,
            "byte 1 sits inside line 0's own consumed whitespace tail, with no exact \
             stop -- falls back to the latest real stop at or before it (byte 0, line 0)"
        );
    }
}
