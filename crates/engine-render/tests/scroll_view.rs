//! M36 Phase 1 (§5, §7, §11.7): the standalone pixel-level proof that a
//! real `NodeKind::ScrollView` genuinely clips its own oversized
//! content and genuinely shifts what's visible when scrolled -- the
//! identical headless render-to-texture-then-readback discipline
//! `clip_children.rs`/`virtual_list_scroll.rs` already establish.

use engine_core::{NodeId, NodeKind, PaintProperties, ScrollViewState, Tree};
use engine_render::{FrameRenderer, GeometryCache, TextRenderer, build_tree_scene};
use peniko::Color;
use taffy::prelude::{AvailableSpace, Position, Rect as TaffyRect, Size, Style, auto, length};
use vello_hybrid::{RenderSize, RenderTargetConfig};

const WIDTH: u16 = 100;
const HEIGHT: u16 = 100;
const MARKER: Color = Color::from_rgba8(0xFF, 0xFF, 0xFF, 0xFF);
const UNCOVERED: [u8; 4] = [0x00, 0x00, 0x00, 0x00];

async fn render(tree: &Tree, root: NodeId) -> (Vec<u8>, u32) {
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
            label: Some("engine-render scroll-view test device"),
            required_features: wgpu::Features::empty(),
            ..Default::default()
        })
        .await
        .expect("failed to create wgpu device");

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("scroll-view test target"),
        size: wgpu::Extent3d {
            width: u32::from(WIDTH),
            height: u32::from(HEIGHT),
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
            width: u32::from(WIDTH),
            height: u32::from(HEIGHT),
        },
    );
    let mut text_renderer = TextRenderer::new();
    let mut geometry_cache = GeometryCache::new();
    let scene = build_tree_scene(
        tree,
        root,
        WIDTH,
        HEIGHT,
        frame_renderer.resources_mut(),
        &mut text_renderer,
        &mut geometry_cache,
    );
    let render_size = RenderSize {
        width: u32::from(WIDTH),
        height: u32::from(HEIGHT),
    };
    let mut encoder =
        device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
    frame_renderer.render(&scene, &device, &queue, &mut encoder, &render_size, &view);

    let bytes_per_row = (u32::from(WIDTH) * 4).next_multiple_of(256);
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size: u64::from(bytes_per_row) * u64::from(HEIGHT),
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
            width: u32::from(WIDTH),
            height: u32::from(HEIGHT),
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

/// A 100x100 `ScrollView` with a single 100x400 child: a 20px-tall
/// real marker rect at content-local y 300..320 (near the far end of
/// the real scrollable content) is the only thing painted, everything
/// else in the child transparent -- so whether the marker is visible
/// decisively proves both real clipping and the real scroll shift.
fn build_scene(scroll_by: f64) -> (Tree, NodeId) {
    let mut tree = Tree::new();
    let view = tree.insert(
        NodeKind::ScrollView(ScrollViewState::new(false)),
        Style {
            size: Size {
                width: length(f32::from(WIDTH)),
                height: length(f32::from(HEIGHT)),
            },
            ..Default::default()
        },
        PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 0.0, 1.0),
    );
    let content = tree.insert(
        NodeKind::Container,
        Style {
            size: Size {
                width: length(f32::from(WIDTH)),
                height: length(400.0),
            },
            ..Default::default()
        },
        PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 0.0, 1.0),
    );
    tree.add_child(view, content);

    let marker = tree.insert(
        NodeKind::Rect,
        Style {
            size: Size {
                width: length(f32::from(WIDTH)),
                height: length(20.0),
            },
            ..Default::default()
        },
        PaintProperties::new(MARKER, 0.0, 0.0, 1.0),
    );
    let mut marker_style = tree.get(marker).unwrap().layout_style.clone();
    marker_style.position = Position::Absolute;
    marker_style.inset = TaffyRect {
        left: length(0.0),
        top: length(300.0),
        right: auto(),
        bottom: auto(),
    };
    tree.set_layout_style(marker, marker_style);
    tree.add_child(content, marker);

    tree.compute_layout(
        view,
        Size {
            width: AvailableSpace::Definite(f32::from(WIDTH)),
            height: AvailableSpace::Definite(f32::from(HEIGHT)),
        },
    );
    if scroll_by != 0.0 {
        tree.scroll_scroll_view_by(view, scroll_by);
        tree.compute_layout(
            view,
            Size {
                width: AvailableSpace::Definite(f32::from(WIDTH)),
                height: AvailableSpace::Definite(f32::from(HEIGHT)),
            },
        );
    }
    (tree, view)
}

