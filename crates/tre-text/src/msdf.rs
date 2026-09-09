//! MSDF (Multi-channel Signed Distance Field) glyph generation
//! (IMPLEMENTATION.md Step 4.2.2), via `fdsm` -- a real pure-Rust
//! reimplementation of `msdfgen`'s own published algorithm, not a
//! from-scratch attempt at this specific, failure-prone technique. Takes
//! this crate's own already-extracted [`crate::Contour`] geometry (Step
//! 4.1) and produces a raw RGB8 pixel buffer; Step 4.2.3's GPU shader is
//! the consumer, not this module -- no rasterization beyond generating
//! the distance field itself happens here.

use fdsm::bezier::scanline::FillRule;
use fdsm::bezier::Segment;
use fdsm::generate::generate_msdf as fdsm_generate_msdf;
use fdsm::render::correct_sign_msdf;
use fdsm::shape::{Contour as FdsmContour, Shape};
use fdsm::transform::Transform as _;
use image::RgbImage;
use nalgebra::{Affine2, Matrix3, Point2};

use crate::outline::{Contour, OutlineSegment};

/// The standard `msdfgen` corner-detection angle-threshold parameter,
/// expressed (as `fdsm`'s own API expects) as the sine of the angle --
/// the same value `fdsm`'s own README usage example uses, not re-derived
/// by this project.
const EDGE_COLORING_ANGLE_THRESHOLD: f64 = 0.03;

/// A fixed seed for `fdsm`'s edge-coloring tie-breaking. Deterministic
/// across runs, matching this project's zero-nondeterminism testing
/// ethos -- the exact value carries no meaning beyond "a fixed `u64`."
const EDGE_COLORING_SEED: u64 = 42;

