//! PyO3 bindings -- the only crate depending on `pyo3`, and the only
//! stability contract for framework users.
//!
//! §14 step 14 (§11.1): `App` collects registered `PyWindow`s and drives
//! them together; each `PyWindow` owns its own `Tree`/root/size,
//! `Node`/`View` unchanged by the split. See `app.rs`/`window.rs`'s own
//! module doc comments for the `PyApp`/`PyWindow` split this step made.

mod app;
mod binding;
mod canvas;
mod component;
mod dispatch;
mod dock;
mod error;
mod event;
mod node;
mod terminal;
mod view;
mod window;
mod window_docking;
mod window_factory;
mod window_input;
mod window_virtual_canvas;

use pyo3::prelude::*;

pub use app::App;
pub use canvas::CanvasContext;
pub use component::Component;
pub use error::EngineError;
pub use event::Event;
pub use node::Node;
pub use view::View;
pub use window::{PyWindow, Theme};

/// The compiled extension module Python actually imports, as
/// `tre._core` (`pyproject.toml`'s `module-name`) -- `python/tre/
/// __init__.py` re-exports `App`/`PyWindow`/`Node`/`View` from here,
/// plus its own pure-Python `Signal`/`ViewModel` (§16.2), as the public
/// `tre` package surface.
#[pymodule]
fn _core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<App>()?;
    m.add_class::<PyWindow>()?;
    m.add_class::<Node>()?;
    m.add_class::<View>()?;
    m.add_class::<Component>()?;
    m.add_class::<CanvasContext>()?;
    m.add_class::<Event>()?;
    m.add_class::<Theme>()?;
    m.add_function(wrap_pyfunction!(view::_record_read, m)?)?;
    m.add_function(wrap_pyfunction!(view::_begin_recording, m)?)?;
    m.add_function(wrap_pyfunction!(view::_end_recording, m)?)?;
    m.add_function(wrap_pyfunction!(register_font, m)?)?;
    Ok(())
}

/// M86: registers a font the caller already loaded (a `.ttf`/`.otf`/
/// `.ttc` file's raw bytes) with every current and future window in
/// this process -- `tre` never reads a font file itself. Returns the
/// family names the data contains, the exact strings a theme's
/// `typography:` `font_family` must use. Raises `ValueError` if the data
/// holds no parseable font face. `&[u8]` borrows a Python `bytes`
/// directly rather than extracting it element by element.
#[pyfunction]
fn register_font(data: &[u8]) -> PyResult<Vec<String>> {
    engine_render::register_font(data.to_vec())
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
}
