//! Cubic/quadratic Bezier flattening into polylines -- Phase 10 Step
//! 10.2 follow-up: now backed by `lyon_geom`'s own tolerance-based curve
//! flattening rather than this crate's original hand-rolled recursive de
//! Casteljau subdivision (see REVIEW.md for the real reason: `lyon` is
//! the industry-standard Rust 2D tessellation library, and this
//! project's own hand-rolled ear-clipping triangulator -- the reason
//! this module existed in the first place -- is retired in favor of it
//! too, in `triangulate.rs`). Kept as small, standalone functions here
//! (not inlined at each call site) since `PLAN_PHASE4_STEP4_1.md`
//! anticipated `tre-text` reusing exactly this signature for glyph
//! outline geometry -- the public contract (excludes the start point,
//! includes the end point) is unchanged, confirmed against
//! `lyon_geom::Flattened`'s own documented behavior ("starting *after*
//! the current point," ending at the segment's `to`), so no caller of
//! the old hand-rolled version needs to change.

use lyon::geom::{CubicBezierSegment, QuadraticBezierSegment};
use lyon::math::point;

/// Maximum perpendicular deviation (in the same units as the input
/// points -- SVG user units, absolute/document space after
/// `crate::to_affine2` has already been applied) a flattened polyline may
/// have from the true curve before a segment is considered flat enough to
/// stop subdividing. Unchanged from this module's original hand-rolled
/// tolerance.
const FLATTEN_TOLERANCE: f32 = 0.25;

/// Appends line-segment endpoints approximating the cubic Bezier
/// `p0 -> p1 -> p2 -> p3` to `out`, NOT including `p0` itself -- the
/// caller is assumed to already have `p0` as the current point, matching
/// how a `LineTo` segment is pushed, so the two cases compose without a
/// duplicate point.
///
/// `pub` (not `pub(crate)`) since `PLAN_PHASE4_STEP4_1.md`: `tre-text`
/// reuses this rather than hand-rolling a second curve flattener for
/// glyph outline geometry (a font glyph's outline is cubic/quadratic
/// Beziers, the same curve types an SVG path uses).
pub fn flatten_cubic(
    p0: [f32; 2],
    p1: [f32; 2],
    p2: [f32; 2],
    p3: [f32; 2],
    out: &mut Vec<[f32; 2]>,
) {
    let segment = CubicBezierSegment {
        from: point(p0[0], p0[1]),
        ctrl1: point(p1[0], p1[1]),
        ctrl2: point(p2[0], p2[1]),
        to: point(p3[0], p3[1]),
    };
    out.extend(segment.flattened(FLATTEN_TOLERANCE).map(|p| [p.x, p.y]));
}

/// Appends line-segment endpoints approximating the quadratic Bezier
/// `p0 -> control -> p1` to `out`, via `lyon_geom`'s own quadratic
/// flattening (previously: degree-elevation to a cubic, done by hand;
/// `lyon_geom::QuadraticBezierSegment` flattens a true quadratic
/// directly, no elevation needed).
///
/// `pub` for the same reason as [`flatten_cubic`] -- reused by `tre-text`.
pub fn flatten_quad(p0: [f32; 2], control: [f32; 2], p1: [f32; 2], out: &mut Vec<[f32; 2]>) {
    let segment = QuadraticBezierSegment {
        from: point(p0[0], p0[1]),
        ctrl: point(control[0], control[1]),
        to: point(p1[0], p1[1]),
    };
    out.extend(segment.flattened(FLATTEN_TOLERANCE).map(|p| [p.x, p.y]));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flatten_cubic_of_a_straight_line_produces_no_extra_points() {
        // Control points collinear with the endpoints -- already flat,
        // must terminate with just the endpoint.
        let mut out = Vec::new();
        flatten_cubic([0.0, 0.0], [1.0, 1.0], [2.0, 2.0], [3.0, 3.0], &mut out);
        assert_eq!(out, vec![[3.0, 3.0]]);
    }

    #[test]
    fn flatten_cubic_approximates_a_quarter_circle_within_tolerance() {
        // Standard cubic Bezier approximation of a unit-radius quarter
        // circle (center at origin, from (1,0) to (0,1)), kappa ~= 0.5523.
        const K: f32 = 0.552_284_8;
        let (p0, p1, p2, p3) = ([1.0, 0.0], [1.0, K], [K, 1.0], [0.0, 1.0]);
        let mut out = Vec::new();
        flatten_cubic(p0, p1, p2, p3, &mut out);

        assert!(
            out.len() > 1,
            "a curved arc should need more than one segment"
        );
        for &[x, y] in &out {
            let radius = x.hypot(y);
            assert!(
                (radius - 1.0).abs() < 0.02,
                "point ({x}, {y}) deviates from the unit circle by more than the flatten tolerance allows"
            );
        }
    }

    #[test]
    #[allow(
        clippy::float_cmp,
        reason = "the flattened curve's final point is exactly the literal endpoint passed in \
                   (lyon_geom's own documented Flattened contract), not a rounded computed value"
    )]
    fn flatten_quad_ends_at_the_requested_endpoint() {
        let mut out = Vec::new();
        flatten_quad([0.0, 0.0], [1.0, 1.0], [2.0, 0.0], &mut out);
        assert!(!out.is_empty());
        assert_eq!(*out.last().unwrap(), [2.0, 0.0]);
    }

    #[test]
    fn a_curve_with_extreme_control_points_still_produces_a_bounded_number_of_points() {
        // Control points far enough apart that a naive fixed-tolerance
        // flattener could in principle subdivide a great many times --
        // lyon_geom's own flattening is iterative (not recursive), so
        // there is no stack-depth concern, but the point count must
        // still stay bounded for a single curve.
        let mut out = Vec::new();
        flatten_cubic(
            [0.0, 0.0],
            [0.0, 1_000_000.0],
            [1_000_000.0, 1_000_000.0],
            [1_000_000.0, 0.0],
            &mut out,
        );
        assert!(out.len() < 10_000, "got {} points for one curve", out.len());
    }
}
