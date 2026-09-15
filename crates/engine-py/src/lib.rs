//! PyO3 bindings -- the only crate depending on `pyo3`, and the only
//! stability contract for framework users.
//!
//! §14 step 6 (minimal): `App` (node creation + the one blocking
//! `run()`) and `Node` (`animate()`, the one property setter). No
//! `add_child`/`set_on_click`/multi-window/YAML views yet -- each lands
//! at its own later build-order step. See `app.rs`/`node.rs`/`error.rs`
//! for what's deliberately deferred and why.

mod app;
mod binding;
mod error;
mod node;
mod view;

use pyo3::prelude::*;

pub use app::App;
pub use error::EngineError;
pub use node::Node;
pub use view::View;

/// The compiled extension module Python actually imports, as
/// `tre._core` (`pyproject.toml`'s `module-name`) -- `python/tre/
/// __init__.py` re-exports `App`/`Node`/`View` from here, plus its own
/// pure-Python `Signal`/`ViewModel` (§16.2), as the public `tre` package
/// surface.
#[pymodule]
fn _core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<App>()?;
    m.add_class::<Node>()?;
    m.add_class::<View>()?;
    m.add_function(wrap_pyfunction!(view::_record_read, m)?)?;
    Ok(())
}
