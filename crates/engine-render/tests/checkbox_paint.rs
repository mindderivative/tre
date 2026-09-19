//! M14 Phase 1 (§5, §7.3): the standalone proof that `NodeKind::
//! Checkbox` genuinely paints a real box plus a real checkmark, driven
//! by `check_progress` -- not just that the match arm compiles. Same
//! headless render-to-texture-then-readback discipline as
//! `shape_morph_paint.rs`.
//!
//! Two claims, kept deliberately separate:
//!
//! 1. `check_progress` at `0.0` (unchecked) paints the box's own plain
//!    background fill, with no checkmark visible anywhere -- a real
//!    "nothing extra painted" claim, not merely "didn't crash."
//! 2. `check_progress` at `1.0` (checked) paints a real, fully-opaque
//!    white checkmark on top of the box, at a point squarely on the
//!    real tick path `paint_node`'s own arm draws.

use engine_core::{CheckboxState, NodeKind, PaintProperties, Tree};
use engine_render::{FrameRenderer, GeometryCache, TextRenderer, build_tree_scene};
use peniko::Color;
use taffy::prelude::{AvailableSpace, Size, Style, length};
use vello_hybrid::{RenderSize, RenderTargetConfig};

const BACKGROUND: Color = Color::from_rgba8(0x11, 0x11, 0x11, 0xFF);
const BOX_COLOR: Color = Color::from_rgba8(0x63, 0x50, 0xA4, 0xFF);

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
            label: Some("engine-render checkbox-paint test device"),
            required_features: wgpu::Features::empty(),
            ..Default::default()
        })
        .await
        .expect("failed to create wgpu device");

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("checkbox-paint test target"),
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

fn build_tree(checked: bool) -> (Tree, engine_core::NodeId) {
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

    let checkbox = tree.insert(
        NodeKind::Checkbox(CheckboxState::new(checked)),
        Style {
            size: Size {
                width: length(100.0),
                height: length(100.0),
            },
            ..Default::default()
        },
        PaintProperties::new(BOX_COLOR, 0.0, 0.0, 1.0),
    );
    tree.add_child(root, checkbox);

    let available = Size {
        width: AvailableSpace::Definite(100.0),
        height: AvailableSpace::Definite(100.0),
    };
    tree.compute_layout(root, available);
    (tree, root)
}

#[test]
fn an_unchecked_checkbox_paints_the_box_with_no_visible_checkmark() {
    pollster::block_on(async {
        let (tree, root) = build_tree(false);
        let (data, bpr) = render(&tree, root, 100, 100).await;

        // A point squarely on the real tick path's own second segment
        // (control points (42,75)-(80,25), midpoint ~(61,50)) -- if a
        // checkmark were painted here regardless of check_progress,
        // this would catch it.
        let mark_point = pixel_at(&data, bpr, 61, 50);
        assert_eq!(
            mark_point,
            [0x63, 0x50, 0xA4, 0xFF],
            "an unchecked box (check_progress 0.0) must show only its own plain fill, \
             no checkmark, got {mark_point:?}"
        );
    });
}

#[test]
fn a_checked_checkbox_paints_a_real_visible_checkmark() {
    pollster::block_on(async {
        let (tree, root) = build_tree(true);
        let (data, bpr) = render(&tree, root, 100, 100).await;

        let mark_point = pixel_at(&data, bpr, 61, 50);
        assert_eq!(
            mark_point,
            [0xFF, 0xFF, 0xFF, 0xFF],
            "a fully-checked box (check_progress 1.0) must paint a real, fully-opaque white \
             checkmark on top of its own fill at a point squarely on the real tick path, \
             got {mark_point:?}"
        );

        // A corner well outside the tick path entirely -- still the
        // box's own plain fill, proving the checkmark doesn't paint
        // over the whole node.
        let corner = pixel_at(&data, bpr, 90, 90);
        assert_eq!(
            corner,
            [0x63, 0x50, 0xA4, 0xFF],
            "a point away from the real tick path must still be the box's own plain fill, \
             got {corner:?}"
        );
    });
}

/// M25 Phase 2 (§5, §6): a real, previously-missing compounding -- the
/// checkmark's own real alpha multiplied only `check_progress` before
/// this, never `node.paint.opacity.current` too, so a checked
/// checkbox mid-fade-out would show its checkmark at full alpha while
/// its box correctly faded. Proven comparatively (opacity 1.0 vs.
/// 0.5 must paint genuinely different mark pixels) rather than
/// predicting an exact blended byte value, the same real reasoning
/// `animated_rect.rs`'s own mid-flight test already uses.
#[test]
fn a_checked_checkboxs_own_checkmark_compounds_with_the_nodes_real_opacity() {
    pollster::block_on(async {
        fn build_tree_with_opacity(opacity: f64) -> (Tree, engine_core::NodeId) {
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
            let checkbox = tree.insert(
                NodeKind::Checkbox(CheckboxState::new(true)),
                Style {
                    size: Size {
                        width: length(100.0),
                        height: length(100.0),
                    },
                    ..Default::default()
                },
                PaintProperties::new(BOX_COLOR, 0.0, 0.0, opacity),
            );
            tree.add_child(root, checkbox);
            let available = Size {
                width: AvailableSpace::Definite(100.0),
                height: AvailableSpace::Definite(100.0),
            };
            tree.compute_layout(root, available);
            (tree, root)
        }

        let (full_tree, full_root) = build_tree_with_opacity(1.0);
        let (full_data, full_bpr) = render(&full_tree, full_root, 100, 100).await;
        let full_mark = pixel_at(&full_data, full_bpr, 61, 50);

        let (faded_tree, faded_root) = build_tree_with_opacity(0.5);
        let (faded_data, faded_bpr) = render(&faded_tree, faded_root, 100, 100).await;
        let faded_mark = pixel_at(&faded_data, faded_bpr, 61, 50);

        assert_ne!(
            full_mark, faded_mark,
            "a checked checkbox's own checkmark must paint genuinely different pixels at \
             node opacity 1.0 ({full_mark:?}) vs. 0.5 ({faded_mark:?}) -- the checkmark isn't \
             compounding with the node's own real opacity"
        );
    });
}

/// M20 Phase 1 (§7.1, §7.3): the real, definitive proof `mark_tint` is
/// genuinely read at paint time, not just stored -- a real, non-
/// default tint (as `Window.set_theme` would push via `Tree::
/// set_all_component_tints`) must reach the actual painted pixel.
#[test]
fn a_themed_checkbox_paints_the_real_resolved_tint_not_the_default() {
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

        let mut state = CheckboxState::new(true);
        state.mark_tint = Color::from_rgba8(0x00, 0xFF, 0x00, 0xFF);
        let checkbox = tree.insert(
            NodeKind::Checkbox(state),
            Style {
                size: Size {
                    width: length(100.0),
                    height: length(100.0),
                },
                ..Default::default()
            },
            PaintProperties::new(BOX_COLOR, 0.0, 0.0, 1.0),
        );
        tree.add_child(root, checkbox);

        let available = Size {
            width: AvailableSpace::Definite(100.0),
            height: AvailableSpace::Definite(100.0),
        };
        tree.compute_layout(root, available);

        let (data, bpr) = render(&tree, root, 100, 100).await;
        let mark_point = pixel_at(&data, bpr, 61, 50);
        assert_eq!(
            mark_point,
            [0x00, 0xFF, 0x00, 0xFF],
            "a real, non-default mark_tint must reach the actual painted checkmark pixel, \
             got {mark_point:?}"
        );
    });
}
