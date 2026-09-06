//! Phase 3 Step 3.3.2 proof: real SIMD path-morphing interpolation
//! (`tre_math::lerp_points_batch`, via `tre_svg::morph`) between two
//! independently-parsed SVG keyframe shapes -- a diamond and a square,
//! both straight-line-only with the same vertex count, so "topological
//! equivalence" is structural rather than a coincidence of independent
//! curve-flattening tolerances. Re-triangulated fresh at each `t` (the
//! interpolated shape's geometry genuinely changes every frame, even
//! though curve flattening does not repeat), rendered through the
//! pre-existing flat-color pipeline -- no new shader needed.
//!
//! Verified by reading back real rendered pixels at `t = 0.0, 0.5, 1.0`
//! using two probe points chosen so all three renders are pairwise
//! distinguished: `POINT_A` is inside the diamond but outside the
//! square, `POINT_B` is outside BOTH keyframes but inside the exact
//! midpoint shape -- the clearest possible proof that `t = 0.5` is a
//! genuine, distinct interpolated shape, not a snap to either endpoint.

use ash::vk;
use tre_engine::{rgba8, RhiDevice};
use tre_rhi_vulkan::{HeadlessSwapchain, VulkanDevice};

const WIDTH: u32 = 300;
const HEIGHT: u32 = 300;

const FROM_SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="300" height="300">
    <path d="M 150 50 L 250 150 L 150 250 L 50 150 Z" fill="white"/>
</svg>"#;
const TO_SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="300" height="300">
    <path d="M 75 75 L 225 75 L 225 225 L 75 225 Z" fill="white"/>
</svg>"#;

/// Inside the diamond keyframe, outside the square keyframe.
const POINT_A: (u32, u32) = (150, 60);
/// Outside BOTH keyframes, but inside the midpoint (t=0.5) shape --
/// proof that morphing produces a genuinely different silhouette, not
/// just a blend confined to the two keyframes' own footprints.
const POINT_B: (u32, u32) = (70, 180);