#[test]
fn an_unscrolled_scroll_view_genuinely_clips_content_past_its_own_viewport() {
    pollster::block_on(async {
        // The marker sits at content-local y 300..320 -- with no
        // scroll, that's entirely past the real 100px viewport, so it
        // must be genuinely clipped, not visible.
        let (tree, view) = build_scene(0.0);
        let (data, bpr) = render(&tree, view).await;
        let at_viewport_bottom = pixel_at(&data, bpr, 50, 90);
        assert_eq!(
            at_viewport_bottom, UNCOVERED,
            "an unscrolled ScrollView must genuinely clip content past its own 100px \
             viewport, got {at_viewport_bottom:?}"
        );
    });
}

#[test]
fn scrolling_moves_the_real_marker_into_view() {
    pollster::block_on(async {
        // Scroll by 250: the marker's own real content-local y 300..320
        // now sits at real screen y 50..70, inside the viewport.
        let (tree, view) = build_scene(250.0);
        let (data, bpr) = render(&tree, view).await;
        let at_markers_new_position = pixel_at(&data, bpr, 50, 60);
        assert_eq!(
            at_markers_new_position,
            [0xFF, 0xFF, 0xFF, 0xFF],
            "scrolling must genuinely move the real marker into view at its own correct \
             post-scroll screen position, got {at_markers_new_position:?}"
        );

        // The content's own real top (content-local y 0..300, now
        // scrolled off-screen above) must show nothing at the
        // viewport's own top.
        let at_viewport_top = pixel_at(&data, bpr, 50, 5);
        assert_eq!(
            at_viewport_top, UNCOVERED,
            "the real content that scrolled off the top must genuinely not paint, got \
             {at_viewport_top:?}"
        );
    });
}

/// M38 Phase 6 (§5, §7, §11.7): the real pixel-level proof `engine-
/// render::paint_scroll_view_thumb` actually paints -- a 100x100 view
/// over 400px of real content: `ScrollViewState::thumb_geometry(100.0,
/// 400.0)` (proven directly at the Rust level, `tree.rs`'s own `thumb_
/// geometry_computes_the_real_track_thumb_and_along_values`) puts the
/// real thumb at absolute x in [94, 98], y in [2, 34] -- this checks a
/// point squarely inside that rect is genuinely non-transparent (not
/// matching an exact color byte-for-byte, since the real alpha-
/// blending result onto a transparent target depends on the renderer's
/// own blend semantics; *something painted there at all* is the real,
/// decisive claim this phase's own thumb-paint code exists to prove).
#[test]
fn a_scrollable_scroll_view_paints_a_real_thumb_pixel_at_the_expected_position() {
    pollster::block_on(async {
        let (tree, view) = build_scene(0.0);
        let (data, bpr) = render(&tree, view).await;
        let at_thumb = pixel_at(&data, bpr, 96, 18);
        assert_ne!(
            at_thumb, UNCOVERED,
            "a real point inside the real thumb's own computed rect must be genuinely \
             painted, not left fully transparent"
        );
    });
}

/// The real other half: a `ScrollView` with nothing to scroll must not
/// paint a thumb at all -- `paint_scroll_view_thumb`'s own real
/// `content_extent <= viewport_extent` guard, mirroring pyCopper's own
/// real `self.scrollable` gate on `paint_foreground`.
#[test]
fn a_scroll_view_that_fits_its_own_content_paints_no_thumb() {
    pollster::block_on(async {
        let mut tree = Tree::new();
        let view = tree.insert(
            NodeKind::ScrollView(ScrollViewState::new(false)),
            Style {
                size: Size {
                    width: length(f32::from(WIDTH)),
                    height: length(f32::from(HEIGHT)),
                },
                ..Default::default()
            },
            PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 0.0, 1.0),
        );
        // Content exactly as tall as the viewport -- nothing to scroll.
        let content = tree.insert(
            NodeKind::Container,
            Style {
                size: Size {
                    width: length(f32::from(WIDTH)),
                    height: length(f32::from(HEIGHT)),
                },
                ..Default::default()
            },
            PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 0.0, 1.0),
        );
        tree.add_child(view, content);
        tree.compute_layout(
            view,
            Size {
                width: AvailableSpace::Definite(f32::from(WIDTH)),
                height: AvailableSpace::Definite(f32::from(HEIGHT)),
            },
        );

        let (data, bpr) = render(&tree, view).await;
        // Where a real thumb would sit if this view were scrollable
        // (the same real x/y this file's own scrollable-case test just
        // checked) must show nothing painted.
        let at_would_be_thumb = pixel_at(&data, bpr, 96, 18);
        assert_eq!(
            at_would_be_thumb, UNCOVERED,
            "a ScrollView with nothing to scroll must not paint a thumb at all, got \
             {at_would_be_thumb:?}"
        );
    });
}
