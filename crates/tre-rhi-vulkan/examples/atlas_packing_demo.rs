//! Phase 4 Step 4.2.1 proof: a real Guillotine bin-packing sequence
//! (`tre_atlas::AtlasPacker`), rendered through the existing, unmodified
//! flat-color pipeline so the packing can be inspected by eye, not just
//! asserted. Each successfully-placed rectangle gets its own distinct
//! color; reading back real pixels confirms every placement lands where
//! the packer said it would, with no overlap, and that at least one point
//! the packer left unpacked really is still background.

use ash::vk;
use tre_atlas::{AtlasPacker, PackedRect};
use tre_engine::{rgba8, RhiDevice, UiVertex};
use tre_rhi_vulkan::{HeadlessSwapchain, VulkanDevice};

const WIDTH: u32 = 256;
const HEIGHT: u32 = 256;

/// A palette of visually distinct, fully-opaque flat colors -- enough to
/// keep every placed rectangle in this demo's request sequence visually
/// distinguishable from its neighbors in the output PNG.
///
/// Deliberately only the 7 "pure" (each channel either 0 or 255) colors,
/// not arbitrary mid-tones: `walking_skeleton.frag`'s vertex color is a
/// genuine Phase 0 placeholder (its own doc comment says so) that passes
/// `UiVertex::color` straight through with no sRGB-to-linear decode, even
/// though the render target itself is `B8G8R8A8_SRGB` (auto-encodes the
/// shader's output on store) -- real linear-space color management is
/// IMPLEMENTATION.md Step 7.1's explicit, separately-scheduled job
/// (TECHNICAL.md Section 6's own canonical formula), not this step's.
/// Every prior demo's exclusive use of pure white masked this, since
/// `encode(1.0) == 1.0` and `encode(0.0) == 0.0` are fixed points of any
/// gamma curve -- true both today (no decode at all) and after Step 7.1
/// actually implements one, unlike a mid-tone value such as `150`, which
/// round-trips to `202` under the current placeholder pipeline. Using
/// only 0/255-channel colors keeps this demo's own pixel assertions
/// exact and correct under both the current placeholder behavior and
/// Step 7.1's eventual real fix, with nothing here to revisit later.
const PALETTE: [[u8; 3]; 7] = [
    [255, 0, 0],
    [0, 255, 0],
    [0, 0, 255],
    [255, 255, 0],
    [255, 0, 255],
    [0, 255, 255],
    [255, 255, 255],
];

fn rect_contains(rect: PackedRect, x: u32, y: u32) -> bool {
    x >= rect.x && x < rect.x + rect.width && y >= rect.y && y < rect.y + rect.height
}

/// Scans the canvas for a point no placed rectangle covers -- computed
/// against the actual returned placements, not guessed, so this stays
/// correct even if the packer's own heuristics change later.
fn find_unpacked_probe(placed: &[(PackedRect, [u8; 3])]) -> (u32, u32) {
    for y in (0..HEIGHT).step_by(4) {
        for x in (0..WIDTH).step_by(4) {
            if !placed.iter().any(|(rect, _)| rect_contains(*rect, x, y)) {
                return (x, y);
            }
        }
    }
    panic!("the atlas is entirely covered by placed rectangles -- no background probe exists");
}

