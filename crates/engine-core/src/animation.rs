//! `Animated<T>` and the tick mechanism that advances it -- §5, §14
//! build-order step 2. Deliberately built and tested standalone here,
//! without `Node`/`Tree`/`PaintProperties` (those arrive at step 3):
//! this step exists to validate the animation core in isolation, per
//! Design Principle 5, before anything is built on top of it.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// 0.5.4 (#103): how many animations have ever been started, in the whole
/// process. `Tree::tick_all` compares it with the count it last saw: equal
/// means nothing new has started, so only the nodes it already knows are
/// animating need ticking; different means one might have, anywhere, so it
/// looks at every node once. Counting starts rather than registering nodes
/// is why no code that starts an animation can forget to say so.
static STARTED: AtomicU64 = AtomicU64::new(0);

/// The number of animations started so far (see `STARTED`).
pub(crate) fn animations_started() -> u64 {
    STARTED.load(Ordering::Relaxed)
}

/// Implemented by every type an `Animated<T>` can wrap -- `f64`,
/// `peniko::Color`, and `kurbo::Affine` here, plus `CornerRadii`,
/// `Shadow`/`Shadows` (`node.rs`), and `PathData` (`path.rs`).
pub trait Interpolate {
    fn interpolate(&self, other: &Self, t: f64) -> Self;

    /// 0.5.4 (#138): the value as one number, for the types that are one
    /// (`f64`): what lets a spring carry its velocity into a retarget.
    fn scalar(&self) -> Option<f64> {
        None
    }
}

impl Interpolate for f64 {
    fn interpolate(&self, other: &Self, t: f64) -> Self {
        self + (other - self) * t
    }

    fn scalar(&self) -> Option<f64> {
        Some(*self)
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
/// one segment of a compound easing curve.
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

/// An animation's easing: linear, or a CSS-style cubic bezier. M99
/// removed the MD3 named curves (`Standard`, `Emphasized`, ...); a
/// framework passes their bezier values instead (`docs/design/
/// md3-handover.md` records them, and `Emphasized`'s two segments).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MotionCurve {
    Linear,
    /// M95: any CSS-style cubic bezier `(x1, y1, x2, y2)` -- the curve a
    /// framework passes as `easing`.
    Bezier(f64, f64, f64, f64),
    /// 0.5.4 (#136): exponential decay, the shape of a flick coming to rest:
    /// fast at the start, easing out with no overshoot. `k` is how many
    /// time constants the animation spans: `(1 - e^(-k t)) / (1 - e^(-k))`
    /// on `t` in `0..=1`, so a larger `k` is a harder stop. Only the engine
    /// builds it (momentum scrolling); `k` must be above 0.
    Decay(f64),
    /// 0.5.4 (#138): a damped spring from 0 to 1, in real time. `omega` is its
    /// natural frequency in radians a second, `zeta` its damping ratio (below 1
    /// overshoots and rings, 1 is the quickest with no overshoot, above 1 is
    /// sluggish), `v0` its starting speed in "distances a second" (0 from rest;
    /// a retarget carries the speed it interrupted), and `settle` the seconds
    /// until it is within a tenth of a percent of rest, which is how long the
    /// animation lasts whatever duration it was given. Build one with
    /// `MotionCurve::spring`.
    Spring {
        omega: f64,
        zeta: f64,
        v0: f64,
        settle: f64,
    },
}

/// A spring's distance from rest at `s` seconds, as a fraction of the way
/// still to go: 1 at the start, 0 at rest. Closed form for each damping.
fn spring_remaining(omega: f64, zeta: f64, v0: f64, s: f64) -> f64 {
    if (zeta - 1.0).abs() < 1e-9 {
        // Critically damped.
        (-omega * s).exp() * (1.0 + (omega - v0) * s)
    } else if zeta < 1.0 {
        let wd = omega * (1.0 - zeta * zeta).sqrt();
        let b = (zeta * omega - v0) / wd;
        (-zeta * omega * s).exp() * ((wd * s).cos() + b * (wd * s).sin())
    } else {
        let root = (zeta * zeta - 1.0).sqrt();
        let (r1, r2) = (-omega * (zeta - root), -omega * (zeta + root));
        let c1 = (-v0 - r2) / (r1 - r2);
        c1 * (r1 * s).exp() + (1.0 - c1) * (r2 * s).exp()
    }
}

impl MotionCurve {
    /// A spring with natural frequency `omega` and damping ratio `zeta`
    /// starting at `v0` (see `Spring`); computes how long it takes to settle.
    pub fn spring(omega: f64, zeta: f64, v0: f64) -> Self {
        let (omega, zeta) = (omega.max(1e-3), zeta.max(0.01));
        // The last moment it is farther than 0.1% from rest, found by
        // stepping (there is no closed form for an under-damped ring).
        let step = 0.002;
        let mut last = 0.0;
        let mut s = 0.0;
        while s < 20.0 {
            if spring_remaining(omega, zeta, v0, s).abs() > 0.001 {
                last = s;
            }
            s += step;
        }
        MotionCurve::Spring {
            omega,
            zeta,
            v0,
            settle: (last + step).max(0.05),
        }
    }

