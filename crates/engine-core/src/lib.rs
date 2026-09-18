//! Pure-Rust node tree, animation core, and the generic interfaces other
//! crates build on. No `pyo3`, no `winit`, MD3-agnostic. Does depend
//! directly on the plain `accesskit` data crate (§14 step 7, §4/§10's
//! own reasoning: `accesskit` is small and OS-agnostic, the same class
//! of dependency as `taffy`/`parley`, both already here).
//!
//! `BindingResolver` lives in `engine-spec` instead (§16.2, the one
//! capability only `engine-py` supplies there). `AppHandler`/
//! `InputEvent` (§4) land here at M4 Phase 1 step 1, alongside
//! `Tree::dispatch`/`hit_test`/`update_hover`/`move_focus` -- the real
//! dispatch core every M3 interaction-dependent step (7, 9, 11, 12, 13,
//! 14, 15) deferred, each exposing a direct `Tree` method instead and
//! explicitly naming this as the eventual real wiring.

mod access;
mod animation;
mod canvas;
mod dock;
mod input;
mod interaction;
mod node;
mod overlay;
mod shape_morph;
mod tree;

pub use access::{AccessNodeData, AccessStates, Action, Role};
pub use animation::{ActiveAnimation, Animated, CompletionHandle, Interpolate, MotionCurve};
pub use canvas::{CanvasState, CustomHitTest, DrawCommand};
pub use dock::{DockLayout, DockSide, DockZone};
pub use input::{
    AppHandler, DispatchOutcome, EventKind, InputEvent, Key, PointerButton, ScrollDelta,
};
pub use interaction::{InteractionState, RippleState};
pub use node::{
    CheckboxState, ContentFit, ICON_VIEWBOX_SIZE, IconState, ImageState, ItemExtent, Node, NodeId,
    NodeKind, PaintProperties, RadioButtonState, SliderState, SplitterState, SwitchState,
    TextAlign, TextFieldState, TextState, VirtualListState,
};
pub use overlay::OverlayMeta;
pub use shape_morph::ShapeKey;
pub use tree::{
    FocusDirection, InteractionConfig, Tree, from_access_id, node_id_as_u64, to_access_id,
};
