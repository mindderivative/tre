//! Pure-Rust node tree, animation core, and the generic interfaces other
//! crates build on. No `pyo3`, no `winit`, MD3-agnostic.
//!
//! `Node`/`Tree`/`NodeId`/`AppHandler` land at build-order step 3 and
//! after (ARCHITECTURE.md §14) -- this crate currently holds only the
//! animation core (§14 step 2), built and tested standalone per Design
//! Principle 5.

mod animation;

pub use animation::{ActiveAnimation, Animated, CompletionHandle, Interpolate, MotionCurve};
