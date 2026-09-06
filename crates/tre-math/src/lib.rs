//! Vector/matrix math, affine transform batching, and SIMD-accelerated path
//! interpolation, built entirely on the `wide` crate's safe portable-SIMD
//! API (TECHNICAL.md Sections 2.2, 5.4, 7.2).
//!
//! Because `wide`'s public surface is safe Rust, this crate needs no
//! `unsafe` of its own and is not on TECHNICAL.md Section 9.1's
//! `unsafe`-permitted list. Per DESIGN.md Section 12.4, this is a stateless
//! evaluation library -- the UI framework's widget tree owns animation
//! state, not this crate.
#![forbid(unsafe_code)]

use wide::f32x8;

/// How many parent-child pairs `compose_batch` processes per SIMD chunk --
/// `wide::f32x8`'s lane width.
const SIMD_WIDTH: usize = 8;

/// A 2D affine transform (TECHNICAL.md Section 7.2), stored as the six
/// meaningful values of the canonical `[[a, b, tx], [c, d, ty], [0, 0, 1]]`
/// matrix. The bottom row is always `[0, 0, 1]` for any genuine affine
/// transform, so storing it (a full dense 3x3, 9 floats) would waste
/// memory and SIMD lanes for no benefit -- every `Affine2` this API can
/// construct is guaranteed affine, never a general projective transform.
#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(C)]
pub struct Affine2 {
    pub a: f32,
    pub b: f32,
    pub tx: f32,
    pub c: f32,
    pub d: f32,
    pub ty: f32,
}

impl Affine2 {
    /// The identity transform: composing with it, or transforming a point
    /// through it, is a no-op.
    pub const IDENTITY: Self = Self {
        a: 1.0,
        b: 0.0,
        tx: 0.0,
        c: 0.0,
        d: 1.0,
        ty: 0.0,
    };

    /// A pure translation by `(tx, ty)`.
    #[must_use]
    pub const fn from_translation(tx: f32, ty: f32) -> Self {
        Self {
            a: 1.0,
            b: 0.0,
            tx,
            c: 0.0,
            d: 1.0,
            ty,
        }
    }

    /// A pure rotation by `theta` radians, counterclockwise (matching
    /// TECHNICAL.md Section 7.2's `cosθ`/`sinθ` convention).
    #[must_use]
    pub fn from_rotation(theta: f32) -> Self {
        let (sin, cos) = theta.sin_cos();
        Self {
            a: cos,
            b: -sin,
            tx: 0.0,
            c: sin,
            d: cos,
            ty: 0.0,
        }
    }

    /// A pure scale by `(sx, sy)`. A negative component is a legitimate
    /// flip, not a validation error -- affine transforms don't require
    /// positive scale.
    #[must_use]
    pub const fn from_scale(sx: f32, sy: f32) -> Self {
        Self {
            a: sx,
            b: 0.0,
            tx: 0.0,
            c: 0.0,
            d: sy,
            ty: 0.0,
        }
    }

    /// TECHNICAL.md Section 7.2's exact combined formula -- the common
    /// case for a single UI node's local transform, built from its
    /// translation, rotation, and scale in one call rather than composing
    /// three separate `Affine2`s.
    #[must_use]
    pub fn from_translation_rotation_scale(
        translation: [f32; 2],
        rotation: f32,
        scale: [f32; 2],
    ) -> Self {
        let (sin, cos) = rotation.sin_cos();
        let [sx, sy] = scale;
        let [tx, ty] = translation;
        Self {
            a: sx * cos,
            b: -sy * sin,
            tx,
            c: sx * sin,
            d: sy * cos,
            ty,
        }
    }

    /// Composes `self` (the parent) with `child`'s local transform:
    /// `self.compose(&child).transform_point(p)` equals
    /// `self.transform_point(child.transform_point(p))` -- `child`'s
    /// transform applies to a point first, then `self`'s. Not commutative:
    /// `a.compose(&b)` and `b.compose(&a)` generally differ.
    #[must_use]
    pub fn compose(&self, child: &Self) -> Self {
        Self {
            a: self.a * child.a + self.b * child.c,
            b: self.a * child.b + self.b * child.d,
            tx: self.a * child.tx + self.b * child.ty + self.tx,
            c: self.c * child.a + self.d * child.c,
            d: self.c * child.b + self.d * child.d,
            ty: self.c * child.tx + self.d * child.ty + self.ty,
        }
    }

