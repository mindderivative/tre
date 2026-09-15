//! Pure-Rust node tree, animation core, and the generic interfaces other
//! crates build on. No `pyo3`, no `winit`, MD3-agnostic.
//!
//! `AppHandler`/`InputEvent`/`BindingResolver` land at later build-order
//! steps (ARCHITECTURE.md §14) -- this crate currently holds the
//! animation core (§14 step 2) and the `Node`/`Tree`/`taffy` layout core
//! (§14 step 3), each built and tested standalone first per Design
//! Principle 5, then proven to compose with `engine-render`.

mod animation;
mod node;
mod tree;

pub use animation::{ActiveAnimation, Animated, CompletionHandle, Interpolate, MotionCurve};
pub use node::{Node, NodeId, NodeKind, PaintProperties, TextState};
pub use tree::Tree;
