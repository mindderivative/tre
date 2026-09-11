//! `tre-tween` (Phase 13 Step 13.2): the pure, stateless tweening math
//! layer `tre-animation` (Step 13.3) sequences on top of. Provides:
//!
//! - [`easing::Easing`] + its free-function curves (the standard
//!   Robert Penner set).
//! - [`tween::Tween`], generic over anything [`tween::Lerp`] (`f32`,
//!   `glam::Vec2` today).
//! - [`spring::Spring`], a real damped mass-spring-damper integrator --
//!   distinct from `tre_math::spring_decay`'s plain exponential
//!   smoothing (see its own module doc comment for the real difference:
//!   this one can overshoot and oscillate, `spring_decay` never does).
//!
//! Depends on `glam` (Phase 13 Step 13.2's own math-library evaluation,
//! Q9) for its `Vec2` lerp -- additive to, not a replacement of,
//! `tre-math`'s own `Affine2`/SIMD-batch functions already proven in
//! `tre-engine`'s hot flatten path.

pub mod easing;
pub mod spring;
pub mod tween;

pub use easing::Easing;
pub use spring::Spring;
pub use tween::{Lerp, Tween};
