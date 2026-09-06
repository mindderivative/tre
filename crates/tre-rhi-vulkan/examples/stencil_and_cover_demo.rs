//! Phase 3 Step 3.3.3 proof: a real two-pass stencil-and-cover GPU
//! technique rendering a genuinely self-intersecting path -- a classic
//! pentagram (five circle points connected in `0, 2, 4, 1, 3` order,
//! crossing its own boundary five times) -- which `tre_svg::triangulate`
//! provably cannot handle (confirmed below, not assumed). Proves BOTH
//! fill rules via the textbook case: a pentagram's central pentagon has
//! winding number 2 (filled under `NonZero`) but is crossed an even
//! number of times (empty under `EvenOdd`) -- the two fill rules
//! genuinely disagree on this exact shape, at this exact point.
//!
//! No new shader: both the stencil pass (color writes masked off, a
//! stencil test that only *writes*) and the cover pass (normal color
//! writes, a stencil test that only *reads and resets*) reuse the
//! existing flat-color `walking_skeleton` shader pair -- the entire
//! technique is pipeline *state* (`create_stencil_and_cover_pipelines`),
//! not new shader code.

use ash::vk;
use tre_engine::{rgba8, FillRule, RhiDevice, UiVertex};
use tre_rhi_vulkan::{HeadlessSwapchain, VulkanDevice};

const WIDTH: u32 = 300;
const HEIGHT: u32 = 300;

// Five points around a circle (center (150, 150), radius 130), connected
// in pentagram order (0, 2, 4, 1, 3) -- the classic self-intersecting
// construction, not a strawman shape.
const PENTAGRAM_SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="300" height="300">
    <path d="M 150 20 L 73.58791720197848 255.17220926874316 L 273.63734711837 109.82779073125687 L 26.36265288163004 109.82779073125683 L 226.41208279802146 255.1722092687432 Z" fill="white"/>
</svg>"#;

/// Well inside one of the pentagram's outer star points (radius 100 from
/// center, along the top spike) -- winding number -1 and crossed an odd
/// number of times, so filled under BOTH fill rules.
const OUTER_TIP_POINT: (u32, u32) = (150, 50);
/// The pentagram's exact geometric center -- winding number -2 (nonzero:
/// filled under `NonZero`) but crossed an even number of times (empty
/// under `EvenOdd`). The decisive, textbook-different point.
const CENTER_POINT: (u32, u32) = (150, 150);

