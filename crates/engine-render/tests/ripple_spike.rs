//! §14 build-order step 9: the standalone ripple/state-layer spike
//! (§7.3) -- same headless render-to-texture-then-readback discipline
//! as `shadow_spike.rs` (step 8).
//!
//! Two claims, kept deliberately separate so a failure in either shows
//! exactly which mechanism broke:
//!
//! 1. `push_layer`'s `opacity` argument genuinely blends the ripple
//!    color into the background rather than painting it fully opaque
//!    (or not at all) -- checked by sampling a point that's inside the
//!    ripple circle in both renders below.
//! 2. `push_layer`'s `clip_path` argument genuinely confines where that
//!    blended color lands to the circle's own area, not the whole
//!    layer -- checked by sampling one fixed point that's *outside* a
//!    small-radius ripple but *inside* a large-radius one: the same
//!    underlying `fill_path` call runs both times (see
//!    `build_ripple_scene`), so only the clip's own shape can explain a
//!    different result at that point.

use engine_render::{FrameRenderer, build_ripple_scene};
use peniko::Color;
use peniko::kurbo::Point;
use vello_hybrid::{RenderSize, RenderTargetConfig};

const BASE: Color = Color::from_rgba8(0x67, 0x50, 0xA4, 0xFF); // MD3-ish purple
const RIPPLE: Color = Color::from_rgba8(0xFF, 0xFF, 0xFF, 0xFF); // white

async fn render_and_sample(radius: f64, sample: (u32, u32)) -> [u8; 4] {
    let width: u16 = 300;
    let height: u16 = 300;
    let origin = Point::new(150.0, 150.0);
    let opacity = 0.5;

    let scene = build_ripple_scene(width, height, BASE, RIPPLE, origin, radius, opacity);

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
            label: Some("engine-render ripple-spike test device"),
            required_features: wgpu::Features::empty(),
            ..Default::default()
        })
        .await
        .expect("failed to create wgpu device");

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("ripple-spike test target"),
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
    let (x, y) = sample;
    let row_start = (y * bytes_per_row) as usize;
    let px_start = row_start + (x * 4) as usize;
    [
        data[px_start],
        data[px_start + 1],
        data[px_start + 2],
        data[px_start + 3],
    ]
}

#[test]
fn ripple_opacity_genuinely_blends_the_ripple_color_into_the_background() {
    pollster::block_on(async {
        // Sampled dead center, at the ripple's own origin -- inside any
        // nonzero-radius circle, so this isolates opacity blending from
        // the clip's shape entirely.
        let pixel = render_and_sample(100.0, (150, 150)).await;

        let base_r = f32::from(0x67_u8);
        let ripple_r = f32::from(0xFF_u8);
        let observed_r = f32::from(pixel[0]);
        assert!(
            observed_r > base_r.min(ripple_r) && observed_r < base_r.max(ripple_r),
            "red channel {observed_r} at the ripple's own center is not strictly \
             between the base color's ({base_r}) and the ripple color's ({ripple_r}) \
             -- push_layer's opacity argument isn't genuinely blending"
        );
    });
}

#[test]
fn ripple_clip_confines_the_blend_to_the_circles_own_radius() {
    pollster::block_on(async {
        // Fixed sample point 50px right of the origin (150,150) ->
        // (200,150). Outside a radius-20 ripple, inside a radius-100
        // one. The exact same fill_path call runs in both renders
        // (see build_ripple_scene) -- only push_layer's clip_path can
        // explain a different pixel at the same coordinate.
        let sample = (200, 150);

        let small_radius = render_and_sample(20.0, sample).await;
        assert_eq!(
            small_radius,
            [0x67, 0x50, 0xA4, 0xFF],
            "50px from the ripple's origin, a 20px-radius ripple shouldn't reach here \
             -- pixel {small_radius:?} is not pure base color, meaning the clip didn't \
             actually confine the fill to the circle"
        );

        let large_radius_r = f32::from(render_and_sample(100.0, sample).await[0]);
        let base_r = f32::from(0x67_u8);
        let ripple_r = f32::from(0xFF_u8);
        assert!(
            large_radius_r > base_r.min(ripple_r) && large_radius_r < base_r.max(ripple_r),
            "the same point, now inside a 100px-radius ripple, should show blended \
             color -- red channel {large_radius_r} isn't strictly between base ({base_r}) \
             and ripple ({ripple_r}), meaning the larger radius didn't actually reach it"
        );
    });
}
