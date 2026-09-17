//! M7 Phase 2 (§7.2): the standalone proof that `PaintProperties.
//! elevation` genuinely paints a real MD3 shadow -- a real, animatable
//! `PaintProperties` field `paint_node` never read before this phase.
//! Same headless render-to-texture-then-readback discipline as every
//! other pixel-level proof in this crate.
//!
//! Two claims, kept deliberately separate:
//!
//! 1. `elevation == 0.0` (`PaintProperties::new`'s own default) paints
//!    no shadow at all -- byte-for-byte identical to a plain fill, the
//!    same "provably a no-op" standard every additive feature since M5
//!    Phase 1 has been held to.
//! 2. A real, positive `elevation` paints real shadow pixels below the
//!    node's own box -- not just "the node's own fill still renders."

use engine_core::{NodeKind, PaintProperties, Tree};
use engine_render::{FrameRenderer, TextRenderer, build_tree_scene};
use peniko::Color;
use peniko::kurbo::Affine;
use taffy::prelude::{AvailableSpace, Position, Rect as TaffyRect, Size, Style, auto, length};
use vello_hybrid::{RenderSize, RenderTargetConfig};

const BACKGROUND: Color = Color::from_rgba8(0x11, 0x11, 0x11, 0xFF);
const CHIP: Color = Color::from_rgba8(0xFF, 0xFF, 0xFF, 0xFF);

fn absolute(left: f32, top: f32, width: f32, height: f32) -> Style {
    Style {
        position: Position::Absolute,
        inset: TaffyRect {
            left: length(left),
            top: length(top),
            right: auto(),
            bottom: auto(),
        },
        size: Size {
            width: length(width),
            height: length(height),
        },
        ..Default::default()
    }
}

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
            label: Some("engine-render elevation-shadow test device"),
            required_features: wgpu::Features::empty(),
            ..Default::default()
        })
        .await
        .expect("failed to create wgpu device");

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("elevation-shadow test target"),
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

fn build_tree(elevation: f64) -> (Tree, engine_core::NodeId) {
    let mut tree = Tree::new();
    let root = tree.insert(
        NodeKind::Rect,
        Style {
            size: Size {
                width: length(200.0),
                height: length(200.0),
            },
            ..Default::default()
        },
        PaintProperties::new(BACKGROUND, 0.0, 0.0, 1.0),
    );

    let chip = tree.insert(
        NodeKind::Rect,
        absolute(60.0, 60.0, 60.0, 60.0),
        PaintProperties::new(CHIP, 0.0, 0.0, 1.0),
    );
    tree.add_child(root, chip);
    tree.get_mut(chip).unwrap().paint.transform.current = Affine::IDENTITY;
    tree.get_mut(chip).unwrap().paint.elevation.current = elevation;

    let available = Size {
        width: AvailableSpace::Definite(200.0),
        height: AvailableSpace::Definite(200.0),
    };
    tree.compute_layout(root, available);
    (tree, root)
}

#[test]
fn zero_elevation_paints_no_shadow_at_all() {
    pollster::block_on(async {
        let (tree, root) = build_tree(0.0);
        let (data, bpr) = render(&tree, root, 200, 200).await;

        // Just below the chip's own bottom edge (60,60)-(120,120): a
        // real shadow would land some pixels here at any real elevation,
        // but must not at elevation 0.
        let below = pixel_at(&data, bpr, 90, 124);
        assert_eq!(
            below,
            [0x11, 0x11, 0x11, 0xFF],
            "elevation 0.0 must paint no shadow -- plain background just below the chip, \
             got {below:?}"
        );
    });
}

#[test]
fn real_elevation_paints_real_shadow_pixels_below_the_node() {
    pollster::block_on(async {
        // A real, fractional elevation (M7 Phase 2's own claim: this is
        // a continuous Animated<f64>, not a discrete 0-5 snap) --
        // level 3 gives a real 4px ambient shadow reaching 4px past the
        // chip's own bottom edge, comfortably wide enough to sample
        // reliably against Gaussian falloff at the very edge.
        let (tree, root) = build_tree(3.0);
        let (data, bpr) = render(&tree, root, 200, 200).await;

        // A few pixels below the chip's own bottom edge (120), still
        // within the ambient shadow's real ~11px reach (4px offset +
        // 8px blur/2 std_dev spread) at level 3.
        let below = pixel_at(&data, bpr, 90, 123);
        assert_ne!(
            below,
            [0x11, 0x11, 0x11, 0xFF],
            "a real elevated node must paint real shadow pixels below its own box, \
             got plain background {below:?}"
        );
        assert_ne!(
            below,
            [0xFF, 0xFF, 0xFF, 0xFF],
            "the shadow pixel must not be the chip's own opaque fill color either, \
             got {below:?}"
        );

        // Far away from the chip entirely -- must still be plain
        // background, proving the shadow doesn't paint everywhere.
        let far = pixel_at(&data, bpr, 10, 10);
        assert_eq!(
            far,
            [0x11, 0x11, 0x11, 0xFF],
            "a point far from the elevated node must show plain background, got {far:?}"
        );
    });
}

/// M25 Phase 2 (§5, §6): a real, previously-missing compounding -- a
/// real elevated node's own shadow painted at its own fixed MD3 alpha
/// regardless of `node.paint.opacity.current` before this, so a fully
/// faded-out node (`opacity: 0.0`) would still cast a fully visible
/// shadow. A node invisible in every other respect must cast none.
#[test]
fn a_fully_faded_elevated_node_casts_no_shadow_at_all() {
    pollster::block_on(async {
        let mut tree = Tree::new();
        let root = tree.insert(
            NodeKind::Rect,
            Style {
                size: Size {
                    width: length(200.0),
                    height: length(200.0),
                },
                ..Default::default()
            },
            PaintProperties::new(BACKGROUND, 0.0, 0.0, 1.0),
        );

        let chip = tree.insert(
            NodeKind::Rect,
            absolute(60.0, 60.0, 60.0, 60.0),
            // opacity = 0.0, the fourth positional field.
            PaintProperties::new(CHIP, 0.0, 0.0, 0.0),
        );
        tree.add_child(root, chip);
        tree.get_mut(chip).unwrap().paint.transform.current = Affine::IDENTITY;
        tree.get_mut(chip).unwrap().paint.elevation.current = 3.0;

        let available = Size {
            width: AvailableSpace::Definite(200.0),
            height: AvailableSpace::Definite(200.0),
        };
        tree.compute_layout(root, available);

        let (data, bpr) = render(&tree, root, 200, 200).await;
        let below = pixel_at(&data, bpr, 90, 123);
        assert_eq!(
            below,
            [0x11, 0x11, 0x11, 0xFF],
            "a fully-faded-out (opacity 0.0) elevated node must cast no shadow at all -- \
             plain background just below its own box, got {below:?}"
        );
    });
}
