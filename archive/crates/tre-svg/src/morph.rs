//! SIMD path-morphing interpolation between two keyframe polygons
//! (IMPLEMENTATION.md Step 3.3 task 2). Pure geometry -- triangulation
//! stays a separate, explicit caller step via [`crate::triangulate`],
//! matching this crate's established parse -> polygon -> triangulate ->
//! vertices pipeline shape rather than folding morphing into any of
//! those stages.

use crate::{Polygon, SvgError};

/// Interpolates between two keyframe polygons at parameter `t` (typically
/// in `[0.0, 1.0]`, though nothing here clamps it -- overshoot is a
/// legitimate easing-curve technique, not this function's concern).
///
/// "Topological equivalence" between already-flattened polygons means
/// equal vertex counts (see this crate's `SvgError::TopologyMismatch`
/// docs for why mismatches are rejected, not auto-resampled). The actual
/// interpolation is `tre_math::lerp_points_batch` -- a real SIMD batch
/// operation (`wide::f32x8`, TECHNICAL.md Section 5.4), not a scalar
/// loop written here.
///
/// Allocates a fresh output `Vec` every call -- convenient for a one-shot
/// caller (this crate's own demo), but per-frame animation callers should
/// use [`morph_into`] instead, reusing one persistent buffer across
/// frames rather than allocating on DESIGN.md Section 2.1's supposed-to-
/// be-zero-allocation steady state (performance-review finding: this
/// function used to be the *only* real caller of the zero-alloc
/// `lerp_points_batch` primitive, and defeated its whole point by
/// allocating before every call anyway).
///
/// # Errors
/// Returns [`SvgError::TopologyMismatch`] if `from.points.len() !=
/// to.points.len()`.
pub fn morph(from: &Polygon, to: &Polygon, t: f32) -> Result<Polygon, SvgError> {
    let mut points = Vec::new();
    morph_into(from, to, t, &mut points)?;
    Ok(Polygon { points })
}

/// Same interpolation as [`morph`], but writes into a caller-supplied
/// `out` buffer instead of returning a freshly allocated one -- the
/// zero-alloc path a real per-frame animation loop should use, reusing
/// one persistent `out` across every frame of a morph animation.
/// `out.resize`'s no-op-when-already-the-right-length behavior means a
/// steady-state loop (same two keyframes' vertex count every frame)
/// performs no allocation here at all after the first call.
///
/// # Errors
/// Returns [`SvgError::TopologyMismatch`] if `from.points.len() !=
/// to.points.len()`. `out` is left unchanged in that case.
pub fn morph_into(
    from: &Polygon,
    to: &Polygon,
    t: f32,
    out: &mut Vec<[f32; 2]>,
) -> Result<(), SvgError> {
    if from.points.len() != to.points.len() {
        return Err(SvgError::TopologyMismatch {
            from_points: from.points.len(),
            to_points: to.points.len(),
        });
    }

    out.resize(from.points.len(), [0.0f32; 2]);
    tre_math::lerp_points_batch(&from.points, &to.points, t, out);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn morph_at_t_zero_and_one_returns_the_keyframes_within_epsilon() {
        let from = Polygon {
            points: vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]],
        };
        let to = Polygon {
            points: vec![[2.0, 2.0], [12.0, 1.0], [11.0, 12.0], [1.0, 9.0]],
        };

        let at_zero = morph(&from, &to, 0.0).expect("equal vertex counts");
        for (a, f) in at_zero.points.iter().zip(&from.points) {
            assert!((a[0] - f[0]).abs() < 1e-4 && (a[1] - f[1]).abs() < 1e-4);
        }

        let at_one = morph(&from, &to, 1.0).expect("equal vertex counts");
        for (a, t) in at_one.points.iter().zip(&to.points) {
            assert!((a[0] - t[0]).abs() < 1e-4 && (a[1] - t[1]).abs() < 1e-4);
        }
    }

    #[test]
    fn morph_at_t_half_returns_the_exact_midpoint() {
        let from = Polygon {
            points: vec![[0.0, 0.0], [10.0, 10.0]],
        };
        let to = Polygon {
            points: vec![[10.0, 0.0], [0.0, 10.0]],
        };

        let midpoint = morph(&from, &to, 0.5).expect("equal vertex counts");
        assert_eq!(midpoint.points, vec![[5.0, 0.0], [5.0, 10.0]]);
    }

    #[test]
    fn morph_into_reuses_the_callers_buffer_across_frames_without_reallocating() {
        let from = Polygon {
            points: vec![[0.0, 0.0], [10.0, 10.0]],
        };
        let to = Polygon {
            points: vec![[10.0, 0.0], [0.0, 10.0]],
        };

        let mut out = Vec::new();
        morph_into(&from, &to, 0.0, &mut out).expect("equal vertex counts");
        let capacity_after_first_frame = out.capacity();

        for i in 0..10 {
            #[allow(clippy::cast_precision_loss, reason = "i is a tiny test loop counter")]
            let t = i as f32 / 10.0;
            morph_into(&from, &to, t, &mut out).expect("equal vertex counts");
            assert_eq!(out.len(), 2);
            // A steady-state per-frame caller (same vertex count every
            // frame) must never need to grow the buffer past its first
            // real allocation.
            assert_eq!(out.capacity(), capacity_after_first_frame);
        }

        morph_into(&from, &to, 0.5, &mut out).expect("equal vertex counts");
        assert_eq!(out, vec![[5.0, 0.0], [5.0, 10.0]]);
    }

    #[test]
    fn morph_rejects_mismatched_vertex_counts() {
        let from = Polygon {
            points: vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0]],
        };
        let to = Polygon {
            points: vec![[0.0, 0.0], [1.0, 0.0]],
        };

        let result = morph(&from, &to, 0.5);
        assert!(matches!(
            result,
            Err(SvgError::TopologyMismatch {
                from_points: 3,
                to_points: 2
            })
        ));
    }
}
