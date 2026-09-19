//! M30 Phase 2 Step 1 (§5, §7.3): the standalone proof that `NodeKind::
//! RadioButton` genuinely paints a real stroked ring plus a real
//! scaling inner dot, driven by `select_progress` -- not just that the
//! match arm compiles. Same headless render-to-texture-then-readback
//! discipline as `checkbox_paint.rs`, and deliberately mirrors its own
//! three-claim structure (unselected/selected/themed).

use engine_core::{NodeKind, PaintProperties, RadioButtonState, Tree};
use engine_render::{FrameRenderer, GeometryCache, TextRenderer, build_tree_scene};
use peniko::Color;
use taffy::prelude::{AvailableSpace, Size, Style, length};
use vello_hybrid::{RenderSize, RenderTargetConfig};

const BACKGROUND: Color = Color::from_rgba8(0x11, 0x11, 0x11, 0xFF);
const UNSELECTED: [u8; 4] = [0x79, 0x74, 0x7E, 0xFF];
const SELECTED: [u8; 4] = [0x67, 0x50, 0xA4, 0xFF];

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
            label: Some("engine-render radio-button-paint test device"),
            required_features: wgpu::Features::empty(),
            ..Default::default()
        })
        .await
        .expect("failed to create wgpu device");

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("radio-button-paint test target"),
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

fn build_tree(selected: bool) -> (Tree, engine_core::NodeId) {
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

    let mut state = RadioButtonState::new(selected);
    state.unselected_tint =
        Color::from_rgba8(UNSELECTED[0], UNSELECTED[1], UNSELECTED[2], UNSELECTED[3]);
    state.selected_tint = Color::from_rgba8(SELECTED[0], SELECTED[1], SELECTED[2], SELECTED[3]);
    let radio = tree.insert(
        NodeKind::RadioButton(state),
        Style {
            size: Size {
                width: length(100.0),
                height: length(100.0),
            },
            ..Default::default()
        },
        PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 0.0, 1.0),
    );
    tree.add_child(root, radio);

    let available = Size {
        width: AvailableSpace::Definite(100.0),
        height: AvailableSpace::Definite(100.0),
    };
    tree.compute_layout(root, available);
    (tree, root)
}

#[test]
fn an_unselected_radio_button_paints_the_unselected_ring_with_no_visible_dot() {
    pollster::block_on(async {
        let (tree, root) = build_tree(false);
        let (data, bpr) = render(&tree, root, 100, 100).await;

        // On the real stroke band (ring_radius = 45.0, stroke_width =
        // 10.0, centered at (50, 50)) -- directly above center.
        let ring_point = pixel_at(&data, bpr, 50, 5);
        assert_eq!(
            ring_point, UNSELECTED,
            "an unselected ring (select_progress 0.0) must paint its own unselected_tint \
             on the stroke band, got {ring_point:?}"
        );

        // Dead center -- dot_radius is 0.0 at select_progress 0.0, so
        // this must still be the root's own plain background, not any
        // dot color.
        let center = pixel_at(&data, bpr, 50, 50);
        assert_eq!(
            center,
            [0x11, 0x11, 0x11, 0xFF],
            "an unselected radio button must show no dot at its own center, got {center:?}"
        );
    });
}

#[test]
fn a_selected_radio_button_paints_a_real_visible_dot_and_the_selected_ring_color() {
    pollster::block_on(async {
        let (tree, root) = build_tree(true);
        let (data, bpr) = render(&tree, root, 100, 100).await;

        let center = pixel_at(&data, bpr, 50, 50);
        assert_eq!(
            center, SELECTED,
            "a fully-selected radio button (select_progress 1.0) must paint a real, \
             fully-opaque dot in its own selected_tint at its own center, got {center:?}"
        );

        let ring_point = pixel_at(&data, bpr, 50, 5);
        assert_eq!(
            ring_point, SELECTED,
            "at select_progress 1.0 the ring must have fully interpolated to \
             selected_tint too, got {ring_point:?}"
        );
    });
}

#[test]
fn a_themed_radio_button_paints_the_real_resolved_tints_not_a_hardcoded_default() {
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

        let mut state = RadioButtonState::new(true);
        state.unselected_tint = Color::from_rgba8(0x11, 0x22, 0x33, 0xFF);
        state.selected_tint = Color::from_rgba8(0x00, 0xFF, 0x00, 0xFF);
        let radio = tree.insert(
            NodeKind::RadioButton(state),
            Style {
                size: Size {
                    width: length(100.0),
                    height: length(100.0),
                },
                ..Default::default()
            },
            PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 0.0, 1.0),
        );
        tree.add_child(root, radio);

        let available = Size {
            width: AvailableSpace::Definite(100.0),
            height: AvailableSpace::Definite(100.0),
        };
        tree.compute_layout(root, available);

        let (data, bpr) = render(&tree, root, 100, 100).await;
        let center = pixel_at(&data, bpr, 50, 50);
        assert_eq!(
            center,
            [0x00, 0xFF, 0x00, 0xFF],
            "a real, non-default selected_tint must reach the actual painted dot pixel, \
             got {center:?}"
        );
    });
}
