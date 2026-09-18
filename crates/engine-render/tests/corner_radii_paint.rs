//! M30 Phase 1 Step 4 (§5, §7): proves `PaintProperties.
//! corner_radii_override` genuinely paints independent per-corner
//! rounding, not just that the field compiles and threads through as
//! inert data -- the real prerequisite `Segmented Button`'s first/
//! last segments need (rounded on their own outer edge, square on the
//! edge touching the next segment). Same headless render-to-texture-
//! then-readback discipline as `border_paint.rs`.

use engine_core::{NodeKind, PaintProperties, Tree};
use engine_render::{FrameRenderer, TextRenderer, build_tree_scene};
use peniko::Color;
use taffy::prelude::{Size, Style, length};
use vello_hybrid::{RenderSize, RenderTargetConfig};

const FILL: Color = Color::from_rgba8(0xFF, 0x00, 0x00, 0xFF);
const SIZE: u16 = 100;
/// Large enough that a real rounded corner clearly clears a pixel a
/// few px in from the corner; small enough to stay well inside the
/// node's own bounds.
const ROUND_RADIUS: f64 = 30.0;

async fn render(radii: Option<[f64; 4]>) -> (Vec<u8>, u32) {
    let mut tree = Tree::new();
    let mut paint = PaintProperties::new(FILL, 0.0, 0.0, 1.0);
    paint.corner_radii_override = radii;
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
            label: Some("engine-render corner-radii test device"),
            required_features: wgpu::Features::empty(),
            ..Default::default()
        })
        .await
        .expect("failed to create wgpu device");

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("corner-radii test target"),
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

const FILLED: [u8; 4] = [0xFF, 0x00, 0x00, 0xFF];
const EMPTY: [u8; 4] = [0x00, 0x00, 0x00, 0x00];

#[test]
fn a_rounded_top_left_corner_clears_a_pixel_the_square_corners_still_fill() {
    pollster::block_on(async {
        // [top_left, top_right, bottom_right, bottom_left] -- only
        // top-left gets a real, large radius; the other three stay 0.
        let (data, bytes_per_row) = render(Some([ROUND_RADIUS, 0.0, 0.0, 0.0])).await;

        let top_left = pixel_at(data.as_slice(), bytes_per_row, 2, 2);
        assert_eq!(
            top_left, EMPTY,
            "a real 30px top-left radius must clear a pixel only 2px from that \
             corner, got {top_left:?}"
        );

        let top_right = pixel_at(data.as_slice(), bytes_per_row, u32::from(SIZE) - 2, 2);
        assert_eq!(
            top_right, FILLED,
            "the top-right corner has radius 0.0 in this override and must stay \
             square (filled right up to the corner), got {top_right:?}"
        );

        let bottom_right = pixel_at(
            data.as_slice(),
            bytes_per_row,
            u32::from(SIZE) - 2,
            u32::from(SIZE) - 2,
        );
        assert_eq!(
            bottom_right, FILLED,
            "the bottom-right corner has radius 0.0 and must stay square, got {bottom_right:?}"
        );

        let bottom_left = pixel_at(data.as_slice(), bytes_per_row, 2, u32::from(SIZE) - 2);
        assert_eq!(
            bottom_left, FILLED,
            "the bottom-left corner has radius 0.0 and must stay square, got {bottom_left:?}"
        );
    });
}

#[test]
fn no_override_paints_the_identical_uniform_rect_as_before_this_field_existed() {
    pollster::block_on(async {
        let (data, bytes_per_row) = render(None).await;
        // A plain node with no override and corner_radius: 0.0 (this
        // test's own `PaintProperties::new` call) is a true square --
        // every corner, right up to 1px in, must be filled.
        for (x, y) in [
            (1, 1),
            (u32::from(SIZE) - 2, 1),
            (1, u32::from(SIZE) - 2),
            (u32::from(SIZE) - 2, u32::from(SIZE) - 2),
        ] {
            let px = pixel_at(data.as_slice(), bytes_per_row, x, y);
            assert_eq!(
                px, FILLED,
                "corner_radii_override: None with corner_radius: 0.0 must paint a \
                 true square, found a gap at ({x},{y}): {px:?}"
            );
        }
    });
}
