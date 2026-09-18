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

/// A single cubic Bézier segment, `P0`→`P1`→`P2`→`P3`, in arbitrary
/// `(x, y)` space -- not the `(0,0)`→`(1,1)`-fixed, 2-control-point
/// CSS `cubic-bezier()` shorthand, so the same type can also represent
/// one segment of a real *compound* easing curve (M7 Phase 1's own
/// `Emphasized`, below).
#[derive(Clone, Copy)]
struct CubicSegment {
    p0: (f64, f64),
    p1: (f64, f64),
    p2: (f64, f64),
    p3: (f64, f64),
}

impl CubicSegment {
    const fn standard(x1: f64, y1: f64, x2: f64, y2: f64) -> Self {
        Self {
            p0: (0.0, 0.0),
            p1: (x1, y1),
            p2: (x2, y2),
            p3: (1.0, 1.0),
        }
    }

    fn eval(&self, t: f64) -> (f64, f64) {
        let mt = 1.0 - t;
        let a = mt * mt * mt;
        let b = 3.0 * mt * mt * t;
        let c = 3.0 * mt * t * t;
        let d = t * t * t;
        (
            a * self.p0.0 + b * self.p1.0 + c * self.p2.0 + d * self.p3.0,
            a * self.p0.1 + b * self.p1.1 + c * self.p2.1 + d * self.p3.1,
        )
    }

    /// Given `x` (assumed within this segment's own `X(t)` range),
    /// returns the `y` on the curve at that `x` -- bisection on `t`
    /// against `X(t)`, the same "given x, find t, then return Y(t)"
    /// problem every browser engine solves for CSS `cubic-bezier()`.
    /// Valid (monotonic `X(t)`) for every real curve this module
    /// defines, all of which keep their control points' own x
    /// coordinates within `[0, 1]`, the same precondition CSS's own
    /// spec requires of a valid `cubic-bezier()`.
    fn solve_y_for_x(&self, x: f64) -> f64 {
        let (mut lo, mut hi) = (0.0_f64, 1.0_f64);
        for _ in 0..40 {
            let mid = f64::midpoint(lo, hi);
            if self.eval(mid).0 < x {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        self.eval(f64::midpoint(lo, hi)).1
    }
}

/// MD3 named easing curves (§7.5), as real cubic-bezier control points
/// -- verified against Android's own `MotionTokens.kt` (generated
/// directly from the official Material Design spec), not recalled from
/// memory (`PLAN.md`): the single-segment curves are standard CSS-style
/// `cubic-bezier(x1, y1, x2, y2)` values; `Emphasized` is a genuine
/// two-segment compound curve (`M 0,0 C 0.05,0 0.133333,0.06 0.166666,
/// 0.4 C 0.208333,0.82 0.25,1 1,1`), not a single 4-parameter curve --
/// a common web/CSS approximation flattens it to `Standard`'s own
/// value, which this implementation deliberately does not do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MotionCurve {
    Linear,
    Standard,
    StandardDecelerate,
    StandardAccelerate,
    Emphasized,
    EmphasizedDecelerate,
    EmphasizedAccelerate,
}

/// `Emphasized`'s own two segments join at this real, documented `x`
/// (`MotionTokens.kt`'s own path data) -- not an even split, and not a
/// rounded fraction.
const EMPHASIZED_SPLIT_X: f64 = 0.166666;

const STANDARD: CubicSegment = CubicSegment::standard(0.2, 0.0, 0.0, 1.0);
const STANDARD_DECELERATE: CubicSegment = CubicSegment::standard(0.0, 0.0, 0.0, 1.0);
const STANDARD_ACCELERATE: CubicSegment = CubicSegment::standard(0.3, 0.0, 1.0, 1.0);
const EMPHASIZED_DECELERATE: CubicSegment = CubicSegment::standard(0.05, 0.7, 0.1, 1.0);
const EMPHASIZED_ACCELERATE: CubicSegment = CubicSegment::standard(0.3, 0.0, 0.8, 0.15);
const EMPHASIZED_1: CubicSegment = CubicSegment {
    p0: (0.0, 0.0),
    p1: (0.05, 0.0),
    p2: (0.133333, 0.06),
    p3: (EMPHASIZED_SPLIT_X, 0.4),
};
const EMPHASIZED_2: CubicSegment = CubicSegment {
    p0: (EMPHASIZED_SPLIT_X, 0.4),
    p1: (0.208333, 0.82),
    p2: (0.25, 1.0),
    p3: (1.0, 1.0),
};

impl MotionCurve {
    fn ease(self, t: f64) -> f64 {
        match self {
            MotionCurve::Linear => t,
            MotionCurve::Standard => STANDARD.solve_y_for_x(t),
            MotionCurve::StandardDecelerate => STANDARD_DECELERATE.solve_y_for_x(t),
            MotionCurve::StandardAccelerate => STANDARD_ACCELERATE.solve_y_for_x(t),
            MotionCurve::EmphasizedDecelerate => EMPHASIZED_DECELERATE.solve_y_for_x(t),
            MotionCurve::EmphasizedAccelerate => EMPHASIZED_ACCELERATE.solve_y_for_x(t),
            MotionCurve::Emphasized => {
                if t <= EMPHASIZED_SPLIT_X {
                    EMPHASIZED_1.solve_y_for_x(t)
                } else {
                    EMPHASIZED_2.solve_y_for_x(t)
                }
            }
        }
    }
}

/// Opaque handle for a finished animation's `on_complete` callback,
/// resolved to an actual Python callback only in `engine-py` (§5's
/// queue-drain decision) -- real as of M9 Phase 2: `engine-py::
/// dispatch::CompletionRegistry`'s own `HashMap<CompletionHandle,
/// Py<PyAny>>` is the allocator/registry this always deferred to.
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
    /// headed. No completion callback -- see `animate_to_with_completion`
    /// for the sibling that attaches a real `CompletionHandle` (M9 Phase
    /// 1, §5); kept separate rather than a 5th parameter here so none of
    /// this method's own ~15+ existing call sites needed touching.
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

