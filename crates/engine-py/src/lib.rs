//! PyO3 bindings -- the only crate depending on `pyo3`, and the only
//! stability contract for framework users.
//!
//! §14 step 14 (§11.1): `App` collects registered `PyWindow`s and drives
//! them together; each `PyWindow` owns its own `Tree`/root/size,
//! `Node` unchanged by the split. See `app.rs`/`window.rs`'s own
//! module doc comments for the `PyApp`/`PyWindow` split this step made.

mod app;
mod canvas;
mod clock;
mod dispatch;
mod dock;
mod error;
mod event;
mod gradient;
mod grid;
mod listeners;
mod node;
mod node_callbacks;
mod node_events;
mod node_handles;
mod node_kind_props;
mod node_layout;
mod node_props;
mod scale;
mod shader;
mod shaper;
mod terminal;
mod thread_bound;
mod thread_handle;
mod touch;
mod window;
mod window_docking;
mod window_events;
mod window_input;
mod window_layers;
mod window_snapshot;

use pyo3::prelude::*;

pub use app::App;
pub use canvas::Painter;
pub use error::EngineError;
pub use event::Event;
pub use node::Node;
pub use shader::Shader;
pub use thread_handle::LoopHandle;
pub use window::PyWindow;

/// The compiled extension module Python actually imports, as
/// `tre._core` (`pyproject.toml`'s `module-name`) -- `python/tre/
/// __init__.py` re-exports these as the public `tre` package surface.
#[pymodule]
fn _core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<App>()?;
    m.add_class::<PyWindow>()?;
    m.add_class::<Node>()?;
    m.add_class::<Painter>()?;
    m.add_class::<gradient::PyGradient>()?;
    m.add_class::<Event>()?;
    m.add_class::<LoopHandle>()?;
    m.add_class::<Shader>()?;
    m.add("ShaderError", m.py().get_type::<shader::ShaderError>())?;
    m.add_function(wrap_pyfunction!(register_font, m)?)?;
    m.add_function(wrap_pyfunction!(set_system_fonts, m)?)?;
    m.add_function(wrap_pyfunction!(system_fonts, m)?)?;
    Ok(())
}

/// M86: registers a font the caller already loaded (a `.ttf`/`.otf`/
/// `.ttc` file's raw bytes) with every current and future window in
/// this process -- `tre` never reads a font file itself. Returns the
/// family names the data contains, the exact strings a node's
/// `font_family` property must use. Raises `ValueError` if the data
/// holds no parseable font face. `&[u8]` borrows a Python `bytes`
/// directly rather than extracting it element by element.
#[pyfunction]
fn register_font(data: &[u8]) -> PyResult<Vec<String>> {
    engine_render::register_font(data.to_vec())
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
}

/// 0.5.4 (#111): lets text use the fonts installed on this machine, for the
/// glyphs the bundled and registered fonts lack (CJK, Hebrew, Indic, colour
/// emoji) and for family names that aren't registered. Off by default, so text
/// is the same on every machine; it applies to every window in this process,
/// live.
#[pyfunction]
fn set_system_fonts(enabled: bool) {
    engine_render::set_system_fonts(enabled);
}

/// Whether system fonts are on (`set_system_fonts`).
#[pyfunction]
fn system_fonts() -> bool {
    engine_render::system_fonts()
}
