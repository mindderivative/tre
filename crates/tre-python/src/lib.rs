//! Direct PyO3 bindings to `tre-engine`'s own native Rust API
//! (IMPLEMENTATION.md Phase 10 Step 10.4) -- not through `tre-ffi`'s
//! C-ABI (DESIGN.md Section 2.7's "Cross-Language Boundary: Two Real
//! Paths"), so there is no shadow-type marshalling or opaque-handle
//! round trip for this, the project's own first-party UI framework
//! binding.
//!
//! **Scope of this first real pass** (real, bounded, matching this
//! project's own established "ship one real slice, disclose the rest"
//! discipline rather than attempting full `RenderingCanvas`/`ShapeRegistry`
//! parity in one step):
//!
//! - [`shapes::PyShapeRegistry`] plus Pythonic `Rectangle`/`Circle`/
//!   `Polygon`/`Path` classes ([`shapes`]) -- solid fill only; `Gradient`/
//!   `Texture` fill are real in `tre-engine` but not yet exposed here.
//! - [`renderer::PyHeadlessRenderer`] -- a real, headless GPU round trip
//!   (`ShapeRegistry::flatten_into` -> upload -> `execute_frame` ->
//!   readback) returning real pixel bytes, GIL released for the blocking
//!   GPU work.
//! - [`windowed_renderer::PyWindowedRenderer`] plus [`input`]'s real
//!   `WindowId`/`MouseButton`/`ElementState`/`InputEvent` bindings -- a
//!   real on-screen renderer with real polled input events, following
//!   the same proven window/surface/swapchain sequence
//!   `tre-rhi-vulkan`'s own `multi_window.rs` example already uses.
//! - [`canvas::PyCanvas`] -- `save()`/`restore()` only, as a context
//!   manager; the other ~20 `RenderingCanvas` methods (direct
//!   immediate-mode drawing, layers, text) are not yet exposed.
//! - [`error::TreError`] -- every real `EngineError` surfaces as this one
//!   Python exception type.
//!
//! One of the four crates permitted to contain `unsafe`
//! (TECHNICAL.md Section 9.1): [`renderer`]'s headless-device setup tears
//! down a raw `VkSurfaceKHR` probe handle, the identical FFI cleanup call
//! every `tre-rhi-vulkan` demo already performs -- this is the first
//! place that call is needed inside a real (non-example) crate rather
//! than a demo.
#![deny(unsafe_op_in_unsafe_fn)]

mod animation;
mod canvas;
mod clock;
mod editable_text;
mod error;
mod font;
mod gradient;
mod input;
mod renderer;
mod shapes;
mod svg;
mod text_atlas;
mod texture;
mod tween;
mod windowed_renderer;

use pyo3::prelude::*;

/// Packs 8-bit RGBA channels into the `u32` every shape's `fill_color`/
/// `border_color` field expects -- a real Python-side equivalent of
/// `tre_engine::rgba8`, whose exact little-endian byte order
/// (`u32::from_le_bytes([r, g, b, a])`) callers would otherwise have to
/// replicate by hand.
#[pyfunction]
fn rgba8(r: u8, g: u8, b: u8, a: u8) -> u32 {
    u32::from_le_bytes([r, g, b, a])
}

#[pymodule]
fn tre_python(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("TreError", m.py().get_type::<error::TreError>())?;
    m.add_function(pyo3::wrap_pyfunction!(rgba8, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(canvas::shadow_layer_bounds, m)?)?;
    m.add_class::<shapes::PyShapeId>()?;
    m.add_class::<shapes::PyRectangle>()?;
    m.add_class::<shapes::PyCircle>()?;
    m.add_class::<shapes::PyPolygon>()?;
    m.add_class::<shapes::PyPath>()?;
    m.add_class::<shapes::PyText>()?;
    m.add_class::<shapes::PyShapeRegistry>()?;
    m.add_class::<font::PyFont>()?;
    m.add_class::<gradient::PyGradient>()?;
    m.add_class::<gradient::PyGradientId>()?;
    m.add_class::<texture::PyTexture>()?;
    m.add_class::<texture::PyTextureFormat>()?;
    m.add_class::<canvas::PyCanvas>()?;
    m.add_class::<canvas::PyClipGuard>()?;
    m.add_class::<canvas::PyLayerGuard>()?;
    m.add_class::<canvas::PyAccessibilityRole>()?;
    m.add_class::<renderer::PyHeadlessRenderer>()?;
    m.add_class::<input::PyWindowId>()?;
    m.add_class::<input::PyMouseButton>()?;
    m.add_class::<input::PyElementState>()?;
    m.add_class::<input::PyInputEvent>()?;
    m.add_class::<windowed_renderer::PyWindowedRenderer>()?;
    m.add_class::<windowed_renderer::PyCursorIcon>()?;
    m.add_class::<svg::PySvg>()?;
    m.add_class::<svg::PyFillRule>()?;
    m.add_class::<svg::PySmilAnimate>()?;
    m.add_class::<svg::PySmilAnimateTranslate>()?;
    m.add_class::<svg::PyParsedSmil>()?;
    m.add_function(pyo3::wrap_pyfunction!(svg::parse_smil, m)?)?;
    m.add_class::<clock::PyClock>()?;
    m.add_class::<tween::PyTween>()?;
    m.add_class::<tween::PyEasing>()?;
    m.add_class::<tween::PySpring>()?;
    m.add_class::<animation::PyTimeline>()?;
    m.add_class::<editable_text::PyEditableText>()?;
    Ok(())
}
