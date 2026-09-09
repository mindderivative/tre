//! Phase 6 Step 6.4.2 proof: `RenderingCanvas::push_layer`/`pop_layer`
//! driving Step 6.4.1's real render-to-texture capability end to end,
//! through one real recorded `Canvas` scene and `execute_frame` --
//! unlike `render_to_texture_demo.rs` (Step 6.4.1's own proof), nothing
//! here is a hand-written RHI call: `push_layer`/draw/`pop_layer` are
//! the only calls that build the scene, and `execute_frame` alone drives
//! every RHI call from the resulting IR.
//!
//! 1. `canvas.push_layer(&LayerDesc { .. })` records a `PushLayer`
//!    marker and pushes it onto the canvas's own layer stack.
//! 2. `canvas.draw_rounded_rect(..)`, called while the layer is still
//!    open, records an ordinary `DrawGeometry` command in the layer's
//!    own local coordinate space -- nothing about `draw_rounded_rect`
//!    itself is layer-aware; it's `execute_frame`'s own command-stream
//!    ordering that makes this land inside the layer's texture, since
//!    the RHI's currently-bound render target is whatever the most
//!    recent `PushLayer`/`PopLayer` left it as.
//! 3. `canvas.pop_layer()` records a `PopLayer` command carrying a real,
//!    already-baked composite-quad geometry (`pop_layer`'s own doc
//!    comment, `tre-engine`) sized to the `LayerDesc`'s own on-screen
//!    `x`/`y`/`width`/`height`.
//! 4. `execute_frame` walks the flattened IR: `PushLayer` acquires a
//!    transient target and redirects rendering into it, the
//!    `DrawGeometry` in between draws the rounded rect into it, `PopLayer`
//!    ends that render, registers the result bindless, resumes swapchain
//!    rendering, and draws the composite quad using the just-registered
//!    index.
//!
//! Two pipelines, two declared color formats, same reasoning as Step
//! 6.4.1's own demo: `PipelineKind::SdfRoundedRect` is built against the
//! layer's own `Rgba16Float` format (every draw that pipeline id ever
//! resolves to in this demo happens inside the layer, so one registered
//! pipeline object suffices -- `PipelineRegistry` maps one id to exactly
//! one pipeline object per frame, so a scene that also drew directly to
//! the swapchain with the same pipeline kind would need a second id).
//! `PipelineKind::TexturedQuad` is built against the swapchain's own
//! `HEADLESS_FORMAT`, exactly as `pop_layer`'s own composite draw
//! expects.
//!
//! Same two real pixel assertions `render_to_texture_demo.rs` proved,
//! at the same coordinates, since this demo reproduces its exact scene
//! -- just recorded through `Canvas`/`execute_frame` instead of by hand.

use tre_engine::{
    execute_frame, rgba8, BufferBinding, CommandType, LayerDesc, PipelineKind, PipelineRegistry,
    RenderingCanvas, RhiDevice, ScissorRect, TextureFormat,
};
use tre_rhi_vulkan::{HeadlessSwapchain, VulkanDevice};

#[path = "support/pixel_helpers.rs"]
mod pixel_helpers;

const SWAPCHAIN_WIDTH: u32 = 200;
const SWAPCHAIN_HEIGHT: u32 = 150;
const LAYER_WIDTH: u32 = 100;
const LAYER_HEIGHT: u32 = 80;
/// Where the composited layer's own top-left corner lands on the
/// swapchain -- matches `render_to_texture_demo.rs`'s own
/// `COMPOSITE_ORIGIN` exactly, so this demo's assertions land at the
/// same swapchain coordinates as that one's hand-written proof.
const COMPOSITE_ORIGIN: (i32, i32) = (50, 40);

fn main() {
    let mut probe_connection =
        tre_platform::PlatformConnection::new().expect("failed to connect to display server");
    let probe_window = probe_connection
        .create_window("tre canvas layer composite probe (never shown)", 1, 1)
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

    // --- Two pipelines, two declared color formats -- see this file's
    // own header for why each needs its own id/format pairing. ---
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

    // --- The real recorded scene: push_layer, draw, pop_layer -- no
    // hand-written RHI calls anywhere in this demo. ---
    let white = rgba8(255, 255, 255, 255);
    let mut canvas = RenderingCanvas::new();
    canvas.push_layer(&LayerDesc {
        x: COMPOSITE_ORIGIN.0,
        y: COMPOSITE_ORIGIN.1,
        width: LAYER_WIDTH,
        height: LAYER_HEIGHT,
        format: TextureFormat::Rgba16Float,
    });
    // Local to the layer's own LAYER_WIDTH x LAYER_HEIGHT bounds --
    // independent of where the layer is later composited.
    canvas.draw_rounded_rect(10.0, 10.0, 80.0, 60.0, 10.0, white);
    canvas.pop_layer();
    let frame = canvas.flatten();

    // --- IR-level sanity check: exactly the 3 commands push_layer/
    // draw_rounded_rect/pop_layer must record, in that order. ---
    assert_eq!(
        frame.commands.len(),
        3,
        "expected exactly PushLayer, DrawGeometry, PopLayer -- got {:?}",
        frame.commands.iter().map(|c| c.kind).collect::<Vec<_>>()
    );
    assert_eq!(frame.commands[0].kind, CommandType::PushLayer);
    assert_eq!(frame.commands[1].kind, CommandType::DrawGeometry);
    assert_eq!(frame.commands[2].kind, CommandType::PopLayer);
    eprintln!("IR level: PushLayer, DrawGeometry, PopLayer, in order -- OK");

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

    // --- The real round trip -- driven entirely by execute_frame. ---
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

    // Far outside the composited region entirely.
    let background = pixel_at(5, 5);
    eprintln!("background (clear color): {background:?}");

    // Same coordinates as render_to_texture_demo.rs's own proof: the
    // rect's own deep interior, composited onto the swapchain -- must be
    // real, exactly opaque foreground, proving content genuinely
    // recorded via Canvas, rendered into the offscreen target by
    // execute_frame's own PushLayer handling, and survived the round
    // trip back via its own PopLayer handling.
    let composited_interior = pixel_at(100, 80);
    assert_eq!(
        composited_interior,
        [255, 255, 255, 255],
        "the composited rect's own interior must be exactly the foreground color, \
         got {composited_interior:?}"
    );
    eprintln!("composited interior: OK ({composited_interior:?})");

    // Inside the composited region's own bounds, but outside the rounded
    // rect's own footprint -- real background must show through, proving
    // the layer target was actually cleared to transparent and the
    // default blend state composites it correctly.
    let composited_transparent = pixel_at(52, 42);
    assert_eq!(
        composited_transparent, background,
        "a genuinely transparent part of the composited layer must show real background \
         through it, got {composited_transparent:?}"
    );
    eprintln!(
        "composited transparent area: OK (real background shows through: {composited_transparent:?})"
    );

    let mut rgba = bgra.clone();
    for px in rgba.chunks_exact_mut(4) {
        px.swap(0, 2);
    }
    let out_path = std::env::var("TRE_LAYER_COMPOSITE_OUTPUT")
        .unwrap_or_else(|_| "canvas_layer_composite_output.png".to_string());
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

    eprintln!("wrote {SWAPCHAIN_WIDTH}x{SWAPCHAIN_HEIGHT} layer composite to {out_path}");
    eprintln!("all canvas layer composite assertions passed");
}
