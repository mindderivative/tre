//! §8's `EngineError` -- one error type funneling every `engine-py`
//! failure into a `PyErr`, rather than ad hoc `PyErr::new_err` scattered
//! per call site. Narrower than §8's own full sketch: no
//! `CycleRejected` yet, since `add_child` (the only thing that could
//! ever produce it) isn't in this step's scope -- §14 step 6 is
//! "node creation + one property setter," not tree mutation in general.

use pyo3::PyErr;
use pyo3::exceptions::{PyTypeError, PyValueError};

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
}

impl From<EngineError> for PyErr {
    fn from(e: EngineError) -> PyErr {
        match e {
            EngineError::UnknownProperty { .. } => PyValueError::new_err(e.to_string()),
            EngineError::TypeMismatch { .. } => PyTypeError::new_err(e.to_string()),
            EngineError::NotAVirtualList => PyValueError::new_err(e.to_string()),
        }
    }
}