fn main() {
    let mut probe_connection =
        tre_platform::PlatformConnection::new().expect("failed to connect to display server");
    let probe_window = probe_connection
        .create_window("tre stencil-and-cover probe (never shown)", 1, 1)
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

    let polygons = tre_svg::parse_svg(PENTAGRAM_SVG.as_bytes(), 1_000_000, 10_000)
        .expect("failed to parse SVG");
    assert_eq!(polygons.len(), 1, "expected exactly one pentagram polygon");
    let pentagram = &polygons[0];

    // The real motivation for this whole step: confirm ear-clipping
    // genuinely cannot handle this path, not assume it.
    let ear_clip_result = tre_svg::triangulate(pentagram);
    assert!(
        matches!(ear_clip_result, Err(tre_svg::SvgError::NotSimplePolygon)),
        "expected a genuinely self-intersecting pentagram to be rejected by ear-clipping, got {ear_clip_result:?}"
    );
    eprintln!(
        "confirmed: triangulate() rejects the pentagram (NotSimplePolygon) -- \
         stencil-and-cover is the correct tool here, not ear-clipping"
    );

    let fan = tre_svg::fan_triangles(pentagram);
    let (bbox_min, bbox_max) = tre_svg::bounding_box(pentagram);
    let white = rgba8(255, 255, 255, 255);

    let stencil_vertices: Vec<UiVertex> = pentagram
        .points
        .iter()
        .map(|&position| UiVertex {
            position,
            uv: [0.0, 0.0],
            color: white,
            params: [0.0; 3],
        })
        .collect();
    let stencil_indices: Vec<u32> = fan.iter().flat_map(|&t| t).collect();

    let cover_vertices: [UiVertex; 4] = [
        UiVertex {
            position: bbox_min,
            uv: [0.0, 0.0],
            color: white,
            params: [0.0; 3],
        },
        UiVertex {
            position: [bbox_max[0], bbox_min[1]],
            uv: [0.0, 0.0],
            color: white,
            params: [0.0; 3],
        },
        UiVertex {
            position: bbox_max,
            uv: [0.0, 0.0],
            color: white,
            params: [0.0; 3],
        },
        UiVertex {
            position: [bbox_min[0], bbox_max[1]],
            uv: [0.0, 0.0],
            color: white,
            params: [0.0; 3],
        },
    ];
    let cover_indices: [u32; 6] = [0, 1, 2, 2, 3, 0];

    let stencil_vertex_buffer = device
        .upload_buffer(
            bytemuck::cast_slice(&stencil_vertices),
            vk::BufferUsageFlags::VERTEX_BUFFER,
        )
        .expect("failed to upload stencil-pass vertex buffer");
    let stencil_index_buffer = device
        .upload_buffer(
            bytemuck::cast_slice(&stencil_indices),
            vk::BufferUsageFlags::INDEX_BUFFER,
        )
        .expect("failed to upload stencil-pass index buffer");
    let cover_vertex_buffer = device
        .upload_buffer(
            bytemuck::cast_slice(&cover_vertices),
            vk::BufferUsageFlags::VERTEX_BUFFER,
        )
        .expect("failed to upload cover-pass vertex buffer");
    let cover_index_buffer = device
        .upload_buffer(
            bytemuck::cast_slice(&cover_indices),
            vk::BufferUsageFlags::INDEX_BUFFER,
        )
        .expect("failed to upload cover-pass index buffer");

    let pixel_at = |bgra: &[u8], x: u32, y: u32| -> [u8; 4] {
        let idx = ((y * WIDTH + x) * 4) as usize;
        [bgra[idx], bgra[idx + 1], bgra[idx + 2], bgra[idx + 3]]
    };

    for (fill_rule, expect_center_filled) in [(FillRule::EvenOdd, false), (FillRule::NonZero, true)]
    {
        let (stencil_pipeline, cover_pipeline) = device
            .create_stencil_and_cover_pipelines(
                &vertex_spv,
                &fragment_spv,
                tre_rhi_vulkan::HEADLESS_FORMAT,
                fill_rule,
            )
            .expect("failed to create stencil-and-cover pipelines");

        let (mut cmd_buffer, image) = device.begin_frame(&swapchain).expect("begin_frame failed");
        cmd_buffer.set_pipeline(&stencil_pipeline);
        cmd_buffer.bind_vertex_buffer(&stencil_vertex_buffer, 0);
        cmd_buffer.bind_index_buffer(&stencil_index_buffer, 0);
        cmd_buffer.draw_indexed(stencil_indices.len() as u32, 0, 0);

        cmd_buffer.set_pipeline(&cover_pipeline);
        cmd_buffer.bind_vertex_buffer(&cover_vertex_buffer, 0);
        cmd_buffer.bind_index_buffer(&cover_index_buffer, 0);
        cmd_buffer.draw_indexed(cover_indices.len() as u32, 0, 0);

        device
            .submit_and_present(cmd_buffer, &swapchain, image)
            .expect("submit_and_present failed");

        let bgra = swapchain
            .read_pixels_bgra8()
            .expect("failed to read back pixels");

        let tip = pixel_at(&bgra, OUTER_TIP_POINT.0, OUTER_TIP_POINT.1);
        assert_eq!(
            tip,
            [255, 255, 255, 255],
            "{fill_rule:?}: an outer star point must be filled under every fill rule, got {tip:?}"
        );

        let center = pixel_at(&bgra, CENTER_POINT.0, CENTER_POINT.1);
        let center_is_filled = center == [255, 255, 255, 255];
        assert_eq!(
            center_is_filled, expect_center_filled,
            "{fill_rule:?}: center fill mismatch, got pixel {center:?}"
        );

        eprintln!("{fill_rule:?}: OK (outer tip filled, center filled={center_is_filled})");

        if matches!(fill_rule, FillRule::NonZero) {
            let mut rgba = bgra.clone();
            for px in rgba.chunks_exact_mut(4) {
                px.swap(0, 2);
            }
            let out_path = std::env::var("TRE_STENCIL_AND_COVER_OUTPUT")
                .unwrap_or_else(|_| "stencil_and_cover_output.png".to_string());
            let file = std::fs::File::create(&out_path).expect("failed to create output PNG file");
            let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), WIDTH, HEIGHT);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().expect("failed to write PNG header");
            writer
                .write_image_data(&rgba)
                .expect("failed to write PNG image data");
            eprintln!("wrote {WIDTH}x{HEIGHT} NonZero-fill render to {out_path}");
        }
    }

    eprintln!("all stencil-and-cover assertions passed");
}
