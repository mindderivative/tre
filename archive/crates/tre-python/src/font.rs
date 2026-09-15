//! `tre.Font` -- real, caller-loaded font bytes, validated eagerly
//! against both real consumers a `Text` shape will need (Phase 12
//! Step 12.3).
//!
//! Deliberately NOT tied to any one `ShapeRegistry`'s own internal
//! `tre_engine::FontRegistry`/`FontId`: a `Font` is a plain, reusable
//! Python value a caller can pass to any number of `Text` shapes across
//! any number of registries. `PyShapeRegistry::insert_text` resolves
//! each distinct `Font` (by its own `uid`, assigned once at construction
//! -- never by re-hashing its bytes on every call) into a real `FontId`
//! scoped to itself, lazily, the first time that `Font` is actually
//! used.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

use crate::error::TreError;
use crate::renderer::setup_err;

static NEXT_FONT_UID: AtomicU64 = AtomicU64::new(0);

/// Real font bytes, PyO3-visible as `tre.Font`. Construct via
/// [`PyFont::load`]/[`PyFont::load_bytes`]/[`PyFont::system_cascade`] --
/// there is no `#[new]`/default constructor, since there is no sensible
/// default font.
#[pyclass(name = "Font", frozen)]
pub struct PyFont {
    pub(crate) uid: u64,
    pub(crate) bytes: Arc<Vec<u8>>,
}

impl PyFont {
    /// Validates `bytes` against both `skrifa` (outline/metrics) and
    /// `rustybuzz` (shaping) before ever handing back a `Font` object --
    /// the same real validation `tre_engine::FontRegistry::load_bytes`
    /// itself performs, done here too so a caller gets a clean `Font`
    /// object or a clean error, never a `Font` that only fails later,
    /// deep inside a future `insert_text` call.
    fn from_bytes(bytes: Vec<u8>) -> PyResult<Self> {
        skrifa::FontRef::new(&bytes)
            .map_err(|_| PyValueError::new_err("font bytes rejected by skrifa (invalid font)"))?;
        if rustybuzz::Face::from_slice(&bytes, 0).is_none() {
            return Err(PyValueError::new_err(
                "font bytes rejected by rustybuzz (invalid font)",
            ));
        }
        Ok(Self {
            uid: NEXT_FONT_UID.fetch_add(1, Ordering::Relaxed),
            bytes: Arc::new(bytes),
        })
    }
}

#[pymethods]
impl PyFont {
    /// Loads a font file from disk.
    ///
    /// # Errors
    /// Raises `RuntimeError` if `path` can't be read, `ValueError` if
    /// its contents aren't a valid font.
    #[staticmethod]
    fn load(path: &str) -> PyResult<Self> {
        let bytes = std::fs::read(path).map_err(setup_err)?;
        Self::from_bytes(bytes)
    }

    /// Loads a font from already-in-memory bytes (e.g. read from a zip
    /// archive, or embedded as a Python resource).
    ///
    /// # Errors
    /// Raises `ValueError` if `data` isn't a valid font.
    #[staticmethod]
    fn load_bytes(data: Vec<u8>) -> PyResult<Self> {
        Self::from_bytes(data)
    }

    /// The primary font from the real system fontconfig cascade
    /// (`tre_text::FontCascade::discover`) -- the same real discovery
    /// mechanism `canvas_draw_text_demo.rs` already proves works, so a
    /// caller with no particular font in mind still gets crisp, real
    /// system text rather than needing to bundle or hunt down a font
    /// file of their own.
    ///
    /// # Errors
    /// Raises `RuntimeError` if fontconfig discovery is unavailable, no
    /// installed font resolves at all, or the discovered font file
    /// can't be read.
    #[staticmethod]
    fn system_cascade() -> PyResult<Self> {
        let cascade = tre_text::FontCascade::discover().map_err(setup_err)?;
        let path = cascade.entries.first().ok_or_else(|| {
            TreError::new_err("fontconfig cascade discovery returned no candidate fonts")
        })?;
        let bytes = std::fs::read(path).map_err(setup_err)?;
        Self::from_bytes(bytes)
    }
}