/// A generated MSDF, in the raw RGB8 pixel layout Step 4.2.3's texture
/// upload will need -- three bytes per pixel, row-major, no padding.
#[derive(Debug, Clone)]
pub struct MsdfBitmap {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

/// Generates a `size x size` MSDF for `contours`, with `range_px` pixels
/// of margin around the glyph on every side -- the distance field's own
/// encoded range, per `fdsm`/`msdfgen`'s convention.
///
/// The glyph is fit into the target box with a single uniform scale
/// (never anisotropic -- that would distort corners/curves and defeat
/// MSDF's whole "preserve sharp corners" purpose) chosen from its own
/// larger bounding-box dimension, then centered on the other axis, and
/// the Y axis is flipped (font design space is Y-up, this output's pixel
/// space is Y-down like every other raster image in this project).
///
/// Returns `None` if `contours` has no real ink to encode: either it's
/// genuinely empty (the common case for e.g. U+0020 SPACE, whose outline
/// legitimately draws zero contours -- not an error, just nothing to
/// render), or every point in it is coincident/non-finite, which would
/// otherwise make [`apply_fit_transform`]'s fit-scale computation divide
/// by a near-zero extent and bake an astronomically large, meaningless
/// scale into the shape instead of failing cleanly. Security/typography
/// review: this used to `panic!` on both cases via an `.expect()` on the
/// premise that "a real glyph always has real extent" -- false for space
/// and for malformed/corrupted font data (a realistic threat per this
/// crate's own attacker-influenceable-font-bytes threat model), neither
/// of which is a caller-programming error. Callers should render nothing
/// (or substitute DESIGN.md Section 2.6's placeholder-glyph fallback) for
/// `None` rather than treating it as fatal.
#[must_use]
pub fn generate_msdf(contours: &[Contour], size: u32, range_px: f64) -> Option<MsdfBitmap> {
    let bbox = bounding_box(contours)?;
    let shape = to_fdsm_shape(contours);
    let transformed = apply_fit_transform(shape, bbox, size, range_px);

    let colored = Shape::edge_coloring_simple(
        transformed,
        EDGE_COLORING_ANGLE_THRESHOLD,
        EDGE_COLORING_SEED,
    );
    let prepared = colored.prepare();

    let mut image = RgbImage::new(size, size);
    fdsm_generate_msdf(&prepared, range_px, &mut image);
    correct_sign_msdf(&mut image, &prepared, FillRule::Nonzero);

    Some(MsdfBitmap {
        width: size,
        height: size,
        pixels: image.into_raw(),
    })
}

/// Font-design-unit threshold below which a bounding box's extent is
/// treated as "no real ink" rather than genuine (if small) glyph detail.
/// Several orders of magnitude below any real glyph's ink -- even a
/// period/dot glyph spans tens of font units at typical `unitsPerEm`
/// scales -- so this only catches a contour whose points are all
/// coincident (a malformed/degenerate glyph), never a legitimately tiny
/// one.
const MIN_REAL_EXTENT: f32 = 1e-3;

/// Axis-aligned min/max corners, in font design units, across every
/// point (endpoints *and* control points) in `contours`. Using raw
/// control points rather than a flattened polyline is a deliberately
/// conservative (never too tight) approximation -- a Bezier curve always
/// stays within the convex hull of its own control points, so this can
/// only ever over-estimate the true bounding box slightly, never
/// under-estimate it and clip the glyph.
///
/// Returns `None` for an empty `contours` slice, a slice whose points are
/// all non-finite (e.g. from corrupted font data), or one whose points
/// are all coincident -- a zero-or-near-zero-area bounding box that would
/// otherwise force [`apply_fit_transform`] to fit content into
/// essentially no extent at all.
fn bounding_box(contours: &[Contour]) -> Option<([f32; 2], [f32; 2])> {
    let mut min = [f32::INFINITY, f32::INFINITY];
    let mut max = [f32::NEG_INFINITY, f32::NEG_INFINITY];
    let mut update = |[x, y]: [f32; 2]| {
        min[0] = min[0].min(x);
        min[1] = min[1].min(y);
        max[0] = max[0].max(x);
        max[1] = max[1].max(y);
    };
    for contour in contours {
        for segment in contour {
            match *segment {
                OutlineSegment::MoveTo(p) | OutlineSegment::LineTo(p) => update(p),
                OutlineSegment::QuadTo { control, end } => {
                    update(control);
                    update(end);
                }
                OutlineSegment::CubicTo {
                    control1,
                    control2,
                    end,
                } => {
                    update(control1);
                    update(control2);
                    update(end);
                }
                OutlineSegment::Close => {}
            }
        }
    }
    if !(min[0].is_finite() && min[1].is_finite() && max[0].is_finite() && max[1].is_finite()) {
        return None;
    }
    let width = max[0] - min[0];
    let height = max[1] - min[1];
    if width < MIN_REAL_EXTENT && height < MIN_REAL_EXTENT {
        return None;
    }
    Some((min, max))
}

/// Whether `contours` has enough real, non-degenerate ink for
/// [`generate_msdf`] to succeed -- i.e. `bounding_box(contours)` would
/// return `Some`.
///
/// REVIEW.md finding #139: `Canvas::draw_text` (`tre-engine`) used to
/// gate `GlyphRasterSource` construction on `!contours.is_empty()`
/// alone -- a narrower, *wrong* condition. A non-empty `Vec<Contour>`
/// (e.g. a single `[MoveTo, Close]` pair with no drawing segments in
/// between, a real shape a corrupted/truncated `glyf` table -- or even
/// an ordinary font's single-point contour, per skrifa's own real
/// `to_path` behavior -- can produce) still fails `bounding_box`'s own
/// degeneracy check, so `generate_msdf` legitimately returns `None` for
/// it. `GlyphRasterSource::rasterize`'s `.expect(...)` on that `None`
/// then panics on the atlas owner's shared background thread. This
/// function exposes `bounding_box`'s exact real check so callers outside
/// this crate can gate construction on the same, correct condition
/// `generate_msdf` itself actually requires, not a narrower one.
#[must_use]
pub fn has_real_ink(contours: &[Contour]) -> bool {
    bounding_box(contours).is_some()
}

fn apply_fit_transform(
    mut shape: Shape<FdsmContour>,
    (min, max): ([f32; 2], [f32; 2]),
    size: u32,
    range_px: f64,
) -> Shape<FdsmContour> {
    let content = f64::from(size) - 2.0 * range_px;
    let bbox_width = f64::from(max[0] - min[0]);
    let bbox_height = f64::from(max[1] - min[1]);
    let largest = bbox_width.max(bbox_height).max(f64::EPSILON);
    let scale = content / largest;

    let extra_x = (content - bbox_width * scale).max(0.0);
    let extra_y = (content - bbox_height * scale).max(0.0);
    let translate_x = range_px + extra_x / 2.0 - f64::from(min[0]) * scale;
    // Y is flipped (font design space is Y-up; output pixel space is
    // Y-down): `output_y = size - (scale * input_y + translate_y_unflipped)`,
    // which rearranges to the `-scale`/adjusted-translation form below.
    let translate_y = f64::from(size) - (range_px + extra_y / 2.0) - f64::from(min[1]) * scale;

    #[rustfmt::skip]
    let matrix = Matrix3::new(
        scale, 0.0,    translate_x,
        0.0,   -scale, translate_y,
        0.0,   0.0,    1.0,
    );
    let transformation = Affine2::from_matrix_unchecked(matrix);
    shape.transform(&transformation);
    shape
}

/// Converts this crate's own already-extracted glyph outline contours
/// into the `fdsm::shape::Shape` its generator expects.
fn to_fdsm_shape(contours: &[Contour]) -> Shape<FdsmContour> {
    Shape {
        contours: contours.iter().map(|c| to_fdsm_contour(c)).collect(),
    }
}

/// `fdsm`'s own `Contour`, unlike this crate's [`Contour`], has no
/// `Close` marker at all -- each [`Segment`] carries its own explicit
/// start and end points rather than relying on an implicit running
/// "current point." A contour whose flattened points don't already end
/// exactly back at their own start needs one final explicit closing
/// segment here, or the shape silently has a gap rather than erroring.
///
/// The closing check runs unconditionally after the loop (not only
/// inside the `Close` arm) so this stays correct even for a contour that
/// never contains an explicit `Close` segment at all -- legal in several
/// outline formats, which can end a subpath implicitly via the next
/// `MoveTo` instead. Today every contour this crate's own `outline`
/// module produces does end in `Close` (skrifa's pen sequencing
/// guarantees it), but that invariant lives entirely in skrifa's
/// internals, not in anything asserted here -- this is a free,
/// zero-cost-on-the-already-correct-path strengthening against a future
/// producer that doesn't guarantee it.
fn to_fdsm_contour(contour: &[OutlineSegment]) -> FdsmContour {
    let mut segments = Vec::new();
    let mut current = Point2::new(0.0, 0.0);
    let mut start = current;

    for segment in contour {
        match *segment {
            OutlineSegment::MoveTo([x, y]) => {
                current = to_point(x, y);
                start = current;
            }
            OutlineSegment::LineTo([x, y]) => {
                let end = to_point(x, y);
                segments.push(Segment::line(current, end));
                current = end;
            }
            OutlineSegment::QuadTo { control, end } => {
                let control = to_point(control[0], control[1]);
                let end = to_point(end[0], end[1]);
                segments.push(Segment::quad(current, control, end));
                current = end;
            }
            OutlineSegment::CubicTo {
                control1,
                control2,
                end,
            } => {
                let control1 = to_point(control1[0], control1[1]);
                let control2 = to_point(control2[0], control2[1]);
                let end = to_point(end[0], end[1]);
                segments.push(Segment::cubic(current, control1, control2, end));
                current = end;
            }
            OutlineSegment::Close => {
                if current != start {
                    segments.push(Segment::line(current, start));
                }
                current = start;
            }
        }
    }
    if current != start && !segments.is_empty() {
        segments.push(Segment::line(current, start));
    }
    FdsmContour { segments }
}

fn to_point(x: f32, y: f32) -> Point2<f64> {
    Point2::new(f64::from(x), f64::from(y))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A hand-built unit-square contour, no font/glyph involved --
    /// `MoveTo(0,0) -> LineTo(10,0) -> LineTo(10,10) -> LineTo(0,10) ->
    /// Close`, a real (if trivial) closed shape.
    fn unit_square_contour() -> Contour {
        vec![
            OutlineSegment::MoveTo([0.0, 0.0]),
            OutlineSegment::LineTo([10.0, 0.0]),
            OutlineSegment::LineTo([10.0, 10.0]),
            OutlineSegment::LineTo([0.0, 10.0]),
            OutlineSegment::Close,
        ]
    }

    /// Standard even-odd/nonzero-agnostic median-of-3 evaluation of a
    /// stored MSDF pixel -- computed independently here (not reused from
    /// `fdsm`'s own private `median`/`median3` helpers, which aren't
    /// `pub` at all) so this test is a real, separate check against the
    /// canonical formula (TECHNICAL.md Section 5.3), not a tautology.
    fn median_at(bitmap: &MsdfBitmap, x: u32, y: u32) -> u8 {
        let idx = ((y * bitmap.width + x) * 3) as usize;
        let mut channels = [
            bitmap.pixels[idx],
            bitmap.pixels[idx + 1],
            bitmap.pixels[idx + 2],
        ];
        channels.sort_unstable();
        channels[1]
    }

    #[test]
    fn to_fdsm_contour_closes_a_contour_whose_points_dont_already_meet() {
        let converted = to_fdsm_contour(&unit_square_contour());
        // 3 explicit LineTo segments + 1 synthesized closing segment.
        assert_eq!(converted.segments.len(), 4);
        let last = converted.segments.last().unwrap();
        // The synthesized closing segment must run from the last point
        // (0,10) back to the very first point (0,0).
        assert_eq!(last.start(), Point2::new(0.0, 10.0));
        assert_eq!(last.end(), Point2::new(0.0, 0.0));
    }

    #[test]
    fn to_fdsm_contour_does_not_add_a_redundant_closing_segment_when_already_closed() {
        let mut already_closed = unit_square_contour();
        already_closed.insert(4, OutlineSegment::LineTo([0.0, 0.0]));
        let converted = to_fdsm_contour(&already_closed);
        // 4 explicit segments (the square's own 3 sides plus the
        // already-present closing LineTo) and no synthesized 5th one.
        assert_eq!(converted.segments.len(), 4);
    }

    #[test]
    fn generate_msdf_of_a_solid_square_is_positive_inside_and_negative_outside() {
        let bitmap = generate_msdf(&[unit_square_contour()], 32, 4.0)
            .expect("a real, non-degenerate square contour must produce a bitmap");
        assert_eq!(bitmap.pixels.len(), 32 * 32 * 3);

        // Deep interior of a solid square, comfortably inside: expect a
        // median clearly above the 0.5-equivalent midpoint (127/128).
        let interior = median_at(&bitmap, 16, 16);
        assert!(
            interior > 160,
            "deep interior median {interior} should be comfortably above the inside/outside \
             threshold (127)"
        );

        // A corner of the 32x32 canvas is well outside the fitted,
        // centered square (which has real margin on every side): expect
        // a median clearly below the threshold.
        let exterior = median_at(&bitmap, 0, 0);
        assert!(
            exterior < 96,
            "exterior median {exterior} should be comfortably below the inside/outside \
             threshold (127)"
        );
    }

    #[test]
    fn generate_msdf_of_an_empty_glyph_returns_none_instead_of_panicking() {
        // The realistic case this covers: U+0020 SPACE and similar
        // no-ink glyphs legitimately produce zero contours from
        // `glyph_outline`, not an error.
        assert!(generate_msdf(&[], 32, 4.0).is_none());
    }

    #[test]
    fn generate_msdf_of_a_contour_collapsed_to_a_single_point_returns_none() {
        let degenerate = vec![
            OutlineSegment::MoveTo([5.0, 5.0]),
            OutlineSegment::LineTo([5.0, 5.0]),
            OutlineSegment::Close,
        ];
        assert!(generate_msdf(&[degenerate], 32, 4.0).is_none());
    }

    #[test]
    fn has_real_ink_agrees_with_generate_msdf_on_a_real_non_degenerate_contour() {
        let square = unit_square_contour();
        assert!(has_real_ink(std::slice::from_ref(&square)));
        assert!(generate_msdf(&[square], 32, 4.0).is_some());
    }

    #[test]
    fn has_real_ink_is_false_for_an_empty_contour_list() {
        // REVIEW.md finding #139: this is the case `Canvas::draw_text`'s
        // old `!outline.is_empty()` guard *did* correctly catch --
        // confirming this function doesn't regress the case that already
        // worked.
        assert!(!has_real_ink(&[]));
    }

    #[test]
    fn has_real_ink_is_false_for_a_non_empty_but_single_point_contour() {
        // REVIEW.md finding #139: the exact case `!outline.is_empty()`
        // missed -- a real, non-empty `Vec<Contour>` (a lone `MoveTo` +
        // `Close`, with no drawing segment between them) that skrifa's
        // own `to_path` can produce for a real single-point TrueType
        // contour, and that malformed/truncated font data can produce
        // too. Must agree with `generate_msdf` returning `None` for the
        // identical shape (see
        // `generate_msdf_of_a_contour_collapsed_to_a_single_point_returns_none`
        // above).
        let degenerate = vec![OutlineSegment::MoveTo([5.0, 5.0]), OutlineSegment::Close];
        assert!(!has_real_ink(&[degenerate]));
    }
}
