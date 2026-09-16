//! `Animated<T>` and the tick mechanism that advances it -- §5, §14
//! build-order step 2. Deliberately built and tested standalone here,
//! without `Node`/`Tree`/`PaintProperties` (those arrive at step 3):
//! this step exists to validate the animation core in isolation, per
//! Design Principle 5, before anything is built on top of it.

use std::time::{Duration, Instant};

/// Implemented by every type an `Animated<T>` can wrap -- `f64` and
/// `peniko::Color` for now (this step's own scope). `kurbo::Affine` and
/// `ShapeKey` (§5, §7.4) land whenever a later step first animates a
/// transform or a shape morph.
pub trait Interpolate {
    fn interpolate(&self, other: &Self, t: f64) -> Self;
}

impl Interpolate for f64 {
    fn interpolate(&self, other: &Self, t: f64) -> Self {
        self + (other - self) * t
    }
}

impl Interpolate for peniko::Color {
    fn interpolate(&self, other: &Self, t: f64) -> Self {
        // lerp_rect, not lerp: peniko::Color is AlphaColor<Srgb>, a
        // rectangular (non-polar) color space, so a straight per-
        // component lerp is the correct choice -- lerp (the other method
        // color::AlphaColor offers) exists for hue-based spaces and takes
        // a HueDirection this type doesn't need.
        self.lerp_rect(*other, t as f32)
    }
}

impl Interpolate for peniko::kurbo::Affine {
    /// A plain componentwise lerp of the 6 matrix coefficients, not a
    /// rotation-aware polar/SVD decomposition -- `kurbo = "0.13.1"`'s
    /// own `Affine::svd()` does exist and computes exactly that, but
    /// it's `pub(crate)`, not exported (confirmed directly in kurbo's
    /// vendored source), and §11.9's own text only ever asks for "pan
    /// offset × zoom scale," never rotation. The subspace of affines
    /// with no rotation/shear (`a == d`, `b == c == 0`) is convex, so a
    /// componentwise lerp between two such affines never introduces
    /// spurious shear or rotation mid-animation -- exact for the
    /// translate+uniform-scale case this milestone targets. A real
    /// interpolated rotation between two differently-rotated affines
    /// would look like a non-circular morph rather than sweeping
    /// through the correct arc -- a named, carried-forward limitation,
    /// not manufactured ahead of a real need for rotation (§11 Milestone
    /// 5 Phase 1, PLAN.md).
    fn interpolate(&self, other: &Self, t: f64) -> Self {
        let a = self.as_coeffs();
        let b = other.as_coeffs();
        let mut out = [0.0; 6];
        for i in 0..6 {
            out[i] = a[i] + (b[i] - a[i]) * t;
        }
        peniko::kurbo::Affine::new(out)
    }
}

/// MD3 named easing curves are cubic-bezier control points (§7.5) --
/// deliberately not built yet. Only `Linear` exists for now, which is
/// all this step's demo needs; real MD3 curves (Standard, Emphasized,
/// ...) land as their own variants whenever a build-order step first
/// needs one -- adding a variant here is additive, not a redesign.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MotionCurve {
    Linear,
}

impl MotionCurve {
    fn ease(self, t: f64) -> f64 {
        match self {
            MotionCurve::Linear => t,
        }
    }
}

/// Opaque handle for a finished animation's `on_complete` callback,
/// resolved to an actual Python callback only in `engine-py` (§5's
/// queue-drain decision). No allocator/registry exists yet -- that's
/// `engine-py`'s own `HashMap<CompletionHandle, PyObject>`, step 6+.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CompletionHandle(pub u64);

pub struct ActiveAnimation<T> {
    pub from: T,
    pub to: T,
    pub start: Instant,
    pub duration: Duration,
    pub curve: MotionCurve,
    pub on_complete: Option<CompletionHandle>,
}

/// The generic animation primitive (§5, Design Principle 2): every
/// animatable property anywhere in the tree is one of these, ticked by
/// the same mechanism regardless of what `T` is.
pub struct Animated<T: Interpolate + Clone> {
    pub current: T,
    pub active: Option<ActiveAnimation<T>>,
}

impl<T: Interpolate + Clone> Animated<T> {
    pub fn new(initial: T) -> Self {
        Self {
            current: initial,
            active: None,
        }
    }

