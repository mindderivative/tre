//! PyO3 bindings -- the only crate depending on `pyo3`, and the only
//! stability contract for framework users.
//!
//! §14 step 14 (§11.1): `App` collects registered `PyWindow`s and drives
//! them together; each `PyWindow` owns its own `Tree`/root/size,
//! `Node`/`View` unchanged by the split. See `app.rs`/`window.rs`'s own
//! module doc comments for the `PyApp`/`PyWindow` split this step made.

mod app;
mod binding;
mod dispatch;
mod error;
mod node;
mod view;
mod window;

use pyo3::prelude::*;

pub use app::App;
pub use error::EngineError;
pub use node::Node;
pub use view::View;
pub use window::PyWindow;

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
    m.add_function(wrap_pyfunction!(view::_record_read, m)?)?;
    Ok(())
}
