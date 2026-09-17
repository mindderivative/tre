//! M22 Phase 1 (§5): `NodeKind::Image`'s own real GPU-backed paint
//! mechanism.
//!
//! **Real finding, confirmed by a failing test, not assumed:**
//! `vello_hybrid` 0.2.0's ordinary `Scene::set_paint`+`fill_path` path
//! panics on any CPU-side pixel data (`ImageSource::Pixmap`) --
//! "pixmap image sources are not supported by Vello Hybrid" -- its own
//! wgpu renderer only ever accepts `ImageSource::OpaqueId` (a real,
//! pre-registered image), and the `ImageCache` that would register one
//! is `pub(crate)` inside `vello_hybrid` itself, unreachable from here.
//!
//! `Scene::draw_texture_rects` + `TextureBindings` (both real, public
//! API, confirmed via direct source read) is the one currently-
//! supported path for CPU-decoded image data: an ordinary, externally-
//! owned `wgpu::Texture`, uploaded once per real `Image` node and
//! cached here -- keyed by a stable per-`NodeId` `u64`
//! (`engine_core::node_id_as_u64`, the identical id scheme
//! `to_access_id` already established for a different foreign-handle
//! consumer), reused every subsequent frame rather than re-uploaded.
//! An `Image` node's pixel data never changes after `Window.add_image`
//! this phase (no swap-the-image API is scoped), so a real upload only
//! ever happens once per node -- the same "built once, not rebuilt
//! per-frame" shape `Resources`' own glyph atlas already has.

use std::collections::HashMap;

use engine_core::{NodeId, Tree, node_id_as_u64};
use vello_hybrid::{TextureBindings, TextureId};

/// Owns every real `Image` node's GPU texture plus the live
/// `TextureBindings` map `FrameRenderer::render` hands to
/// `vello_hybrid`.
#[derive(Default)]
pub struct ImageTextureCache {
    textures: HashMap<NodeId, wgpu::Texture>,
    bindings: TextureBindings,
}

impl ImageTextureCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub(crate) fn bindings(&self) -> &TextureBindings {
        &self.bindings
    }

    /// Ensures every real `Image` node in `tree` has a real, uploaded
    /// GPU texture bound under its own deterministic `TextureId` --
    /// call once per frame, before `FrameRenderer::render`, so every
    /// `Scene::draw_texture_rects` call `paint_node` already recorded
    /// this frame resolves to a real bound texture at render time (the
    /// real contract `TextureBindings::insert`'s own doc comment
    /// states: "a texture with the given `TextureId` must be supplied
    /// at render time").
    pub fn sync(&mut self, tree: &Tree, device: &wgpu::Device, queue: &wgpu::Queue) {
        for (id, state) in tree.image_nodes() {
            if self.textures.contains_key(&id) {
                continue;
            }

            // Reuses the exact real RGBA8/BGRA8 + premultiply-alpha
            // conversion `vello_hybrid`'s own glyph-atlas path already
            // relies on (`ImageSource::from_peniko_image_data`), rather
            // than duplicating that logic here -- `TextureBindings`'s
            // own doc comment requires the texture be premultiplied,
            // and `Pixmap::data_as_u8_slice` is exactly that, tightly
            // packed, ready for `Queue::write_texture`.
            let source = vello_common::paint::ImageSource::from_peniko_image_data(&state.image);
            let vello_common::paint::ImageSource::Pixmap(pixmap) = source else {
                unreachable!("from_peniko_image_data always returns ImageSource::Pixmap")
            };
            let width = u32::from(pixmap.width());
            let height = u32::from(pixmap.height());

            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("engine-render Image node texture"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                pixmap.data_as_u8_slice(),
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(width * 4),
                    rows_per_image: Some(height),
                },
                wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
            );

            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            self.bindings.insert(texture_id_for(id), view);
            self.textures.insert(id, texture);
        }
    }
}

/// The deterministic `TextureId` a given `Image` node's real GPU
/// texture is bound under -- `paint_node`'s own `NodeKind::Image` arm
/// computes the identical value when recording `Scene::
/// draw_texture_rects`, so the two always agree without either side
/// needing to look anything up in the other.
pub(crate) fn texture_id_for(id: NodeId) -> TextureId {
    TextureId(node_id_as_u64(id))
}
