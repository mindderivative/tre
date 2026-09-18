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
//!
//! M30 Phase 9 Step 1 (§5): `Video`'s own real "frame sink" design --
//! reusing `NodeKind::Image` directly rather than a new `NodeKind`,
//! see `Node.push_frame`'s own doc comment -- means an `Image` node's
//! pixel data genuinely *can* change after creation now, real, live,
//! at whatever cadence the app decides. `sync`'s own original re-
//! upload guard (`if self.textures.contains_key(&id) { continue; }`)
//! predates this and would silently keep painting a video's very
//! first frame forever. Fixed by keying re-upload on real content
//! identity, not node presence: `ImageState.image.data` is a
//! `peniko::Blob<u8>`, whose own `PartialEq`/`id()` are a cheap O(1)
//! comparison against a real, unique, monotonically-assigned `u64` --
//! confirmed by direct source read of the vendored
//! `linebender_resource_handle` crate, not assumed -- never a byte-
//! level memcmp of the pixel buffer itself, which would be a real,
//! meaningful per-frame cost at video resolutions. `push_frame`
//! constructs a genuinely new `Blob` every call (the same "resolved
//! ahead of time" decode-elsewhere split `add_image` already
//! established), so a fresh id here always means fresh content, and
//! an unchanged id always means the same frame still showing --
//! `sync` re-uploads exactly when, and only when, that id changes.
//! **Real, deliberate scope simplification, not silently missed:** a
//! changed frame always recreates the whole GPU texture (rather than
//! reusing the existing one via `write_texture` alone when the
//! resolution is unchanged) -- correct for both same-size and resized
//! frames uniformly, at the real cost of a full texture allocation on
//! every pushed frame rather than only on a resolution change.
//! `Video`'s own scope (a frame sink for whatever cadence the app
//! feeds it, not a real-time decode pipeline this codebase has no
//! codec dependency for) doesn't need the incremental fast path yet
//! -- the same "don't build ahead of need" discipline this codebase
//! applies throughout.

use std::collections::{HashMap, HashSet};

use engine_core::{NodeId, Tree, node_id_as_u64};
use vello_hybrid::{TextureBindings, TextureId};

/// Owns every real `Image` node's GPU texture plus the live
/// `TextureBindings` map `FrameRenderer::render` hands to
/// `vello_hybrid`.
#[derive(Default)]
pub struct ImageTextureCache {
    textures: HashMap<NodeId, wgpu::Texture>,
    bindings: TextureBindings,
    /// The real `peniko::Blob<u8>` content id last uploaded for each
    /// node -- `sync`'s own real re-upload guard, see this module's
    /// own doc comment.
    uploaded: HashMap<NodeId, u64>,
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
        let current: HashSet<NodeId> = tree.image_nodes().map(|(id, _)| id).collect();