    /// Applies this transform to a point.
    #[must_use]
    pub fn transform_point(&self, point: [f32; 2]) -> [f32; 2] {
        let [x, y] = point;
        [
            self.a * x + self.b * y + self.tx,
            self.c * x + self.d * y + self.ty,
        ]
    }
}

/// Gathers one field from 8 (or fewer, zero-padded) items into a
/// `wide::f32x8` lane vector -- the structure-of-arrays layout SIMD
/// batching needs, built from the array-of-structures slices callers
/// naturally have. Generic over the item type so both `compose_batch`
/// (gathering `Affine2` fields) and `lerp_points_batch` (gathering
/// `[f32; 2]` components) share one gather implementation rather than
/// duplicating the same 8-lane loop.
fn gather<T>(items: &[T], field: impl Fn(&T) -> f32) -> f32x8 {
    let mut lanes = [0.0f32; SIMD_WIDTH];
    for (lane, item) in lanes.iter_mut().zip(items) {
        *lane = field(item);
    }
    f32x8::new(lanes)
}

/// SIMD-batched version of [`Affine2::compose`] (TECHNICAL.md Section 7.2:
/// "matrix multiplications for parent-child world transforms must be
/// batched and executed via the `wide` crate's portable SIMD types").
/// Processes `parents`/`children` 8 pairs at a time via `wide::f32x8`, with
/// a plain scalar [`Affine2::compose`] call for the final
/// `parents.len() % 8` remainder.
///
/// Writes into `out` rather than returning a freshly allocated `Vec`,
/// since a per-frame scene-graph-flattening caller (TECHNICAL.md Section
/// 7.2's eventual consumer) cannot allocate on that path (DESIGN.md
/// Section 2.1's zero-allocation steady state).
///
/// # Panics
/// Panics if `parents`, `children`, and `out` don't all have the same
/// length -- a length mismatch is a programmer error, not a recoverable
/// runtime condition, so this deliberately isn't a `Result`.
// `out_tx`/`out_ty` (translate-x/translate-y) trip `clippy::similar_names`
// on their trailing letter alone -- they're exactly `Affine2::tx`/`ty`,
// this codebase's own field names, not an accidental near-collision.
#[allow(clippy::similar_names)]
pub fn compose_batch(parents: &[Affine2], children: &[Affine2], out: &mut [Affine2]) {
    assert_eq!(
        parents.len(),
        children.len(),
        "compose_batch: parents/children length mismatch"
    );
    assert_eq!(
        parents.len(),
        out.len(),
        "compose_batch: parents/out length mismatch"
    );

    let full_chunks = parents.len() / SIMD_WIDTH;

    for chunk in 0..full_chunks {
        let base = chunk * SIMD_WIDTH;
        let p = &parents[base..base + SIMD_WIDTH];
        let c = &children[base..base + SIMD_WIDTH];

        let pa = gather(p, |t| t.a);
        let pb = gather(p, |t| t.b);
        let ptx = gather(p, |t| t.tx);
        let pc = gather(p, |t| t.c);
        let pd = gather(p, |t| t.d);
        let pty = gather(p, |t| t.ty);

        let ca = gather(c, |t| t.a);
        let cb = gather(c, |t| t.b);
        let ctx = gather(c, |t| t.tx);
        let cc = gather(c, |t| t.c);
        let cd = gather(c, |t| t.d);
        let cty = gather(c, |t| t.ty);

        // The same closed-form composition as `Affine2::compose`, six
        // components computed 8-wide at once via hardware FMA where the
        // target supports it (`mul_add`, TECHNICAL.md Section 2.2).
        let out_a = pa.mul_add(ca, pb * cc).to_array();
        let out_b = pa.mul_add(cb, pb * cd).to_array();
        let out_tx = pa.mul_add(ctx, pb.mul_add(cty, ptx)).to_array();
        let out_c = pc.mul_add(ca, pd * cc).to_array();
        let out_d = pc.mul_add(cb, pd * cd).to_array();
        let out_ty = pc.mul_add(ctx, pd.mul_add(cty, pty)).to_array();

        for lane in 0..SIMD_WIDTH {
            out[base + lane] = Affine2 {
                a: out_a[lane],
                b: out_b[lane],
                tx: out_tx[lane],
                c: out_c[lane],
                d: out_d[lane],
                ty: out_ty[lane],
            };
        }
    }

    for i in (full_chunks * SIMD_WIDTH)..parents.len() {
        out[i] = parents[i].compose(&children[i]);
    }
}

