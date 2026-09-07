//! Phase 5 Step 5.1.1 proof: `RenderingCanvas`'s new hierarchical Drawing
//! Context state -- `save`/`restore` (transform + alpha) and the
//! separate `push_clip`/`pop_clip` scissor stack -- wired into the one
//! real primitive it already had (`draw_rounded_rect`, Step 3.2),
//! rendered through the existing, unmodified `sdf_rounded_rect` pipeline.
//!
//! Two effects are proven with real, rendered GPU pixels, since they
//! actually change the vertex data this step uploads: a rect drawn
//! inside a `save()`/`transform()`/`restore()` bracket lands at its
//! transformed world position, not its raw local one; a rect drawn
//! inside a `save()`/`set_alpha()`/`restore()` bracket blends visibly
//! against the background rather than rendering fully opaque.
//!
//! `push_clip`/`pop_clip` is checked at the IR level instead (the
//! `UiDrawCommand::clip_bounds` this step records), not with a GPU
//! scissor test -- nothing in the render pipeline consumes
//! `clip_bounds` yet (that wiring is Step 5.1.3/Phase 6's real
//! batch-flattening job); `tre-engine`'s own unit tests already prove
//! the intersection logic in isolation, and this demo's own IR check
//! confirms it still reaches a real, real-geometry-carrying frame
//! correctly, not just an isolated `RenderingCanvas`.

use ash::vk;
use tre_engine::{rgba8, CommandType, RenderingCanvas, RhiDevice, ScissorRect};
use tre_math::Affine2;
use tre_rhi_vulkan::{HeadlessSwapchain, VulkanDevice};

#[path = "support/pixel_helpers.rs"]
mod pixel_helpers;

const CANVAS_WIDTH: u32 = 200;
const CANVAS_HEIGHT: u32 = 150;

