//! Phase 9 Step 9.1 proof: a batching-equivalence pixel-diff test.
//! Records the *identical* real scene twice -- once flattened via
//! `RenderingCanvas::flatten()` (the real, production batched path:
//! adjacent same-Layer/Pipeline/Texture/clip commands merge into fewer
//! draw calls) and once via `RenderingCanvas::flatten_unbatched()` (the
//! same real sort, deliberately skipping the merge step -- one draw
//! call per original command) -- renders both through the real,
//! unmodified `execute_frame`, and asserts the two resulting images are
//! byte-for-byte identical across every pixel.
//!
//! This is the test IMPLEMENTATION.md Phase 9 Step 9.1's own rationale
//! names directly: "the performance suite alone cannot catch a
//! batching pass that is fast but wrong (e.g., silently dropping or
//! misordering a draw command)." Batching is specified as a pure
//! performance optimization -- fewer draw calls, identical visual
//! result -- so any pixel mismatch between the two renders here is a
//! real batching or sort-key bug, never a "just needs updating"
//! performance regression.
//!
//! The scene: four non-overlapping SDF rects sharing the exact same
//! Layer/Pipeline/Texture/clip state (all standard-plane, all the
//! `SdfRoundedRect` pipeline, no texture, no active clip beyond the
//! full window) -- real `flatten()` merges all four into one 24-index
//! batch; `flatten_unbatched()` keeps all four as separate 6-index
//! commands. Both must still place every rect at its own correct
//! position with nothing dropped, misordered, or overdrawn.

use ash::vk;
use tre_engine::{
    execute_frame, rgba8, BufferBinding, PipelineKind, PipelineRegistry, RenderingCanvas,
    RhiDevice, ScissorRect,
};
use tre_rhi_vulkan::{HeadlessSwapchain, VulkanDevice};

#[path = "support/pixel_helpers.rs"]
mod pixel_helpers;

const CANVAS_WIDTH: u32 = 160;
const CANVAS_HEIGHT: u32 = 120;
const RECT_SIZE: f32 = 30.0;
const RECT_ORIGINS: [(f32, f32); 4] = [(10.0, 10.0), (70.0, 10.0), (10.0, 70.0), (70.0, 70.0)];

/// Records the exact same real scene every time it's called -- four
/// non-overlapping, same-Layer/Pipeline/Texture/clip SDF rects, the
/// only variable this whole demo is testing the equivalence of.
fn record_scene() -> RenderingCanvas {
    let white = rgba8(255, 255, 255, 255);
    let mut canvas = RenderingCanvas::new();
    for &(x, y) in &RECT_ORIGINS {
        canvas.draw_rounded_rect(x, y, RECT_SIZE, RECT_SIZE, 0.0, white);
    }
    canvas
}

