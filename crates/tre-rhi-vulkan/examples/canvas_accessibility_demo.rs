//! Phase 5 Step 5.3.3 proof: the capstone closing Step 5.3 in full. A
//! single real render produces data that flows through all three
//! systems DESIGN.md Section 5.2's "$100\%$ alignment" claim spans --
//! the real GPU framebuffer, the tagged `AccessibilityNode` IR, and a
//! real, live AT-SPI2 bus -- checked against each other, rather than
//! three isolated tests that each merely trust the others' contracts
//! (Step 5.3.1's own IR-only unit tests; Step 5.3.2's own D-Bus round
//! trip against a synthetic, never-rendered node).
//!
//! Three rects are both drawn and tagged from the exact same local
//! coordinates: a plain `Generic` rect, a plain `Button` rect, and a
//! **rotated** `Image` rect via `Affine2::from_translation_rotation_scale`
//! -- the one genuinely tricky case Step 5.3.1's own `PLAN.md` singled
//! out. No clipping/overlays (Step 5.3.1's own disclosed, still-open
//! clip-stack gap would make a real, already-known limitation look like
//! a new bug) and no multi-threading (already Step 5.2.3's own
//! capstone, and already unit-tested for accessibility nodes
//! specifically in Step 5.3.1) -- this sub-step closes new ground, not
//! already-closed ground.
//!
//! **This binary renders and tags only -- it never touches
//! `tre_a11y`/AT-SPI2 itself.** A real, extensive CI-only investigation
//! (REVIEW.md finding #126, seven real pushes) isolated the actual
//! cause of a repeated CI failure: `accesskit_unix`'s own background
//! thread, which does the real AT-SPI2 registration, cannot complete
//! that registration inside a process that also links real Vulkan/X11
//! shared libraries (`ash`/`x11rb`, pulled in by this demo's own real
//! GPU rendering) -- confirmed by the fact that `tre-a11y`'s own
//! round-trip test (zero Vulkan/X11 code) succeeds reliably in the
//! exact same CI job this demo's own accessibility work kept failing
//! in. The fix is a real two-process split, not a workaround: this
//! binary writes its tagged `AccessibilityNode`s out to a plain text
//! file (`TRE_CANVAS_ACCESSIBILITY_NODES_PATH`, one line per node) for
//! `canvas_accessibility_verify` -- a separate, genuinely Vulkan-free
//! binary -- to publish and verify against the real bus. This also
//! matches real AT-SPI2 practice more closely than a self-verifying
//! single process ever did: a real screen reader is always a separate
//! process from the application it inspects.

use ash::vk;
use tre_engine::{rgba8, submit_frame, AccessibilityNodeId, AccessibilityRole, RenderingCanvas};
use tre_math::Affine2;
use tre_rhi_vulkan::{HeadlessSwapchain, VulkanDevice};

#[path = "support/pixel_helpers.rs"]
mod pixel_helpers;

const CANVAS_WIDTH: u32 = 300;
const CANVAS_HEIGHT: u32 = 200;

const RECT_A_ORIGIN: (f32, f32) = (20.0, 20.0); // Generic, plain
const RECT_B_ORIGIN: (f32, f32) = (200.0, 20.0); // Button, plain
const RECT_C_TRANSLATION: [f32; 2] = [100.0, 120.0]; // Image, rotated
const RECT_SIZE: (f32, f32) = (60.0, 40.0);
const RECT_C_ROTATION_DEGREES: f32 = 30.0;

const NODE_A: AccessibilityNodeId = AccessibilityNodeId(1);
const NODE_B: AccessibilityNodeId = AccessibilityNodeId(2);
const NODE_C: AccessibilityNodeId = AccessibilityNodeId(3);