fn main() {
    let mut probe_connection =
        tre_platform::PlatformConnection::new().expect("failed to connect to display server");
    let probe_window = probe_connection
        .create_window("tre canvas state stack probe (never shown)", 1, 1)
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

    let white = rgba8(255, 255, 255, 255);
    let mut canvas = RenderingCanvas::new();

    // --- Rect A: save()/transform()/restore() -- a 50x50 square drawn at
    // local (0, 0), translated by (70, 60), so it must land at world
    // (70, 60)-(120, 110), not at its raw local position at all. ---
    // Command 0: DrawGeometry.
    canvas.save();
    canvas.transform(&Affine2::from_translation(70.0, 60.0));
    canvas.draw_rounded_rect(0.0, 0.0, 50.0, 50.0, 8.0, white);
    canvas.restore();

    // --- Rect B: save()/set_alpha()/restore() -- a plain (unrounded)
    // 50x50 square at local (10, 10), alpha halved, non-overlapping with
    // Rect A. ---
    // Command 1: DrawGeometry.
    canvas.save();
    canvas.set_alpha(0.5);
    canvas.draw_rounded_rect(10.0, 10.0, 50.0, 50.0, 0.0, white);
    canvas.restore();

    // --- Rect C: push_clip()/pop_clip() -- checked at the IR level only,
    // see this file's own doc comment for why. ---
    // Step 5.1.3's real batch flattening merges Rect A and Rect B (both
    // default Layer/Pipeline/Texture, both drawn before any push_clip,
    // so both share the full-window clip_bounds) into a single command
    // -- so the sequence is: Command 0: DrawGeometry (Rect A + Rect B
    // merged). Command 1: PushScissor. Command 2: DrawGeometry (Rect C,
    // this is the one this demo inspects). Command 3: PopScissor.
    const RECT_C_COMMAND_INDEX: usize = 2;
    let clip_rect = ScissorRect {
        x: 150,
        y: 10,
        width: 30,
        height: 30,
    };
    canvas.push_clip(&clip_rect);
    canvas.draw_rounded_rect(150.0, 10.0, 30.0, 30.0, 0.0, white);
    canvas.pop_clip();

    let frame = canvas.flatten();
    assert_eq!(
        frame.commands[RECT_C_COMMAND_INDEX].kind,
        CommandType::DrawGeometry
    );
    assert_eq!(
        frame.commands[RECT_C_COMMAND_INDEX].clip_bounds, clip_rect,
        "Rect C's DrawGeometry command must carry the pushed clip rect"
    );
    eprintln!("push_clip/pop_clip: IR-level clip_bounds verified: {clip_rect:?}");

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

    let (mut cmd_buffer, image) = device.begin_frame(&swapchain).expect("begin_frame failed");
    cmd_buffer.set_pipeline(&pipeline);
    cmd_buffer.bind_vertex_buffer(&vertex_buffer, 0);
    cmd_buffer.bind_index_buffer(&index_buffer, 0);
    cmd_buffer.draw_indexed(frame.indices.len() as u32, 0, 0);
    device
        .submit_and_present(cmd_buffer, &swapchain, image)
        .expect("submit_and_present failed");

    let bgra = swapchain
        .read_pixels_bgra8()
        .expect("failed to read back pixels");
    let pixel_at =
        |x: u32, y: u32| -> [u8; 4] { pixel_helpers::bgra_pixel_at(&bgra, CANVAS_WIDTH, x, y) };
    let background = pixel_at(0, 0);
    eprintln!("background (clear color): {background:?}");

    // Rect A: the transformed world position must be filled...
    let transformed_center = pixel_at(95, 85);
    assert_eq!(
        transformed_center,
        [255, 255, 255, 255],
        "Rect A's transformed world position must be exactly white, got {transformed_center:?}"
    );
    // ...and its raw, untransformed local footprint (0,0)-(50,50) must NOT
    // show it either -- proving the transform actually moved it rather
    // than drawing an extra copy. (5, 5), not (25, 25): the latter falls
    // inside Rect B's own footprint (10,10)-(60,60), which would still be
    // real, blended, non-background content for an unrelated reason.
    let untransformed_corner = pixel_at(5, 5);
    assert_eq!(
        untransformed_corner, background,
        "Rect A's raw local footprint must be untouched background, got {untransformed_corner:?}"
    );
    eprintln!("save/transform/restore: OK (world position filled, local position untouched)");

    // Rect B: alpha 0.5 must produce a genuine partial blend -- neither
    // the fully-opaque foreground nor untouched background, the same
    // gamma-agnostic check `sdf_rounded_rect_demo`'s own AA-band
    // assertion uses (the exact blended byte value depends on a gamma
    // question -- REVIEW.md finding #92 -- this step doesn't need to
    // settle).
    let alpha_blended = pixel_at(35, 35);
    assert_ne!(
        alpha_blended,
        [255, 255, 255, 255],
        "Rect B at alpha 0.5 must not render fully opaque, got {alpha_blended:?}"
    );
    assert_ne!(
        alpha_blended, background,
        "Rect B at alpha 0.5 must not render fully transparent, got {alpha_blended:?}"
    );
    eprintln!("save/set_alpha/restore: OK (genuine partial blend {alpha_blended:?})");

    let mut rgba_out = bgra.clone();
    for px in rgba_out.chunks_exact_mut(4) {
        px.swap(0, 2);
    }
    let out_path = std::env::var("TRE_CANVAS_STATE_STACK_OUTPUT")
        .unwrap_or_else(|_| "canvas_state_stack_output.png".to_string());
    let file = std::fs::File::create(&out_path).expect("failed to create output PNG file");
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), CANVAS_WIDTH, CANVAS_HEIGHT);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().expect("failed to write PNG header");
    writer
        .write_image_data(&rgba_out)
        .expect("failed to write PNG image data");

    eprintln!("wrote {CANVAS_WIDTH}x{CANVAS_HEIGHT} canvas state stack render to {out_path}");
    eprintln!("all canvas state stack assertions passed");
}
