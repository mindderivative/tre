//! M30 Phase 3 Step 2 (§5, §7): the standalone proof that `NodeKind::
//! LinearProgress`/`NodeKind::CircularProgress` genuinely paint a real
//! track/indicator driven by `value`, not just that the match arms
//! compile. Same headless render-to-texture-then-readback discipline
//! as `slider_paint.rs`/`radio_button_paint.rs`.

use engine_core::{CircularProgressState, LinearProgressState, NodeKind, PaintProperties, Tree};
use engine_render::{FrameRenderer, TextRenderer, build_tree_scene};
use peniko::Color;
use taffy::prelude::{AvailableSpace, Size, Style, length};
use vello_hybrid::{RenderSize, RenderTargetConfig};

const BACKGROUND: Color = Color::from_rgba8(0x11, 0x11, 0x11, 0xFF);
const TRACK: [u8; 4] = [0xE6, 0xE0, 0xE9, 0xFF];
const INDICATOR: [u8; 4] = [0x67, 0x50, 0xA4, 0xFF];

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
            label: Some("engine-render progress-paint test device"),
            required_features: wgpu::Features::empty(),
            ..Default::default()
        })
        .await
        .expect("failed to create wgpu device");

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("progress-paint test target"),
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
    let scene = build_tree_scene(
        tree,
        root,
        width,
        height,
        frame_renderer.resources_mut(),
        &mut text_renderer,
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

const LINEAR_WIDTH: u16 = 100;
const LINEAR_HEIGHT: u16 = 20;

fn build_linear_tree(value: f64) -> (Tree, engine_core::NodeId) {
    let mut tree = Tree::new();
    let root = tree.insert(
        NodeKind::Rect,
        Style {
            size: Size {
                width: length(f32::from(LINEAR_WIDTH)),
                height: length(f32::from(LINEAR_HEIGHT)),
            },
            ..Default::default()
        },
        PaintProperties::new(BACKGROUND, 0.0, 0.0, 1.0),
    );

    let mut state = LinearProgressState::new(value);
    state.track_tint = Color::from_rgba8(TRACK[0], TRACK[1], TRACK[2], TRACK[3]);
    state.indicator_tint =
        Color::from_rgba8(INDICATOR[0], INDICATOR[1], INDICATOR[2], INDICATOR[3]);
    let bar = tree.insert(
        NodeKind::LinearProgress(state),
        Style {
            size: Size {
                width: length(f32::from(LINEAR_WIDTH)),
                height: length(f32::from(LINEAR_HEIGHT)),
            },
            ..Default::default()
        },
        PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 0.0, 1.0),
    );
    tree.add_child(root, bar);

    let available = Size {
        width: AvailableSpace::Definite(f32::from(LINEAR_WIDTH)),
        height: AvailableSpace::Definite(f32::from(LINEAR_HEIGHT)),
    };
    tree.compute_layout(root, available);
    (tree, root)
}

#[test]
fn a_half_filled_linear_indicator_shows_the_indicator_on_the_left_and_track_on_the_right() {
    pollster::block_on(async {
        let (tree, root) = build_linear_tree(0.5);
        let (data, bpr) = render(&tree, root, LINEAR_WIDTH, LINEAR_HEIGHT).await;

        let filled = pixel_at(&data, bpr, 10, 10);
        assert_eq!(
            filled, INDICATOR,
            "at value 0.5, x=10 (well inside the left half) must be the real indicator \
             color, got {filled:?}"
        );

        let unfilled = pixel_at(&data, bpr, 90, 10);
        assert_eq!(
            unfilled, TRACK,
            "at value 0.5, x=90 (well inside the right half) must be the real track \
             color, got {unfilled:?}"
        );
    });
}

