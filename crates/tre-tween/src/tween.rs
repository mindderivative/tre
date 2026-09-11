//! `Tween<T>`: a pure, stateless interpolation between two values of the
//! same lerpable type, sampled by progress `t in [0.0, 1.0]` through a
//! chosen [`Easing`] curve. Deliberately has no notion of *when* `t`
//! should advance or how many tweens are running -- that's
//! `tre-animation`'s job (Phase 13 Step 13.3); this crate is the pure
//! math layer underneath it.

use crate::easing::Easing;

/// Anything a [`Tween`] can interpolate between. Implemented here for
/// `f32` and `glam::Vec2` -- the two real, immediate needs (scalar
/// shape properties like opacity/rotation, and 2D properties like
/// position/scale). `glam::Vec3`/`Quat` are real, straightforward
/// additions once a real 3D consumer exists; not added speculatively.
pub trait Lerp: Copy {
    #[must_use]
    fn lerp(self, other: Self, t: f32) -> Self;
}

impl Lerp for f32 {
    fn lerp(self, other: Self, t: f32) -> Self {
        self + (other - self) * t
    }
}

impl Lerp for glam::Vec2 {
    fn lerp(self, other: Self, t: f32) -> Self {
        glam::Vec2::lerp(self, other, t)
    }
}

/// A stateless interpolation between `from` and `to` over `duration`
/// real seconds, through `easing`.
#[derive(Debug, Clone, Copy)]
pub struct Tween<T: Lerp> {
    pub from: T,
    pub to: T,
    /// Real seconds this tween spans. A `sample`d `elapsed` beyond this
    /// is clamped to `1.0` progress (holds at `to`), not extrapolated --
    /// unlike a raw [`Easing`] call, which does extrapolate; a tween's
    /// own contract is "reaches and holds `to`," not "keeps moving past
    /// it."
    pub duration: f32,
    pub easing: Easing,
}

impl<T: Lerp> Tween<T> {
    #[must_use]
    pub fn new(from: T, to: T, duration: f32, easing: Easing) -> Self {
        Self {
            from,
            to,
            duration,
            easing,
        }
    }

    /// Samples this tween at `elapsed` real seconds since it started.
    /// `elapsed` is clamped into `[0.0, duration]` before computing
    /// progress, so a caller can pass any real elapsed time (including
    /// past `duration`) and always get a well-defined result: `from` at
    /// or before `0.0`, `to` at or after `duration`.
    #[must_use]
    pub fn sample(&self, elapsed: f32) -> T {
        let progress = if self.duration <= 0.0 {
            1.0
        } else {
            (elapsed / self.duration).clamp(0.0, 1.0)
        };
        let eased = self.easing.apply(progress);
        self.from.lerp(self.to, eased)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_linear_tween_samples_exactly_at_the_midpoint() {
        let tween = Tween::new(0.0_f32, 10.0, 1.0, Easing::Linear);
        assert!((tween.sample(0.5) - 5.0).abs() <= 1e-5);
    }

    #[test]
    fn sampling_before_zero_or_past_duration_clamps_to_the_endpoints() {
        let tween = Tween::new(0.0_f32, 10.0, 1.0, Easing::Linear);
        assert_eq!(tween.sample(-1.0), 0.0);
        assert_eq!(tween.sample(5.0), 10.0);
    }

    #[test]
    fn a_zero_duration_tween_is_immediately_at_its_end_value() {
        let tween = Tween::new(0.0_f32, 10.0, 0.0, Easing::Linear);
        assert_eq!(tween.sample(0.0), 10.0);
    }

    #[test]
    fn a_vec2_tween_interpolates_both_components() {
        let tween = Tween::new(
            glam::Vec2::new(0.0, 0.0),
            glam::Vec2::new(10.0, 20.0),
            1.0,
            Easing::Linear,
        );
        let mid = tween.sample(0.5);
        assert!((mid.x - 5.0).abs() <= 1e-5);
        assert!((mid.y - 10.0).abs() <= 1e-5);
    }

    #[test]
    fn an_eased_tween_differs_from_a_linear_one_at_the_same_progress() {
        let linear = Tween::new(0.0_f32, 10.0, 1.0, Easing::Linear);
        let eased = Tween::new(0.0_f32, 10.0, 1.0, Easing::EaseInQuad);
        assert!(
            (linear.sample(0.25) - eased.sample(0.25)).abs() > 0.1,
            "EaseInQuad must sample meaningfully differently from Linear at t=0.25"
        );
    }
}
