//! M22 Phase 1 (§5): `NodeKind::Image`'s own real GPU-backed paint
//! mechanism.
//!
//! The renderer takes no CPU-side pixel data in a scene: `vello_hybrid`
//! 0.2.0 panicked on `ImageSource::Pixmap`, and so does the pinned
//! `vello_gpu` (`paint.rs`, `unimplemented!`). `vello_gpu` does offer an
//! image atlas (`Renderer::upload_image`, drawn by `ImageSource::OpaqueId`)
//! that 0.2.0's private cache didn't. 0.4.0 M6 looked at moving images onto
//! it and kept tre's own textures, for three reasons in the pinned source:
//! writing into an existing allocation (`write_to_atlas`) is `pub(crate)`,
//! so a video's every frame would be a `destroy_image` plus a fresh
//! `upload_image`, where a texture of its own is written in place;
//! `upload_image` unwraps its allocation, so an image larger than an atlas
//! page panics rather than failing; and the default page is 4096x4096,
//! smaller than the 8192 `MAX_IMAGE_DIMENSION` tre accepts. An image is
//! instead an ordinary, externally
//! owned `wgpu::Texture`, uploaded once per real `Image` node and
//! cached here -- keyed by a stable per-`NodeId` `u64`
//! (`engine_core::node_id_as_u64`, the identical id scheme
//! `to_access_id` already established for a different foreign-handle
//! consumer), reused every subsequent frame rather than re-uploaded.
//!
//! M30 Phase 9 Step 1 (§5): the real "frame sink" design -- an image
//! node takes caller-decoded frames through `node.set(rgba=,
//! pixel_width=, pixel_height=)` -- means an `Image` node's
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
//! meaningful per-frame cost at video resolutions. Every new `rgba`
//! frame (and every decoded `src`) is a genuinely new `Blob`, so a fresh id here always means fresh content, and
//! an unchanged id always means the same frame still showing --
//! `sync` re-uploads exactly when, and only when, that id changes.
//! 0.4.0 review: a frame at the texture's own size is written into it in
//! place; a new size allocates a new texture. An image larger than
//! `MAX_IMAGE_DIMENSION` either way isn't uploaded (or painted) at all,
//! with a warning, rather than failing inside `wgpu` mid-frame.

use std::collections::{HashMap, HashSet};

use engine_core::{NodeId, Tree, node_id_as_u64};
use vello_gpu::{TextureBindings, TextureId};

/// The largest image side uploaded: `wgpu`'s default
/// `max_texture_dimension_2d`, which every device `tre` creates requests.
/// Larger images are neither uploaded nor painted.
pub const MAX_IMAGE_DIMENSION: u32 = 8192;

/// Owns every real `Image` node's GPU texture plus the live
/// `TextureBindings` map `FrameRenderer::render` hands to
/// `vello_gpu`, which draws each as an `ImageSource::ExternalTexture`.
#[derive(Default)]
pub struct ImageTextureCache {
    textures: HashMap<NodeId, wgpu::Texture>,
    bindings: TextureBindings,
    /// The real `peniko::Blob<u8>` content id last uploaded for each
    /// node -- `sync`'s own real re-upload guard, see this module's
    /// own doc comment.
    uploaded: HashMap<NodeId, u64>,
    /// Oversized images already warned about, by content id, so the
    /// warning comes once per image rather than every frame.
    too_large: HashSet<u64>,
    /// 0.5.4 (#147): the textures of the raster images SVG nodes were given,
    /// by `SvgBitmap::id`.
    svg_textures: HashMap<u64, wgpu::Texture>,
}

