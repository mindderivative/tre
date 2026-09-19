//! M22 Phase 1 (§5): the standalone proof that `NodeKind::Image`
//! genuinely paints real, loaded pixel data -- not just that the
//! `ImageSource::from_peniko_image_data` conversion compiles. Same
//! headless render-to-texture-then-readback discipline as
//! `checkbox_paint.rs`/`slider_paint.rs`.
//!
//! Uses a synthesized in-memory `peniko::ImageData` (a real, tiny,
//! solid-color pixel buffer), not a file on disk -- `engine-render`
//! has no file-loading concern at all (`ImageState`'s own doc comment:
//! decoding is `engine-py`'s job); this test only proves the *paint*
//! half, with a real, known-value image the test itself controls.

use engine_core::{ContentFit, ImageState, NodeKind, PaintProperties, Tree};
use engine_render::{FrameRenderer, GeometryCache, TextRenderer, build_tree_scene};
use peniko::Color;
use taffy::prelude::{AvailableSpace, Size, Style, length};
use vello_hybrid::{RenderSize, RenderTargetConfig};

const BACKGROUND: Color = Color::from_rgba8(0x11, 0x11, 0x11, 0xFF);

async fn render(tree: &Tree, root: engine_core::NodeId, width: u16, height: u16) -> (Vec<u8>, u32) {
    let instance = wgpu::Instance::default();
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            force_fallback_adapter: false,
            compatible_surface: None,
        })
        .await
        .expect("no wgpu adapter available in this environment");
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("engine-render image-paint test device"),
            required_features: wgpu::Features::empty(),
            ..Default::default()
        })
        .await
        .expect("failed to create wgpu device");

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("image-paint test target"),
        size: wgpu::Extent3d {
            width: u32::from(width),
            height: u32::from(height),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

    let mut frame_renderer = FrameRenderer::new(
        &device,
        &RenderTargetConfig {
            format: texture.format(),
            width: u32::from(width),
            height: u32::from(height),
        },
    );
    let mut text_renderer = TextRenderer::new();
    let mut geometry_cache = GeometryCache::new();
    let scene = build_tree_scene(
        tree,
        root,
        width,
        height,
        frame_renderer.resources_mut(),
        &mut text_renderer,
        &mut geometry_cache,
    );
    // M22 Phase 1 (§5): every real `Image` node needs a real, uploaded
    // GPU texture bound before `render` -- `ImageTextureCache::sync`'s
    // own doc comment, `FrameRenderer::sync_image_textures`'s real
    // public entry point for it.
    frame_renderer.sync_image_textures(tree, &device, &queue);
    let render_size = RenderSize {
        width: u32::from(width),
        height: u32::from(height),
    };
    let mut encoder =
        device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
    frame_renderer.render(&scene, &device, &queue, &mut encoder, &render_size, &view);

    let bytes_per_row = (u32::from(width) * 4).next_multiple_of(256);
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size: u64::from(bytes_per_row) * u64::from(height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_row),
                rows_per_image: None,
            },
        },
        wgpu::Extent3d {
            width: u32::from(width),
            height: u32::from(height),
            depth_or_array_layers: 1,
        },
    );
    queue.submit([encoder.finish()]);

    let slice = readback.slice(..);
    slice.map_async(wgpu::MapMode::Read, |result| {
        result.expect("failed to map readback buffer");
    });
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .expect("device poll failed");

    let data = slice.get_mapped_range();
    let mut out = vec![0u8; data.len()];
    out.copy_from_slice(&data);
    (out, bytes_per_row)
}

fn pixel_at(data: &[u8], bytes_per_row: u32, x: u32, y: u32) -> [u8; 4] {
    let row_start = (y * bytes_per_row) as usize;
    let start = row_start + (x * 4) as usize;
    [
        data[start],
        data[start + 1],
        data[start + 2],
        data[start + 3],
    ]
}

