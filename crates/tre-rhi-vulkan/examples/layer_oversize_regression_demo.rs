//! REVIEW.md finding #152: a real, GPU-backed regression proof for a
//! bug found while checking whether Step 7.2.2 (wiring Dual-Kawase blur
//! into `push_layer`/`pop_layer`) would be building on solid ground.
//!
//! `RhiDevice::acquire_transient_target`'s own documented "oversized
//! borrow" fallback can hand back a texture *larger* than requested,
//! whenever no free bucket of the exact requested size exists yet but a
//! larger, already-freed one does. Before this finding's fix,
//! `RhiCommandBuffer::begin_render_to_texture`/`begin_render_to_texture_
//! no_end` fed that texture's own *real* dimensions into `draw_indexed`'s
//! NDC-mapping push constant -- so any layer whose content was recorded
//! (at `Canvas` record time) assuming its own smaller, *requested* size
//! would have its vertex positions mapped against the wrong, larger
//! `screen_size`, confining the actual draw to a small corner of the
//! oversized image. From the outside, this looked like the layer's own
//! content simply vanishing from the composited frame -- no error, no
//! validation warning, just missing content. Same underlying mechanism
//! as REVIEW.md finding #130's real root cause, reached through the
//! standard, production `PushLayer`/`PopLayer` path instead of a
//! hand-rolled demo.
//!
//! This demo reproduces the exact triggering sequence: push+pop a first,
//! larger layer (a fresh pool allocation), letting it release back to
//! the pool at frame end, then push+pop a second, smaller,
//! never-before-requested layer in a later frame. The pool has no exact
//! bucket for the second size yet, but does have the first layer's own
//! freed, larger one -- so it hands that back instead. The second
//! layer's own content (a rect nearly filling its own local bounds) must
//! still composite correctly.
//!
//! The fix: `begin_render_to_texture`/`begin_render_to_texture_no_end`
//! now take an explicit `logical_width`/`logical_height` -- the caller's
//! own *intended* size, always already known (it's exactly what was
//! passed to `acquire_transient_target`) -- and use that, not the
//! texture's own real size, for `self.width`/`self.height`. Viewport/
//! scissor/render area are untouched, still driven by the texture's real
//! size -- content simply draws "stretched" to fill it, a stretch
//! exactly undone later when `PopLayer`'s own composite quad samples it
//! back through a normalized `(0,0)`-`(1,1)` UV read and redraws it at
//! its own real, requested on-screen size. No UV rescaling or dynamic
//! vertex-buffer rewriting needed anywhere -- see `tre-engine`'s own
//! `RhiCommandBuffer::begin_render_to_texture` doc comment for the full
//! account.

use tre_engine::{
    execute_frame, rgba8, BufferBinding, LayerDesc, PipelineKind, PipelineRegistry,
    RenderingCanvas, RhiDevice, ScissorRect, TextureFormat,
};
use tre_rhi_vulkan::{HeadlessSwapchain, VulkanDevice};

#[path = "support/pixel_helpers.rs"]
mod pixel_helpers;

const SWAPCHAIN_WIDTH: u32 = 300;
const SWAPCHAIN_HEIGHT: u32 = 200;

/// The first, larger layer -- a fresh pool allocation, released at the
/// end of its own frame.
const FIRST_LAYER_SIZE: (u32, u32) = (200, 150);
const FIRST_LAYER_ORIGIN: (i32, i32) = (10, 10);

/// The second, smaller, never-before-requested layer size -- small
/// enough that the pool's own "oversized borrow" fallback (any free
/// bucket at least as large as requested) should hand back the first
/// layer's freed 200x150 texture instead of allocating fresh.
const SECOND_LAYER_SIZE: (u32, u32) = (50, 40);
const SECOND_LAYER_ORIGIN: (i32, i32) = (220, 20);

