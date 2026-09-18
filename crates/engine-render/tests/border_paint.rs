//! M30 Phase 1 (§5, §7): the standalone proof that `PaintProperties.
//! border_color`/`border_width` genuinely paint a real stroked border,
//! inset entirely inside the node's own bounds -- not just that the
//! new fields compile and tick. Same headless render-to-texture-then-
//! readback discipline as `checkbox_paint.rs`.
//!
//! Three claims, kept deliberately separate:
//!
//! 1. A real border paints its own real color in a real band near the
//!    node's own edge.
//! 2. That same node's own fill color still paints at its own center,
//!    genuinely different from the border color there -- proving the
//!    border doesn't just flood-fill the whole node.
//! 3. `border_width: 0.0` (the real default every existing `Rect`
//!    already has) paints no border-colored pixels anywhere -- a true
//!    no-op, not a regression for the ~60 existing call sites that
//!    never touch this field at all.

use engine_core::{NodeKind, PaintProperties, Tree};
use engine_render::{FrameRenderer, TextRenderer, build_tree_scene};
use peniko::Color;
use taffy::prelude::{Size, Style, length};
use vello_hybrid::{RenderSize, RenderTargetConfig};

const FILL: Color = Color::from_rgba8(0x11, 0x11, 0x11, 0xFF);
const BORDER: Color = Color::from_rgba8(0xFF, 0x00, 0x00, 0xFF);
const SIZE: u16 = 100;
const BORDER_WIDTH: f64 = 8.0;

async fn render(border_width: f64) -> (Vec<u8>, u32) {
    let mut tree = Tree::new();
    let mut paint = PaintProperties::new(FILL, 0.0, 0.0, 1.0);
    paint.border_color = engine_core::Animated::new(BORDER);
    paint.border_width = engine_core::Animated::new(border_width);
    let root = tree.insert(
        NodeKind::Rect,
        Style {
            size: Size {
                width: length(f32::from(SIZE)),
                height: length(f32::from(SIZE)),
            },
            ..Default::default()
        },
        paint,
    );
    tree.compute_layout(
        root,
        Size {
            width: taffy::prelude::AvailableSpace::Definite(f32::from(SIZE)),
            height: taffy::prelude::AvailableSpace::Definite(f32::from(SIZE)),
        },
    );

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
            label: Some("engine-render border-paint test device"),
            required_features: wgpu::Features::empty(),
            ..Default::default()
        })
        .await
        .expect("failed to create wgpu device");

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("border-paint test target"),
        size: wgpu::Extent3d {
            width: u32::from(SIZE),
            height: u32::from(SIZE),
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
            width: u32::from(SIZE),
            height: u32::from(SIZE),
        },
    );
    let mut text_renderer = TextRenderer::new();
    let scene = build_tree_scene(
        &tree,
        root,
        SIZE,
        SIZE,
        frame_renderer.resources_mut(),
        &mut text_renderer,
    );
    let render_size = RenderSize {
        width: u32::from(SIZE),
        height: u32::from(SIZE),
    };
    let mut encoder =
        device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
    frame_renderer.render(&scene, &device, &queue, &mut encoder, &render_size, &view);

    let bytes_per_row = (u32::from(SIZE) * 4).next_multiple_of(256);
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size: u64::from(bytes_per_row) * u64::from(SIZE),
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
            width: u32::from(SIZE),
            height: u32::from(SIZE),
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

#[test]
fn a_real_border_paints_its_own_color_near_the_edge_and_the_fill_survives_at_the_center() {
    pollster::block_on(async {
        let (data, bytes_per_row) = render(BORDER_WIDTH).await;

        // Squarely inside the real stroke band (the stroke is centered
        // on a rect inset by half the border width, per paint_node's
        // own real implementation) -- must be the real border color.
        let edge = pixel_at(data.as_slice(), bytes_per_row, 4, SIZE as u32 / 2);
        assert_eq!(
            edge,
            [0xFF, 0x00, 0x00, 0xFF],
            "a pixel inside the real stroke band must be the real border color, got {edge:?}"
        );

        // Squarely inside the real fill, well clear of the border band.
        let center = pixel_at(
            data.as_slice(),
            bytes_per_row,
            SIZE as u32 / 2,
            SIZE as u32 / 2,
        );
        assert_eq!(
            center,
            [0x11, 0x11, 0x11, 0xFF],
            "the node's own real fill must still paint at its center, got {center:?}"
        );
        assert_ne!(
            edge, center,
            "the border and the fill must be genuinely different colors, not the same paint"
        );
    });
}

#[test]
fn border_width_zero_is_a_true_no_op() {
    pollster::block_on(async {
        let (data, bytes_per_row) = render(0.0).await;
        for y in (0..SIZE as u32).step_by(5) {
            for x in (0..SIZE as u32).step_by(5) {
                let px = pixel_at(data.as_slice(), bytes_per_row, x, y);
                assert_eq!(
                    px,
                    [0x11, 0x11, 0x11, 0xFF],
                    "border_width: 0.0 must never paint the border color anywhere, found {px:?} at ({x},{y})"
                );
            }
        }
    });
}
