//! `tre.CustomShaderId` (Phase 13 Step 13.8: custom shader API, Q13) --
//! a stable handle to a real pipeline a renderer's own `create_custom_
//! shader(fragment_source)` call compiled and registered, scoped to
//! that specific renderer's own `PipelineRegistry` (mirrors `PyGradientId`'s
//! own real "scoped to the registry that created it" pattern).

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use tre_engine::EngineError;

use crate::error::engine_err;

/// A real, renderer-scoped handle to a compiled custom shader pipeline.
/// Pass this to `ShapeRegistry.insert_custom_shaded(...)` to render a
/// shape through it -- using it against a DIFFERENT renderer than the
/// one that created it is a real, disclosed caller error (that
/// renderer's own `PipelineRegistry` never registered this id).
#[pyclass(name = "CustomShaderId", frozen)]
#[derive(Clone, Copy)]
pub struct PyCustomShaderId(pub u16);

#[pymethods]
impl PyCustomShaderId {
    fn __repr__(&self) -> String {
        format!("CustomShaderId({})", self.0)
    }
}

/// Maps a real `EngineError` from custom shader compilation/pipeline
/// creation to the matching Python exception -- `ValueError` for a real
/// caller mistake (the fragment shader source itself failed to
/// compile, carrying `shaderc`'s own real diagnostic), `TreError` for
/// every other, genuinely internal failure.
pub(crate) fn custom_shader_err(e: EngineError) -> PyErr {
    match e {
        EngineError::ShaderCompilationFailed(_) => PyValueError::new_err(e.to_string()),
        other => engine_err(other),
    }
}
