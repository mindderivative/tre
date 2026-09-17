//! M14 Phase 2 (§5, §7.3): the standalone proof that `NodeKind::Slider`
//! genuinely paints a real track plus a real thumb whose own position
//! is driven by `thumb_position` -- not just that the match arm
//! compiles. Same headless render-to-texture-then-readback discipline
//! as `checkbox_paint.rs`.
//!
//! Two claims, kept deliberately separate:
//!
//! 1. A thumb at `thumb_position = 0.0` paints at the slider's own
//!    left edge -- a real, distinct position, not a fixed spot.
//! 2. A thumb at `thumb_position = 1.0` paints at the slider's own
//!    right edge, genuinely different from claim 1's position.

use engine_core::{NodeKind, PaintProperties, SliderState, Tree};
use engine_render::{FrameRenderer, TextRenderer, build_tree_scene};
use peniko::Color;
use taffy::prelude::{AvailableSpace, Size, Style, length};
use vello_hybrid::{RenderSize, RenderTargetConfig};

const BACKGROUND: Color = Color::from_rgba8(0x11, 0x11, 0x11, 0xFF);
const THUMB_COLOR: Color = Color::from_rgba8(0x03, 0xDA, 0xC6, 0xFF);

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
            label: Some("engine-render slider-paint test device"),
            required_features: wgpu::Features::empty(),
            ..Default::default()
        })
        .await
        .expect("failed to create wgpu device");

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("slider-paint test target"),
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

fn build_tree(thumb_position: f64) -> (Tree, engine_core::NodeId) {
    let mut tree = Tree::new();
    let root = tree.insert(
        NodeKind::Rect,
        Style {
            size: Size {
                width: length(200.0),
                height: length(40.0),
            },
            ..Default::default()
        },
        PaintProperties::new(BACKGROUND, 0.0, 0.0, 1.0),
    );

    let slider = tree.insert(
        NodeKind::Slider(SliderState::new(thumb_position)),
        Style {
            size: Size {
                width: length(200.0),
                height: length(40.0),
            },
            ..Default::default()
        },
        PaintProperties::new(THUMB_COLOR, 0.0, 0.0, 1.0),
    );
    tree.add_child(root, slider);

    let available = Size {
        width: AvailableSpace::Definite(200.0),
        height: AvailableSpace::Definite(40.0),
    };
    tree.compute_layout(root, available);
    (tree, root)
}

#[test]
fn a_thumb_at_zero_paints_at_the_real_left_edge() {
    pollster::block_on(async {
        let (tree, root) = build_tree(0.0);
        let (data, bpr) = render(&tree, root, 200, 40).await;

        let left_edge = pixel_at(&data, bpr, 5, 20);
        assert_eq!(
            left_edge,
            [0x03, 0xDA, 0xC6, 0xFF],
            "thumb_position 0.0 must paint the real thumb at the slider's own left edge, \
             got {left_edge:?}"
        );

        // y=5 is outside the track's own real vertical band (y=17..23
        // for a 40px-tall node) and, at x=195, far outside the thumb's
        // own 16px-radius reach when it's sitting at the *left* edge --
        // so only plain background should show here.
        let right_edge = pixel_at(&data, bpr, 195, 5);
        assert_eq!(
            right_edge,
            [0x11, 0x11, 0x11, 0xFF],
            "a point away from both the track's own band and the thumb (at the left edge) \
             must show only plain background, got {right_edge:?}"
        );
    });
}

#[test]
fn a_thumb_at_one_paints_at_the_real_right_edge() {
    pollster::block_on(async {
        let (tree, root) = build_tree(1.0);
        let (data, bpr) = render(&tree, root, 200, 40).await;

        let right_edge = pixel_at(&data, bpr, 195, 20);
        assert_eq!(
            right_edge,
            [0x03, 0xDA, 0xC6, 0xFF],
            "thumb_position 1.0 must paint the real thumb at the slider's own right edge, \
             got {right_edge:?}"
        );

        // Same off-track-band, off-thumb-reach point as the other test,
        // mirrored to the left side now that the thumb sits at the
        // right edge instead.
        let left_edge = pixel_at(&data, bpr, 5, 5);
        assert_eq!(
            left_edge,
            [0x11, 0x11, 0x11, 0xFF],
            "a point away from both the track's own band and the thumb (at the right edge) \
             must show only plain background, got {left_edge:?}"
        );
    });
}