fn main() {
    // --- The scene: each rect is drawn and tagged from the exact same
    // local coordinates, one shared source of truth for both -- pure
    // CPU IR, no GPU/window dependency at all ---
    let white = rgba8(255, 255, 255, 255);
    let mut canvas = RenderingCanvas::new();

    canvas.draw_rounded_rect(
        RECT_A_ORIGIN.0,
        RECT_A_ORIGIN.1,
        RECT_SIZE.0,
        RECT_SIZE.1,
        0.0,
        white,
    );
    canvas.tag_accessibility_node(
        NODE_A,
        RECT_A_ORIGIN.0,
        RECT_A_ORIGIN.1,
        RECT_SIZE.0,
        RECT_SIZE.1,
        AccessibilityRole::Generic,
    );

    canvas.draw_rounded_rect(
        RECT_B_ORIGIN.0,
        RECT_B_ORIGIN.1,
        RECT_SIZE.0,
        RECT_SIZE.1,
        0.0,
        white,
    );
    canvas.tag_accessibility_node(
        NODE_B,
        RECT_B_ORIGIN.0,
        RECT_B_ORIGIN.1,
        RECT_SIZE.0,
        RECT_SIZE.1,
        AccessibilityRole::Button,
    );

    let rect_c_transform = Affine2::from_translation_rotation_scale(
        RECT_C_TRANSLATION,
        RECT_C_ROTATION_DEGREES.to_radians(),
        [1.0, 1.0],
    );
    canvas.save();
    canvas.transform(&rect_c_transform);
    canvas.draw_rounded_rect(0.0, 0.0, RECT_SIZE.0, RECT_SIZE.1, 0.0, white);
    canvas.tag_accessibility_node(
        NODE_C,
        0.0,
        0.0,
        RECT_SIZE.0,
        RECT_SIZE.1,
        AccessibilityRole::Image,
    );
    canvas.restore();

    let frame = canvas.flatten();
    assert_eq!(
        frame.accessibility_nodes.len(),
        3,
        "exactly 3 tagged nodes were recorded"
    );

    // --- Real device/pipeline, matching sdf_rounded_rect_demo's own
    // minimal single-pipeline setup exactly -- no atlas/text needed ---
    let mut probe_connection =
        tre_platform::PlatformConnection::new().expect("failed to connect to display server");
    let probe_window = probe_connection
        .create_window("tre canvas accessibility probe (never shown)", 1, 1)
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
    let swapchain = HeadlessSwapchain::new(&device, CANVAS_WIDTH, CANVAS_HEIGHT)
        .expect("failed to create HeadlessSwapchain");

    let out_dir = env!("OUT_DIR");
    let vertex_spv = std::fs::read(format!("{out_dir}/sdf_rounded_rect.vert.spv"))
        .expect("failed to read compiled vertex shader");
    let fragment_spv = std::fs::read(format!("{out_dir}/sdf_rounded_rect.frag.spv"))
        .expect("failed to read compiled fragment shader");
    let pipeline = device
        .create_pipeline(&vertex_spv, &fragment_spv, tre_rhi_vulkan::HEADLESS_FORMAT)
        .expect("failed to create pipeline");

    // --- Real GPU render ---
    let vertex_buffer = device
        .upload_buffer(
            bytemuck::cast_slice(&frame.vertices),
            vk::BufferUsageFlags::VERTEX_BUFFER,
        )
        .expect("failed to upload vertex buffer");
    let index_buffer = device
        .upload_buffer(
            bytemuck::cast_slice(&frame.indices),
            vk::BufferUsageFlags::INDEX_BUFFER,
        )
        .expect("failed to upload index buffer");

    submit_frame(&device, &swapchain, |cmd_buffer| {
        cmd_buffer.set_pipeline(&pipeline);
        cmd_buffer.bind_vertex_buffer(&vertex_buffer, 0);
        cmd_buffer.bind_index_buffer(&index_buffer, 0);
        #[allow(
            clippy::cast_possible_truncation,
            reason = "this demo's index count is far below u32::MAX"
        )]
        cmd_buffer.draw_indexed(frame.indices.len() as u32, 0, 0);
    })
    .expect("submit_frame failed");

    let bgra = swapchain
        .read_pixels_bgra8()
        .expect("failed to read back pixels");
    let pixel_at =
        |x: u32, y: u32| -> [u8; 4] { pixel_helpers::bgra_pixel_at(&bgra, CANVAS_WIDTH, x, y) };
    let background = pixel_at(0, 0);

    // --- Pixel verification: rect A/B's own plain centers, and rect
    // C's real transformed center -- computed via the exact same
    // Affine2 used to draw and tag it, not an independently-guessed
    // coordinate ---
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "this canvas's coordinates are fixed, well within [0, CANVAS_WIDTH/HEIGHT)"
    )]
    let to_pixel = |p: [f32; 2]| -> (u32, u32) { (p[0].round() as u32, p[1].round() as u32) };

    let (ax, ay) = to_pixel([
        RECT_A_ORIGIN.0 + RECT_SIZE.0 / 2.0,
        RECT_A_ORIGIN.1 + RECT_SIZE.1 / 2.0,
    ]);
    assert_eq!(
        pixel_at(ax, ay),
        [255, 255, 255, 255],
        "Rect A must render at its own center"
    );

    let (bx, by) = to_pixel([
        RECT_B_ORIGIN.0 + RECT_SIZE.0 / 2.0,
        RECT_B_ORIGIN.1 + RECT_SIZE.1 / 2.0,
    ]);
    assert_eq!(
        pixel_at(bx, by),
        [255, 255, 255, 255],
        "Rect B must render at its own center"
    );

    let rect_c_center_world =
        rect_c_transform.transform_point([RECT_SIZE.0 / 2.0, RECT_SIZE.1 / 2.0]);
    let (cx, cy) = to_pixel(rect_c_center_world);
    assert_eq!(
        pixel_at(cx, cy),
        [255, 255, 255, 255],
        "rotated Rect C must render at its own real transformed center ({cx}, {cy})"
    );

    let (gap_x, gap_y) = to_pixel([
        (RECT_A_ORIGIN.0 + RECT_SIZE.0 + RECT_B_ORIGIN.0) / 2.0,
        RECT_A_ORIGIN.1 + RECT_SIZE.1 / 2.0,
    ]);
    assert_eq!(
        pixel_at(gap_x, gap_y),
        background,
        "the gap between Rect A and Rect B must stay background"
    );
    eprintln!(
        "pixel level: all 3 rects (including rotated Rect C) rendered at their real transformed \
         positions -- OK"
    );

    let mut rgba_out = bgra.clone();
    for px in rgba_out.chunks_exact_mut(4) {
        px.swap(0, 2);
    }
    let out_path = std::env::var("TRE_CANVAS_ACCESSIBILITY_OUTPUT")
        .unwrap_or_else(|_| "canvas_accessibility_output.png".to_string());
    let file = std::fs::File::create(&out_path).expect("failed to create output PNG file");
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), CANVAS_WIDTH, CANVAS_HEIGHT);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().expect("failed to write PNG header");
    writer
        .write_image_data(&rgba_out)
        .expect("failed to write PNG image data");
    eprintln!("wrote {CANVAS_WIDTH}x{CANVAS_HEIGHT} canvas accessibility render to {out_path}");

    // --- Hand off the tagged IR to canvas_accessibility_verify, a
    // separate, genuinely Vulkan-free process, for the real AT-SPI2
    // publish-and-verify step (see this file's own top-level doc
    // comment for why that can't happen in this process) ---
    let nodes_path = std::env::var("TRE_CANVAS_ACCESSIBILITY_NODES_PATH")
        .unwrap_or_else(|_| "canvas_accessibility_nodes.txt".to_string());
    let mut nodes_text = String::new();
    for node in &frame.accessibility_nodes {
        let role = match node.role {
            AccessibilityRole::Generic => "Generic",
            AccessibilityRole::Button => "Button",
            AccessibilityRole::TextLabel => "TextLabel",
            AccessibilityRole::Image => "Image",
        };
        nodes_text.push_str(&format!(
            "{} {} {} {} {} {role}\n",
            node.node_id.0, node.x, node.y, node.width, node.height
        ));
    }
    std::fs::write(&nodes_path, nodes_text).expect("failed to write tagged-node handoff file");
    eprintln!(
        "wrote {} tagged accessibility nodes to {nodes_path} for canvas_accessibility_verify",
        frame.accessibility_nodes.len()
    );
    eprintln!("all canvas accessibility rendering assertions passed");
}
