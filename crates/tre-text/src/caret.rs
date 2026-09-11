//! Real caret hit-testing (Phase 13 Step 13.5: editable text) -- pure
//! logic over already-[`ShapedRun`]/[`ShapedGlyph`] data, engine-side so
//! it's testable without a real font or a Python binding. Mirrors
//! `tre_engine::text::flatten_text`'s own pen-advance formula exactly
//! (`scale = px_size / units_per_em`, `pen.x += glyph.x_advance *
//! scale`), confirmed by reading that function's own source, so a
//! caret computed here lands exactly where the matching glyph actually
//! renders -- not a second, potentially-diverging position formula.

use crate::shape::ShapedRun;

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
}
