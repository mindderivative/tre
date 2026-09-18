//! §8's `EngineError` -- one error type funneling every `engine-py`
//! failure into a `PyErr`, rather than ad hoc `PyErr::new_err` scattered
//! per call site. `CycleRejected` (M6 Phase 1) is the first real use of
//! §8's own original sketch -- `add_child` (the only thing that could
//! ever produce it) wasn't in scope until now.

use pyo3::PyErr;
use pyo3::exceptions::{PyIOError, PyTypeError, PyValueError};

#[derive(thiserror::Error, Debug)]
pub enum EngineError {
    #[error("{kind} has no property '{property}'")]
    UnknownProperty {
        kind: &'static str,
        property: String,
    },
    #[error("property '{property}' expects {expected}, got {actual}")]
    TypeMismatch {
        property: String,
        expected: &'static str,
        actual: String,
    },
    /// §14 step 15 (§11.7): `Window.set_virtual_list_window` was called
    /// on a `Node` that either isn't a `VirtualList` at all, or is one
    /// this particular `Window` didn't create (so it has no recorded
    /// materializer callback for it).
    #[error("this Node is not a VirtualList added via Window.add_virtual_list on this Window")]
    NotAVirtualList,
    /// M5 Phase 3 (§11.10/§11.11): `Window.redraw_canvas` was called on
    /// a `Node` that either isn't a `Canvas` at all, or is one this
    /// particular `Window` didn't create (so it has no recorded `draw`
    /// callback for it) -- the exact same shape as `NotAVirtualList`.
    #[error("this Node is not a Canvas added via Window.add_canvas on this Window")]
    NotACanvas,
    /// M6 Phase 1 (§8): `Node.add_child` would attach a node as a child
    /// of its own descendant -- `Tree::try_add_child` rejected it rather
    /// than corrupting the tree into a cycle. Message verbatim from
    /// §8's own original sketch.
    #[error("cannot add a node as a child of its own descendant")]
    CycleRejected,
    /// M6 Phase 1 (§8): `Node.add_child` was called with a `child` from
    /// a different `Window`'s `Tree` -- rejected via `Rc::ptr_eq` before
    /// either `Tree` is touched, since a `NodeId` is only unique within
    /// the `Tree` that minted it (a `slotmap` generational key, not a
    /// cross-map identity) and handing a foreign one to `taffy` risks
    /// real corruption, not just a wrong result.
    #[error("this Node belongs to a different Window's Tree")]
    ForeignNode,
    /// M22 Phase 1 (§5): `Window.add_image` couldn't read or decode
    /// the file at `path` -- a real I/O/format failure, not a value or
    /// type mismatch the way the two variants above represent, so this
    /// maps to `PyIOError` rather than `PyValueError`/`PyTypeError`.
    #[error("failed to load image '{path}': {reason}")]
    ImageLoadFailed { path: String, reason: String },
}

impl From<EngineError> for PyErr {
    fn from(e: EngineError) -> PyErr {
        match e {
            EngineError::UnknownProperty { .. }
            | EngineError::NotAVirtualList
            | EngineError::NotACanvas
            | EngineError::CycleRejected
            | EngineError::ForeignNode => PyValueError::new_err(e.to_string()),
            EngineError::TypeMismatch { .. } => PyTypeError::new_err(e.to_string()),
            EngineError::ImageLoadFailed { .. } => PyIOError::new_err(e.to_string()),
        }
    }
}
