//! Glyph outline extraction via `skrifa` (PLAN_PHASE4_STEP4_1.md task 3) --
//! the pure-Rust replacement for `FT_Outline_Decompose`. Returns raw,
//! unscaled (font design-unit) vector control points; Step 4.2's MSDF
//! generator is what turns these into a rasterized atlas entry, not this
//! module.

use skrifa::instance::Size;
use skrifa::outline::OutlinePen;
use skrifa::{FontRef, GlyphId, MetadataProvider};

use crate::TextError;

/// One segment of a glyph contour, in the same shape `usvg`/`tre-svg`
/// already model SVG path data with (`tiny_skia_path::PathSegment`) --
/// deliberately mirrored so a future step feeding these into `tre-svg`'s
/// existing [`tre_svg::flatten_cubic`]/[`tre_svg::flatten_quad`] needs no
/// translation layer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OutlineSegment {
    MoveTo([f32; 2]),
    LineTo([f32; 2]),
    QuadTo {
        control: [f32; 2],
        end: [f32; 2],
    },
    CubicTo {
        control1: [f32; 2],
        control2: [f32; 2],
        end: [f32; 2],
    },
    Close,
}

/// One closed contour of a glyph outline -- a glyph with a counter (e.g.
/// 'O', 'e') has more than one `Contour` with opposing winding; this
/// module makes no assumption about winding or hole-ness, it only records
/// exactly the segments `skrifa` reports.
pub type Contour = Vec<OutlineSegment>;

/// [`OutlinePen`] implementation that records commands into owned
/// [`Contour`]s instead of drawing anything -- `skrifa`'s outline API is
/// push-based (it calls into a "pen" as it walks the glyph program), so
/// this is the sink task 3 needs.
#[derive(Default)]
struct ContourRecorder {
    contours: Vec<Contour>,
}

impl OutlinePen for ContourRecorder {
    fn move_to(&mut self, x: f32, y: f32) {
        self.contours.push(vec![OutlineSegment::MoveTo([x, y])]);
    }

    fn line_to(&mut self, x: f32, y: f32) {
        if let Some(contour) = self.contours.last_mut() {
            contour.push(OutlineSegment::LineTo([x, y]));
        }
    }

    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        if let Some(contour) = self.contours.last_mut() {
            contour.push(OutlineSegment::QuadTo {
                control: [cx0, cy0],
                end: [x, y],
            });
        }
    }

    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        if let Some(contour) = self.contours.last_mut() {
            contour.push(OutlineSegment::CubicTo {
                control1: [cx0, cy0],
                control2: [cx1, cy1],
                end: [x, y],
            });
        }
    }

    fn close(&mut self) {
        if let Some(contour) = self.contours.last_mut() {
            contour.push(OutlineSegment::Close);
        }
    }
}

/// Extracts `glyph_id`'s outline from `font` as raw, unscaled (font
/// design-unit) contours -- `skrifa::instance::Size::unscaled()`, not a
/// concrete pixel size, since this step only extracts control points;
/// Step 4.2 decides the eventual rasterization resolution.
///
/// # Errors
///
/// [`TextError::InvalidFontForOutlines`] if `glyph_id` has no outline
/// entry in `font` at all (e.g. a bitmap-only font, or an out-of-range
/// glyph ID). [`TextError::OutlineDrawFailed`] if the glyph resolves to an
/// outline entry but `skrifa`'s own draw call fails on it (a malformed
/// `glyf`/CFF table).
pub fn glyph_outline(font: &FontRef, glyph_id: GlyphId) -> Result<Vec<Contour>, TextError> {
    let outline_glyph = font
        .outline_glyphs()
        .get(glyph_id)
        .ok_or(TextError::InvalidFontForOutlines)?;
    let mut recorder = ContourRecorder::default();
    outline_glyph
        .draw(Size::unscaled(), &mut recorder)
        .map_err(|_| TextError::OutlineDrawFailed)?;
    Ok(recorder.contours)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dejavu_sans_bytes() -> Vec<u8> {
        let fc =
            fontconfig::Fontconfig::new().expect("fontconfig must be available to run this test");
        let font = fc
            .find("DejaVu Sans", None)
            .expect("DejaVu Sans must be installed to run this test");
        std::fs::read(&font.path).expect("failed to read the resolved DejaVu Sans font file")
    }

    #[test]
    fn glyph_outline_of_a_real_glyph_from_a_real_font_is_a_single_closed_contour() {
        let bytes = dejavu_sans_bytes();
        let font = FontRef::new(&bytes).unwrap();
        let glyph_id = font.charmap().map('I').expect("DejaVu Sans must cover 'I'");

        let contours = glyph_outline(&font, glyph_id).unwrap();

        assert_eq!(
            contours.len(),
            1,
            "'I' in DejaVu Sans is a single filled rectangle, one contour, no counter/hole"
        );
        let contour = &contours[0];
        assert!(matches!(contour.first(), Some(OutlineSegment::MoveTo(_))));
        assert!(matches!(contour.last(), Some(OutlineSegment::Close)));
        assert!(
            contour.len() >= 5,
            "a rectangle needs at least MoveTo + 3 LineTo + Close: got {contour:?}"
        );
    }

    #[test]
    fn glyph_outline_rejects_an_out_of_range_glyph_id() {
        let bytes = dejavu_sans_bytes();
        let font = FontRef::new(&bytes).unwrap();
        let absurd_glyph_id = GlyphId::from(0xFFFF_u16);

        let result = glyph_outline(&font, absurd_glyph_id);

        assert!(matches!(result, Err(TextError::InvalidFontForOutlines)));
    }

    #[test]
    fn contour_recorder_ignores_commands_before_the_first_move_to() {
        // A malformed pen-command sequence (shouldn't happen from a real
        // `skrifa` draw call, but this recorder's own robustness against
        // it is worth locking in) -- line_to/quad_to/curve_to/close
        // before any move_to must not panic on an empty `contours` list.
        let mut recorder = ContourRecorder::default();
        recorder.line_to(1.0, 1.0);
        recorder.close();
        assert!(recorder.contours.is_empty());
    }

    #[test]
    fn contour_recorder_starts_a_new_contour_on_each_move_to() {
        let mut recorder = ContourRecorder::default();
        recorder.move_to(0.0, 0.0);
        recorder.line_to(1.0, 0.0);
        recorder.line_to(1.0, 1.0);
        recorder.close();
        recorder.move_to(2.0, 2.0);
        recorder.line_to(3.0, 2.0);
        recorder.close();

        assert_eq!(recorder.contours.len(), 2);
        assert_eq!(
            recorder.contours[0],
            vec![
                OutlineSegment::MoveTo([0.0, 0.0]),
                OutlineSegment::LineTo([1.0, 0.0]),
                OutlineSegment::LineTo([1.0, 1.0]),
                OutlineSegment::Close,
            ]
        );
        assert_eq!(
            recorder.contours[1],
            vec![
                OutlineSegment::MoveTo([2.0, 2.0]),
                OutlineSegment::LineTo([3.0, 2.0]),
                OutlineSegment::Close,
            ]
        );
    }
}