impl ImageTextureCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub(crate) fn bindings(&self) -> &TextureBindings {
        &self.bindings
    }

    /// 0.5.1 (#67, #68): the binding map shader textures join, beside a
    /// read-only view of the image textures a shader may sample as inputs --
    /// split so both can be held at once.
    pub(crate) fn split(&mut self) -> (&mut TextureBindings, ImageInputs<'_>) {
        (
            &mut self.bindings,
            ImageInputs {
                textures: &self.textures,
                uploaded: &self.uploaded,
            },
        )
    }

    /// Ensures every real `Image` node in `tree` has a real, uploaded
    /// GPU texture bound under its own deterministic `TextureId` --
    /// call once per frame, before `FrameRenderer::render`, so every
    /// `Scene::draw_texture_rects` call `draw_own` already recorded
    /// this frame resolves to a real bound texture at render time (the
    /// real contract `TextureBindings::insert`'s own doc comment
    /// states: "a texture with the given `TextureId` must be supplied
    /// at render time").
    pub fn sync(&mut self, tree: &Tree, device: &wgpu::Device, queue: &wgpu::Queue) {
        // 0.5.4 (#150): a tree with no image nodes and no texture to free has
        // nothing to scan for, which on most frames of most windows is so.
        if tree.has_images() || !self.textures.is_empty() {
            self.sync_nodes(tree, device, queue);
        }
        if tree.has_svgs() || !self.svg_textures.is_empty() {
            self.sync_svg_images(tree, device, queue);
        }
    }

    fn sync_nodes(&mut self, tree: &Tree, device: &wgpu::Device, queue: &wgpu::Queue) {
        let current: HashSet<NodeId> = tree.image_nodes().map(|(id, _)| id).collect();

        for (id, state) in tree.image_nodes() {
            let blob_id = state.image.data.id();
            if self.uploaded.get(&id) == Some(&blob_id) {
                continue;
            }
            if state.image.width > MAX_IMAGE_DIMENSION || state.image.height > MAX_IMAGE_DIMENSION {
                if self.too_large.insert(blob_id) {
                    tracing::warn!(
                        width = state.image.width,
                        height = state.image.height,
                        max = MAX_IMAGE_DIMENSION,
                        "an image is larger than the GPU's textures can be; not drawn"
                    );
                }
                continue;
            }

            // Reuses the exact real RGBA8/BGRA8 + premultiply-alpha
            // conversion the renderer's own glyph-atlas path already
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

            // A frame at the texture's own size is written in place; only
            // a new size (or a first frame) allocates.
            let reuse = self
                .textures
                .get(&id)
                .is_some_and(|t| t.width() == width && t.height() == height);
            if !reuse {
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
                let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
                self.bindings.insert(texture_id_for(id), view);
                self.textures.insert(id, texture);
            }
            let texture = &self.textures[&id];
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture,
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

            self.uploaded.insert(id, blob_id);
        }

        // Real, confirmed bug found in review: this loop above was
        // purely additive -- a node's own uploaded GPU texture (and
        // its `TextureBindings` entry) was never freed once the node
        // was removed from the tree (e.g. `node.remove()` on an Image
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

impl ImageTextureCache {
    /// 0.5.4 (#147): uploads each raster image an SVG node was given, once,
    /// and frees those no node has any more.
    fn sync_svg_images(&mut self, tree: &Tree, device: &wgpu::Device, queue: &wgpu::Queue) {
        let bitmaps = tree.svg_bitmaps();
        for bitmap in &bitmaps {
            let image = &bitmap.image;
            if self.svg_textures.contains_key(&bitmap.id)
                || image.width > MAX_IMAGE_DIMENSION
                || image.height > MAX_IMAGE_DIMENSION
            {
                continue;
            }
            let source = vello_common::paint::ImageSource::from_peniko_image_data(image);
            let vello_common::paint::ImageSource::Pixmap(pixmap) = source else {
                unreachable!("from_peniko_image_data always returns ImageSource::Pixmap")
            };
            let (width, height) = (u32::from(pixmap.width()), u32::from(pixmap.height()));
            let size = wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            };
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("engine-render SVG image texture"),
                size,
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
                size,
            );
            self.bindings.insert(
                svg_texture_id(bitmap.id),
                texture.create_view(&wgpu::TextureViewDescriptor::default()),
            );
            self.svg_textures.insert(bitmap.id, texture);
        }
        let live: HashSet<u64> = bitmaps.iter().map(|b| b.id).collect();
        let gone: Vec<u64> = self
            .svg_textures
            .keys()
            .filter(|id| !live.contains(id))
            .copied()
            .collect();
        for id in gone {
            self.svg_textures.remove(&id);
            self.bindings.remove(svg_texture_id(id));
        }
    }
}

/// The `TextureId` an SVG node's raster image is bound under: a range
/// no `Image` node's id reaches (those are slot-map keys, far below it).
pub(crate) fn svg_texture_id(bitmap: u64) -> TextureId {
    TextureId((1 << 62) | bitmap)
}

/// 0.5.1 (#68): the uploaded image textures, for a shader that samples an
/// image or video node.
pub(crate) struct ImageInputs<'a> {
    textures: &'a HashMap<NodeId, wgpu::Texture>,
    uploaded: &'a HashMap<NodeId, u64>,
}

impl ImageInputs<'_> {
    /// The node's image texture and its content id (which changes with every
    /// new frame), if one was uploaded.
    pub(crate) fn get(&self, id: NodeId) -> Option<(wgpu::TextureView, u64)> {
        let texture = self.textures.get(&id)?;
        Some((
            texture.create_view(&wgpu::TextureViewDescriptor::default()),
            *self.uploaded.get(&id)?,
        ))
    }
}

/// The deterministic `TextureId` a given `Image` node's real GPU
/// texture is bound under -- `draw_own`'s own `NodeKind::Image` arm
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
                apply_limit_buckets: false,
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
                PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 1.0),
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
    /// (a new `rgba` frame) would silently keep painting the very first
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
                PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 1.0),
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
                "a real content change (what node.set(rgba=...) does) must trigger a \
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