/// SIMD-batched per-point linear interpolation (TECHNICAL.md Section 5.4:
/// "keyframed SMIL/CSS path morphing evaluates topological interpolation
/// using the `wide` crate's `f32x8` vector type"). Processes
/// `from`/`to` 8 points at a time via `wide::f32x8::mul_add`
/// (`lerp(a, b, t) = (b - a).mul_add(t, a)`), with a plain scalar lerp
/// for the final `from.len() % 8` remainder -- the same structure as
/// [`compose_batch`], generalized to `[f32; 2]` points instead of
/// `Affine2` transforms.
///
/// Writes into `out` rather than returning a freshly allocated `Vec`,
/// matching `compose_batch`'s zero-allocation rationale: an eventual
/// per-frame animation-morphing caller cannot allocate on that path
/// (DESIGN.md Section 2.1's zero-allocation steady state).
///
/// # Panics
/// Panics if `from`, `to`, and `out` don't all have the same length -- a
/// length mismatch is a programmer error (the caller controls all three
/// slice lengths directly), not a recoverable runtime condition, so this
/// deliberately isn't a `Result`. A caller morphing between two
/// independently-parsed, genuinely untrusted keyframe shapes (e.g.
/// `tre-svg::morph`) is expected to validate topological equivalence
/// itself and report a mismatch via `Result` *before* ever reaching this
/// function -- this function's own contract is "already-equal-length
/// inputs," the same contract `compose_batch` places on its callers.
pub fn lerp_points_batch(from: &[[f32; 2]], to: &[[f32; 2]], t: f32, out: &mut [[f32; 2]]) {
    assert_eq!(
        from.len(),
        to.len(),
        "lerp_points_batch: from/to length mismatch"
    );
    assert_eq!(
        from.len(),
        out.len(),
        "lerp_points_batch: from/out length mismatch"
    );

    let t_vec = f32x8::splat(t);
    let full_chunks = from.len() / SIMD_WIDTH;

    for chunk in 0..full_chunks {
        let base = chunk * SIMD_WIDTH;
        let f = &from[base..base + SIMD_WIDTH];
        let d = &to[base..base + SIMD_WIDTH];

        let fx = gather(f, |p| p[0]);
        let fy = gather(f, |p| p[1]);
        let dx = gather(d, |p| p[0]);
        let dy = gather(d, |p| p[1]);

        let out_x = (dx - fx).mul_add(t_vec, fx).to_array();
        let out_y = (dy - fy).mul_add(t_vec, fy).to_array();

        for lane in 0..SIMD_WIDTH {
            out[base + lane] = [out_x[lane], out_y[lane]];
        }
    }

    for i in (full_chunks * SIMD_WIDTH)..from.len() {
        out[i] = [
            (to[i][0] - from[i][0]).mul_add(t, from[i][0]),
            (to[i][1] - from[i][1]).mul_add(t, from[i][1]),
        ];
    }
}

#[cfg(test)]
mod tests {
    use super::{compose_batch, lerp_points_batch, Affine2};
    use std::f32::consts::PI;

    /// `wide::f32x8::mul_add` is true hardware FMA (one rounding) wherever
    /// the target supports it, a separate multiply-then-add (two
    /// roundings) otherwise (TECHNICAL.md Section 2.2) -- so the SIMD
    /// batch path can legitimately differ from the scalar reference in
    /// the last bit or two of an `f32`. Comparing with this tolerance,
    /// not exact equality, is what actually verifies the math is right
    /// rather than asserting a result that only happens to hold on
    /// today's specific CPU.
    const EPSILON: f32 = 1e-5;

    fn assert_affine_approx_eq(actual: Affine2, expected: Affine2) {
        for (name, a, e) in [
            ("a", actual.a, expected.a),
            ("b", actual.b, expected.b),
            ("tx", actual.tx, expected.tx),
            ("c", actual.c, expected.c),
            ("d", actual.d, expected.d),
            ("ty", actual.ty, expected.ty),
        ] {
            assert!(
                (a - e).abs() <= EPSILON,
                "field `{name}`: expected {e}, got {a} (diff {})",
                (a - e).abs()
            );
        }
    }

    // These four tests compare with exact equality, not the epsilon helper
    // above, quite deliberately: translation and scale by small integer
    // values involve no rounding at all (unlike the rotation/composition
    // tests, which go through `sin_cos` and are only approximately exact).
    // `clippy::float_cmp` can't see that distinction statically.
    #[allow(clippy::float_cmp)]
    #[test]
    fn identity_transforms_a_point_unchanged() {
        assert_eq!(Affine2::IDENTITY.transform_point([3.0, -7.0]), [3.0, -7.0]);
    }

