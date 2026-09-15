//! `tre.Clipboard` (GUI-readiness assessment recommendation #4) -- real
//! system clipboard text access, binding directly to `tre_platform::
//! Clipboard`, which itself wraps `arboard::Clipboard`. See
//! `tre_platform::clipboard`'s own module doc comment for the real,
//! disclosed v1 scope (plain text only).

use pyo3::prelude::*;

use crate::renderer::setup_err;

/// A real handle to the system clipboard. Construct once, reuse across
/// calls -- opening the underlying platform clipboard connection is not
/// something cheap to repeat per call.
#[pyclass(name = "Clipboard", unsendable)]
pub struct PyClipboard {
    inner: tre_platform::Clipboard,
}

#[pymethods]
impl PyClipboard {
    /// # Errors
    /// Raises `RuntimeError` if no real clipboard service is reachable
    /// (e.g. no display server connection).
    #[new]
    fn new() -> PyResult<Self> {
        Ok(Self {
            inner: tre_platform::Clipboard::new().map_err(setup_err)?,
        })
    }

    /// Reads the clipboard's current real plain-text content.
    ///
    /// # Errors
    /// Raises `RuntimeError` if the clipboard is empty, holds non-text
    /// content, or the platform clipboard service itself failed.
    // `pub(crate)`, not private: `PyEditableText::paste` (Phase 14 Step
    // 14.2) calls this directly as a plain Rust method, not through the
    // Python-visible `#[pymethods]` dispatch.
    pub(crate) fn get_text(&mut self) -> PyResult<String> {
        self.inner.get_text().map_err(setup_err)
    }

    /// Writes `text` as the clipboard's new real plain-text content,
    /// replacing whatever was there before.
    ///
    /// # Errors
    /// Raises `RuntimeError` if the platform clipboard service rejected
    /// the write.
    // `pub(crate)` for the identical reason `get_text` is -- called
    // directly by `PyEditableText::copy`/`cut`.
    pub(crate) fn set_text(&mut self, text: &str) -> PyResult<()> {
        self.inner.set_text(text).map_err(setup_err)
    }
}
