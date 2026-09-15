//! §14 build-order step 8: the standalone shadow spike (§7.2/§15 risk --
//! `fill_blurred_rounded_rect` is early-stage per Vello's own release
//! notes, no stability guarantee, uneven parity across the `vello`/
//! `vello_cpu`/`vello_hybrid` variants). Same headless render-to-
//! texture-then-readback discipline as `animated_rect.rs` (step 2) and
//! `layout_tree.rs` (step 3).
//!
//! The claim this test exists to prove isn't "it compiles" -- it's that
//! the exact pinned `vello_hybrid` 0.2.0 genuinely runs a Gaussian blur
//! over the rect's coverage, not a hard-edged fill. Four points sampled
//! along one line, straight out from a non-corner edge, must show a
//! real falloff: fully opaque deep inside, roughly half-coverage right
//! at the raw (unblurred) edge, a partial value further out but still
//! inside the blur kernel's spread, and fully transparent once outside
//! it. A hard-edged (unblurred) fill would jump straight from opaque to
//! transparent at the raw edge instead.

use engine_render::{FrameRenderer, build_shadow_scene};
use peniko::Color;
use vello_hybrid::{RenderSize, RenderTargetConfig};

#[test]
fn blurred_rounded_rect_shows_a_real_gaussian_falloff_at_its_edge() {
    pollster::block_on(async {
        let width: u16 = 300;
        let height: u16 = 300;
        let color = Color::from_rgba8(0x00, 0x00, 0x00, 0xFF);
        let corner_radius = 16.0;
        let std_dev = 10.0; // kernel spread (2.5 * std_dev) = 25px

        // build_shadow_scene's own fixed margin (60.0): rect spans
        // x/y in [60, 240] on a 300x300 canvas -- the same constant
        // duplicated here only for the test's own point selection below,
        // not fed back into the function under test.
        let rect_left = 60.0_f64;

        let scene = build_shadow_scene(width, height, color, corner_radius, std_dev);

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
                label: Some("engine-render shadow-spike test device"),
                required_features: wgpu::Features::empty(),
                ..Default::default()
            })
            .await
            .expect("failed to create wgpu device");

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("shadow-spike test target"),
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
        let alpha_at = |x: u32, y: u32| -> u8 {
            let row_start = (y * bytes_per_row) as usize;
            data[row_start + (x * 4) as usize + 3]
        };

        // All four points sit on the same horizontal line (y=150), well
        // clear of any corner, so each one only ever sees the blur
        // falloff from the single left edge at x=60 -- not a corner's
        // two-edge falloff, which would make "roughly half-coverage at
        // the raw edge" the wrong prediction.
        let y = 150;
        let deep_inside = alpha_at(150, y); // 90px inside -- far past the 25px kernel spread
        let at_raw_edge = alpha_at(rect_left as u32, y); // exactly on the unblurred rect boundary
        let just_outside = alpha_at((rect_left - 10.0) as u32, y); // 10px out -- still inside the 25px kernel
        let far_outside = alpha_at(5, y); // 55px out -- well past the kernel spread

        assert!(
            deep_inside > 250,
            "deep-interior alpha {deep_inside} should be ~fully opaque (255) -- \
             far enough from every edge that no blur kernel reaches it"
        );
        assert!(
            (80..=180).contains(&at_raw_edge),
            "alpha {at_raw_edge} at the raw (unblurred) edge should be roughly \
             half-coverage (~127) for a real Gaussian blur -- a hard-edged fill \
             would instead jump straight to ~255 or ~0 here"
        );
        assert!(
            just_outside > 0 && just_outside < at_raw_edge,
            "alpha {just_outside} 10px outside the raw edge (still inside the \
             25px kernel spread) should be a nonzero value strictly less than \
             the edge's own {at_raw_edge} -- proving a real falloff curve, not \
             a step function"
        );
        assert!(
            far_outside < 10,
            "alpha {far_outside} 55px outside the raw edge (well past the 25px \
             kernel spread) should be ~fully transparent"
        );
    });
}
