//! 0.4.0 M3: a window's persistent render target.
//!
//! A swapchain image keeps nothing from the frame before -- each
//! `get_current_texture()` may hand back a different image of the chain,
//! with undefined contents. Partial redraw (0.4.0 M5) repaints only what
//! changed, so it needs a texture that does keep the previous frame: the
//! scene renders into this target, and the target is copied whole into
//! the acquired swapchain image every frame. The copy is a plain
//! texture-to-texture copy, byte-exact, so the window shows exactly what
//! rendering straight into the swapchain image would have.

/// A texture the size of a window's client area, in its surface format,
/// that keeps its contents from frame to frame.
pub struct PersistentTarget {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    format: wgpu::TextureFormat,
    width: u32,
    height: u32,
}

impl PersistentTarget {
    /// A target of `width` x `height` pixels in `format` -- the window
    /// surface's format, so it can be copied into the surface's images.
    /// Zero-sized dimensions are raised to 1, as `wgpu` requires.
    pub fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Self {
        let (width, height) = (width.max(1), height.max(1));
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("tre persistent target"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            // Rendered into by `vello_gpu`, copied out to the swapchain.
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Self {
            texture,
            view,
            format,
            width,
            height,
        }
    }

    /// Makes the target `width` x `height`, recreating it if its size
    /// changed. Returns `true` when it was recreated: its contents are
    /// then undefined, and the next frame must be redrawn in full.
    pub fn ensure_size(&mut self, device: &wgpu::Device, width: u32, height: u32) -> bool {
        let (width, height) = (width.max(1), height.max(1));
        if (width, height) == (self.width, self.height) {
            return false;
        }
        *self = Self::new(device, self.format, width, height);
        true
    }

    /// The view to render the scene into.
    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    /// The target's size in pixels.
    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Copies the target into `destination` -- the acquired swapchain
    /// image, in the same format and allowing `COPY_DST`. Normally the two
    /// are the same size; if a resize ever leaves them apart for a frame,
    /// only the area both cover is copied, rather than panicking. The
    /// target keeps its contents.
    pub fn copy_to(&self, encoder: &mut wgpu::CommandEncoder, destination: &wgpu::Texture) {
        encoder.copy_texture_to_texture(
            self.texture.as_image_copy(),
            destination.as_image_copy(),
            wgpu::Extent3d {
                width: self.width.min(destination.width()),
                height: self.height.min(destination.height()),
                depth_or_array_layers: 1,
            },
        );
    }
}
