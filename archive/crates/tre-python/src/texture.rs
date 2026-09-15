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
use tre_engine::{RhiDevice, RhiTexture};

use crate::error::TreError;

/// `tre_engine::TextureFormat`, bound directly.
#[pyclass(name = "TextureFormat", eq, from_py_object)]
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
/// A GPU texture bundled with a keep-alive reference to the device that
/// owns it. The device reference is what makes teardown across the Python
/// boundary safe (REVIEW.md finding #258): a `VulkanTexture`'s `Drop`
/// frees its GPU image through a *cloned* `ash::Device` handle, which
/// becomes dangling the moment the real `VulkanDevice` is destroyed, and
/// Python finalizes the renderer and any handed-out textures (or a
/// registry still holding one) in an unspecified order at interpreter
/// shutdown. Holding an `Arc<dyn RhiDevice>` here guarantees the device
/// object -- and therefore its `vkDestroyDevice` in `Drop` -- cannot run
/// until every texture that references it is gone. Field order is load-
/// bearing: `texture` is declared before `_device`, so the image is freed
/// (device still alive) before this `Arc` is released. On a real GPU the
/// wrong order merely leaks or silently misbehaves; on a software driver
/// (lavapipe, CI) it corrupts the C heap -- which is how this surfaced.
pub(crate) struct SharedTexture {
    // Held only to keep the GPU texture alive until this value drops (its
    // `Drop` frees the image); never read after construction, hence the
    // leading underscore. Declared before `_device` so the image is freed
    // while the device is still alive -- see this struct's doc comment.
    pub(crate) _texture: Box<dyn RhiTexture>,
    pub(crate) _device: Arc<dyn RhiDevice>,
}

#[pyclass(name = "Texture", frozen, from_py_object)]
#[derive(Clone)]
pub struct PyTexture {
    pub(crate) texture: Arc<SharedTexture>,
    pub(crate) bindless_index: u32,
}

impl PyTexture {
    pub(crate) fn new(texture: Box<dyn RhiTexture>, device: Arc<dyn RhiDevice>) -> PyResult<Self> {
        let bindless_index = texture
            .bindless_index()
            .ok_or_else(|| TreError::new_err("newly created texture has no bindless index"))?;
        Ok(Self {
            texture: Arc::new(SharedTexture {
                _texture: texture,
                _device: device,
            }),
            bindless_index,
        })
    }
}
