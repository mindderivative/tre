//! Pure-Rust node tree, animation core, and the generic interfaces other
//! crates build on. No `pyo3`, no `winit`, MD3-agnostic. Does depend
//! directly on the plain `accesskit` data crate (§14 step 7, §4/§10's
//! own reasoning: `accesskit` is small and OS-agnostic, the same class
//! of dependency as `taffy`/`parley`, both already here).
//!
//! `AppHandler`/`InputEvent`/`BindingResolver` land at later build-order
//! steps (ARCHITECTURE.md §14) -- this crate currently holds the
//! animation core (§14 step 2), the `Node`/`Tree`/`taffy` layout core
//! (§14 step 3), and `Tree::build_access_update` (§14 step 7), each
//! built and tested standalone first per Design Principle 5, then
//! proven to compose with `engine-render`/`engine-platform`.

mod access;
mod animation;
mod interaction;
mod node;
mod overlay;
mod tree;

pub use access::{AccessNodeData, AccessStates, Action, Role};
pub use animation::{ActiveAnimation, Animated, CompletionHandle, Interpolate, MotionCurve};
pub use interaction::{InteractionState, RippleState};
pub use node::{Node, NodeId, NodeKind, PaintProperties, SplitterState, TextState};
pub use overlay::OverlayMeta;
pub use tree::Tree;
