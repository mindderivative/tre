//! `tre.Gradient` -- linear/radial gradient definitions (Phase 12 Step
//! 12.4), registered per-`ShapeRegistry` via `PyShapeRegistry::
//! create_gradient` into a real `tre_engine::GradientId`. Mirrors
//! `tre_engine::ShapeRegistry::create_gradient`'s own real, already-
//! working API (Phase 10 Step 10.2.1) directly -- no new evaluation
//! mechanism, just a Python-facing constructor for the same
//! `GradientDef` it already accepts.

use pyo3::prelude::*;
use tre_engine::{GradientDef, GradientKind, GradientStop};

/// A real, evaluatable linear-or-radial gradient definition. Not itself
/// usable as a fill -- pass to `registry.create_gradient(...)` first to
/// get back a real [`PyGradientId`] scoped to that registry.
#[pyclass(name = "Gradient", frozen)]
#[derive(Clone)]
pub struct PyGradient {
    pub(crate) def: GradientDef,
}

#[pymethods]
impl PyGradient {
    /// `stops`: a list of `(position, color)` pairs. `position` must be
    /// `0.0..=1.0`, non-decreasing across the list -- validated for real
    /// by `create_gradient` itself, not here.
    #[staticmethod]
    fn linear(start: (f32, f32), end: (f32, f32), stops: Vec<(f32, u32)>) -> Self {
        Self {
            def: GradientDef {
                kind: GradientKind::Linear {
                    start: [start.0, start.1],
                    end: [end.0, end.1],
                },
                stops: stops
                    .into_iter()
                    .map(|(position, color)| GradientStop { position, color })
                    .collect(),
            },
        }
    }

    /// See [`PyGradient::linear`] for `stops`' own contract.
    #[staticmethod]
    fn radial(center: (f32, f32), radius: f32, stops: Vec<(f32, u32)>) -> Self {
        Self {
            def: GradientDef {
                kind: GradientKind::Radial {
                    center: [center.0, center.1],
                    radius,
                },
                stops: stops
                    .into_iter()
                    .map(|(position, color)| GradientStop { position, color })
                    .collect(),
            },
        }
    }
}

/// A stable handle to a gradient already registered on one specific
/// `ShapeRegistry` -- scoped to that registry, exactly like the real
/// `tre_engine::GradientId` it wraps: passing one to a *different*
/// registry's `insert_*` call is a real, foreign/stale handle, and
/// panics at that registry's own `flatten_into` time with a clear
/// message (the engine's own existing, documented contract for this
/// exact mistake -- not re-validated here).
#[pyclass(name = "GradientId", frozen)]
#[derive(Clone, Copy)]
pub struct PyGradientId(pub tre_engine::GradientId);

#[pymethods]
impl PyGradientId {
    fn __repr__(&self) -> String {
        format!("{:?}", self.0)
    }
}
