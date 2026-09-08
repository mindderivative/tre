//! Phase 7 Step 7.2.1 -- **STOPPED, not working yet.** REVIEW.md finding
//! #130 has the full account: sampling a bindless texture while the
//! active render target is an offscreen texture (not the swapchain)
//! reads back all-zero data on real hardware, with no validation error.
//! Twelve independent hypotheses were tested and ruled out (see that
//! finding); the real root cause was not found. This file is kept as a
//! real, precise reproduction case for future debugging with a real GPU
//! frame-capture tool (e.g. RenderDoc) -- running it will panic on its
//! own real pixel assertions, which is expected and documented, not a
//! regression. Deliberately not wired into `ci.yml`.
//!
//! Intended design (the downsample/upsample math itself, TECHNICAL.md
//! Section 5.5, is real and correct regardless of this blocker): drive
//! the real Dual-Kawase downsample/upsample blur capability entirely via
//! hand-written RHI calls -- no `Canvas`/IR involvement, matching Step
//! 6.4.1's own precedent (`push_layer`/`pop_layer` stay untouched; wiring
//! this capability to them is Step 7.2.2's own job, blocked on this).
//!
//! Real investigation before writing any code found this needs **no new
//! `RhiDevice`/`RhiCommandBuffer` trait methods** for the chain mechanism
//! itself -- `begin_render_to_texture`'s own existing implementation
//! already ends whatever rendering scope is active before beginning the
//! next. One new method, `begin_render_to_texture_no_end`, was added
//! along the way to fix a real, separate, already-resolved bug (see its
//! own doc comment) -- unrelated to the still-open blocker above.
//!
//! 1. Draw a small white square into a full-size transient target (L0),
//!    via the existing, unmodified `sdf_rounded_rect` pipeline.
//! 2. Downsample L0 -> L1 (half size) -> L2 (quarter size), each pass
//!    the real 5-tap `kawase_downsample` filter.
//! 3. Upsample L2 -> U1 (half size) -> U0 (full size), each pass the
//!    real 8-tap `kawase_upsample` filter.
//! 4. Composite U0 back onto the swapchain via the existing, unmodified
//!    bindless-textured pipeline -- the same role Step 6.4.1's own
//!    composite step already played.
//!
//! `create_pipeline`'s shared push-constant layout has no room for a
//! dedicated `half_pixel` field, so each Kawase shader derives it from
//! `screen_size` (its own destination's real dimensions) alone --
//! correct only because this demo always sizes an adjacent level at
//! exactly 2x/0.5x, which it does by construction (see each shader's own
//! comment for the derivation).

use ash::vk;
use tre_engine::{rgba8, RenderingCanvas, RhiDevice, TextureFormat, UiVertex};
use tre_rhi_vulkan::{HeadlessSwapchain, VulkanDevice};

#[path = "support/pixel_helpers.rs"]
mod pixel_helpers;

const SWAPCHAIN_WIDTH: u32 = 256;
const SWAPCHAIN_HEIGHT: u32 = 128;

// Three distinct real sizes in the chain: full, half, quarter -- each
// level after L0 exactly half its predecessor, matching what every
// Kawase shader's own `half_pixel` derivation requires by construction.
const SIZE_FULL: (u32, u32) = (SWAPCHAIN_WIDTH, SWAPCHAIN_HEIGHT);
const SIZE_HALF: (u32, u32) = (SWAPCHAIN_WIDTH / 2, SWAPCHAIN_HEIGHT / 2);
const SIZE_QUARTER: (u32, u32) = (SWAPCHAIN_WIDTH / 4, SWAPCHAIN_HEIGHT / 4);

// A small square, well clear of every edge, so the blur's own spread has
// real background on all sides to bleed into.
const SQUARE_SIZE: f32 = 20.0;
const SQUARE_X: f32 = (SWAPCHAIN_WIDTH as f32 - SQUARE_SIZE) / 2.0;
const SQUARE_Y: f32 = (SWAPCHAIN_HEIGHT as f32 - SQUARE_SIZE) / 2.0;

