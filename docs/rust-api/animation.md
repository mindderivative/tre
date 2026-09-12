# Animation

Two small crates, layered: `tre-tween` is a pure, stateless interpolation math layer, and `tre-animation` is the real sequencer built on top of it. `tre-python`'s own `Tween`/`Timeline`/`Spring` bindings (see the [Python API](../python-api/animation.md)) wrap these directly.

## `tre-tween`

Depends on `glam` for its `Vec2` lerp, additive to (not a replacement of) `tre-math`'s own `Affine2`/SIMD-batch functions already proven in `tre-engine`'s hot flatten path.

### `Easing`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Easing {
    #[default]
    Linear,
    EaseInQuad, EaseOutQuad, EaseInOutQuad,
    EaseInCubic, EaseOutCubic, EaseInOutCubic,
    EaseInQuart, EaseOutQuart, EaseInOutQuart,
    EaseInQuint, EaseOutQuint, EaseInOutQuint,
}

impl Easing {
    pub fn apply(self, t: f32) -> f32;
}
```

A named easing curve -- lets a caller (especially `tre-python`, which cannot pass a raw Rust function pointer across the PyO3 boundary as a first-class value) select a curve by a plain `Copy` value. Each variant dispatches to a corresponding free function in `tre_tween::easing` (the standard Robert Penner set, e.g. `ease_in_out_cubic(t: f32) -> f32`), all pure `f32 -> f32` functions on `t in [0.0, 1.0]` that **extrapolate rather than clamp** out-of-range `t` -- occasionally useful for a deliberate overshoot, never silently wrong.

### `Lerp` / `Tween<T>`

```rust
pub trait Lerp: Copy {
    fn lerp(self, other: Self, t: f32) -> Self;
}
// implemented for f32 and glam::Vec2

pub struct Tween<T: Lerp> {
    pub from: T,
    pub to: T,
    pub duration: f32,
    pub easing: Easing,
}

impl<T: Lerp> Tween<T> {
    pub fn new(from: T, to: T, duration: f32, easing: Easing) -> Self;
    pub fn sample(&self, elapsed: f32) -> T;
}
```

`sample` clamps `elapsed` into `[0.0, duration]` before computing progress -- unlike a raw `Easing::apply` call (which extrapolates), a tween's own contract is "reaches and holds `to`," not "keeps moving past it." A zero or negative `duration` is special-cased to progress `1.0` (immediately `to`).

### `Spring`

```rust
pub struct Spring {
    pub stiffness: f32,  // k -- higher pulls toward target faster
    pub damping: f32,    // c -- higher settles faster, reduces overshoot
    pub mass: f32,       // m -- higher responds more sluggishly
    // position, velocity: private
}

impl Spring {
    pub fn new(stiffness: f32, damping: f32, mass: f32, initial_position: f32) -> Self; // debug_assert!(mass > 0.0)
    pub fn position(&self) -> f32;
    pub fn velocity(&self) -> f32;
    pub fn update(&mut self, target: f32, dt: f32) -> f32;
}
```

A **real** damped mass-spring-damper second-order ODE, integrated with semi-implicit ("symplectic") Euler each step -- unconditionally stable for a damped oscillator at reasonable frame-rate step sizes, unlike plain (explicit) Euler. This is the deliberate contrast with `tre-math::spring_decay` (see [Math & Memory](math-and-memory.md)): `Spring` genuinely overshoots and oscillates when underdamped, the "bouncy" motion a UI spring animation is expected to have, which `spring_decay`'s plain exponential smoothing cannot produce by design.

## `tre-animation`

Deliberately depends on nothing PyO3-related -- `tre-python` depends on this crate, not the reverse. `Timeline` identifies its animation targets by an opaque `u64` (mirroring `tre_engine::shapes::AnimationId`'s own pattern: "the registry/caller knows what the id means, the sequencer doesn't have to"). `tre-python`'s binding maps each `u64` back to a real Python object + attribute name and applies the sampled value via `setattr`.

```rust
pub struct Timeline { /* private: entries, elapsed */ }

impl Timeline {
    pub fn new() -> Self;
    pub fn animate(&mut self, target: u64, tween: Tween<f32>);
    pub fn elapsed(&self) -> f32;
    pub fn advance(&mut self, dt: f32) -> (Vec<(u64, f32)>, bool);
    pub fn prune_finished(&mut self) -> Vec<u64>;
    pub fn active_count(&self) -> usize;
}

pub use tre_tween::Tween as TimelineTween;
pub fn tween(from: f32, to: f32, duration: f32, easing: Easing) -> Tween<f32>;
```

- **`animate`** schedules `tween` to start now, against `target`. Multiple tweens can target the same id (e.g. animating two different properties of the same shape) -- each is sampled and reported independently.
- **`advance`** moves the timeline's own clock forward by `dt` and samples every scheduled entry, returning `(target, sampled_value)` pairs -- including already-finished entries, which keep reporting their final `to` value (matching `Tween::sample`'s clamp-past-duration contract) -- plus whether at least one entry is still running.
- **`prune_finished`** drops every entry that has finished as of the timeline's current elapsed time, returning the `target` id of each one removed, so a caller keeping its own side-table (like `tre-python`'s `PyTimeline`) can prune it in lockstep.

Note: `Easing` itself is **not** re-exported from `tre_animation` -- it must come from `tre_tween` directly, even though `TimelineTween` (an alias for `tre_tween::Tween`) is.
