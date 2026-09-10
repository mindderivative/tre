//! Phase 6 Step 6.4.1 proof: the real RHI render-to-texture capability
//! -- the first time this project has ever rendered into an offscreen
//! target and sampled the result back, on real Vulkan hardware.
//!
//! No `Canvas`/IR involvement at all (`push_layer`/`pop_layer` stay real
//! but inert markers until Step 6.4.2 wires them to this same
//! capability) -- every RHI call below is hand-written, proving the
//! capability itself in isolation:
//!
//! 1. Acquire a transient target (`Rgba16Float`, ARCHITECTURE.md
//!    Section 5's own prescription for offscreen layers).
//! 2. `begin_render_to_texture`/`end_render_to_texture` redirect
//!    rendering into it -- a real rounded rect, via the existing,
//!    unmodified `sdf_rounded_rect` pipeline (its own `create_pipeline`
//!    call built against the layer's own `R16G16B16A16_SFLOAT` format,
//!    not the swapchain's -- dynamic rendering requires a pipeline's
//!    declared color format to match whatever it's actually bound
//!    against at draw time).
//! 3. `register_bindless` makes the rendered-into texture sample-able.
//! 4. `resume_swapchain_rendering` returns to the swapchain
//!    `begin_frame` originally set up, preserving what it already had
//!    drawn.
//! 5. The layer is composited back as a textured quad via the existing,
//!    unmodified bindless-textured pipeline and the default
//!    premultiplied-alpha blend state every pipeline already carries --
//!    no special-casing needed for a correct "over" composite.
//! 6. `deregister_bindless` then `release_transient_target` return the
//!    texture to the pool, exactly as any other transient-target user
//!    must.

use ash::vk;
use tre_engine::{rgba8, submit_frame, RenderingCanvas, RhiDevice, TextureFormat, UiVertex};
use tre_rhi_vulkan::{HeadlessSwapchain, VulkanDevice};

#[path = "support/pixel_helpers.rs"]
mod pixel_helpers;

const SWAPCHAIN_WIDTH: u32 = 200;
const SWAPCHAIN_HEIGHT: u32 = 150;
const LAYER_WIDTH: u32 = 100;
const LAYER_HEIGHT: u32 = 80;
/// Where the composited layer's own top-left corner lands on the
/// swapchain -- a plain 1:1 blit, no scaling.
const COMPOSITE_ORIGIN: (f32, f32) = (50.0, 40.0);

/// A plain textured quad covering `(x, y)`-`(x + width, y + height)` in
/// whatever coordinate space the caller draws it into, sampling the
/// bindless-textured pipeline's bound texture across its full `(0,0)`-
/// `(1,1)` extent -- the same shape `bindless_textures_demo.rs`'s own
/// `quad_vertices` builds, just parameterized by width/height separately
/// since a layer composite is not generally square.
fn textured_quad(x: f32, y: f32, width: f32, height: f32) -> [UiVertex; 4] {
    let white = rgba8(255, 255, 255, 255);
    [
        UiVertex {
            position: [x, y],
            uv: [0.0, 0.0],
            color: white,
            params: [0.0; 3],
        },
        UiVertex {
            position: [x + width, y],
            uv: [1.0, 0.0],
            color: white,
            params: [0.0; 3],
        },
        UiVertex {
            position: [x + width, y + height],
            uv: [1.0, 1.0],
            color: white,
            params: [0.0; 3],
        },
        UiVertex {
            position: [x, y + height],
            uv: [0.0, 1.0],
            color: white,
            params: [0.0; 3],
        },
    ]
}

