//! Cubic/quadratic Bezier flattening into polylines via recursive de
//! Casteljau subdivision, tolerance-based rather than a fixed segment
//! count -- IMPLEMENTATION.md Step 3.3.1's own hand-rolled tessellation
//! primitive (`usvg` supplies the curve control points; this module turns
//! them into the straight edges [`crate::triangulate`] needs).

/// Maximum perpendicular deviation (in the same units as the input
/// points -- SVG user units, absolute/document space after
/// `crate::to_affine2` has already been applied) a flattened polyline may
/// have from the true curve before a segment is considered flat enough to
/// stop subdividing.
const FLATTEN_TOLERANCE: f32 = 0.25;

/// Hard recursion-depth cap, independent of the tolerance check above --
/// defense in depth against a pathological curve (e.g. control points at
/// extreme coordinates) for which tolerance-based termination alone could
/// recurse far more than any real icon geometry ever needs. `2^10 = 1024`
/// points is already generous for a single curve.
const MAX_SUBDIVISION_DEPTH: u32 = 10;

fn lerp(a: [f32; 2], b: [f32; 2], t: f32) -> [f32; 2] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
}

fn point_line_distance(p: [f32; 2], line_a: [f32; 2], line_b: [f32; 2]) -> f32 {
    let (dx, dy) = (line_b[0] - line_a[0], line_b[1] - line_a[1]);
    let len_sq = dx.mul_add(dx, dy * dy);
    if len_sq < f32::EPSILON {
        let (px, py) = (p[0] - line_a[0], p[1] - line_a[1]);
        return px.hypot(py);
    }
    ((p[0] - line_a[0]) * dy - (p[1] - line_a[1]) * dx).abs() / len_sq.sqrt()
}

fn cubic_is_flat(p0: [f32; 2], p1: [f32; 2], p2: [f32; 2], p3: [f32; 2]) -> bool {
    point_line_distance(p1, p0, p3) <= FLATTEN_TOLERANCE
        && point_line_distance(p2, p0, p3) <= FLATTEN_TOLERANCE
}

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
    flatten_cubic_recursive(p0, p1, p2, p3, out, 0);
}

#[allow(
    clippy::many_single_char_names,
    reason = "p0/p1/p2/p3 are the four canonical Bezier control point names -- renaming them \
               to satisfy this lint would make the de Casteljau construction below harder to \
               read, not easier"
)]
#[allow(
    clippy::similar_names,
    reason = "p01/p12/p23/p012/p123 are the standard de Casteljau midpoint labels (subscripts \
               denote which original control points each midpoint was interpolated between) -- \
               a real, well-known naming convention, not an accidental near-collision"
)]
fn flatten_cubic_recursive(
    p0: [f32; 2],
    p1: [f32; 2],
    p2: [f32; 2],
    p3: [f32; 2],
    out: &mut Vec<[f32; 2]>,
    depth: u32,
) {
    if depth >= MAX_SUBDIVISION_DEPTH || cubic_is_flat(p0, p1, p2, p3) {
        out.push(p3);
        return;
    }
    let p01 = lerp(p0, p1, 0.5);
    let p12 = lerp(p1, p2, 0.5);
    let p23 = lerp(p2, p3, 0.5);
    let p012 = lerp(p01, p12, 0.5);
    let p123 = lerp(p12, p23, 0.5);
    let p0123 = lerp(p012, p123, 0.5);
    flatten_cubic_recursive(p0, p01, p012, p0123, out, depth + 1);
    flatten_cubic_recursive(p0123, p123, p23, p3, out, depth + 1);
}

/// Appends line-segment endpoints approximating the quadratic Bezier
/// `p0 -> control -> p1` to `out`, via the standard degree-elevation to a
/// cubic (`c1 = p0 + 2/3*(control - p0)`, `c2 = p1 + 2/3*(control - p1)`)
/// rather than a second, separately-tuned flattening routine.
///
/// `pub` for the same reason as [`flatten_cubic`] -- reused by `tre-text`.
pub fn flatten_quad(p0: [f32; 2], control: [f32; 2], p1: [f32; 2], out: &mut Vec<[f32; 2]>) {
    const TWO_THIRDS: f32 = 2.0 / 3.0;
    let c1 = lerp(p0, control, TWO_THIRDS);
    let c2 = lerp(p1, control, TWO_THIRDS);
    flatten_cubic(p0, c1, c2, p1, out);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flatten_cubic_of_a_straight_line_produces_no_extra_points() {
        // Control points collinear with the endpoints -- already flat,
        // must terminate at depth 0 with just the endpoint.
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
                   (out.push(p3) in the base case), not a rounded computed value"
    )]
    fn flatten_quad_ends_at_the_requested_endpoint() {
        let mut out = Vec::new();
        flatten_quad([0.0, 0.0], [1.0, 1.0], [2.0, 0.0], &mut out);
        assert!(!out.is_empty());
        assert_eq!(*out.last().unwrap(), [2.0, 0.0]);
    }

    #[test]
    fn deeply_recursive_curve_still_terminates() {
        // Control points far enough apart that the flatness tolerance
        // alone would keep subdividing well past any real icon's needs --
        // MAX_SUBDIVISION_DEPTH must still bound the output.
        let mut out = Vec::new();
        flatten_cubic(
            [0.0, 0.0],
            [0.0, 1_000_000.0],
            [1_000_000.0, 1_000_000.0],
            [1_000_000.0, 0.0],
            &mut out,
        );
        assert!(out.len() <= (1 << MAX_SUBDIVISION_DEPTH));
    }
}