/// A real, solid, opaque bright-green 2x2 `peniko::ImageData` -- small
/// enough to hand-construct directly (no `image` crate/file I/O
/// needed, §4's own crate-boundary rule keeps that entirely out of
/// `engine-render`), large enough that `ImageSource::from_peniko_
/// image_data`'s real per-pixel conversion loop actually runs over
/// more than one pixel.
fn green_2x2_image_data() -> peniko::ImageData {
    let px = [0x00u8, 0xFF, 0x00, 0xFF];
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

fn build_tree() -> (Tree, engine_core::NodeId) {
    let mut tree = Tree::new();
    let root = tree.insert(
        NodeKind::Rect,
        Style {
            size: Size {
                width: length(100.0),
                height: length(100.0),
            },
            ..Default::default()
        },
        PaintProperties::new(BACKGROUND, 0.0, 0.0, 1.0),
    );

    let image = tree.insert(
        NodeKind::Image(ImageState::new(green_2x2_image_data())),
        Style {
            size: Size {
                width: length(40.0),
                height: length(40.0),
            },
            ..Default::default()
        },
        // M22 Phase 1 (§5): `add_image`'s own real hardcoded transparent
        // fill (mirroring `add_canvas`) -- irrelevant to this test since
        // the image itself fully covers its own node, but kept
        // byte-for-byte the same as the real construction path.
        PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 0.0, 1.0),
    );
    tree.add_child(root, image);

    let available = Size {
        width: AvailableSpace::Definite(100.0),
        height: AvailableSpace::Definite(100.0),
    };
    tree.compute_layout(root, available);
    (tree, root)
}

/// A real, solid, opaque `width`x`height` `peniko::ImageData` of one
/// uniform color -- M22 Phase 2's own content-fit tests only need to
/// check *where* the image's own pixels land relative to the node's
/// box and its `background`, not per-pixel content, so a uniform fill
/// (unlike `green_2x2_image_data`'s own checkerboard-adjacent use) is
/// the simplest real image that still proves real cropping/
/// letterboxing geometry, not just "something painted somewhere."
fn solid_image_data(width: u32, height: u32, color: [u8; 4]) -> peniko::ImageData {
    let mut bytes = Vec::with_capacity((width * height * 4) as usize);
    for _ in 0..(width * height) {
        bytes.extend_from_slice(&color);
    }
    peniko::ImageData {
        data: peniko::Blob::from(bytes),
        format: peniko::ImageFormat::Rgba8,
        alpha_type: peniko::ImageAlphaType::Alpha,
        width,
        height,
    }
}

const GREEN: [u8; 4] = [0x00, 0xFF, 0x00, 0xFF];

/// A 4x4 (square) solid-green image in a real, non-square 100x50 box
/// -- `content_fit` picks apart `Contain` (letterboxes -- background
/// visible on the box's own longer axis) from `Cover` (crops -- the
/// box is fully covered, no background visible anywhere inside it).
fn build_tree_with_fit(content_fit: ContentFit) -> (Tree, engine_core::NodeId) {
    let mut tree = Tree::new();
    let root = tree.insert(
        NodeKind::Rect,
        Style {
            size: Size {
                width: length(100.0),
                height: length(50.0),
            },
            ..Default::default()
        },
        PaintProperties::new(BACKGROUND, 0.0, 0.0, 1.0),
    );

    let mut state = ImageState::new(solid_image_data(4, 4, GREEN));
    state.content_fit = content_fit;
    let image = tree.insert(
        NodeKind::Image(state),
        Style {
            size: Size {
                width: length(100.0),
                height: length(50.0),
            },
            ..Default::default()
        },
        PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 0.0, 1.0),
    );
    tree.add_child(root, image);

    let available = Size {
        width: AvailableSpace::Definite(100.0),
        height: AvailableSpace::Definite(50.0),
    };
    tree.compute_layout(root, available);
    (tree, root)
}

