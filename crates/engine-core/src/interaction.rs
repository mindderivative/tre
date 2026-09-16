//! §7.3's mechanical interaction-state model: ripple/hover/focus state
//! layers, ticked by the same central tick every other `Animated<T>`
//! uses (§14 step 9).
//!
//! Deliberately narrower than §7.3's full picture. Its text describes
//! `hover_opacity` falling out of hit-testing (§11.10, "run every
//! pointer-move") and `RippleState`s spawned from real pointer-press
//! events routed through `AppHandler`/`InputEvent` (§4) -- checked
//! directly before writing anything here: neither exists anywhere in
//! this codebase yet, only forward-reference comments. This module
//! builds and proves the *animation* half standalone -- a ripple can be
//! spawned directly, ticks its radius/opacity correctly, and prunes
//! itself once finished -- deferring "wire this to a real press/hover
//! source" to whichever later build-order step first lands real pointer
//! input, the same scope narrowing step 7 applied to keyboard dispatch.
//!
//! Also deliberately skips building a completion-queue for pruning
//! finished ripples, despite §7.3's text framing removal as reusing
//! "the same completion-queue mechanism §5 already defines for
//! `on_complete`". That queue (`CompletionHandle`, still just an unused
//! struct field -- checked directly, no drain exists anywhere) is
//! `engine-py`'s future mechanism for handing a finished animation to a
//! *Python-visible* callback. Nothing about ripple pruning is
//! Python-visible; checking each `RippleState`'s own `Animated::tick`
//! return value directly is the same underlying fact that queue would
//! ultimately be built from, without manufacturing unused machinery
//! ahead of a caller that needs it.

use std::time::{Duration, Instant};

use peniko::Color;
use peniko::kurbo::Point;
use smallvec::SmallVec;

use crate::animation::{Animated, MotionCurve};

/// One expanding-and-fading ripple, matching §7.3's own struct sketch.
pub struct RippleState {
    pub origin: Point,
    pub radius: Animated<f64>,
    pub opacity: Animated<f64>,
}

impl RippleState {
    /// A ripple animating outward to `target_radius` while fading from
    /// `start_opacity` to `0.0`, both over `duration` -- a single
    /// combined press+release approximation. §7.3's real model expects
    /// separate press (radius grows, opacity holds) and release
    /// (opacity fades) phases driven by real pointer-down/pointer-up
    /// events; collapsing both into one animation is what a caller with
    /// only a single "trigger" moment (this step's own spike/tests, not
    /// a real dispatch source) can actually drive today.
    pub fn new(
        origin: Point,
        target_radius: f64,
        start_opacity: f64,
        duration: Duration,
        now: Instant,
    ) -> Self {
        let mut radius = Animated::new(0.0);
        radius.animate_to(target_radius, duration, MotionCurve::Linear, now);
        let mut opacity = Animated::new(start_opacity);
        opacity.animate_to(0.0, duration, MotionCurve::Linear, now);
        Self {
            origin,
            radius,
            opacity,
        }
    }

    /// Advances both animations; `true` while either is still running.
    fn tick(&mut self, now: Instant) -> bool {
        let radius_active = self.radius.tick(now);
        let opacity_active = self.opacity.tick(now);
        radius_active || opacity_active
    }
}

/// Mechanical interaction state (§2 Principle 6: a geometric/positional
/// fact the engine can detect and animate entirely on its own, zero app
/// involvement). Lives on `Node` as `Option<InteractionState>` (§7.3) --
/// only a node that actually opts into pointer interaction carries the
/// cost; every other node's field stays `None`.
pub struct InteractionState {
    pub ripples: SmallVec<[RippleState; 4]>,
    pub hover_opacity: Animated<f64>,
    pub focus_ring: Animated<f64>,
    /// M7 Phase 3 (§7.1/§7.3): the state layer's own paint color --
    /// real MD3 uses the "on-surface" scheme role, but this field is
    /// plain, already-resolved data (Design Principle 6), same as
    /// `PaintProperties.background`; `engine-render` cannot depend on
    /// `engine-md3` (§4), so whatever real color this holds has to be
    /// resolved and pushed in by the app layer (`engine-py::Window.
    /// set_theme`/`Node.enable_interaction`), never computed here.
    /// Defaults to real black -- byte-for-byte the hardcoded value
    /// `engine-render`'s ripple/hover paint used before this phase, so
    /// a node that opts into interaction with no theme ever set sees
    /// zero behavior change.
    pub tint: Color,
}