    #[allow(clippy::float_cmp)]
    #[test]
    fn translation_moves_a_point_by_the_given_offset() {
        let t = Affine2::from_translation(10.0, -5.0);
        assert_eq!(t.transform_point([1.0, 1.0]), [11.0, -4.0]);
    }

    #[test]
    fn rotation_by_quarter_turn_matches_hand_computed_result() {
        // A 90-degree counterclockwise rotation sends (1, 0) to (0, 1),
        // per TECHNICAL.md Section 7.2's cosθ/sinθ convention.
        let r = Affine2::from_rotation(PI / 2.0);
        let [x, y] = r.transform_point([1.0, 0.0]);
        assert!((x - 0.0).abs() <= EPSILON, "x = {x}");
        assert!((y - 1.0).abs() <= EPSILON, "y = {y}");
    }

    #[test]
    fn rotation_by_half_turn_negates_a_point() {
        let r = Affine2::from_rotation(PI);
        let [x, y] = r.transform_point([2.0, 3.0]);
        assert!((x - (-2.0)).abs() <= EPSILON, "x = {x}");
        assert!((y - (-3.0)).abs() <= EPSILON, "y = {y}");
    }

    #[allow(clippy::float_cmp)]
    #[test]
    fn scale_multiplies_each_axis_independently() {
        let s = Affine2::from_scale(2.0, -3.0);
        assert_eq!(s.transform_point([4.0, 5.0]), [8.0, -15.0]);
    }

    #[allow(clippy::float_cmp)]
    #[test]
    fn negative_scale_is_a_legitimate_flip_not_rejected() {
        let flip = Affine2::from_scale(-1.0, 1.0);
        assert_eq!(flip.transform_point([5.0, 5.0]), [-5.0, 5.0]);
    }

    #[test]
    fn combined_translate_rotate_scale_matches_hand_computed_result() {
        // scale (2, 2), rotate 90 degrees, translate (10, 0) -- applied in
        // that order, matching TECHNICAL.md 7.2's combined formula.
        let m = Affine2::from_translation_rotation_scale([10.0, 0.0], PI / 2.0, [2.0, 2.0]);
        // (1, 0) scaled -> (2, 0); rotated 90 deg -> (0, 2); translated -> (10, 2).
        let [x, y] = m.transform_point([1.0, 0.0]);
        assert!((x - 10.0).abs() <= EPSILON, "x = {x}");
        assert!((y - 2.0).abs() <= EPSILON, "y = {y}");
    }

    #[test]
    fn composing_with_identity_is_a_no_op() {
        let m = Affine2::from_translation_rotation_scale([3.0, -2.0], 0.7, [1.5, 0.5]);
        assert_affine_approx_eq(m.compose(&Affine2::IDENTITY), m);
        assert_affine_approx_eq(Affine2::IDENTITY.compose(&m), m);
    }

    #[test]
    fn compose_applies_child_before_parent_and_is_not_commutative() {
        let translate = Affine2::from_translation(10.0, 0.0);
        let rotate = Affine2::from_rotation(PI / 2.0);

        // translate.compose(&rotate): rotate first, then translate.
        // (1, 0) rotated 90 deg -> (0, 1); translated -> (10, 1).
        let [x, y] = translate.compose(&rotate).transform_point([1.0, 0.0]);
        assert!((x - 10.0).abs() <= EPSILON, "x = {x}");
        assert!((y - 1.0).abs() <= EPSILON, "y = {y}");

        // rotate.compose(&translate): translate first, then rotate.
        // (1, 0) translated -> (11, 0); rotated 90 deg -> (0, 11).
        let [x2, y2] = rotate.compose(&translate).transform_point([1.0, 0.0]);
        assert!((x2 - 0.0).abs() <= EPSILON, "x2 = {x2}");
        assert!((y2 - 11.0).abs() <= EPSILON, "y2 = {y2}");

        // The two orders give different results -- composition is not
        // commutative.
        assert!((x - x2).abs() > EPSILON || (y - y2).abs() > EPSILON);
    }

    /// A fixed, varied set of transforms reused across every
    /// `compose_batch` remainder-length test below, so each length
    /// exercises real, distinct (not all-identity) parent/child data.
    fn sample_transforms(count: usize) -> Vec<Affine2> {
        (0..count)
            .map(|i| {
                #[allow(clippy::cast_precision_loss)]
                let i = i as f32;
                Affine2::from_translation_rotation_scale(
                    [i, -i * 0.5],
                    i * 0.3,
                    [1.0 + i * 0.1, 1.0 - i * 0.05],
                )
            })
            .collect()
    }

