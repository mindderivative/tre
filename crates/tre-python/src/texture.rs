//! `tre.Texture` -- a real, GPU-resident texture (Phase 12 Step 12.4),
//! created via a renderer's `create_texture(...)` (mirrors
//! `RhiDevice::create_texture`'s own real upload + bindless
//! registration, Phase 10 Step 10.2.2) and usable as a shape's
//! `fill_color`.
//!
//! Renderer-scoped, not registry-scoped, unlike [`crate::gradient::
//! PyGradient`]: creating one needs a live GPU device, which only a
//! renderer owns.

use std::sync::Arc;

use pyo3::prelude::*;
use tre_engine::RhiTexture;

use crate::error::TreError;

/// `tre_engine::TextureFormat`, bound directly.
#[pyclass(name = "TextureFormat", eq)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PyTextureFormat {
    Bgra8Srgb,
    Rgba16Float,
    Rgba8Unorm,
}

impl From<PyTextureFormat> for tre_engine::TextureFormat {
    fn from(f: PyTextureFormat) -> Self {
        match f {
            PyTextureFormat::Bgra8Srgb => Self::Bgra8Srgb,
            PyTextureFormat::Rgba16Float => Self::Rgba16Float,
            PyTextureFormat::Rgba8Unorm => Self::Rgba8Unorm,
        }
    }
}

/// A real, GPU-resident texture. Keeps its own underlying `RhiTexture`
/// alive via a shared, cloneable `Arc` -- a shape referencing this
/// texture's bindless index (`registry.insert_*`) keeps its own clone of
/// that `Arc` (`PyShapeRegistry`'s own `textures_kept_alive`), so the
/// real GPU texture survives for as long as any shape -- or this
/// `Texture` object itself -- still needs it, independent of whichever
/// Python object happens to be garbage-collected first.
#[pyclass(name = "Texture", frozen)]
#[derive(Clone)]
pub struct PyTexture {
    pub(crate) texture: Arc<Box<dyn RhiTexture>>,
    pub(crate) bindless_index: u32,
}

impl PyTexture {
    pub(crate) fn new(texture: Box<dyn RhiTexture>) -> PyResult<Self> {
        let bindless_index = texture
            .bindless_index()
            .ok_or_else(|| TreError::new_err("newly created texture has no bindless index"))?;
        Ok(Self {
            texture: Arc::new(texture),
            bindless_index,
        })
    }
}