fn main() {
    let mut probe_connection =
        tre_platform::PlatformConnection::new().expect("failed to connect to display server");
    let probe_window = probe_connection
        .create_window("tre render-to-texture probe (never shown)", 1, 1)
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

    // --- Two pipelines, two different declared color formats: the rect
    // pipeline draws into the layer's own R16G16B16A16_SFLOAT target, the
    // textured pipeline composites onto the swapchain's own format.
    // Dynamic rendering requires a pipeline's declared format to match
    // whatever it's actually bound against at draw time. ---
    let out_dir = env!("OUT_DIR");
    let rect_vertex_spv = std::fs::read(format!("{out_dir}/sdf_rounded_rect.vert.spv"))
        .expect("failed to read compiled rect vertex shader");
    let rect_fragment_spv = std::fs::read(format!("{out_dir}/sdf_rounded_rect.frag.spv"))
        .expect("failed to read compiled rect fragment shader");
    let rect_pipeline = device
        .create_pipeline(
            &rect_vertex_spv,
            &rect_fragment_spv,
            vk::Format::R16G16B16A16_SFLOAT,
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

    // --- The shape drawn into the layer: a rounded rect filling most of
    // the layer's own local bounds, via RenderingCanvas purely as a
    // convenient vertex-data builder (the same way `sdf_rounded_rect_
    // demo.rs` uses it) -- no push_layer/pop_layer/execute_frame
    // involved anywhere in this demo. ---
    let white = rgba8(255, 255, 255, 255);
    let mut layer_canvas = RenderingCanvas::new();
    layer_canvas.draw_rounded_rect(10.0, 10.0, 80.0, 60.0, 10.0, white);
    let layer_frame = layer_canvas.flatten();
    let layer_vertex_buffer = device
        .upload_buffer(
            bytemuck::cast_slice(&layer_frame.vertices),
            vk::BufferUsageFlags::VERTEX_BUFFER,
        )
        .expect("failed to upload layer vertex buffer");
    let layer_index_buffer = device
        .upload_buffer(
            bytemuck::cast_slice(&layer_frame.indices),
            vk::BufferUsageFlags::INDEX_BUFFER,
        )
        .expect("failed to upload layer index buffer");

    // --- The composite quad: a plain textured quad in swapchain pixel
    // coordinates, covering exactly the composited region. ---
    let quad = textured_quad(
        COMPOSITE_ORIGIN.0,
        COMPOSITE_ORIGIN.1,
        LAYER_WIDTH as f32,
        LAYER_HEIGHT as f32,
    );
    let quad_indices: [u32; 6] = [0, 1, 2, 2, 3, 0];
    let quad_vertex_buffer = device
        .upload_buffer(
            bytemuck::cast_slice(&quad),
            vk::BufferUsageFlags::VERTEX_BUFFER,
        )
        .expect("failed to upload composite quad vertex buffer");
    let quad_index_buffer = device
        .upload_buffer(
            bytemuck::cast_slice(&quad_indices),
            vk::BufferUsageFlags::INDEX_BUFFER,
        )
        .expect("failed to upload composite quad index buffer");

    // --- The real round trip ---
    let layer_texture = device
        .acquire_transient_target(LAYER_WIDTH, LAYER_HEIGHT, TextureFormat::Rgba16Float)
        .expect("failed to acquire transient layer target");

    // Hoisted above the closure: `deregister_bindless` below needs this
    // value again after `submit_frame` returns, and registration itself
    // is a pure device-side call with no dependency on `cmd_buffer`.
    let bindless_index = device
        .register_bindless(&*layer_texture)
        .expect("failed to register the rendered-into layer as bindless");

    submit_frame(&device, &swapchain, |cmd_buffer| {
        cmd_buffer.begin_render_to_texture(&*layer_texture, LAYER_WIDTH, LAYER_HEIGHT);
        cmd_buffer.set_pipeline(&rect_pipeline);
        cmd_buffer.bind_vertex_buffer(&layer_vertex_buffer, 0);
        cmd_buffer.bind_index_buffer(&layer_index_buffer, 0);
        cmd_buffer.draw_indexed(layer_frame.indices.len() as u32, 0, 0);
        cmd_buffer.end_render_to_texture(&*layer_texture);

        cmd_buffer.resume_swapchain_rendering();
        cmd_buffer.set_pipeline(&textured_pipeline);
        cmd_buffer.bind_texture(0, bindless_index);
        cmd_buffer.bind_vertex_buffer(&quad_vertex_buffer, 0);
        cmd_buffer.bind_index_buffer(&quad_index_buffer, 0);
        cmd_buffer.draw_indexed(quad_indices.len() as u32, 0, 0);
    })
    .expect("submit_frame failed");

    device.deregister_bindless(bindless_index);
    device.release_transient_target(layer_texture);

    let bgra = swapchain
        .read_pixels_bgra8()
        .expect("failed to read back pixels");
    let pixel_at =
        |x: u32, y: u32| -> [u8; 4] { pixel_helpers::bgra_pixel_at(&bgra, SWAPCHAIN_WIDTH, x, y) };

    // Far outside the composited region entirely.
    let background = pixel_at(5, 5);
    eprintln!("background (clear color): {background:?}");

    // The layer-local rect's own deep interior (local (50, 40), well
    // inside its (10,10)-(90,70) footprint) composited at swapchain
    // (COMPOSITE_ORIGIN + local) = (100, 80) -- must be real, exactly
    // opaque foreground, proving content genuinely rendered into the
    // offscreen target and survived the round trip back to the
    // swapchain.
    let composited_interior = pixel_at(100, 80);
    assert_eq!(
        composited_interior,
        [255, 255, 255, 255],
        "the composited rect's own interior must be exactly the foreground color, \
         got {composited_interior:?}"
    );
    eprintln!("composited interior: OK ({composited_interior:?})");

    // Inside the composited region's own bounds, but outside the rounded
    // rect's own footprint (layer-local (2, 2), cut away by the layer's
    // own clear-to-transparent -- composited swapchain (52, 42)). Real
    // premultiplied-alpha "over" blending of a genuinely transparent
    // source must reproduce the swapchain's own background exactly,
    // proving the layer target was actually cleared to transparent (not
    // opaque or garbage) and that the default blend state composites it
    // correctly with no special-casing.
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
    let out_path = std::env::var("TRE_RENDER_TO_TEXTURE_OUTPUT")
        .unwrap_or_else(|_| "render_to_texture_output.png".to_string());
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

    eprintln!(
        "wrote {SWAPCHAIN_WIDTH}x{SWAPCHAIN_HEIGHT} render-to-texture composite to {out_path}"
    );
    eprintln!("all render-to-texture assertions passed");
}
