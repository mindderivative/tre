//! Phase 7 Step 7.2.2 proof: `RenderingCanvas::push_layer`/`pop_layer`
//! driving a *real*, own-content Dual-Kawase blur end to end, through
//! one real recorded `Canvas` scene and `execute_frame` -- unlike
//! `dual_kawase_blur_demo.rs` (Step 7.2.1's own proof), nothing here is
//! a hand-written RHI call: `push_layer(&LayerDesc { blur: true, .. })`/
//! draw/`pop_layer` are the only calls that build the scene, and
//! `execute_frame` alone drives every RHI call (including the real blur
//! chain, `RhiCommandBuffer::apply_layer_blur`) from the resulting IR --
//! the same graduation `canvas_layer_composite_demo.rs` was to
//! `render_to_texture_demo.rs` at Step 6.4.2.
//!
//! 1. `canvas.push_layer(&LayerDesc { blur: true, .. })` records a
//!    `PushLayer` marker and pushes it onto the canvas's own layer
//!    stack.
//! 2. `canvas.draw_rounded_rect(..)`, called while the layer is still
//!    open, records an ordinary `DrawGeometry` command in the layer's
//!    own local coordinate space -- a small, isolated opaque square,
//!    well clear of every edge, matching `dual_kawase_blur_demo.rs`'s
//!    own real verification shape (a real blur's own spread needs real
//!    background on all sides to bleed into).
//! 3. `canvas.pop_layer()` records a `PopLayer` command carrying both
//!    the real, already-baked composite-quad geometry (Step 6.4.2) and
//!    the popped `LayerDesc`'s own `blur` flag (Step 7.2.2, smuggled
//!    through this command's `texture_handle` field -- `pop_layer`'s own
//!    doc comment, `tre-engine`).
//! 4. `execute_frame` walks the flattened IR: `PushLayer` acquires a
//!    transient target and redirects rendering into it, the
//!    `DrawGeometry` in between draws the square into it, `PopLayer` ends
//!    that render, sees the smuggled `blur` flag set, calls `cmd_buffer.
//!    apply_layer_blur` (the real 4-hop Dual-Kawase chain graduated from
//!    `dual_kawase_blur_demo.rs`'s own proven design), releases the
//!    original unblurred layer texture, then registers/composites/
//!    releases the *returned*, blurred texture instead of the raw one.
//!
//! Two pipelines, two declared color formats, same reasoning as Step
//! 6.4.2's own demo: `PipelineKind::SdfRoundedRect` is built against the
//! layer's own `Rgba16Float` format; `PipelineKind::TexturedQuad` is
//! built against the swapchain's own `HEADLESS_FORMAT`, matching
//! `pop_layer`'s own composite draw. `apply_layer_blur`'s own downsample/
//! upsample pipelines are never registered here at all -- they're
//! created lazily, internally, by `VulkanDevice` on first real use,
//! entirely invisible to this file (or any other caller).
//!
//! Same two real pixel assertions `dual_kawase_blur_demo.rs` proved, at
//! the same relative shape: the square's own deep interior stays
//! foreground after blur, and a point just outside its original hard
//! edge (but still inside the composited layer's own bounds) shows a
//! genuine partial blend -- now proven through the real `Canvas`/
//! `execute_frame` path instead of hand-written RHI calls.

use tre_engine::{
    execute_frame, rgba8, BufferBinding, LayerDesc, PipelineKind, PipelineRegistry,
    RenderingCanvas, RhiDevice, ScissorRect, TextureFormat,
};
use tre_rhi_vulkan::{HeadlessSwapchain, VulkanDevice};

#[path = "support/pixel_helpers.rs"]
mod pixel_helpers;

const SWAPCHAIN_WIDTH: u32 = 256;
const SWAPCHAIN_HEIGHT: u32 = 128;
const LAYER_WIDTH: u32 = 200;
const LAYER_HEIGHT: u32 = 100;
/// Where the composited layer's own top-left corner lands on the
/// swapchain.
const COMPOSITE_ORIGIN: (i32, i32) = (28, 14);

/// A small square, well clear of every edge of the layer's own local
/// bounds, so the blur's own spread has real, transparent layer
/// background on all sides to bleed into (matching `dual_kawase_blur_
/// demo.rs`'s own reasoning exactly).
const SQUARE_SIZE: f32 = 20.0;
const SQUARE_X: f32 = (LAYER_WIDTH as f32 - SQUARE_SIZE) / 2.0;
const SQUARE_Y: f32 = (LAYER_HEIGHT as f32 - SQUARE_SIZE) / 2.0;