    #[test]
    fn compose_batch_matches_scalar_reference_across_every_simd_remainder() {
        // 0, 1, and every remainder relative to the 8-wide SIMD chunk
        // size, plus a couple of full-chunk-plus-remainder lengths.
        for len in [0, 1, 7, 8, 9, 16, 17] {
            let parents = sample_transforms(len);
            let children = sample_transforms(len).into_iter().rev().collect::<Vec<_>>();

            let mut simd_out = vec![Affine2::IDENTITY; len];
            compose_batch(&parents, &children, &mut simd_out);

            for i in 0..len {
                let scalar = parents[i].compose(&children[i]);
                assert_affine_approx_eq(simd_out[i], scalar);
            }
        }
    }

    #[test]
    #[should_panic(expected = "length mismatch")]
    fn compose_batch_panics_on_mismatched_lengths() {
        let parents = sample_transforms(4);
        let children = sample_transforms(3);
        let mut out = vec![Affine2::IDENTITY; 4];
        compose_batch(&parents, &children, &mut out);
    }

    /// A fixed, varied set of points reused across every
    /// `lerp_points_batch` remainder-length test below, matching
    /// `sample_transforms`'s own rationale: real, distinct (not
    /// all-identical) data at every length.
    fn sample_points(count: usize) -> Vec<[f32; 2]> {
        (0..count)
            .map(|i| {
                #[allow(clippy::cast_precision_loss)]
                let i = i as f32;
                [i * 1.5, -i * 0.7]
            })
            .collect()
    }

    fn scalar_lerp(a: [f32; 2], b: [f32; 2], t: f32) -> [f32; 2] {
        [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
    }

    #[test]
    fn lerp_points_batch_matches_scalar_reference_across_every_simd_remainder() {
        // 0, 1, and every remainder relative to the 8-wide SIMD chunk
        // size, plus a couple of full-chunk-plus-remainder lengths --
        // the same lengths `compose_batch`'s own test exercises.
        for len in [0, 1, 7, 8, 9, 16, 17] {
            let from = sample_points(len);
            let to = sample_points(len).into_iter().rev().collect::<Vec<_>>();
            let t = 0.37;

            let mut simd_out = vec![[0.0f32; 2]; len];
            lerp_points_batch(&from, &to, t, &mut simd_out);

            for i in 0..len {
                let scalar = scalar_lerp(from[i], to[i], t);
                assert!(
                    (simd_out[i][0] - scalar[0]).abs() <= EPSILON
                        && (simd_out[i][1] - scalar[1]).abs() <= EPSILON,
                    "index {i}: expected {scalar:?}, got {:?}",
                    simd_out[i]
                );
            }
        }
    }

    #[test]
    fn lerp_points_batch_at_t_zero_and_one_returns_the_endpoints_within_epsilon() {
        // `(b - a).mul_add(t, a)` at t=1 is mathematically `b`, but is not
        // guaranteed bit-exact to `b` -- `b - a` rounds once, and adding
        // `a` back rounds again, so the two roundings don't always
        // perfectly cancel (confirmed empirically: this test originally
        // asserted exact equality and failed on real sample data, e.g.
        // `-1.4` round-tripping to `-1.4000001`). An epsilon comparison,
        // not exact equality, is what actually verifies the math is
        // right rather than asserting a result that only happens to hold
        // bit-exactly for some inputs.
        let from = sample_points(9);
        let to = sample_points(9).into_iter().rev().collect::<Vec<_>>();

        let mut at_zero = vec![[0.0f32; 2]; 9];
        lerp_points_batch(&from, &to, 0.0, &mut at_zero);
        for i in 0..9 {
            assert!(
                (at_zero[i][0] - from[i][0]).abs() <= EPSILON
                    && (at_zero[i][1] - from[i][1]).abs() <= EPSILON
            );
        }

        let mut at_one = vec![[0.0f32; 2]; 9];
        lerp_points_batch(&from, &to, 1.0, &mut at_one);
        for i in 0..9 {
            assert!(
                (at_one[i][0] - to[i][0]).abs() <= EPSILON
                    && (at_one[i][1] - to[i][1]).abs() <= EPSILON
            );
        }
    }

    #[test]
    #[should_panic(expected = "length mismatch")]
    fn lerp_points_batch_panics_on_mismatched_lengths() {
        let from = sample_points(4);
        let to = sample_points(3);
        let mut out = vec![[0.0f32; 2]; 4];
        lerp_points_batch(&from, &to, 0.5, &mut out);
    }
}