    /// A spring described the way people think of one: it takes about
    /// `duration` for one cycle of its main motion, and `bounce` says how much
    /// it overshoots: `0.0` the quickest settle with none, towards `1.0` more
    /// and longer ringing, below `0.0` slower and softer (range -1 to 1,
    /// exclusive).
    pub fn spring_for(duration: Duration, bounce: f64) -> Self {
        let omega = std::f64::consts::TAU / duration.as_secs_f64().max(1e-3);
        Self::spring(omega, 1.0 - bounce.clamp(-0.99, 0.99), 0.0)
    }

    /// How fast the curve is moving at `t` (0 to 1), per unit of `t`.
    fn slope(self, t: f64) -> f64 {
        let eps = 1e-4;
        let (a, b) = ((t - eps).max(0.0), (t + eps).min(1.0));
        if b <= a {
            return 0.0;
        }
        (self.ease(b) - self.ease(a)) / (b - a)
    }
}

impl MotionCurve {
    fn ease(self, t: f64) -> f64 {
        match self {
            MotionCurve::Linear => t,
            MotionCurve::Bezier(x1, y1, x2, y2) => {
                CubicSegment::standard(x1, y1, x2, y2).solve_y_for_x(t)
            }
            MotionCurve::Decay(k) => (1.0 - (-k * t).exp()) / (1.0 - (-k).exp()),
            MotionCurve::Spring {
                omega,
                zeta,
                v0,
                settle,
            } => 1.0 - spring_remaining(omega, zeta, v0, t * settle),
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

thread_local! {
    /// 0.5.6 (#161): handles of animations that were replaced or stopped before
    /// they finished, until the registry that owns each takes it.
    static CANCELLED: std::cell::RefCell<Vec<CompletionHandle>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// 0.5.6 (#161): takes the cancelled-animation handles `owned` claims; the
/// rest stay for their own window. A handle is cancelled when its animation is
/// replaced by another on the same value, or stopped.
pub fn take_cancelled(owned: impl Fn(CompletionHandle) -> bool) -> Vec<CompletionHandle> {
    CANCELLED.with(|cell| {
        let mut cancelled = cell.borrow_mut();
        let mut taken = Vec::new();
        cancelled.retain(|handle| {
            if owned(*handle) {
                taken.push(*handle);
                false
            } else {
                true
            }
        });
        taken
    })
}

fn note_cancelled(handle: Option<CompletionHandle>) {
    if let Some(handle) = handle {
        CANCELLED.with(|cell| cell.borrow_mut().push(handle));
    }
}

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
    /// Boxed (0.5.4, #106): most values most of the time are not animating,
    /// and an inline animation more than tripled every one of them.
    pub active: Option<Box<ActiveAnimation<T>>>,
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
    /// M95: stops a running animation where it is -- `current` keeps the
    /// value it had reached, and its completion never fires.
    pub fn stop(&mut self) {
        if let Some(anim) = self.active.take() {
            note_cancelled(anim.on_complete);
        }
    }

    /// 0.5.6 (#161): sets the value outright, ending a running animation as
    /// cancelled -- what `Animated::new` in its place would do, without
    /// dropping the animation unseen.
    pub fn set_now(&mut self, value: T) {
        self.stop();
        self.current = value;
    }

    /// M95: where a running animation is heading, or `current` when none
    /// runs -- what a reconciler compares against, since mid-animation
    /// `current` is still in between.
    pub fn target(&self) -> &T {
        self.active.as_ref().map_or(&self.current, |anim| &anim.to)
    }

    pub fn animate_to(&mut self, to: T, duration: Duration, curve: MotionCurve, now: Instant) {
        self.start(to, duration, curve, now, None);
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
        self.start(to, duration, curve, now, Some(on_complete));
    }

    fn start(
        &mut self,
        to: T,
        duration: Duration,
        curve: MotionCurve,
        now: Instant,
        on_complete: Option<CompletionHandle>,
    ) {
        STARTED.fetch_add(1, Ordering::Relaxed);
        // 0.5.4 (#138): a spring lasts until it settles, and a spring that
        // interrupts a moving number carries that number's speed on.
        let (curve, duration) = match curve {
            MotionCurve::Spring {
                omega,
                zeta,
                settle,
                ..
            } => {
                let v0 = self.speed_toward(&to, now).unwrap_or(0.0);
                let curve = if v0 == 0.0 {
                    // From rest (or not a number): the curve as given.
                    MotionCurve::spring(omega, zeta, 0.0)
                } else {
                    MotionCurve::spring(omega, zeta, v0)
                };
                let settle = match curve {
                    MotionCurve::Spring { settle, .. } => settle,
                    _ => settle,
                };
                (curve, Duration::from_secs_f64(settle))
            }
            other => (other, duration),
        };
        if let Some(replaced) = &self.active {
            note_cancelled(replaced.on_complete);
        }
        self.active = Some(Box::new(ActiveAnimation {
            from: self.current.clone(),
            to,
            start: now,
            duration,
            curve,
            on_complete,
        }));
    }

    /// 0.5.4 (#138): how fast this value is moving toward `to` right now, as a
    /// fraction of the distance still to go, per second: `None` for a value
    /// that is not a number, one that is at rest, or one with nowhere to go.
    fn speed_toward(&self, to: &T, now: Instant) -> Option<f64> {
        let anim = self.active.as_ref()?;
        let (from, old_to, current, target) = (
            anim.from.scalar()?,
            anim.to.scalar()?,
            self.current.scalar()?,
            to.scalar()?,
        );
        let elapsed = now.saturating_duration_since(anim.start).as_secs_f64();
        let d = anim.duration.as_secs_f64();
        if d <= 0.0 || elapsed >= d {
            return None;
        }
        let per_second = (old_to - from) * anim.curve.slope(elapsed / d) / d;
        let distance = target - current;
        (distance.abs() > 1e-9).then(|| per_second / distance)
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

    /// M7 Phase 1 (§7.5): every curve -- here the MD3 values, as the
    /// beziers a framework passes -- must start at `y=0` and end at
    /// `y=1`, the boundary condition every CSS easing curve satisfies.
    #[test]
    fn every_motion_curve_satisfies_its_own_boundary_conditions() {
        for curve in [
            MotionCurve::Linear,
            MotionCurve::Bezier(0.2, 0.0, 0.0, 1.0),
            MotionCurve::Bezier(0.0, 0.0, 0.0, 1.0),
            MotionCurve::Bezier(0.3, 0.0, 1.0, 1.0),
            MotionCurve::Bezier(0.05, 0.7, 0.1, 1.0),
            MotionCurve::Bezier(0.3, 0.0, 0.8, 0.15),
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

    /// Forward-evaluates a point on each curve at a chosen `t` via
    /// `CubicSegment::eval`, then confirms `solve_y_for_x` recovers the
    /// same `y` from that `x` -- the solver checked against the curve's
    /// own forward math.
    #[test]
    fn cubic_segment_solve_y_for_x_round_trips_with_eval() {
        for segment in [
            CubicSegment::standard(0.2, 0.0, 0.0, 1.0),
            CubicSegment::standard(0.0, 0.0, 0.0, 1.0),
            CubicSegment::standard(0.3, 0.0, 1.0, 1.0),
            CubicSegment::standard(0.05, 0.7, 0.1, 1.0),
            CubicSegment::standard(0.3, 0.0, 0.8, 0.15),
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

    /// Every curve here is monotonically non-decreasing --
    /// sampled, not proven analytically, but at a fine enough
    /// resolution (200 points) to catch a real ordering bug (e.g. a
    /// wrong segment split, a transposed control point).
    #[test]
    fn every_motion_curve_is_monotonically_non_decreasing() {
        for curve in [
            MotionCurve::Linear,
            MotionCurve::Bezier(0.2, 0.0, 0.0, 1.0),
            MotionCurve::Bezier(0.0, 0.0, 0.0, 1.0),
            MotionCurve::Bezier(0.3, 0.0, 1.0, 1.0),
            MotionCurve::Bezier(0.05, 0.7, 0.1, 1.0),
            MotionCurve::Bezier(0.3, 0.0, 0.8, 0.15),
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

    // --- 0.5.4 (#138): springs.

    fn at(curve: MotionCurve, t: f64) -> f64 {
        curve.ease(t)
    }

    #[test]
    fn a_spring_starts_at_zero_and_arrives_at_one() {
        for bounce in [-0.5, 0.0, 0.3, 0.7] {
            let c = MotionCurve::spring_for(Duration::from_millis(400), bounce);
            assert!(at(c, 0.0).abs() < 1e-9);
            assert!(
                (at(c, 1.0) - 1.0).abs() < 0.0011,
                "bounce {bounce}: {}",
                at(c, 1.0)
            );
        }
    }

    #[test]
    fn a_bouncy_spring_overshoots_and_a_critically_damped_one_does_not() {
        let peak = |c: MotionCurve| {
            (0..=1000)
                .map(|i| at(c, f64::from(i) / 1000.0))
                .fold(f64::MIN, f64::max)
        };
        let bouncy = MotionCurve::spring_for(Duration::from_millis(400), 0.5);
        assert!(
            peak(bouncy) > 1.05,
            "rings past the target: {}",
            peak(bouncy)
        );
        for bounce in [0.0, -0.5] {
            let c = MotionCurve::spring_for(Duration::from_millis(400), bounce);
            assert!(
                peak(c) <= 1.0005,
                "bounce {bounce} never passes it: {}",
                peak(c)
            );
            // And it only ever moves forward.
            let mut last = 0.0;
            for i in 0..=1000 {
                let v = at(c, f64::from(i) / 1000.0);
                assert!(v >= last - 1e-9);
                last = v;
            }
        }
    }

    #[test]
    fn a_bouncier_spring_takes_longer_to_settle() {
        let settle = |bounce| match MotionCurve::spring_for(Duration::from_millis(400), bounce) {
            MotionCurve::Spring { settle, .. } => settle,
            _ => unreachable!(),
        };
        assert!(settle(0.7) > settle(0.3) && settle(0.3) > settle(0.0));
        assert!(settle(0.0) > 0.1 && settle(0.7) < 20.0);
    }

    #[test]
    fn a_spring_lasts_until_it_settles_whatever_duration_is_asked_for() {
        let mut value = Animated::new(0.0_f64);
        let t0 = Instant::now();
        value.animate_to(
            10.0,
            Duration::from_millis(1),
            MotionCurve::spring_for(Duration::from_millis(400), 0.3),
            t0,
        );
        assert!(
            value.tick(t0 + Duration::from_millis(50), &mut Vec::new()),
            "still going"
        );
        assert!(value.tick(t0 + Duration::from_millis(300), &mut Vec::new()));
        assert!(!value.tick(t0 + Duration::from_secs(30), &mut Vec::new()));
        assert_eq!(value.current, 10.0, "and lands exactly");
    }

    #[test]
    fn a_retarget_carries_the_speed_of_the_animation_it_interrupts() {
        let spring = MotionCurve::spring_for(Duration::from_millis(300), 0.2);
        let mut value = Animated::new(0.0_f64);
        let t0 = Instant::now();
        // 100 units over a second, linear: 100 a second.
        value.animate_to(100.0, Duration::from_secs(1), MotionCurve::Linear, t0);
        let t1 = t0 + Duration::from_millis(500);
        value.tick(t1, &mut Vec::new());
        let at_interrupt = value.current;
        assert!((at_interrupt - 50.0).abs() < 1e-6);
        // Retarget with a spring: it keeps moving at about 100 a second at first.
        value.animate_to(200.0, Duration::from_millis(1), spring, t1);
        // The spring accelerates hard, so look just after the hand-over.
        let dt = Duration::from_micros(200);
        value.tick(t1 + dt, &mut Vec::new());
        let speed = (value.current - at_interrupt) / dt.as_secs_f64();
        assert!(
            (speed - 100.0).abs() < 8.0,
            "continues at the old speed: {speed}"
        );

        // Without the carry (an animation that was at rest) it starts from zero speed.
        let mut rest = Animated::new(50.0_f64);
        rest.animate_to(200.0, Duration::from_millis(1), spring, t1);
        rest.tick(t1 + dt, &mut Vec::new());
        let from_rest = (rest.current - 50.0) / dt.as_secs_f64();
        assert!(
            from_rest < speed,
            "a spring from rest starts slower: {from_rest}"
        );
    }

    #[test]
    fn a_spring_works_on_values_that_are_not_numbers() {
        let mut color = Animated::new(peniko::Color::from_rgba8(0, 0, 0, 255));
        let t0 = Instant::now();
        color.animate_to(
            peniko::Color::from_rgba8(255, 0, 0, 255),
            Duration::from_millis(1),
            MotionCurve::spring_for(Duration::from_millis(300), 0.0),
            t0,
        );
        color.tick(t0 + Duration::from_millis(100), &mut Vec::new());
        assert!(color.current.to_rgba8().r > 0);
        assert!(!color.tick(t0 + Duration::from_secs(10), &mut Vec::new()));
    }
}