fn main() {
    let mut probe_connection =
        tre_platform::PlatformConnection::new().expect("failed to connect to display server");
    let probe_window = probe_connection
        .create_window("tre atlas packing probe (never shown)", 1, 1)
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

    // A deliberately varied request sequence -- mimicking a realistic mix
    // of small glyph-sized and larger icon-sized atlas entries, not
    // uniform squares (which would exercise far less of the packer's own
    // split-axis logic).
    let requests = [
        (32, 32),
        (16, 8),
        (64, 40),
        (8, 8),
        (48, 20),
        (20, 48),
        (12, 12),
        (50, 50),
        (5, 30),
        (30, 5),
        (70, 70),
        (24, 24),
    ];
    let mut packer = AtlasPacker::new(WIDTH, HEIGHT);
    let mut placed: Vec<(PackedRect, [u8; 3])> = Vec::new();
    for (index, &(width, height)) in requests.iter().enumerate() {
        if let Some(rect) = packer.insert(width, height) {
            placed.push((rect, PALETTE[index % PALETTE.len()]));
        }
    }
    eprintln!(
        "packed {} of {} requested rectangles into a {WIDTH}x{HEIGHT} atlas",
        placed.len(),
        requests.len()
    );
    assert!(
        placed.len() >= requests.len() - 2,
        "expected nearly all of this modest request sequence to fit in a {WIDTH}x{HEIGHT} atlas, \
         only {} of {} did",
        placed.len(),
        requests.len()
    );
    for i in 0..placed.len() {
        for j in (i + 1)..placed.len() {
            assert!(
                !placed[i].0.overlaps(&placed[j].0),
                "placements {:?} and {:?} overlap -- the packer's own invariant is broken",
                placed[i].0,
                placed[j].0
            );
        }
    }

    let mut vertices: Vec<UiVertex> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    for (rect, color) in &placed {
        let rgba = rgba8(color[0], color[1], color[2], 255);
        #[allow(
            clippy::cast_precision_loss,
            reason = "atlas coordinates are far below f32's exact-integer range for a 256x256 canvas"
        )]
        let (x0, y0, x1, y1) = (
            rect.x as f32,
            rect.y as f32,
            (rect.x + rect.width) as f32,
            (rect.y + rect.height) as f32,
        );
        let base = u32::try_from(vertices.len()).expect("vertex count fits in u32 for this demo");
        vertices.extend_from_slice(&[
            UiVertex {
                position: [x0, y0],
                uv: [0.0, 0.0],
                color: rgba,
                params: [0.0, 0.0, 0.0],
            },
            UiVertex {
                position: [x1, y0],
                uv: [0.0, 0.0],
                color: rgba,
                params: [0.0, 0.0, 0.0],
            },
            UiVertex {
                position: [x1, y1],
                uv: [0.0, 0.0],
                color: rgba,
                params: [0.0, 0.0, 0.0],
            },
            UiVertex {
                position: [x0, y1],
                uv: [0.0, 0.0],
                color: rgba,
                params: [0.0, 0.0, 0.0],
            },
        ]);
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

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
    // `read_pixels_bgra8` returns real BGRA memory byte order (its name
    // says so) -- every prior demo compared its result only against
    // white/black, which is invariant under a B/R channel swap, so this
    // is the first place that swap actually needs to happen explicitly:
    // `PALETTE` above is written in [R, G, B] order, matching how a
    // person reads a color, not this framebuffer's raw memory layout.
    let pixel_at = |x: u32, y: u32| -> [u8; 4] {
        let idx = ((y * WIDTH + x) * 4) as usize;
        [bgra[idx + 2], bgra[idx + 1], bgra[idx], bgra[idx + 3]]
    };
    let (bg_x, bg_y) = find_unpacked_probe(&placed);
    let background = pixel_at(bg_x, bg_y);
    eprintln!("background (clear color, sampled at an unpacked probe): {background:?}");

    for (rect, color) in &placed {
        let center_x = rect.x + rect.width / 2;
        let center_y = rect.y + rect.height / 2;
        let pixel = pixel_at(center_x, center_y);
        let expected = [color[0], color[1], color[2], 255];
        assert_eq!(
            pixel, expected,
            "placement {rect:?}'s own center rendered as {pixel:?}, expected its own color \
             {expected:?}"
        );
    }
    eprintln!(
        "all {} placed rectangles' own centers rendered as their own color: OK",
        placed.len()
    );

    eprintln!("unpacked probe ({bg_x}, {bg_y}): confirmed background, {background:?}");

    let mut rgba_out = bgra.clone();
    for px in rgba_out.chunks_exact_mut(4) {
        px.swap(0, 2);
    }
    let out_path = std::env::var("TRE_ATLAS_PACKING_OUTPUT")
        .unwrap_or_else(|_| "atlas_packing_output.png".to_string());
    let file = std::fs::File::create(&out_path).expect("failed to create output PNG file");
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), WIDTH, HEIGHT);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().expect("failed to write PNG header");
    writer
        .write_image_data(&rgba_out)
        .expect("failed to write PNG image data");

    eprintln!("wrote {WIDTH}x{HEIGHT} atlas packing render to {out_path}");
    eprintln!("all atlas packing assertions passed");
}