        for (id, state) in tree.image_nodes() {
            let blob_id = state.image.data.id();
            if self.uploaded.get(&id) == Some(&blob_id) {
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
            self.uploaded.insert(id, blob_id);
        }

        // Real, confirmed bug found in review: this loop above was
        // purely additive -- a node's own uploaded GPU texture (and
        // its `TextureBindings` entry) was never freed once the node
        // was removed from the tree (e.g. `Node.remove()` on an Image
        // node, the real, documented way to swap images -- a gallery,
        // a carousel, an avatar update, a virtualized image list),
        // leaking VRAM and a growing `HashMap` entry for the life of
        // the window. `sync` already walks the tree's current real
        // image nodes every frame, so evicting anything no longer
        // present is a direct, symmetric addition, not new state to
        // track separately.
        let removed: Vec<NodeId> = self
            .textures
            .keys()
            .filter(|id| !current.contains(id))
            .copied()
            .collect();
        for id in removed {
            self.textures.remove(&id);
            self.bindings.remove(texture_id_for(id));
            self.uploaded.remove(&id);
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

#[cfg(test)]
mod tests {
    use super::*;
    use engine_core::{ImageState, NodeKind, PaintProperties};
    use peniko::Color;
    use taffy::prelude::{Size, Style, length};

    fn solid_2x2_image_data() -> peniko::ImageData {
        let px = [0xFFu8, 0x00, 0x00, 0xFF];
        let mut bytes = Vec::with_capacity(px.len() * 4);
        for _ in 0..4 {
            bytes.extend_from_slice(&px);
        }
        peniko::ImageData {
            data: peniko::Blob::from(bytes),
            format: peniko::ImageFormat::Rgba8,
            alpha_type: peniko::ImageAlphaType::Alpha,
            width: 2,
            height: 2,
        }
    }

    async fn device_and_queue() -> (wgpu::Device, wgpu::Queue) {
        let instance = wgpu::Instance::default();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                force_fallback_adapter: false,
                compatible_surface: None,
            })
            .await
            .expect("no wgpu adapter available in this environment");
        adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("image_cache test device"),
                required_features: wgpu::Features::empty(),
                ..Default::default()
            })
            .await
            .expect("failed to create wgpu device")
    }

    /// Real, regression coverage for the review-found leak: a removed
    /// `Image` node's own uploaded GPU texture must actually be freed
    /// on the next `sync`, not accumulate forever.
    #[test]
    fn sync_evicts_a_texture_for_a_node_removed_from_the_tree() {
        pollster::block_on(async {
            let (device, queue) = device_and_queue().await;

            let mut tree = Tree::new();
            let image_id = tree.insert(
                NodeKind::Image(ImageState::new(solid_2x2_image_data())),
                Style {
                    size: Size {
                        width: length(2.0),
                        height: length(2.0),
                    },
                    ..Default::default()
                },
                PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 0.0, 1.0),
            );

            let mut cache = ImageTextureCache::new();
            cache.sync(&tree, &device, &queue);
            assert_eq!(
                cache.textures.len(),
                1,
                "a real Image node must upload a real texture"
            );
            assert!(
                cache.bindings.remove(texture_id_for(image_id)).is_some(),
                "sync must have bound the new texture under its own deterministic TextureId"
            );

            tree.remove(image_id);
            cache.sync(&tree, &device, &queue);

            assert!(
                cache.textures.is_empty(),
                "sync must evict a texture whose node no longer exists in the tree, not leak it"
            );
        });
    }

    fn solid_2x2_image_data_tinted(byte: u8) -> peniko::ImageData {
        let px = [byte, 0x00, 0x00, 0xFF];
        let mut bytes = Vec::with_capacity(px.len() * 4);
        for _ in 0..4 {
            bytes.extend_from_slice(&px);
        }
        peniko::ImageData {
            data: peniko::Blob::from(bytes),
            format: peniko::ImageFormat::Rgba8,
            alpha_type: peniko::ImageAlphaType::Alpha,
            width: 2,
            height: 2,
        }
    }

    /// M30 Phase 9 Step 1 (§5): direct, dedicated coverage for the real
    /// `Video`-driven re-upload fix, this module's own doc comment --
    /// `sync`'s original guard only ever checked whether a texture
    /// already existed for a node, so a real, live content change
    /// (`Node.push_frame`) would silently keep painting the very first
    /// frame forever. Proves the fix directly by real content identity,
    /// not node presence: replacing a node's own `ImageState.image`
    /// with a genuinely new `Blob` (a different real `id()`, `peniko`'s
    /// own content-identity, not a byte-level pixel comparison) must
    /// trigger a real re-upload, tracked in `ImageTextureCache`'s own
    /// `uploaded` map.
    #[test]
    fn sync_reuploads_a_texture_when_its_own_node_content_genuinely_changes() {
        pollster::block_on(async {
            let (device, queue) = device_and_queue().await;

            let mut tree = Tree::new();
            let frame1 = solid_2x2_image_data_tinted(0xFF);
            let frame1_blob_id = frame1.data.id();
            let image_id = tree.insert(
                NodeKind::Image(ImageState::new(frame1)),
                Style {
                    size: Size {
                        width: length(2.0),
                        height: length(2.0),
                    },
                    ..Default::default()
                },
                PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 0.0, 1.0),
            );

            let mut cache = ImageTextureCache::new();
            cache.sync(&tree, &device, &queue);
            assert_eq!(
                cache.uploaded.get(&image_id),
                Some(&frame1_blob_id),
                "the first real sync must upload the first frame's own real content id"
            );

            let frame2 = solid_2x2_image_data_tinted(0x00);
            let frame2_blob_id = frame2.data.id();
            assert_ne!(
                frame1_blob_id, frame2_blob_id,
                "sanity: every real peniko::Blob::from call gets its own unique id"
            );
            match &mut tree.get_mut(image_id).expect("just inserted above").kind {
                NodeKind::Image(state) => state.image = frame2,
                _ => panic!("expected NodeKind::Image"),
            }

            cache.sync(&tree, &device, &queue);
            assert_eq!(
                cache.uploaded.get(&image_id),
                Some(&frame2_blob_id),
                "a real content change (Node.push_frame's own real effect) must trigger a \
                 real re-upload, not keep the stale first frame's own id forever"
            );
            assert_eq!(
                cache.textures.len(),
                1,
                "a re-uploaded node must still have exactly one real texture, not a second \
                 one accumulating alongside it"
            );
        });
    }
}