fn main() {
    let mut probe_connection =
        tre_platform::PlatformConnection::new().expect("failed to connect to display server");
    let probe_window = probe_connection
        .create_window("tre layer oversize regression probe (never shown)", 1, 1)
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
    let read_spv = |name: &str| {
        std::fs::read(format!("{out_dir}/{name}.spv"))
            .unwrap_or_else(|e| panic!("failed to read compiled shader {name}: {e}"))
    };
    let rect_pipeline = device
        .create_pipeline(
            &read_spv("sdf_rounded_rect.vert"),
            &read_spv("sdf_rounded_rect.frag"),
            ash::vk::Format::R16G16B16A16_SFLOAT,
        )
        .expect("failed to create rect pipeline");
    let textured_pipeline = device
        .create_pipeline(
            &read_spv("bindless_textured.vert"),
            &read_spv("bindless_textured.frag"),
            tre_rhi_vulkan::HEADLESS_FORMAT,
        )
        .expect("failed to create textured pipeline");

    let mut pipelines = PipelineRegistry::new();
    pipelines.register(PipelineKind::SdfRoundedRect as u16, Box::new(rect_pipeline));
    pipelines.register(
        PipelineKind::TexturedQuad as u16,
        Box::new(textured_pipeline),
    );

    let white = rgba8(255, 255, 255, 255);
    let full_window = ScissorRect {
        x: 0,
        y: 0,
        width: SWAPCHAIN_WIDTH,
        height: SWAPCHAIN_HEIGHT,
    };

    let run_frame = |canvas: RenderingCanvas| {
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
    };

    // --- Frame 1: the first, larger layer -- a fresh pool allocation,
    // released back to the pool once this frame's own PopLayer runs. ---
    let mut first_canvas = RenderingCanvas::new();
    first_canvas.push_layer(&LayerDesc {
        x: FIRST_LAYER_ORIGIN.0,
        y: FIRST_LAYER_ORIGIN.1,
        width: FIRST_LAYER_SIZE.0,
        height: FIRST_LAYER_SIZE.1,
        format: TextureFormat::Rgba16Float,
    });
    first_canvas.draw_rounded_rect(
        5.0,
        5.0,
        (FIRST_LAYER_SIZE.0 - 10) as f32,
        (FIRST_LAYER_SIZE.1 - 10) as f32,
        4.0,
        white,
    );
    first_canvas.pop_layer();
    run_frame(first_canvas);
    eprintln!(
        "frame 1 ({}x{} layer, fresh alloc) submitted",
        FIRST_LAYER_SIZE.0, FIRST_LAYER_SIZE.1
    );

    // --- Frame 2: the second, smaller, never-before-requested layer --
    // the first layer's own freed 200x150 texture should get handed
    // back by acquire_transient_target's oversized-borrow fallback. ---
    let mut second_canvas = RenderingCanvas::new();
    second_canvas.push_layer(&LayerDesc {
        x: SECOND_LAYER_ORIGIN.0,
        y: SECOND_LAYER_ORIGIN.1,
        width: SECOND_LAYER_SIZE.0,
        height: SECOND_LAYER_SIZE.1,
        format: TextureFormat::Rgba16Float,
    });
    // Nearly fills the layer's own local bounds.
    second_canvas.draw_rounded_rect(
        2.0,
        2.0,
        (SECOND_LAYER_SIZE.0 - 4) as f32,
        (SECOND_LAYER_SIZE.1 - 4) as f32,
        2.0,
        white,
    );
    second_canvas.pop_layer();
    run_frame(second_canvas);
    eprintln!(
        "frame 2 ({}x{} layer, expected to borrow the freed {}x{} texture) submitted",
        SECOND_LAYER_SIZE.0, SECOND_LAYER_SIZE.1, FIRST_LAYER_SIZE.0, FIRST_LAYER_SIZE.1
    );

    let bgra = swapchain
        .read_pixels_bgra8()
        .expect("failed to read back pixels");
    let pixel_at =
        |x: u32, y: u32| -> [u8; 4] { pixel_helpers::bgra_pixel_at(&bgra, SWAPCHAIN_WIDTH, x, y) };

    let background = pixel_at(280, 5);
    eprintln!("background (clear color): {background:?}");

    // The second layer's own deep interior, in swapchain space.
    let second_layer_center = (
        (SECOND_LAYER_ORIGIN.0 as u32) + SECOND_LAYER_SIZE.0 / 2,
        (SECOND_LAYER_ORIGIN.1 as u32) + SECOND_LAYER_SIZE.1 / 2,
    );
    let interior = pixel_at(second_layer_center.0, second_layer_center.1);
    assert!(
        interior[0] > 200 && interior[1] > 200 && interior[2] > 200,
        "the second, smaller layer's own content must composite correctly even though it \
         borrows the first, larger layer's freed texture -- got {interior:?} at its own center, \
         expected real foreground (this is REVIEW.md finding #152's own exact regression: \
         missing/wrong content when acquire_transient_target's oversized-borrow fallback fires)"
    );
    eprintln!("second layer's own composited center: OK ({interior:?})");

    // A point just outside the second layer's own bounds entirely --
    // real background must show through, proving the layer's own
    // composite quad stayed confined to its own requested on-screen size
    // and didn't bleed into the oversized backing texture's own unused
    // area some other way.
    let just_outside = pixel_at(
        (SECOND_LAYER_ORIGIN.0 as u32) + SECOND_LAYER_SIZE.0 + 10,
        second_layer_center.1,
    );
    assert_eq!(
        just_outside, background,
        "just outside the second layer's own composited bounds must show real background, \
         got {just_outside:?}"
    );
    eprintln!("outside second layer's own bounds: OK (real background: {just_outside:?})");

    let mut rgba = bgra.clone();
    for px in rgba.chunks_exact_mut(4) {
        px.swap(0, 2);
    }
    let out_path = std::env::var("TRE_LAYER_OVERSIZE_REGRESSION_OUTPUT")
        .unwrap_or_else(|_| "layer_oversize_regression_output.png".to_string());
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
        "wrote {SWAPCHAIN_WIDTH}x{SWAPCHAIN_HEIGHT} layer oversize regression render to {out_path}"
    );
    eprintln!("all layer oversize regression assertions passed");
}