/// M20 Phase 1 (§7.1, §7.3): the track's own real default color --
/// byte-for-byte the historical hardcoded literal, proving `Slider
/// State::new`'s own default keeps zero visual change from before this
/// phase. Thumb at 0.0 (near x=0) leaves x=100 (track band, y=20) a
/// pure track-only pixel, well outside the thumb's own 16px reach.
#[test]
fn an_unthemed_slider_paints_the_real_default_gray_track() {
    pollster::block_on(async {
        let (tree, root) = build_tree(0.0);
        let (data, bpr) = render(&tree, root, 200, 40).await;

        let track_point = pixel_at(&data, bpr, 100, 20);
        assert_eq!(
            track_point,
            [0x79, 0x74, 0x7A, 0xFF],
            "an un-themed slider must paint the real, historical default gray track, \
             got {track_point:?}"
        );
    });
}

/// M20 Phase 1 (§7.1, §7.3): the real, definitive proof `track_tint`
/// is genuinely read at paint time, not just stored -- a real, non-
/// default tint (as `Window.set_theme` would push via `Tree::
/// set_all_component_tints`) must reach the actual painted track
/// pixel.
#[test]
fn a_themed_slider_paints_the_real_resolved_track_tint() {
    pollster::block_on(async {
        let mut tree = Tree::new();
        let root = tree.insert(
            NodeKind::Rect,
            Style {
                size: Size {
                    width: length(200.0),
                    height: length(40.0),
                },
                ..Default::default()
            },
            PaintProperties::new(BACKGROUND, 0.0, 0.0, 1.0),
        );

        let mut state = SliderState::new(0.0);
        state.track_tint = Color::from_rgba8(0x00, 0xFF, 0x00, 0xFF);
        let slider = tree.insert(
            NodeKind::Slider(state),
            Style {
                size: Size {
                    width: length(200.0),
                    height: length(40.0),
                },
                ..Default::default()
            },
            PaintProperties::new(THUMB_COLOR, 0.0, 0.0, 1.0),
        );
        tree.add_child(root, slider);

        let available = Size {
            width: AvailableSpace::Definite(200.0),
            height: AvailableSpace::Definite(40.0),
        };
        tree.compute_layout(root, available);

        let (data, bpr) = render(&tree, root, 200, 40).await;
        let track_point = pixel_at(&data, bpr, 100, 20);
        assert_eq!(
            track_point,
            [0x00, 0xFF, 0x00, 0xFF],
            "a real, non-default track_tint must reach the actual painted track pixel, \
             got {track_point:?}"
        );
    });
}

/// M25 Phase 2 (§5, §6): a real, previously-missing compounding --
/// only the thumb multiplied by `node.paint.opacity.current` before
/// this; the track painted its own real `track_tint` raw, a genuine
/// internal inconsistency within one `NodeKind`. Same range-check
/// pattern `animated_rect.rs`'s own mid-flight test already
/// established.
#[test]
fn a_faded_slider_compounds_the_nodes_own_real_opacity_into_the_track_too() {
    pollster::block_on(async {
        let mut tree = Tree::new();
        let root = tree.insert(
            NodeKind::Rect,
            Style {
                size: Size {
                    width: length(200.0),
                    height: length(40.0),
                },
                ..Default::default()
            },
            PaintProperties::new(BACKGROUND, 0.0, 0.0, 1.0),
        );

        // Thumb at 0.0 (near x=0) keeps x=100 (track band, y=20) a
        // pure track-only pixel, well outside the thumb's own reach --
        // the same real isolation `an_unthemed_slider_paints_the_real_
        // default_gray_track` already established.
        let slider = tree.insert(
            NodeKind::Slider(SliderState::new(0.0)),
            Style {
                size: Size {
                    width: length(200.0),
                    height: length(40.0),
                },
                ..Default::default()
            },
            // opacity = 0.5, the fourth positional field.
            PaintProperties::new(THUMB_COLOR, 0.0, 0.0, 0.5),
        );
        tree.add_child(root, slider);

        let available = Size {
            width: AvailableSpace::Definite(200.0),
            height: AvailableSpace::Definite(40.0),
        };
        tree.compute_layout(root, available);

        let (data, bpr) = render(&tree, root, 200, 40).await;
        let track_point = pixel_at(&data, bpr, 100, 20);

        // The real default gray track is 0x79 -- a faded track at
        // 0.5 opacity, blended over 0x11 background, must land
        // strictly between the two, not stay at the full 0x79.
        let observed_r = f32::from(track_point[0]);
        assert!(
            observed_r > f32::from(0x11_u8) && observed_r < f32::from(0x79_u8),
            "track red channel {observed_r} is not strictly between background (0x11) and \
             the real full-opacity default gray (0x79) -- the track isn't compounding with \
             the node's own real opacity, got {track_point:?}"
        );
    });
}
