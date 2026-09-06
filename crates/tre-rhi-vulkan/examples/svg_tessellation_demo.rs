//! Phase 3 Step 3.3.1 proof: real SVG path data (parsed via `usvg`, not
//! hand-authored `UiVertex` arrays), tessellated by `tre-svg`'s own
//! ear-clipping triangulator, rendered through the pre-existing
//! flat-color `walking_skeleton` pipeline -- no new shader needed, since
//! a plain triangle soup has no SDF to evaluate.
//!
//! Verified by reading back real rendered pixels, not just "it compiles":
//! a point deep inside a five-pointed star is the fill color, and a point
//! in one of the star's concave notches (inside the star's bounding box,
//! but outside the actual polygon) is the background -- proving the
//! triangulation is topologically correct, not just "some triangles got
//! drawn somewhere." A non-convex shape is used deliberately so
//! ear-clipping's real behavior is exercised, not the trivial convex case.

use ash::vk;
use std::fmt::Write as _;
use tre_engine::{rgba8, RhiDevice};
use tre_rhi_vulkan::{HeadlessSwapchain, VulkanDevice};

const WIDTH: u32 = 300;
const HEIGHT: u32 = 300;
const CENTER: (f32, f32) = (150.0, 150.0);
const OUTER_RADIUS: f32 = 130.0;
const INNER_RADIUS: f32 = 50.0;

/// Builds a 10-vertex five-pointed star path (`M ... L ... Z`), alternating
/// between `OUTER_RADIUS` and `INNER_RADIUS`, starting straight up -- a
/// real non-convex polygon, not the trivial convex case ear-clipping
/// would handle by definition.
fn star_svg() -> String {
    let mut d = String::new();
    for i in 0..10u32 {
        #[allow(
            clippy::cast_precision_loss,
            reason = "i ranges 0..10, exactly representable as f32"
        )]
        let angle = std::f32::consts::FRAC_PI_2 + (i as f32) * std::f32::consts::PI / 5.0;
        let r = if i % 2 == 0 {
            OUTER_RADIUS
        } else {
            INNER_RADIUS
        };
        let x = CENTER.0 + r * angle.cos();
        let y = CENTER.1 - r * angle.sin();
        let command = if i == 0 { "M" } else { "L" };
        let _ = write!(d, "{command} {x} {y} ");
    }
    d.push('Z');
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{WIDTH}" height="{HEIGHT}"><path d="{d}" fill="white"/></svg>"#
    )
}

fn main() {
    let mut probe_connection =
        tre_platform::PlatformConnection::new().expect("failed to connect to display server");
    let probe_window = probe_connection
        .create_window("tre svg tessellation probe (never shown)", 1, 1)
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

    let svg = star_svg();
    let polygons =
        tre_svg::parse_svg(svg.as_bytes(), 1_000_000, 10_000).expect("failed to parse SVG");
    assert_eq!(polygons.len(), 1, "expected exactly one star polygon");
    let star = &polygons[0];

    let triangles =
        tre_svg::triangulate(star).expect("the star is a simple (non-self-intersecting) polygon");
    let white = rgba8(255, 255, 255, 255);
    let (vertices, indices) = tre_svg::to_ui_vertices(star, &triangles, white);

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
    let pixel_at = |x: u32, y: u32| -> [u8; 4] {
        let idx = ((y * WIDTH + x) * 4) as usize;
        [bgra[idx], bgra[idx + 1], bgra[idx + 2], bgra[idx + 3]]
    };

    // Never inside the star's own polygon at any radius -- the actual
    // clear-color background, read dynamically rather than hardcoding
    // Vulkan's sRGB-attachment clear conversion behavior.
    let background = pixel_at(0, 0);
    eprintln!("background (clear color): {background:?}");

    // Deep interior (the star's own center) -- always inside a star
    // polygon by construction, regardless of point count or radii.
    #[allow(
        clippy::cast_possible_truncation,
        reason = "CENTER's components are far below u32::MAX"
    )]
    let interior = pixel_at(CENTER.0 as u32, CENTER.1 as u32);
    assert_eq!(
        interior,
        [255, 255, 255, 255],
        "the star's center must be exactly the fill color, got {interior:?}"
    );
    eprintln!("interior: OK ({interior:?})");

    // A concave notch: exactly at an inner vertex's own angle, pushed
    // radially outward past it (INNER_RADIUS + 15) -- inside the star's
    // bounding box, but outside the actual polygon, since the two edges
    // meeting at that inner vertex slope back outward from there.
    let notch_angle = std::f32::consts::FRAC_PI_2 + std::f32::consts::PI / 5.0; // vertex index 1's angle
    let notch_radius = INNER_RADIUS + 15.0;
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "CENTER + notch offset stays well within [0, WIDTH)/[0, HEIGHT)"
    )]
    let (notch_x, notch_y) = (
        (CENTER.0 + notch_radius * notch_angle.cos()) as u32,
        (CENTER.1 - notch_radius * notch_angle.sin()) as u32,
    );
    let notch = pixel_at(notch_x, notch_y);
    assert_eq!(
        notch, background,
        "a point in the star's concave notch must be exactly the background color, got {notch:?}"
    );
    eprintln!("notch (concave region, inside bounding box but outside polygon): OK ({notch:?})");

    let mut rgba = bgra.clone();
    for px in rgba.chunks_exact_mut(4) {
        px.swap(0, 2);
    }
    let out_path = std::env::var("TRE_SVG_TESSELLATION_OUTPUT")
        .unwrap_or_else(|_| "svg_tessellation_output.png".to_string());
    let file = std::fs::File::create(&out_path).expect("failed to create output PNG file");
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), WIDTH, HEIGHT);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().expect("failed to write PNG header");
    writer
        .write_image_data(&rgba)
        .expect("failed to write PNG image data");

    eprintln!("wrote {WIDTH}x{HEIGHT} SVG tessellation render to {out_path}");
    eprintln!("all SVG tessellation assertions passed");
}