#[test]
fn a_zero_value_linear_indicator_shows_only_the_track() {
    pollster::block_on(async {
        let (tree, root) = build_linear_tree(0.0);
        let (data, bpr) = render(&tree, root, LINEAR_WIDTH, LINEAR_HEIGHT).await;

        for x in [1, 50, 98] {
            let px = pixel_at(&data, bpr, x, 10);
            assert_eq!(
                px, TRACK,
                "at value 0.0, every point must be the real track color, found \
                 {px:?} at x={x}"
            );
        }
    });
}

const CIRCULAR_SIZE: u16 = 100;

fn build_circular_tree(value: f64) -> (Tree, engine_core::NodeId) {
    let mut tree = Tree::new();
    let root = tree.insert(
        NodeKind::Rect,
        Style {
            size: Size {
                width: length(f32::from(CIRCULAR_SIZE)),
                height: length(f32::from(CIRCULAR_SIZE)),
            },
            ..Default::default()
        },
        PaintProperties::new(BACKGROUND, 0.0, 0.0, 1.0),
    );

    let mut state = CircularProgressState::new(value);
    state.indicator_tint =
        Color::from_rgba8(INDICATOR[0], INDICATOR[1], INDICATOR[2], INDICATOR[3]);
    let ring = tree.insert(
        NodeKind::CircularProgress(state),
        Style {
            size: Size {
                width: length(f32::from(CIRCULAR_SIZE)),
                height: length(f32::from(CIRCULAR_SIZE)),
            },
            ..Default::default()
        },
        PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 0.0, 1.0),
    );
    tree.add_child(root, ring);

    let available = Size {
        width: AvailableSpace::Definite(f32::from(CIRCULAR_SIZE)),
        height: AvailableSpace::Definite(f32::from(CIRCULAR_SIZE)),
    };
    tree.compute_layout(root, available);
    (tree, root)
}

#[test]
fn a_quarter_value_circular_indicator_paints_the_arc_from_twelve_to_three_oclock() {
    pollster::block_on(async {
        let (tree, root) = build_circular_tree(0.25);
        let (data, bpr) = render(&tree, root, CIRCULAR_SIZE, CIRCULAR_SIZE).await;
        let center = f64::from(CIRCULAR_SIZE) / 2.0;

        // 12 o'clock (top-center, the real start angle) -- on the arc.
        let top = pixel_at(&data, bpr, center as u32, 2);
        assert_eq!(
            top, INDICATOR,
            "at value 0.25, the arc must start at 12 o'clock (top-center), got {top:?}"
        );

        // 3 o'clock (right-center) -- a real 0.25 sweep (90 degrees
        // clockwise from 12) ends exactly here.
        let right = pixel_at(&data, bpr, (CIRCULAR_SIZE - 2) as u32, center as u32);
        assert_eq!(
            right, INDICATOR,
            "at value 0.25, the arc's own real 90-degree clockwise sweep must reach \
             3 o'clock (right-center), got {right:?}"
        );

        // 6 o'clock (bottom-center) -- well past a real 0.25 sweep,
        // must not be painted.
        let bottom = pixel_at(&data, bpr, center as u32, (CIRCULAR_SIZE - 2) as u32);
        assert_eq!(
            bottom,
            [0x11, 0x11, 0x11, 0xFF],
            "at value 0.25, 6 o'clock (bottom-center) is well past the real sweep and \
             must show the plain background, got {bottom:?}"
        );
    });
}

#[test]
fn a_zero_value_circular_indicator_paints_nothing() {
    pollster::block_on(async {
        let (tree, root) = build_circular_tree(0.0);
        let (data, bpr) = render(&tree, root, CIRCULAR_SIZE, CIRCULAR_SIZE).await;
        let center = (f64::from(CIRCULAR_SIZE) / 2.0) as u32;

        let top = pixel_at(&data, bpr, center, 2);
        assert_eq!(
            top,
            [0x11, 0x11, 0x11, 0xFF],
            "at value 0.0, no arc must be painted anywhere, found ink at top, got {top:?}"
        );
    });
}