fn main() {
    let mut probe_connection =
        tre_platform::PlatformConnection::new().expect("failed to connect to display server");
    let probe_window = probe_connection
        .create_window("tre svg morph probe (never shown)", 1, 1)
        .expect("failed to open probe window");
    use raw_window_handle::HasDisplayHandle;
    let display_handle = probe_connection.display_handle().unwrap().as_raw();
    let window_handle = probe_connection
        .window_handle(probe_window)
        .unwrap()
        .as_raw();
    let (device, surface_loader, surface) =
        VulkanDevice::new(display_handle, window_handle).expect("failed to create VulkanDevice");
    unsafe {
        surface_loader.destroy_surface(surface, None);
    }

    let swapchain =
        HeadlessSwapchain::new(&device, WIDTH, HEIGHT).expect("failed to create HeadlessSwapchain");

    let out_dir = env!("OUT_DIR");
    let vertex_spv = std::fs::read(format!("{out_dir}/walking_skeleton.vert.spv"))
        .expect("failed to read compiled vertex shader");
    let fragment_spv = std::fs::read(format!("{out_dir}/walking_skeleton.frag.spv"))
        .expect("failed to read compiled fragment shader");
    let pipeline = device
        .create_pipeline(&vertex_spv, &fragment_spv, tre_rhi_vulkan::HEADLESS_FORMAT)
        .expect("failed to create pipeline");

    let from_polygons = tre_svg::parse_svg(FROM_SVG.as_bytes(), 1_000_000, 10_000)
        .expect("failed to parse the 'from' keyframe SVG");
    let to_polygons = tre_svg::parse_svg(TO_SVG.as_bytes(), 1_000_000, 10_000)
        .expect("failed to parse the 'to' keyframe SVG");
    assert_eq!(
        from_polygons.len(),
        1,
        "expected exactly one 'from' polygon"
    );
    assert_eq!(to_polygons.len(), 1, "expected exactly one 'to' polygon");
    let from = &from_polygons[0];
    let to = &to_polygons[0];
    assert_eq!(
        from.points.len(),
        to.points.len(),
        "both keyframes must have the same vertex count for this demo"
    );

    let white = rgba8(255, 255, 255, 255);

    let pixel_at = |bgra: &[u8], x: u32, y: u32| -> [u8; 4] {
        let idx = ((y * WIDTH + x) * 4) as usize;
        [bgra[idx], bgra[idx + 1], bgra[idx + 2], bgra[idx + 3]]
    };

    // `expected`: (point_a_is_foreground, point_b_is_foreground) at each t,
    // computed independently via point-in-polygon reasoning about the
    // diamond, square, and their exact vertex-wise midpoint quad -- not
    // derived from this pipeline itself, so this genuinely checks the
    // real shapes rather than tautologically re-deriving its own answer.
    let cases: [(f32, bool, bool); 3] =
        [(0.0, true, false), (0.5, false, true), (1.0, false, false)];

    for (t, point_a_expected_fg, point_b_expected_fg) in cases {
        let morphed = tre_svg::morph(from, to, t).expect("equal vertex counts");
        let triangles = tre_svg::triangulate(&morphed)
            .expect("both keyframes and their interpolation are simple polygons");
        let (vertices, indices) = tre_svg::to_ui_vertices(&morphed, &triangles, white);

        let vertex_buffer = device
            .upload_buffer(
                bytemuck::cast_slice(&vertices),
                vk::BufferUsageFlags::VERTEX_BUFFER,
            )
            .expect("failed to upload vertex buffer");
        let index_buffer = device
            .upload_buffer(
                bytemuck::cast_slice(&indices),
                vk::BufferUsageFlags::INDEX_BUFFER,
            )
            .expect("failed to upload index buffer");

        let (mut cmd_buffer, image) = device.begin_frame(&swapchain).expect("begin_frame failed");
        cmd_buffer.set_pipeline(&pipeline);
        cmd_buffer.bind_vertex_buffer(&vertex_buffer, 0);
        cmd_buffer.bind_index_buffer(&index_buffer, 0);
        cmd_buffer.draw_indexed(indices.len() as u32, 0, 0);
        device
            .submit_and_present(cmd_buffer, &swapchain, image)
            .expect("submit_and_present failed");

        let bgra = swapchain
            .read_pixels_bgra8()
            .expect("failed to read back pixels");

        let a = pixel_at(&bgra, POINT_A.0, POINT_A.1);
        let b = pixel_at(&bgra, POINT_B.0, POINT_B.1);
        let a_is_fg = a == [255, 255, 255, 255];
        let b_is_fg = b == [255, 255, 255, 255];

        assert_eq!(
            a_is_fg, point_a_expected_fg,
            "t={t}: POINT_A foreground-ness mismatch, got pixel {a:?}"
        );
        assert_eq!(
            b_is_fg, point_b_expected_fg,
            "t={t}: POINT_B foreground-ness mismatch, got pixel {b:?}"
        );
        eprintln!("t={t}: OK (POINT_A fg={a_is_fg}, POINT_B fg={b_is_fg})");

        if (t - 0.5).abs() < f32::EPSILON {
            let mut rgba = bgra.clone();
            for px in rgba.chunks_exact_mut(4) {
                px.swap(0, 2);
            }
            let out_path = std::env::var("TRE_SVG_MORPH_OUTPUT")
                .unwrap_or_else(|_| "svg_morph_output.png".to_string());
            let file = std::fs::File::create(&out_path).expect("failed to create output PNG file");
            let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), WIDTH, HEIGHT);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().expect("failed to write PNG header");
            writer
                .write_image_data(&rgba)
                .expect("failed to write PNG image data");
            eprintln!("wrote {WIDTH}x{HEIGHT} t=0.5 morph render to {out_path}");
        }
    }

    eprintln!("all SVG morph assertions passed");
}