impl InteractionState {
    pub fn new() -> Self {
        Self {
            ripples: SmallVec::new(),
            hover_opacity: Animated::new(0.0),
            focus_ring: Animated::new(0.0),
            tint: Color::from_rgba8(0, 0, 0, 255),
        }
    }

    /// Appends a new ripple. Real callers spawn one per pointer-down
    /// once a real dispatch source exists (see this module's own doc
    /// comment); this step's own tests/spike call it directly.
    pub fn spawn_ripple(
        &mut self,
        origin: Point,
        target_radius: f64,
        start_opacity: f64,
        duration: Duration,
        now: Instant,
    ) {
        self.ripples.push(RippleState::new(
            origin,
            target_radius,
            start_opacity,
            duration,
            now,
        ));
    }

    /// The central tick's per-node interaction half (§5/§7.3): advances
    /// hover/focus and every ripple, pruning any ripple whose radius
    /// and opacity have both finished animating -- a finished ripple is
    /// fully determined by its own `Animated` fields, so once neither is
    /// still animating it can never visibly change again. Returns `true`
    /// if anything here is still mid-animation (stays dirty another
    /// frame), matching `PaintProperties::tick`'s own contract.
    pub fn tick(&mut self, now: Instant) -> bool {
        let hover_active = self.hover_opacity.tick(now);
        let focus_active = self.focus_ring.tick(now);
        self.ripples.retain(|ripple| ripple.tick(now));
        hover_active || focus_active || !self.ripples.is_empty()
    }
}

impl Default for InteractionState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spawned_ripple_animates_radius_out_and_opacity_to_zero() {
        let start = Instant::now();
        let mut state = InteractionState::new();
        state.spawn_ripple(
            Point::new(10.0, 10.0),
            100.0,
            0.5,
            Duration::from_millis(200),
            start,
        );

        let still_active = state.tick(start + Duration::from_millis(100));
        assert!(still_active, "ripple should still be mid-animation at 50%");
        let ripple = &state.ripples[0];
        assert!(
            (ripple.radius.current - 50.0).abs() < 0.01,
            "radius should be ~halfway to 100.0, got {}",
            ripple.radius.current
        );
        assert!(
            (ripple.opacity.current - 0.25).abs() < 0.01,
            "opacity should be ~halfway from 0.5 to 0.0, got {}",
            ripple.opacity.current
        );
    }

    #[test]
    fn finished_ripple_is_pruned_from_the_collection() {
        let start = Instant::now();
        let mut state = InteractionState::new();
        state.spawn_ripple(
            Point::new(0.0, 0.0),
            50.0,
            1.0,
            Duration::from_millis(100),
            start,
        );
        assert_eq!(state.ripples.len(), 1);

        // Past the ripple's own duration: both radius and opacity have
        // finished, so tick() must prune it, not just report inactive.
        let still_active = state.tick(start + Duration::from_millis(500));
        assert!(!still_active);
        assert!(
            state.ripples.is_empty(),
            "a fully-finished ripple must be removed, not left inert in the collection"
        );
    }

    #[test]
    fn hover_and_focus_alone_drive_the_active_flag_with_no_ripples() {
        let start = Instant::now();
        let mut state = InteractionState::new();
        state
            .hover_opacity
            .animate_to(1.0, Duration::from_millis(100), MotionCurve::Linear, start);

        assert!(state.tick(start + Duration::from_millis(50)));
        assert!(!state.tick(start + Duration::from_millis(200)));
    }

    #[test]
    fn new_state_defaults_to_real_black_tint_matching_the_old_hardcoded_value() {
        let state = InteractionState::new();
        assert_eq!(
            state.tint,
            Color::from_rgba8(0, 0, 0, 255),
            "a node that opts into interaction with no theme ever set must see the exact same \
             plain-black ripple/hover color `engine-render` hardcoded before M7 Phase 3, not \
             some other default"
        );
    }
}
