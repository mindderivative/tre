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
/// # Errors
/// Returns [`SvgError::TopologyMismatch`] if `from.points.len() !=
/// to.points.len()`.
pub fn morph(from: &Polygon, to: &Polygon, t: f32) -> Result<Polygon, SvgError> {
    if from.points.len() != to.points.len() {
        return Err(SvgError::TopologyMismatch {
            from_points: from.points.len(),
            to_points: to.points.len(),
        });
    }

    let mut points = vec![[0.0f32; 2]; from.points.len()];
    tre_math::lerp_points_batch(&from.points, &to.points, t, &mut points);
    Ok(Polygon { points })
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
