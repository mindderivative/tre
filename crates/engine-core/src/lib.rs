//! Pure-Rust node tree, animation core, and the generic interfaces other
//! crates build on. No `pyo3`, no `winit`, no design system. Does depend
//! directly on the plain `accesskit` data crate (§14 step 7, §4/§10's
//! own reasoning: `accesskit` is small and OS-agnostic, the same class
//! of dependency as `taffy`/`parley`, both already here).
//!
//! `InputEvent` (§4) lands
//! here at M4 Phase 1 step 1, alongside `Tree::dispatch`/`hit_test`/
//! `update_hover`/`move_focus` -- the real dispatch core every M3
//! interaction-dependent step (7, 9, 11, 12, 13, 14, 15) deferred, each
//! exposing a direct `Tree` method instead and explicitly naming this
//! as the eventual real wiring. `input.rs`'s own doc comment has the
//! real correction: this step's original plan sketched a generic
//! `AppHandler` trait alongside `InputEvent` for the one meaning-
//! dependent hook `Tree::dispatch` can't resolve itself -- never
//! actually implemented anywhere; `engine-py::dispatch.rs`'s own
//! `HandlerMap` and `listeners.rs` delivery are the real mechanism
//! instead, so the dead trait was removed.

mod access;
mod animation;
mod canvas;
mod dock;
mod gesture;
mod gradient;
mod input;
mod node;
mod overlay;
mod path;
mod shader;
mod svg;
mod tree;

pub use access::{AccessNodeData, AccessStates, AccessValue, Action, ActionData, Live, Role};
pub use animation::{ActiveAnimation, Animated, CompletionHandle, Interpolate, MotionCurve};
pub use canvas::{CanvasState, CustomHitTest, DrawCommand};
pub use dock::{DockLayout, DockSide, DockZone};
pub use gesture::{
    Gesture, GestureConfig, GestureKind, GesturePhase, GestureRecognizer, TouchPhase,
};
pub use gradient::{Gradient, GradientShape, GradientStop};
pub use input::{
    ChangedValue, DispatchOutcome, InputEvent, Key, Modifiers, PointerButton, ScrollDelta,
    ctrl_shortcut,
};
pub use node::{
    Blend, CellColor, ContentFit, CornerRadii, Cursor, ImageState, ItemExtent, Node, NodeId,
    NodeKind, NodeTransform, PaintProperties, SCROLLBAR_GRAB_SLOP, SCROLLBAR_MARGIN,
    SCROLLBAR_MIN_LENGTH, SCROLLBAR_THICKNESS, ScrollViewState, Shadow, Shadows, TerminalCell,
    TerminalPalette, TerminalState, TextAlign, TextFieldState, TextOptions, TextSpan, TextState,
    VirtualListState, WindowRegion,
};
pub use overlay::{OverlayMeta, Placement};
pub use path::{PathData, PathState, fit_transform, trim};
pub use shader::{
    FRAME_BLOCK_SIZE, Shader, ShaderError, ShaderMode, UniformKind, UniformValue, frame_block,
};
pub use svg::{
    SvgBitmap, SvgClip, SvgDocument, SvgFill, SvgFonts, SvgGroup, SvgImage, SvgImages, SvgMask,
    SvgNode, SvgPaint, SvgPath, SvgPattern, SvgShadow, SvgState, SvgStroke,
};
pub use tree::{
    FocusDirection, KEY_SCROLL_LINE, TextPart, Touched, Tree, from_access_id, node_id_as_u64,
    to_access_id,
};
