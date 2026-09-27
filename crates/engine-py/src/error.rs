//! §8's `EngineError` -- one error type funneling every `engine-py`
//! failure into a `PyErr`, rather than ad hoc `PyErr::new_err` scattered
//! per call site. `CycleRejected` (M6 Phase 1) is the first real use of
//! §8's own original sketch -- `add_child` (the only thing that could
//! ever produce it) wasn't in scope until now.

use pyo3::PyErr;
use pyo3::exceptions::{PyIOError, PyTypeError, PyValueError};

#[derive(thiserror::Error, Debug)]
pub enum EngineError {
    #[error("property '{property}' expects {expected}, got {actual}")]
    TypeMismatch {
        property: String,
        expected: &'static str,
        actual: String,
    },
    /// `Node.redraw()` on a node that isn't a canvas.
    #[error("redraw() applies only to a canvas node")]
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
    /// M96: the node behind this handle was destroyed (`Node.destroy()`,
    /// or an ancestor's).
    #[error("this Node was destroyed")]
    Destroyed,
    /// `Window.create("terminal", ...)` couldn't open a PTY or spawn
    /// `shell` on it -- an OS failure, not a bad value, so it maps to
    /// `PyIOError`.
    #[error("failed to start terminal shell '{shell}': {reason}")]
    TerminalSpawnFailed { shell: String, reason: String },
}

impl From<EngineError> for PyErr {
    fn from(e: EngineError) -> PyErr {
        match e {
            EngineError::NotACanvas
            | EngineError::CycleRejected
            | EngineError::ForeignNode
            | EngineError::Destroyed => PyValueError::new_err(e.to_string()),
            EngineError::TypeMismatch { .. } => PyTypeError::new_err(e.to_string()),
            EngineError::TerminalSpawnFailed { .. } => PyIOError::new_err(e.to_string()),
        }
    }
}