fn main() {
    let mut probe_connection =
        tre_platform::PlatformConnection::new().expect("failed to connect to display server");
    let probe_window = probe_connection
        .create_window("tre batching equivalence probe (never shown)", 1, 1)
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
    let rect_vertex_spv = std::fs::read(format!("{out_dir}/sdf_rounded_rect.vert.spv"))
        .expect("failed to read compiled rect vertex shader");
    let rect_fragment_spv = std::fs::read(format!("{out_dir}/sdf_rounded_rect.frag.spv"))
        .expect("failed to read compiled rect fragment shader");
    let rect_pipeline = device
        .create_pipeline(
            &rect_vertex_spv,
            &rect_fragment_spv,
            tre_rhi_vulkan::HEADLESS_FORMAT,
        )
        .expect("failed to create rect pipeline");
    let mut pipelines = PipelineRegistry::new();
    pipelines.register(PipelineKind::SdfRoundedRect as u16, Box::new(rect_pipeline));

    let full_window = ScissorRect {
        x: 0,
        y: 0,
        width: CANVAS_WIDTH,
        height: CANVAS_HEIGHT,
    };

    let render = |frame: &tre_engine::FlattenedFrame| -> Vec<u8> {
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
        execute_frame(
            frame,
            &pipelines,
            BufferBinding {
                buffer: &vertex_buffer,
                offset: 0,
            },
            BufferBinding {
                buffer: &index_buffer,
                offset: 0,
            },
            &full_window,
            &device,
            &mut *cmd_buffer,
        );
        device
            .submit_and_present(cmd_buffer, &swapchain, image)
            .expect("submit_and_present failed");
        swapchain
            .read_pixels_bgra8()
            .expect("failed to read back pixels")
    };

    // --- Batched: the real, production flatten() path. ---
    let batched_frame = record_scene().flatten();
    let batched_draws = batched_frame
        .commands
        .iter()
        .filter(|c| c.kind == tre_engine::CommandType::DrawGeometry)
        .count();
    assert_eq!(
        batched_draws, 1,
        "four rects sharing Layer/Pipeline/Texture/clip must merge into exactly one batch, got {batched_draws}"
    );
    let batched_pixels = render(&batched_frame);
    eprintln!("batched render: {batched_draws} draw call(s), as expected");

    // --- Unbatched: the same real sort, merge step skipped. ---
    let unbatched_frame = record_scene().flatten_unbatched();
    let unbatched_draws = unbatched_frame
        .commands
        .iter()
        .filter(|c| c.kind == tre_engine::CommandType::DrawGeometry)
        .count();
    assert_eq!(
        unbatched_draws, 4,
        "flatten_unbatched must keep all four rects as separate draw calls, got {unbatched_draws}"
    );
    let unbatched_pixels = render(&unbatched_frame);
    eprintln!("unbatched render: {unbatched_draws} draw call(s), as expected");

    // --- The real equivalence check: every pixel, byte for byte. ---
    assert_eq!(
        batched_pixels.len(),
        unbatched_pixels.len(),
        "both renders must read back the exact same buffer size"
    );
    let mut first_mismatch: Option<usize> = None;
    for (i, (b, u)) in batched_pixels
        .iter()
        .zip(unbatched_pixels.iter())
        .enumerate()
    {
        if b != u {
            first_mismatch = Some(i);
            break;
        }
    }
    assert!(
        first_mismatch.is_none(),
        "batched and unbatched renders must be byte-for-byte identical -- first mismatch at \
         byte offset {:?} (batched={:?}, unbatched={:?}); a mismatch here is a real batching or \
         sort-key bug, not a performance regression",
        first_mismatch,
        first_mismatch.map(|i| batched_pixels[i]),
        first_mismatch.map(|i| unbatched_pixels[i]),
    );
    eprintln!("batched vs. unbatched: byte-for-byte identical across the whole framebuffer -- OK");

    // --- Sanity: real, non-background content actually landed where
    // expected, so an all-background false pass (both renders equally
    // broken) can't slip through as "equivalent." ---
    let pixel_at = |buf: &[u8], x: u32, y: u32| -> [u8; 4] {
        pixel_helpers::bgra_pixel_at(buf, CANVAS_WIDTH, x, y)
    };
    let background = pixel_at(&batched_pixels, 0, 0);
    for &(x, y) in &RECT_ORIGINS {
        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "canvas coordinates are fixed, well within [0, CANVAS_WIDTH/HEIGHT)"
        )]
        let center = ((x + RECT_SIZE / 2.0) as u32, (y + RECT_SIZE / 2.0) as u32);
        let observed = pixel_at(&batched_pixels, center.0, center.1);
        assert_ne!(
            observed, background,
            "rect at {x},{y} must have rendered real, non-background content"
        );
    }
    eprintln!("all four rects rendered real, non-background content -- OK");

    let mut rgba_out = batched_pixels.clone();
    for px in rgba_out.chunks_exact_mut(4) {
        px.swap(0, 2);
    }
    let out_path = std::env::var("TRE_BATCHING_EQUIVALENCE_OUTPUT")
        .unwrap_or_else(|_| "batching_equivalence_output.png".to_string());
    let file = std::fs::File::create(&out_path).expect("failed to create output PNG file");
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), CANVAS_WIDTH, CANVAS_HEIGHT);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().expect("failed to write PNG header");
    writer
        .write_image_data(&rgba_out)
        .expect("failed to write PNG image data");

    eprintln!("wrote {CANVAS_WIDTH}x{CANVAS_HEIGHT} batching equivalence render to {out_path}");
    eprintln!("all batching equivalence assertions passed");
}