fn main() {
    let mut probe_connection =
        tre_platform::PlatformConnection::new().expect("failed to connect to display server");
    let probe_window = probe_connection
        .create_window("tre layer blur probe (never shown)", 1, 1)
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

    let swapchain = HeadlessSwapchain::new(&device, SWAPCHAIN_WIDTH, SWAPCHAIN_HEIGHT)
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
            ash::vk::Format::R16G16B16A16_SFLOAT,
        )
        .expect("failed to create rect pipeline");

    let textured_vertex_spv = std::fs::read(format!("{out_dir}/bindless_textured.vert.spv"))
        .expect("failed to read compiled textured vertex shader");
    let textured_fragment_spv = std::fs::read(format!("{out_dir}/bindless_textured.frag.spv"))
        .expect("failed to read compiled textured fragment shader");
    let textured_pipeline = device
        .create_pipeline(
            &textured_vertex_spv,
            &textured_fragment_spv,
            tre_rhi_vulkan::HEADLESS_FORMAT,
        )
        .expect("failed to create textured pipeline");

    let mut pipelines = PipelineRegistry::new();
    pipelines.register(PipelineKind::SdfRoundedRect as u16, Box::new(rect_pipeline));
    pipelines.register(
        PipelineKind::TexturedQuad as u16,
        Box::new(textured_pipeline),
    );

    // --- The real recorded scene: push_layer with blur, draw, pop_layer
    // -- no hand-written RHI calls anywhere in this demo. ---
    let white = rgba8(255, 255, 255, 255);
    let mut canvas = RenderingCanvas::new();
    canvas.push_layer(&LayerDesc {
        x: COMPOSITE_ORIGIN.0,
        y: COMPOSITE_ORIGIN.1,
        width: LAYER_WIDTH,
        height: LAYER_HEIGHT,
        format: TextureFormat::Rgba16Float,
        blur: true,
    });
    canvas.draw_rounded_rect(SQUARE_X, SQUARE_Y, SQUARE_SIZE, SQUARE_SIZE, 0.0, white);
    canvas.pop_layer();
    let frame = canvas.flatten();

    let vertex_buffer = device
        .upload_buffer(
            bytemuck::cast_slice(&frame.vertices),
            ash::vk::BufferUsageFlags::VERTEX_BUFFER,
        )
        .expect("failed to upload vertex buffer");
    let index_buffer = device
        .upload_buffer(
            bytemuck::cast_slice(&frame.indices),
            ash::vk::BufferUsageFlags::INDEX_BUFFER,
        )
        .expect("failed to upload index buffer");

    let full_window = ScissorRect {
        x: 0,
        y: 0,
        width: SWAPCHAIN_WIDTH,
        height: SWAPCHAIN_HEIGHT,
    };

    // --- The real round trip -- driven entirely by execute_frame,
    // including the real blur chain. ---
    let (mut cmd_buffer, image) = device.begin_frame(&swapchain).expect("begin_frame failed");
    execute_frame(
        &frame,
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

    let bgra = swapchain
        .read_pixels_bgra8()
        .expect("failed to read back pixels");
    let pixel_at =
        |x: u32, y: u32| -> [u8; 4] { pixel_helpers::bgra_pixel_at(&bgra, SWAPCHAIN_WIDTH, x, y) };

    let background = pixel_at(5, 5);
    eprintln!("background (clear color): {background:?}");

    // The square's own deep interior, in swapchain space.
    let center = (
        (COMPOSITE_ORIGIN.0 as f32 + SQUARE_X + SQUARE_SIZE / 2.0) as u32,
        (COMPOSITE_ORIGIN.1 as f32 + SQUARE_Y + SQUARE_SIZE / 2.0) as u32,
    );
    let interior = pixel_at(center.0, center.1);
    assert!(
        interior[0] > 150 && interior[1] > 150 && interior[2] > 150,
        "the square's own deep interior must stay mostly foreground after a real blur \
         recorded/driven entirely through Canvas/execute_frame -- got {interior:?}"
    );
    eprintln!("bounded interior: OK (still mostly foreground, {interior:?})");

    // Just outside the square's own original hard left edge, but still
    // inside the composited layer's own bounds -- pure background before
    // any blur -- must now show a genuine partial blend.
    let edge_x = (COMPOSITE_ORIGIN.0 as f32 + SQUARE_X - 6.0) as u32;
    let bled = pixel_at(edge_x, center.1);
    assert_ne!(
        bled, background,
        "a point just outside the square's own original boundary must show real blur bleed, \
         not pure background -- got {bled:?}"
    );
    assert!(
        bled[0] < 250,
        "a point just outside the square's own original boundary must be a genuine partial \
         blend, not pure foreground either -- got {bled:?}"
    );
    eprintln!("edge bleed: OK (real partial blend outside the original hard edge, {bled:?})");

    let mut rgba = bgra.clone();
    for px in rgba.chunks_exact_mut(4) {
        px.swap(0, 2);
    }
    let out_path = std::env::var("TRE_LAYER_BLUR_OUTPUT")
        .unwrap_or_else(|_| "layer_blur_output.png".to_string());
    let file = std::fs::File::create(&out_path).expect("failed to create output PNG file");
    let mut encoder = png::Encoder::new(
        std::io::BufWriter::new(file),
        SWAPCHAIN_WIDTH,
        SWAPCHAIN_HEIGHT,
    );
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().expect("failed to write PNG header");
    writer
        .write_image_data(&rgba)
        .expect("failed to write PNG image data");

    eprintln!("wrote {SWAPCHAIN_WIDTH}x{SWAPCHAIN_HEIGHT} layer blur render to {out_path}");
    eprintln!("all layer blur assertions passed");
}