    /// M9 Phase 1 (§5): `animate_to`'s own real completion-callback
    /// counterpart -- identical body, except `on_complete: Some(...)`
    /// instead of `None`. `Tree::tick_all`'s own real drain (below,
    /// `tick`) is what actually reports `on_complete` back out once
    /// this animation genuinely finishes.
    pub fn animate_to_with_completion(
        &mut self,
        to: T,
        duration: Duration,
        curve: MotionCurve,
        now: Instant,
        on_complete: CompletionHandle,
    ) {
        self.active = Some(ActiveAnimation {
            from: self.current.clone(),
            to,
            start: now,
            duration,
            curve,
            on_complete: Some(on_complete),
        });
    }

    /// Advances this value to `now`, returning `true` if it's still
    /// mid-animation afterward (stays dirty for another frame). This is
    /// the tick mechanism §5 describes applied to one value -- the
    /// "central" half (walking only the active set across a whole Tree,
    /// not matching on any specific component) arrives once `Node`/`Tree`
    /// exist to walk (step 3); until then, a caller ticks each `Animated<T>`
    /// it owns directly, which is what this step's demo does.
    ///
    /// M9 Phase 1 (§5): `completed` is a shared, caller-owned
    /// accumulator (not a return value) so a caller ticking many
    /// `Animated<T>` fields in one pass (`PaintProperties::tick`, the
    /// central `Tree::tick_all` walk) can collect every real completion
    /// across all of them without allocating a fresh `Vec` per field --
    /// pushed into *before* `self.active` is cleared, the same "read
    /// what's needed out of the borrowed `ActiveAnimation` before
    /// clearing it" order this method's own `anim.to.clone()` already
    /// used.
    pub fn tick(&mut self, now: Instant, completed: &mut Vec<CompletionHandle>) -> bool {
        let Some(anim) = &self.active else {
            return false;
        };
        let elapsed = now.saturating_duration_since(anim.start);
        if elapsed >= anim.duration {
            self.current = anim.to.clone();
            if let Some(handle) = anim.on_complete {
                completed.push(handle);
            }
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

    /// M7 Phase 1 (§7.5): every real MD3 curve must start at `y=0` and
    /// end at `y=1`, the same boundary condition every CSS/MD3 easing
    /// curve is defined to satisfy.
    #[test]
    fn every_motion_curve_satisfies_its_own_boundary_conditions() {
        for curve in [
            MotionCurve::Linear,
            MotionCurve::Standard,
            MotionCurve::StandardDecelerate,
            MotionCurve::StandardAccelerate,
            MotionCurve::Emphasized,
            MotionCurve::EmphasizedDecelerate,
            MotionCurve::EmphasizedAccelerate,
        ] {
            assert!(
                (curve.ease(0.0) - 0.0).abs() < 1e-9,
                "{curve:?}.ease(0.0) must be 0.0"
            );
            assert!(
                (curve.ease(1.0) - 1.0).abs() < 1e-9,
                "{curve:?}.ease(1.0) must be 1.0"
            );
        }
    }

    /// Every real MD3 curve here is monotonically non-decreasing --
    /// sampled, not proven analytically, but at a fine enough
    /// resolution (200 points) to catch a real ordering bug (e.g. a
    /// wrong segment split, a transposed control point).
    #[test]
    fn every_motion_curve_is_monotonically_non_decreasing() {
        for curve in [
            MotionCurve::Linear,
            MotionCurve::Standard,
            MotionCurve::StandardDecelerate,
            MotionCurve::StandardAccelerate,
            MotionCurve::Emphasized,
            MotionCurve::EmphasizedDecelerate,
            MotionCurve::EmphasizedAccelerate,
        ] {
            let mut previous = curve.ease(0.0);
            for i in 1..=200 {
                let t = f64::from(i) / 200.0;
                let y = curve.ease(t);
                assert!(
                    y + 1e-9 >= previous,
                    "{curve:?} must be non-decreasing: ease({t}) = {y} < previous {previous}"
                );
                previous = y;
            }
        }
    }

    /// The bisection solver's own round-trip consistency, the same
    /// rigor `kurbo::Affine::inverse()`'s own tests use: forward-
    /// evaluate a real `(x, y)` point on each single-segment curve at a
    /// chosen `t` via `CubicSegment::eval`, then confirm `solve_y_for_x`
    /// recovers the same `y` from that `x` -- validates the solver
    /// against the curve's own forward math, not just plausibility.
    #[test]
    fn cubic_segment_solve_y_for_x_round_trips_with_eval() {
        for segment in [
            STANDARD,
            STANDARD_DECELERATE,
            STANDARD_ACCELERATE,
            EMPHASIZED_DECELERATE,
            EMPHASIZED_ACCELERATE,
            EMPHASIZED_1,
            EMPHASIZED_2,
        ] {
            for i in 1..10 {
                let t = f64::from(i) / 10.0;
                let (x, y) = segment.eval(t);
                let recovered = segment.solve_y_for_x(x);
                assert!(
                    (recovered - y).abs() < 1e-6,
                    "solve_y_for_x({x}) = {recovered}, expected {y} (round-trip from t={t})"
                );
            }
        }
    }

    /// `Emphasized`'s own real, externally-verified landmark (`PLAN.md`):
    /// its two segments join at `(0.166666, 0.4)`, confirmed directly
    /// against Android's own `MotionTokens.kt` path data -- not a
    /// rounded/guessed split, and continuous (no jump) across the join.
    #[test]
    fn emphasized_curve_matches_its_real_documented_split_point() {
        let just_before = MotionCurve::Emphasized.ease(EMPHASIZED_SPLIT_X - 1e-6);
        let at_split = MotionCurve::Emphasized.ease(EMPHASIZED_SPLIT_X);
        let just_after = MotionCurve::Emphasized.ease(EMPHASIZED_SPLIT_X + 1e-6);

        assert!(
            (at_split - 0.4).abs() < 1e-4,
            "Emphasized's real split-point y must be ~0.4 (MotionTokens.kt), got {at_split}"
        );
        assert!(
            (just_before - just_after).abs() < 1e-4,
            "Emphasized must be continuous across its segment join: {just_before} vs {just_after}"
        );
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
        assert!(!value.tick(Instant::now(), &mut Vec::new()));
        assert_eq!(value.current, 5.0);
    }

    #[test]
    fn tick_advances_partway_then_snaps_on_completion() {
        let start = Instant::now();
        let mut value = Animated::new(0.0_f64);
        value.animate_to(100.0, Duration::from_secs(1), MotionCurve::Linear, start);

        // Halfway through: still active, current ~50.
        let still_active = value.tick(start + Duration::from_millis(500), &mut Vec::new());
        assert!(still_active);
        assert!((value.current - 50.0).abs() < 0.01);

        // Past the end: snaps exactly to `to`, reports not-active.
        let still_active = value.tick(start + Duration::from_millis(1500), &mut Vec::new());
        assert!(!still_active);
        assert_eq!(value.current, 100.0);
        assert!(value.active.is_none());
    }

    #[test]
    fn zero_duration_animation_snaps_immediately_no_panic() {
        let start = Instant::now();
        let mut value = Animated::new(1.0_f64);
        value.animate_to(9.0, Duration::ZERO, MotionCurve::Linear, start);
        assert!(!value.tick(start, &mut Vec::new()));
        assert_eq!(value.current, 9.0);
    }

    #[test]
    fn interrupting_a_running_animation_starts_from_current_not_old_target() {
        let start = Instant::now();
        let mut value = Animated::new(0.0_f64);
        value.animate_to(100.0, Duration::from_secs(1), MotionCurve::Linear, start);
        value.tick(start + Duration::from_millis(500), &mut Vec::new()); // current ~= 50
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

    #[test]
    fn animate_to_with_completion_reports_its_handle_exactly_on_the_completing_tick() {
        let start = Instant::now();
        let mut value = Animated::new(0.0_f64);
        let handle = CompletionHandle(42);
        value.animate_to_with_completion(
            100.0,
            Duration::from_secs(1),
            MotionCurve::Linear,
            start,
            handle,
        );

        // Halfway through: still animating, no completion yet.
        let mut completed = Vec::new();
        value.tick(start + Duration::from_millis(500), &mut completed);
        assert!(
            completed.is_empty(),
            "must not report completion before the animation genuinely finishes"
        );

        // Past the end: the completing tick reports the real handle,
        // exactly once.
        let mut completed = Vec::new();
        value.tick(start + Duration::from_millis(1500), &mut completed);
        assert_eq!(completed, vec![handle]);

        // A later tick on an already-inactive value must not re-report
        // it -- `self.active` is `None` by now, so `tick`'s own
        // early-return path never touches `completed` at all.
        let mut completed_again = Vec::new();
        value.tick(start + Duration::from_secs(2), &mut completed_again);
        assert!(
            completed_again.is_empty(),
            "a finished animation must report its completion exactly once, not on every \
             later tick"
        );
    }

    #[test]
    fn plain_animate_to_never_reports_a_completion() {
        // The existing, real no-op case: an animation started the plain
        // way (no completion callback) must never populate `completed`,
        // even on the exact tick it finishes -- `on_complete: None` is
        // `animate_to`'s own real, unchanged default.
        let start = Instant::now();
        let mut value = Animated::new(0.0_f64);
        value.animate_to(100.0, Duration::from_secs(1), MotionCurve::Linear, start);

        let mut completed = Vec::new();
        value.tick(start + Duration::from_secs(2), &mut completed);
        assert!(completed.is_empty());
    }
}
