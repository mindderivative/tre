//! Maps `tre_engine::EngineError` to a real Python exception
//! (IMPLEMENTATION.md Phase 10 Step 10.4 task 2's "Python exceptions
//! raised via a `From<EngineError> for PyErr` impl"). One exception type
//! for the whole domain, carrying the error's own `Display` text (added
//! to `tre_engine::EngineError` alongside this crate, since it was the
//! only error type in the workspace without one) -- matching how every
//! other error type in this workspace (`SvgError`, `TextError`,
//! `GradientError`) is surfaced, rather than inventing one exception
//! class per `EngineError` variant for a distinction no real caller has
//! asked to catch separately yet.

use pyo3::exceptions::PyException;
use pyo3::{create_exception, PyErr};
use tre_engine::EngineError;

create_exception!(
    tre_python,
    TreError,
    PyException,
    "A recoverable engine failure (GPU device loss, pipeline creation \
     failure, resource exhaustion, ...) -- never raised for a programmer \
     error, which panics instead, matching the Rust engine's own \
     DESIGN.md Section 2.6 policy."
);

/// Converts a real `EngineError` into a real `TreError` Python exception.
#[must_use]
pub fn engine_err(e: EngineError) -> PyErr {
    TreError::new_err(e.to_string())
}
