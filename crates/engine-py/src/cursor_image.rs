//! 0.5.4 (#140): `tre.CursorImage`, a pointer shape drawn from pixels.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

/// A cursor image for a node's `cursor`: `node.set(cursor=CursorImage(rgba, 32, 32, hotspot=(4, 4)))`.
///
/// `rgba` is straight-alpha RGBA8 bytes, `width * height * 4` of them, each side 1
/// to 256 pixels; `hotspot` is the pixel that is the pointer's position. The same
/// image made twice is one cursor. The image is in device pixels: it is not scaled
/// for a HiDPI display, so give a larger one there. The OS cursor is made when
/// `App.run()` first runs the loop (`ready` says), and shows the default shape
/// until then.
#[pyclass(frozen, name = "CursorImage", eq, hash, from_py_object)]
#[derive(Clone, PartialEq, Eq, Hash)]
pub(crate) struct PyCursorImage {
    pub(crate) id: u64,
}

impl PyCursorImage {
    pub(crate) fn from_id(id: u64) -> Self {
        Self { id }
    }
}

#[pymethods]
impl PyCursorImage {
    #[new]
    #[pyo3(signature = (rgba, width, height, hotspot=(0, 0)))]
    fn new(rgba: &[u8], width: u32, height: u32, hotspot: (u32, u32)) -> PyResult<Self> {
        let side = |name: &str, v: u32| {
            u16::try_from(v).map_err(|_| {
                PyValueError::new_err(format!("a cursor image's {name} must be 1 to 256 pixels"))
            })
        };
        let (w, h) = (side("width", width)?, side("height", height)?);
        let hotspot = (side("hotspot x", hotspot.0)?, side("hotspot y", hotspot.1)?);
        let id = engine_platform::cursors::register(rgba.to_vec(), w, h, hotspot)
            .map_err(PyValueError::new_err)?;
        Ok(Self { id })
    }

    /// `(width, height)` in pixels.
    #[getter]
    fn size(&self) -> (u16, u16) {
        engine_platform::cursors::info(self.id).map_or((0, 0), |(size, _)| size)
    }

    /// The pixel `(x, y)` that is the pointer's position.
    #[getter]
    fn hotspot(&self) -> (u16, u16) {
        engine_platform::cursors::info(self.id).map_or((0, 0), |(_, hotspot)| hotspot)
    }

    /// Whether the OS cursor has been made (`False` until `App.run()` has run
    /// its loop; the default shape shows meanwhile).
    #[getter]
    fn ready(&self) -> bool {
        engine_platform::cursors::is_ready(self.id)
    }

    fn __repr__(&self) -> String {
        let (w, h) = self.size();
        format!("CursorImage({w}x{h}, hotspot={:?})", self.hotspot())
    }
}