/// A plain textured quad covering `(x, y)`-`(x + width, y + height)`,
/// sampling its bound texture across its full `(0,0)`-`(1,1)` extent --
/// the same shape `render_to_texture_demo.rs`'s own `textured_quad`
/// already established (Step 6.4.1), reused unchanged here for every
/// downsample/upsample/composite pass's own full-target quad.
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
        .create_window("tre dual-kawase blur probe (never shown)", 1, 1)
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

    // --- Four pipelines: the square (into L0), the downsample/upsample
    // filters (every intermediate level, all Rgba16Float), and the final
    // composite (onto the swapchain's own HEADLESS_FORMAT). ---
    let out_dir = env!("OUT_DIR");
    let read_spv = |name: &str| {
        std::fs::read(format!("{out_dir}/{name}.spv"))
            .unwrap_or_else(|e| panic!("failed to read compiled shader {name}: {e}"))
    };

    let rect_pipeline = device
        .create_pipeline(
            &read_spv("sdf_rounded_rect.vert"),
            &read_spv("sdf_rounded_rect.frag"),
            vk::Format::R16G16B16A16_SFLOAT,
        )
        .expect("failed to create rect pipeline");
    let downsample_pipeline = device
        .create_pipeline(
            &read_spv("bindless_textured.vert"),
            &read_spv("kawase_downsample.frag"),
            vk::Format::R16G16B16A16_SFLOAT,
        )
        .expect("failed to create downsample pipeline");
    let upsample_pipeline = device
        .create_pipeline(
            &read_spv("bindless_textured.vert"),
            &read_spv("kawase_upsample.frag"),
            vk::Format::R16G16B16A16_SFLOAT,
        )
        .expect("failed to create upsample pipeline");
    let composite_pipeline = device
        .create_pipeline(
            &read_spv("bindless_textured.vert"),
            &read_spv("bindless_textured.frag"),
            tre_rhi_vulkan::HEADLESS_FORMAT,
        )
        .expect("failed to create composite pipeline");

    // --- The square's own geometry, via RenderingCanvas purely as a
    // convenient vertex-data builder -- the same non-Canvas-flow use
    // `render_to_texture_demo.rs` already established. ---
    let white = rgba8(255, 255, 255, 255);
    let mut square_canvas = RenderingCanvas::new();
    square_canvas.draw_rounded_rect(SQUARE_X, SQUARE_Y, SQUARE_SIZE, SQUARE_SIZE, 0.0, white);
    let square_frame = square_canvas.flatten();
    let square_vertex_buffer = device
        .upload_buffer(
            bytemuck::cast_slice(&square_frame.vertices),
            vk::BufferUsageFlags::VERTEX_BUFFER,
        )
        .expect("failed to upload square vertex buffer");
    let square_index_buffer = device
        .upload_buffer(
            bytemuck::cast_slice(&square_frame.indices),
            vk::BufferUsageFlags::INDEX_BUFFER,
        )
        .expect("failed to upload square index buffer");

    // --- One full-target quad buffer per distinct real size in the
    // chain -- reused across every pass that renders into a target of
    // that same size, regardless of which pipeline/texture is active. ---
    let upload_quad = |width: u32, height: u32| {
        let quad = textured_quad(0.0, 0.0, width as f32, height as f32);
        let indices: [u32; 6] = [0, 1, 2, 2, 3, 0];
        let vertex_buffer = device
            .upload_buffer(
                bytemuck::cast_slice(&quad),
                vk::BufferUsageFlags::VERTEX_BUFFER,
            )
            .expect("failed to upload quad vertex buffer");
        let index_buffer = device
            .upload_buffer(
                bytemuck::cast_slice(&indices),
                vk::BufferUsageFlags::INDEX_BUFFER,
            )
            .expect("failed to upload quad index buffer");
        (vertex_buffer, index_buffer)
    };
    let (full_quad_vb, full_quad_ib) = upload_quad(SIZE_FULL.0, SIZE_FULL.1);
    let (half_quad_vb, half_quad_ib) = upload_quad(SIZE_HALF.0, SIZE_HALF.1);
    let (quarter_quad_vb, quarter_quad_ib) = upload_quad(SIZE_QUARTER.0, SIZE_QUARTER.1);

    // --- The real chain. ---
    let (mut cmd_buffer, image) = device.begin_frame(&swapchain).expect("begin_frame failed");

    // L0: the square, rendered into a full-size transient target.
    let l0 = device
        .acquire_transient_target(SIZE_FULL.0, SIZE_FULL.1, TextureFormat::Rgba16Float)
        .expect("failed to acquire L0");
    cmd_buffer.begin_render_to_texture(&*l0);
    cmd_buffer.set_pipeline(&rect_pipeline);
    cmd_buffer.bind_vertex_buffer(&square_vertex_buffer, 0);
    cmd_buffer.bind_index_buffer(&square_index_buffer, 0);
    cmd_buffer.draw_indexed(square_frame.indices.len() as u32, 0, 0);

    // L0 -> L1: downsample to half size. Every subsequent transition
    // pairs a plain `end_render_to_texture(previous)` + `RhiDevice::
    // register_bindless(previous)` (both with no render pass active) with
    // `begin_render_to_texture_no_end(next)` -- NOTE: as of this commit,
    // this real ordering was tested exhaustively and STILL produces
    // all-zero sampled output; see REVIEW.md finding #130 and this step's
    // own IMPLEMENTATION.md write-up for the full, honest account of the
    // investigation and its current STOPPED status.
    cmd_buffer.end_render_to_texture(&*l0);
    let l0_index = device
        .register_bindless(&*l0)
        .expect("failed to register L0 bindless");
    let l1 = device
        .acquire_transient_target(SIZE_HALF.0, SIZE_HALF.1, TextureFormat::Rgba16Float)
        .expect("failed to acquire L1");
    cmd_buffer.begin_render_to_texture_no_end(&*l1);
    cmd_buffer.set_pipeline(&downsample_pipeline);
    cmd_buffer.bind_texture(0, l0_index);
    cmd_buffer.bind_vertex_buffer(&half_quad_vb, 0);
    cmd_buffer.bind_index_buffer(&half_quad_ib, 0);
    cmd_buffer.draw_indexed(6, 0, 0);
    device.deregister_bindless(l0_index);
    device.release_transient_target(l0);

    // L1 -> L2: downsample to quarter size.
    cmd_buffer.end_render_to_texture(&*l1);
    let l1_index = device
        .register_bindless(&*l1)
        .expect("failed to register L1 bindless");
    let l2 = device
        .acquire_transient_target(SIZE_QUARTER.0, SIZE_QUARTER.1, TextureFormat::Rgba16Float)
        .expect("failed to acquire L2");
    cmd_buffer.begin_render_to_texture_no_end(&*l2);
    cmd_buffer.set_pipeline(&downsample_pipeline);
    cmd_buffer.bind_texture(0, l1_index);
    cmd_buffer.bind_vertex_buffer(&quarter_quad_vb, 0);
    cmd_buffer.bind_index_buffer(&quarter_quad_ib, 0);
    cmd_buffer.draw_indexed(6, 0, 0);
    device.deregister_bindless(l1_index);
    device.release_transient_target(l1);

    // L2 -> U1: upsample back to half size.
    cmd_buffer.end_render_to_texture(&*l2);
    let l2_index = device
        .register_bindless(&*l2)
        .expect("failed to register L2 bindless");
    let u1 = device
        .acquire_transient_target(SIZE_HALF.0, SIZE_HALF.1, TextureFormat::Rgba16Float)
        .expect("failed to acquire U1");
    cmd_buffer.begin_render_to_texture_no_end(&*u1);
    cmd_buffer.set_pipeline(&upsample_pipeline);
    cmd_buffer.bind_texture(0, l2_index);
    cmd_buffer.bind_vertex_buffer(&half_quad_vb, 0);
    cmd_buffer.bind_index_buffer(&half_quad_ib, 0);
    cmd_buffer.draw_indexed(6, 0, 0);
    device.deregister_bindless(l2_index);
    device.release_transient_target(l2);

    // U1 -> U0: upsample back to full size.
    cmd_buffer.end_render_to_texture(&*u1);
    let u1_index = device
        .register_bindless(&*u1)
        .expect("failed to register U1 bindless");
    let u0 = device
        .acquire_transient_target(SIZE_FULL.0, SIZE_FULL.1, TextureFormat::Rgba16Float)
        .expect("failed to acquire U0");
    cmd_buffer.begin_render_to_texture_no_end(&*u0);
    cmd_buffer.set_pipeline(&upsample_pipeline);
    cmd_buffer.bind_texture(0, u1_index);
    cmd_buffer.bind_vertex_buffer(&full_quad_vb, 0);
    cmd_buffer.bind_index_buffer(&full_quad_ib, 0);
    cmd_buffer.draw_indexed(6, 0, 0);
    device.deregister_bindless(u1_index);
    device.release_transient_target(u1);

    // U0 is the chain's final texture -- what follows is `resume_
    // swapchain_rendering`, which never calls `cmd_end_rendering` itself
    // (Step 6.4.1's own original single-level pairing, unchanged).
    cmd_buffer.end_render_to_texture(&*u0);
    let u0_index = device
        .register_bindless(&*u0)
        .expect("failed to register U0 bindless");

    // Composite the final blurred result back onto the swapchain.
    cmd_buffer.resume_swapchain_rendering();
    cmd_buffer.set_pipeline(&composite_pipeline);
    cmd_buffer.bind_texture(0, u0_index);
    cmd_buffer.bind_vertex_buffer(&full_quad_vb, 0);
    cmd_buffer.bind_index_buffer(&full_quad_ib, 0);
    cmd_buffer.draw_indexed(6, 0, 0);

    device
        .submit_and_present(cmd_buffer, &swapchain, image)
        .expect("submit_and_present failed");

    device.deregister_bindless(u0_index);
    device.release_transient_target(u0);

    let bgra = swapchain
        .read_pixels_bgra8()
        .expect("failed to read back pixels");
    let pixel_at =
        |x: u32, y: u32| -> [u8; 4] { pixel_helpers::bgra_pixel_at(&bgra, SWAPCHAIN_WIDTH, x, y) };

    let background = pixel_at(5, 5);
    eprintln!("background (clear color): {background:?}");

    // Deep in the square's own center: coverage is 1.0 pre-blur, and the
    // blur at this depth must not wash it out to uniform gray.
    let center = (
        (SQUARE_X + SQUARE_SIZE / 2.0) as u32,
        (SQUARE_Y + SQUARE_SIZE / 2.0) as u32,
    );
    let interior = pixel_at(center.0, center.1);
    assert!(
        interior[0] > 150 && interior[1] > 150 && interior[2] > 150,
        "the square's own deep interior must stay mostly foreground after blur, not be washed \
         to uniform gray -- got {interior:?}"
    );
    eprintln!("bounded interior: OK (still mostly foreground, {interior:?})");

    // Just outside the square's own original hard left edge -- pure
    // background before any blur -- must now show a genuine partial
    // blend, proving real blur spread outward.
    let edge_x = (SQUARE_X - 6.0) as u32;
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
    let out_path = std::env::var("TRE_DUAL_KAWASE_BLUR_OUTPUT")
        .unwrap_or_else(|_| "dual_kawase_blur_output.png".to_string());
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

    eprintln!("wrote {SWAPCHAIN_WIDTH}x{SWAPCHAIN_HEIGHT} dual-kawase blur render to {out_path}");
    eprintln!("all dual-kawase blur assertions passed");
}