    /// Starts (or replaces) this value's animation toward `to`. `from` is
    /// always the value's current state, not whatever the previous
    /// animation's own `to` was -- interrupting a still-running animation
    /// starts smoothly from wherever it actually is, not where it was
    /// headed.
    pub fn animate_to(&mut self, to: T, duration: Duration, curve: MotionCurve, now: Instant) {
        self.active = Some(ActiveAnimation {
            from: self.current.clone(),
            to,
            start: now,
            duration,
            curve,
            on_complete: None,
        });
    }

    /// Advances this value to `now`, returning `true` if it's still
    /// mid-animation afterward (stays dirty for another frame). This is
    /// the tick mechanism §5 describes applied to one value -- the
    /// "central" half (walking only the active set across a whole Tree,
    /// not matching on any specific component) arrives once `Node`/`Tree`
    /// exist to walk (step 3); until then, a caller ticks each `Animated<T>`
    /// it owns directly, which is what this step's demo does.
    pub fn tick(&mut self, now: Instant) -> bool {
        let Some(anim) = &self.active else {
            return false;
        };
        let elapsed = now.saturating_duration_since(anim.start);
        if elapsed >= anim.duration {
            self.current = anim.to.clone();
            self.active = None;
            return false;
        }
        // duration > 0 is guaranteed here: elapsed >= Duration::ZERO
        // always, so a zero-length animation already took the branch
        // above and never reaches this division.
        let raw_t = elapsed.as_secs_f64() / anim.duration.as_secs_f64();
        let eased_t = anim.curve.ease(raw_t);
        self.current = anim.from.interpolate(&anim.to, eased_t);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f64_interpolate_is_linear() {
        assert_eq!(10.0_f64.interpolate(&20.0, 0.0), 10.0);
        assert_eq!(10.0_f64.interpolate(&20.0, 0.5), 15.0);
        assert_eq!(10.0_f64.interpolate(&20.0, 1.0), 20.0);
    }

    #[test]
    fn color_interpolate_lerps_each_channel() {
        let black = peniko::Color::from_rgba8(0, 0, 0, 255);
        let white = peniko::Color::from_rgba8(255, 255, 255, 255);
        let mid = black.interpolate(&white, 0.5);
        // AlphaColor stores components as f32 in 0.0..=1.0, not u8 -- a
        // straight rectangular lerp of (0,0,0,1) and (1,1,1,1) at t=0.5
        // is exactly (0.5,0.5,0.5,1), unlike a naive u8-domain lerp
        // which would round differently.
        for c in mid.components.iter().take(3) {
            assert!((c - 0.5).abs() < 1e-6, "expected ~0.5, got {c}");
        }
    }

    #[test]
    fn tick_with_no_active_animation_is_a_no_op() {
        let mut value = Animated::new(5.0_f64);
        assert!(!value.tick(Instant::now()));
        assert_eq!(value.current, 5.0);
    }

    #[test]
    fn tick_advances_partway_then_snaps_on_completion() {
        let start = Instant::now();
        let mut value = Animated::new(0.0_f64);
        value.animate_to(100.0, Duration::from_secs(1), MotionCurve::Linear, start);

        // Halfway through: still active, current ~50.
        let still_active = value.tick(start + Duration::from_millis(500));
        assert!(still_active);
        assert!((value.current - 50.0).abs() < 0.01);

        // Past the end: snaps exactly to `to`, reports not-active.
        let still_active = value.tick(start + Duration::from_millis(1500));
        assert!(!still_active);
        assert_eq!(value.current, 100.0);
        assert!(value.active.is_none());
    }

    #[test]
    fn zero_duration_animation_snaps_immediately_no_panic() {
        let start = Instant::now();
        let mut value = Animated::new(1.0_f64);
        value.animate_to(9.0, Duration::ZERO, MotionCurve::Linear, start);
        assert!(!value.tick(start));
        assert_eq!(value.current, 9.0);
    }

    #[test]
    fn interrupting_a_running_animation_starts_from_current_not_old_target() {
        let start = Instant::now();
        let mut value = Animated::new(0.0_f64);
        value.animate_to(100.0, Duration::from_secs(1), MotionCurve::Linear, start);
        value.tick(start + Duration::from_millis(500)); // current ~= 50
        let current_before_interrupt = value.current;

        // Retarget mid-flight -- `from` should be ~50, not 0 or 100.
        value.animate_to(
            0.0,
            Duration::from_secs(1),
            MotionCurve::Linear,
            start + Duration::from_millis(500),
        );
        let from = value.active.as_ref().unwrap().from;
        assert_eq!(from, current_before_interrupt);
    }
}