#[test]
fn content_fit_contain_letterboxes_the_boxs_own_longer_axis() {
    pollster::block_on(async {
        let (tree, root) = build_tree_with_fit(ContentFit::Contain);
        let (data, bpr) = render(&tree, root, 100, 50).await;

        // A real 4x4 image uniformly scaled to fit inside a 100x50 box
        // (scale = min(100/4, 50/4) = 12.5) draws a real 50x50 square,
        // centered -- x in [25, 75), y in [0, 50). (10, 25) is well
        // inside the real left letterbox bar; (50, 25) is the box's
        // own real center, well inside the drawn square.
        let letterbox = pixel_at(&data, bpr, 10, 25);
        assert_eq!(
            letterbox,
            [0x11, 0x11, 0x11, 0xFF],
            "Contain must leave the box's own real background visible in its letterbox bars, \
             got {letterbox:?}"
        );
        let center = pixel_at(&data, bpr, 50, 25);
        assert_eq!(
            center, GREEN,
            "Contain must still paint the real image at the box's own center, got {center:?}"
        );
    });
}

#[test]
fn content_fit_cover_fills_the_box_with_no_letterboxing() {
    pollster::block_on(async {
        let (tree, root) = build_tree_with_fit(ContentFit::Cover);
        let (data, bpr) = render(&tree, root, 100, 50).await;

        // Cover crops (never letterboxes) -- every real point inside
        // the 100x50 box, including near its own edges, must be the
        // image's own real color, not the background `Contain`'s own
        // test just proved shows through at the identical point.
        let near_left_edge = pixel_at(&data, bpr, 10, 25);
        assert_eq!(
            near_left_edge, GREEN,
            "Cover must fill the box's own full extent with no real background showing \
             through, got {near_left_edge:?}"
        );
        let center = pixel_at(&data, bpr, 50, 25);
        assert_eq!(
            center, GREEN,
            "Cover must still paint the real image at the box's own center, got {center:?}"
        );
    });
}

#[test]
fn an_image_node_paints_the_real_loaded_pixel_color() {
    pollster::block_on(async {
        let (tree, root) = build_tree();
        let (data, bpr) = render(&tree, root, 100, 100).await;

        // The image node occupies (0,0)-(40,40); (20,20) is well inside
        // it, far from any edge-sampling softness.
        let inside = pixel_at(&data, bpr, 20, 20);
        assert_eq!(
            inside,
            [0x00, 0xFF, 0x00, 0xFF],
            "a point inside the Image node must show the real loaded image's own color, \
             got {inside:?}"
        );

        // (80, 80) is well outside the 40x40 image node -- plain root
        // background only.
        let outside = pixel_at(&data, bpr, 80, 80);
        assert_eq!(
            outside,
            [0x11, 0x11, 0x11, 0xFF],
            "a point outside the Image node must show only plain background, got {outside:?}"
        );
    });
}

/// M25 Phase 2 (§5, §6): a real, previously-missing compounding --
/// `Scene::draw_texture_rects` has no opacity parameter of its own at
/// all, so `PaintProperties.opacity` was silently ignored for every
/// `Image` node before this. Same range-check pattern `animated_
/// rect.rs`'s own mid-flight test already established.
#[test]
fn an_image_node_compounds_with_the_nodes_own_real_opacity() {
    pollster::block_on(async {
        let mut tree = Tree::new();
        let root = tree.insert(
            NodeKind::Rect,
            Style {
                size: Size {
                    width: length(100.0),
                    height: length(100.0),
                },
                ..Default::default()
            },
            PaintProperties::new(BACKGROUND, 0.0, 0.0, 1.0),
        );

        let image = tree.insert(
            NodeKind::Image(ImageState::new(green_2x2_image_data())),
            Style {
                size: Size {
                    width: length(100.0),
                    height: length(100.0),
                },
                ..Default::default()
            },
            // opacity = 0.5, the fourth positional field.
            PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 0.0, 0.5),
        );
        tree.add_child(root, image);

        let available = Size {
            width: AvailableSpace::Definite(100.0),
            height: AvailableSpace::Definite(100.0),
        };
        tree.compute_layout(root, available);

        let (data, bpr) = render(&tree, root, 100, 100).await;
        let center = pixel_at(&data, bpr, 50, 50);

        let observed_g = f32::from(center[1]);
        assert!(
            observed_g > f32::from(0x11_u8) && observed_g < f32::from(0xFF_u8),
            "green channel {observed_g} is not strictly between background (0x11) and the \
             image's own full-alpha green (0xFF) -- the Image paint isn't compounding with \
             the node's own real opacity, got {center:?}"
        );
    });
}
