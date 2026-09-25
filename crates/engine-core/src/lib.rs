//! Pure-Rust node tree, animation core, and the generic interfaces other
//! crates build on. No `pyo3`, no `winit`, MD3-agnostic. Does depend
//! directly on the plain `accesskit` data crate (§14 step 7, §4/§10's
//! own reasoning: `accesskit` is small and OS-agnostic, the same class
//! of dependency as `taffy`/`parley`, both already here).
//!
//! `BindingResolver` lives in `engine-spec` instead (§16.2, the one
//! capability only `engine-py` supplies there). `InputEvent` (§4) lands
//! here at M4 Phase 1 step 1, alongside `Tree::dispatch`/`hit_test`/
//! `update_hover`/`move_focus` -- the real dispatch core every M3
//! interaction-dependent step (7, 9, 11, 12, 13, 14, 15) deferred, each
//! exposing a direct `Tree` method instead and explicitly naming this
//! as the eventual real wiring. `input.rs`'s own doc comment has the
//! real correction: this step's original plan sketched a generic
//! `AppHandler` trait alongside `InputEvent` for the one meaning-
//! dependent hook `Tree::dispatch` can't resolve itself -- never
//! actually implemented anywhere; `engine-py::dispatch.rs`'s own
//! `HandlerMap`/`call_handler` is the real mechanism that shipped
//! instead, so the dead trait was removed.

mod access;
mod animation;
mod canvas;
mod dock;
mod input;
mod interaction;
mod node;
mod overlay;
mod path;
mod shape_morph;
mod tree;

pub use access::{AccessNodeData, AccessStates, AccessValue, Action, ActionData, Live, Role};
pub use animation::{ActiveAnimation, Animated, CompletionHandle, Interpolate, MotionCurve};
pub use canvas::{CanvasState, CustomHitTest, DrawCommand};
pub use dock::{DockLayout, DockSide, DockZone};
pub use input::{
    ChangedValue, DispatchOutcome, EventKind, InputEvent, Key, Modifiers, PointerButton,
    ScrollDelta,
};
pub use interaction::{InteractionState, RippleState};
pub use node::{
    CAROUSEL_DRAG_INDEX_THRESHOLD, CAROUSEL_GAP, CAROUSEL_HEIGHT, CAROUSEL_ITEM_RADIUS,
    CAROUSEL_MEDIUM, CAROUSEL_PAD_X, CAROUSEL_PAD_Y, CAROUSEL_SMALL_MAX,
    CAROUSEL_UNCONTAINED_WIDTH, CarouselLayout, CarouselState, CellColor, CheckboxState,
    CircularProgressState, ContentFit, CornerRadii, Cursor, ICON_VIEWBOX_SIZE, IconState,
    ImageState, ItemExtent, LinearProgressState, LoadingIndicatorState, Node, NodeId, NodeKind,
    NodeTransform, PaintProperties, RadioButtonState, SCROLLBAR_GRAB_SLOP, SCROLLBAR_MARGIN,
    SCROLLBAR_MIN_LENGTH, SCROLLBAR_THICKNESS, ScrollViewState, Shadow, Shadows, SliderState,
    SplitterState, SwitchState, TerminalCell, TerminalPalette, TerminalState, TextAlign,
    TextFieldState, TextOptions, TextState, TimePickerDialMode, TimePickerDialState,
    VirtualListState,
};
pub use overlay::OverlayMeta;
pub use path::{PathData, PathState, fit_transform, trim};
pub use shape_morph::ShapeKey;
pub use tree::{
    FocusDirection, InteractionConfig, Tree, from_access_id, node_id_as_u64, to_access_id,
};
