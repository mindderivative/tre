//! Phase 3 Step 3.3.3 proof, superseded by Phase 10 Step 10.2's move to
//! `lyon`: a genuinely self-intersecting path -- a classic pentagram
//! (five circle points connected in `0, 2, 4, 1, 3` order, crossing its
//! own boundary five times) -- which the OLD hand-rolled ear-clipping
//! triangulator provably could not handle (it rejected any self-
//! intersecting contour outright, `SvgError::NotSimplePolygon`), and
//! which the retired stencil-and-cover GPU technique existed
//! specifically to work around via a two-pass stencil-buffer trick.
//!
//! `lyon`'s real sweep-line fill tessellator handles this shape directly,
//! as one ordinary tessellation call, through the SAME single-pass
//! flat-color pipeline every other tessellated shape uses -- no stencil
//! buffer, no second pass, no special-cased fallback technique at all.
//! This demo proves that directly: it renders the pentagram under BOTH
//! real fill rules via the textbook case -- a pentagram's central
//! pentagon has winding number 2 (filled under `NonZero`) but is crossed
//! an even number of times (empty under `EvenOdd`) -- and asserts the
//! two fill rules genuinely disagree on this exact shape, at this exact
//! point, exactly as the retired stencil-and-cover demo once did.

use ash::vk;
use tre_engine::{rgba8, submit_frame};
use tre_rhi_vulkan::{HeadlessSwapchain, VulkanDevice};

#[path = "support/pixel_helpers.rs"]
mod pixel_helpers;

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
        .create_window("tre self-intersecting fill probe (never shown)", 1, 1)
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

    let polygons = tre_svg::parse_svg(PENTAGRAM_SVG.as_bytes(), 1_000_000, 10_000)
        .expect("failed to parse SVG");
    assert_eq!(polygons.len(), 1, "expected exactly one pentagram polygon");
    let pentagram = &polygons[0];

    let white = rgba8(255, 255, 255, 255);
    let pixel_at = |bgra: &[u8], x: u32, y: u32| -> [u8; 4] {
        pixel_helpers::bgra_pixel_at(bgra, WIDTH, x, y)
    };

    for (fill_rule, expect_center_filled) in [
        (tre_svg::FillRule::EvenOdd, false),
        (tre_svg::FillRule::NonZero, true),
    ] {
        // The real point of this demo: lyon's fill tessellator succeeds
        // directly on a genuinely self-intersecting contour -- no
        // rejection, no fallback technique, just a real tessellation.
        let (vertices, indices) =
            tre_svg::tessellate_fill(std::slice::from_ref(pentagram), fill_rule, white)
                .unwrap_or_else(|e| panic!("{fill_rule:?}: pentagram tessellation failed: {e}"));
        assert!(
            !indices.is_empty(),
            "{fill_rule:?}: a pentagram has real area under either fill rule"
        );

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

        submit_frame(&device, &swapchain, |cmd_buffer| {
            cmd_buffer.set_pipeline(&pipeline);
            cmd_buffer.bind_vertex_buffer(&vertex_buffer, 0);
            cmd_buffer.bind_index_buffer(&index_buffer, 0);
            cmd_buffer.draw_indexed(indices.len() as u32, 0, 0);
        })
        .expect("submit_frame failed");

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

        if matches!(fill_rule, tre_svg::FillRule::NonZero) {
            let mut rgba = bgra.clone();
            for px in rgba.chunks_exact_mut(4) {
                px.swap(0, 2);
            }
            let out_path = std::env::var("TRE_SELF_INTERSECTING_FILL_OUTPUT")
                .unwrap_or_else(|_| "self_intersecting_fill_output.png".to_string());
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

    eprintln!("all self-intersecting fill assertions passed");
}
