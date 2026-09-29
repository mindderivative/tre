//! §14 build-order step 2: proves `engine-core`'s `Animated<T>` and
//! `engine-render`'s rendering actually compose correctly, not just that
//! each compiles alone. Headless (render-to-texture, read back pixels),
//! same discipline as `engine-render`'s own `rect_scene_renders_expected_pixels`
//! test -- real correctness, not "it didn't panic".

use std::time::{Duration, Instant};

use engine_core::{Animated, MotionCurve};
use engine_render::{FrameRenderer, INITIAL_COLOR, build_rect_scene};
use vello_gpu::{RenderSize, RenderTargetConfig};

mod support;

#[test]
fn animated_color_and_opacity_render_the_interpolated_value_mid_flight() {
    pollster::block_on(async {
        let width: u16 = 200;
        let height: u16 = 200;
        let target_color = peniko::Color::from_rgba8(0x03, 0xDA, 0xC6, 0xFF); // MD3-ish teal

        // Two genuinely different Animated<T> instantiations, ticked
        // independently -- Design Principle 2's claim ("every animatable
        // property is the same primitive") only means something if it
        // actually holds for more than one T.
        let start = Instant::now();
        let mut color = Animated::new(INITIAL_COLOR);
        let mut opacity = Animated::new(1.0_f64);
        color.animate_to(
            target_color,
            Duration::from_secs(1),
            MotionCurve::Linear,
            start,
        );
        opacity.animate_to(0.4, Duration::from_secs(1), MotionCurve::Linear, start);

        // Halfway through: both should still be mid-flight.
        let halfway = start + Duration::from_millis(500);
        assert!(
            color.tick(halfway, &mut Vec::new()),
            "color animation ended early"
        );
        assert!(
            opacity.tick(halfway, &mut Vec::new()),
            "opacity animation ended early"
        );

        let (device, queue) = support::device("engine-render animated-rect test device").await;

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("animated-rect test target"),
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

        let scene = build_rect_scene(width, height, color.current, opacity.current);
        let mut frame_renderer = FrameRenderer::new(
            &device,
            &RenderTargetConfig {
                format: texture.format(),
                width,
                height,
            },
        );
        let render_size = RenderSize { width, height };
        let mut encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        frame_renderer.render(&scene, &device, &queue, &mut encoder, &render_size, &view);

        queue.submit([encoder.finish()]);
        let bytes_per_row = (u32::from(width) * 4).next_multiple_of(256);
        let data = support::read_texture(&device, &queue, &texture);
        let row_start = ((u32::from(height) / 2) * bytes_per_row) as usize;
        let px_start = row_start + (u32::from(width) / 2 * 4) as usize;
        let center = [
            data[px_start],
            data[px_start + 1],
            data[px_start + 2],
            data[px_start + 3],
        ];

        // Expected: color and opacity each halfway between their start
        // and end, premultiplied by vello_hybrid's own compositing over
        // the (transparent) background -- so just assert the rendered
        // pixel is neither the start color/opacity nor the end
        // color/opacity, but genuinely between them. This is the actual
        // claim step 2 exists to prove: engine-core's tick() output is
        // what engine-render actually painted, not a coincidence of two
        // independently-correct halves.
        let start_r = f32::from(u8::from_str_radix("67", 16).unwrap());
        let end_r = f32::from(0x03_u8);
        let observed_r = f32::from(center[0]);
        assert!(
            observed_r > end_r.min(start_r) && observed_r < end_r.max(start_r),
            "red channel {observed_r} is not strictly between the start ({start_r}) and end ({end_r}) values -- \
             the rendered frame doesn't reflect a mid-flight animation"
        );

        // Opacity halfway (1.0 -> 0.4 = 0.7) means the rect is no longer
        // fully opaque -- composited alpha should be measurably below
        // 255 but still clearly present (not fully transparent).
        assert!(
            center[3] > 50 && center[3] < 255,
            "alpha {} is not consistent with a ~0.7 mid-flight opacity",
            center[3]
        );
    });
}
