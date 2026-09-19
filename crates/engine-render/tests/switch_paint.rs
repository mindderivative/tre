//! M30 Phase 2 Step 2 (§5, §7.3): the standalone proof that `NodeKind::
//! Switch` genuinely paints a real track plus a real handle that both
//! slides *and* grows, driven by `toggle_progress` -- not just that
//! the match arm compiles. Same headless render-to-texture-then-
//! readback discipline as `radio_button_paint.rs`.
//!
//! Track is 100x50 (a real 2:1 ratio, close to MD3's own real 52:32);
//! at these dimensions the handle center travels between `x = h*0.5 =
//! 25` (off) and `x = w - h*0.5 = 75` (on) -- see `paint_node`'s own
//! doc comment for why that formula is exact, not approximated.

use engine_core::{NodeKind, PaintProperties, SwitchState, Tree};
use engine_render::{FrameRenderer, GeometryCache, TextRenderer, build_tree_scene};
use peniko::Color;
use taffy::prelude::{AvailableSpace, Size, Style, length};
use vello_hybrid::{RenderSize, RenderTargetConfig};

const BACKGROUND: Color = Color::from_rgba8(0x11, 0x11, 0x11, 0xFF);
const TRACK_OFF: [u8; 4] = [0xE6, 0xE0, 0xE9, 0xFF];
const TRACK_ON: [u8; 4] = [0x67, 0x50, 0xA4, 0xFF];
const HANDLE_OFF: [u8; 4] = [0x79, 0x74, 0x7E, 0xFF];
const HANDLE_ON: [u8; 4] = [0xFF, 0xFF, 0xFF, 0xFF];

const WIDTH: u16 = 100;
const HEIGHT: u16 = 50;

async fn render(tree: &Tree, root: engine_core::NodeId) -> (Vec<u8>, u32) {
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
            label: Some("engine-render switch-paint test device"),
            required_features: wgpu::Features::empty(),
            ..Default::default()
        })
        .await
        .expect("failed to create wgpu device");

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("switch-paint test target"),
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

fn build_tree(on: bool) -> (Tree, engine_core::NodeId) {
    let mut tree = Tree::new();
    let root = tree.insert(
        NodeKind::Rect,
        Style {
            size: Size {
                width: length(f32::from(WIDTH)),
                height: length(f32::from(HEIGHT)),
            },
            ..Default::default()
        },
        PaintProperties::new(BACKGROUND, 0.0, 0.0, 1.0),
    );

    let mut state = SwitchState::new(on);
    state.track_off_tint =
        Color::from_rgba8(TRACK_OFF[0], TRACK_OFF[1], TRACK_OFF[2], TRACK_OFF[3]);
    state.track_on_tint = Color::from_rgba8(TRACK_ON[0], TRACK_ON[1], TRACK_ON[2], TRACK_ON[3]);
    state.track_outline_tint = Color::from_rgba8(0x40, 0x40, 0x40, 0xFF);
    state.handle_off_tint =
        Color::from_rgba8(HANDLE_OFF[0], HANDLE_OFF[1], HANDLE_OFF[2], HANDLE_OFF[3]);
    state.handle_on_tint =
        Color::from_rgba8(HANDLE_ON[0], HANDLE_ON[1], HANDLE_ON[2], HANDLE_ON[3]);
    let switch = tree.insert(
        NodeKind::Switch(state),
        Style {
            size: Size {
                width: length(f32::from(WIDTH)),
                height: length(f32::from(HEIGHT)),
            },
            ..Default::default()
        },
        PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 0.0, 1.0),
    );
    tree.add_child(root, switch);

    let available = Size {
        width: AvailableSpace::Definite(f32::from(WIDTH)),
        height: AvailableSpace::Definite(f32::from(HEIGHT)),
    };
    tree.compute_layout(root, available);
    (tree, root)
}

#[test]
fn an_off_switch_paints_the_off_track_and_a_handle_at_the_left() {
    pollster::block_on(async {
        let (tree, root) = build_tree(false);
        let (data, bpr) = render(&tree, root).await;

        // Track fill, well clear of the handle (right side, off).
        let track = pixel_at(&data, bpr, 90, 25);
        assert_eq!(
            track, TRACK_OFF,
            "an off switch (toggle_progress 0.0) must paint its own track_off_tint, \
             got {track:?}"
        );

        // Handle center: off = h*0.5 = 25.
        let handle = pixel_at(&data, bpr, 25, 25);
        assert_eq!(
            handle, HANDLE_OFF,
            "an off switch's handle must paint at the left (x=25) in handle_off_tint, \
             got {handle:?}"
        );

        // Where the ON handle would be (x=75) must still be plain
        // track, not any handle color -- proves the handle actually
        // moved, not just always painted everywhere.
        let no_handle_here = pixel_at(&data, bpr, 75, 25);
        assert_eq!(
            no_handle_here, TRACK_OFF,
            "no handle should paint at the on-position (x=75) while off, got {no_handle_here:?}"
        );
    });
}

#[test]
fn an_on_switch_paints_the_on_track_and_a_larger_handle_at_the_right() {
    pollster::block_on(async {
        let (tree, root) = build_tree(true);
        let (data, bpr) = render(&tree, root).await;

        let track = pixel_at(&data, bpr, 10, 25);
        assert_eq!(
            track, TRACK_ON,
            "an on switch (toggle_progress 1.0) must paint its own track_on_tint, \
             got {track:?}"
        );

        // Handle center: on = w - h*0.5 = 75.
        let handle = pixel_at(&data, bpr, 75, 25);
        assert_eq!(
            handle, HANDLE_ON,
            "an on switch's handle must paint at the right (x=75) in handle_on_tint, \
             got {handle:?}"
        );

        // The selected handle is a real 24/32 = 0.75 diameter ratio
        // (larger than the unselected 16/32 = 0.5) -- a point 15px
        // from the on-handle's own center (well inside its real
        // radius of h*0.375=18.75, but outside the unselected radius
        // of h*0.25=12.5) proves it actually grew, not just moved.
        let grown_handle_edge = pixel_at(&data, bpr, 75 + 15, 25);
        assert_eq!(
            grown_handle_edge, HANDLE_ON,
            "the selected handle's real larger radius must reach 15px from its own \
             center, got {grown_handle_edge:?}"
        );
    });
}
