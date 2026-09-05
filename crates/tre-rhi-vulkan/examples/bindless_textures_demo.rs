//! Phase 2 Step 2.1 proof: real Vulkan bindless texture arrays
//! (IMPLEMENTATION.md Step 2.1, `VK_EXT_descriptor_indexing`). Uploads
//! three distinct real textures via `RhiDevice::create_texture` and draws
//! each with its own draw call through the SAME bound pipeline and SAME
//! bound descriptor set -- varying only the push-constant bindless index
//! between draws, with zero descriptor-set rebinding. A fourth draw binds
//! no texture at all, proving the "no texture" sentinel still falls back
//! to Phase 0's flat vertex-color path unchanged.
//!
//! Verified by inspecting the actual output pixel colors from a headless
//! readback (like the `headless` example), not just that the draw calls
//! didn't crash.

use ash::vk;
use tre_engine::{rgba8, RhiDevice, TextureFormat, UiVertex};
use tre_rhi_vulkan::{HeadlessSwapchain, VulkanDevice};

/// A `width` x `height` solid-color pixel buffer, tightly packed to match
/// `TextureFormat::Bgra8Srgb`'s in-memory `B, G, R, A` byte order (the same
/// order `HEADLESS_FORMAT` uses, so uploading and reading back agree).
fn solid_bgra8(width: u32, height: u32, r: u8, g: u8, b: u8, a: u8) -> Vec<u8> {
    let mut pixels = Vec::with_capacity((width * height) as usize * 4);
    for _ in 0..(width * height) {
        pixels.extend_from_slice(&[b, g, r, a]);
    }
    pixels
}

fn quad_vertices(x: f32, y: f32, size: f32, color: u32) -> [UiVertex; 4] {
    [
        UiVertex {
            position: [x, y],
            uv: [0.0, 0.0],
            color,
            params: [0.0; 3],
        },
        UiVertex {
            position: [x + size, y],
            uv: [1.0, 0.0],
            color,
            params: [0.0; 3],
        },
        UiVertex {
            position: [x + size, y + size],
            uv: [1.0, 1.0],
            color,
            params: [0.0; 3],
        },
        UiVertex {
            position: [x, y + size],
            uv: [0.0, 1.0],
            color,
            params: [0.0; 3],
        },
    ]
}

fn main() {
    // Same headless-probe-window bootstrap as the `headless` example --
    // `VulkanDevice::new` needs a transient surface to select a physical
    // device/queue family, destroyed immediately below since this demo
    // never presents to it.
    let mut probe_connection =
        tre_platform::PlatformConnection::new().expect("failed to connect to display server");
    let probe_window = probe_connection
        .create_window("tre bindless textures probe (never shown)", 1, 1)
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

    const QUAD_SIZE: u32 = 160;
    let width = QUAD_SIZE * 4;
    let height = QUAD_SIZE;
    let swapchain =
        HeadlessSwapchain::new(&device, width, height).expect("failed to create HeadlessSwapchain");

    let out_dir = env!("OUT_DIR");
    let vertex_spv = std::fs::read(format!("{out_dir}/bindless_textured.vert.spv"))
        .expect("failed to read compiled vertex shader");
    let fragment_spv = std::fs::read(format!("{out_dir}/bindless_textured.frag.spv"))
        .expect("failed to read compiled fragment shader");
    let pipeline = device
        .create_pipeline(&vertex_spv, &fragment_spv, tre_rhi_vulkan::HEADLESS_FORMAT)
        .expect("failed to create pipeline");

    // Three real, distinct textures uploaded through `create_texture` --
    // each a tiny 4x4 solid color. Pure 0/255 channel values are used
    // throughout this demo specifically so the SRGB encode/decode this
    // format applies round-trips exactly (0 and 255 are the two fixed
    // points of the sRGB transfer curve), keeping the pixel assertions
    // below exact rather than approximate.
    let red = device.create_texture(
        4,
        4,
        TextureFormat::Bgra8Srgb,
        &solid_bgra8(4, 4, 255, 0, 0, 255),
    );
    let green = device.create_texture(
        4,
        4,
        TextureFormat::Bgra8Srgb,
        &solid_bgra8(4, 4, 0, 255, 0, 255),
    );
    let blue = device.create_texture(
        4,
        4,
        TextureFormat::Bgra8Srgb,
        &solid_bgra8(4, 4, 0, 0, 255, 255),
    );

    let white = rgba8(255, 255, 255, 255);
    let yellow = rgba8(255, 255, 0, 255);
    let size = QUAD_SIZE as f32;
    let mut vertices = Vec::new();
    vertices.extend(quad_vertices(0.0, 0.0, size, white));
    vertices.extend(quad_vertices(size, 0.0, size, white));
    vertices.extend(quad_vertices(size * 2.0, 0.0, size, white));
    // Fourth quad: drawn with the bindless sentinel explicitly bound (see
    // below) -- proves the "no texture" fallback still renders the
    // vertex's own color, exactly like every pre-existing flat-color draw.
    vertices.extend(quad_vertices(size * 3.0, 0.0, size, yellow));
    let indices: [u32; 6] = [0, 1, 2, 2, 3, 0];

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

    // Same bound pipeline, same bound descriptor set (see
    // `VulkanCommandBuffer::set_pipeline`) for all four draws below --
    // only the push-constant texture index (or its absence) changes
    // between them. This is the property that makes it "bindless": no
    // `vkCmdBindDescriptorSets` call happens again after the one inside
    // `set_pipeline` above, no matter how many different textures get
    // drawn.
    cmd_buffer.bind_texture(
        0,
        red.bindless_index()
            .expect("red texture has no bindless index"),
    );
    cmd_buffer.draw_indexed(indices.len() as u32, 0, 0);

    cmd_buffer.bind_texture(
        0,
        green
            .bindless_index()
            .expect("green texture has no bindless index"),
    );
    cmd_buffer.draw_indexed(indices.len() as u32, 0, 4);

    cmd_buffer.bind_texture(
        0,
        blue.bindless_index()
            .expect("blue texture has no bindless index"),
    );
    cmd_buffer.draw_indexed(indices.len() as u32, 0, 8);

    // `bind_texture`'s bound index is command-buffer state that persists
    // across draws until explicitly changed (like every other piece of
    // Vulkan command-buffer state -- pipeline, vertex buffer, scissor), so
    // proving the "no texture" fallback means explicitly rebinding the
    // sentinel here, not simply skipping the call (which would just keep
    // sampling `blue` from the previous draw).
    cmd_buffer.bind_texture(0, u32::MAX);
    cmd_buffer.draw_indexed(indices.len() as u32, 0, 12);

    device
        .submit_and_present(cmd_buffer, &swapchain, image)
        .expect("submit_and_present failed");

    let bgra = swapchain
        .read_pixels_bgra8()
        .expect("failed to read back pixels");
    let pixel_at = |x: u32, y: u32| -> [u8; 4] {
        let idx = ((y * width + x) * 4) as usize;
        [bgra[idx], bgra[idx + 1], bgra[idx + 2], bgra[idx + 3]]
    };

    // Center of each 160x160 column -- independent of exact quad-edge
    // placement. Expected values are in the same B, G, R, A byte order
    // `read_pixels_bgra8` returns.
    let center_y = height / 2;
    let checks: [(&str, u32, [u8; 4]); 4] = [
        ("red (bindless texture 0)", QUAD_SIZE / 2, [0, 0, 255, 255]),
        (
            "green (bindless texture 1)",
            QUAD_SIZE + QUAD_SIZE / 2,
            [0, 255, 0, 255],
        ),
        (
            "blue (bindless texture 2)",
            QUAD_SIZE * 2 + QUAD_SIZE / 2,
            [255, 0, 0, 255],
        ),
        (
            "yellow (no texture bound, vertex color fallback)",
            QUAD_SIZE * 3 + QUAD_SIZE / 2,
            [0, 255, 255, 255],
        ),
    ];
    for (label, x, expected_bgra) in checks {
        let actual = pixel_at(x, center_y);
        assert_eq!(
            actual, expected_bgra,
            "{label}: expected BGRA {expected_bgra:?}, got {actual:?} at ({x}, {center_y})"
        );
        eprintln!("{label}: OK ({actual:?})");
    }

    // Same PNG-writing convention as the `headless` example, for a visual
    // artifact alongside the pixel assertions above.
    let mut rgba = bgra.clone();
    for px in rgba.chunks_exact_mut(4) {
        px.swap(0, 2);
    }
    let out_path = std::env::var("TRE_BINDLESS_TEXTURES_OUTPUT")
        .unwrap_or_else(|_| "bindless_textures_output.png".to_string());
    let file = std::fs::File::create(&out_path).expect("failed to create output PNG file");
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().expect("failed to write PNG header");
    writer
        .write_image_data(&rgba)
        .expect("failed to write PNG image data");

    eprintln!("wrote {width}x{height} bindless-textures render to {out_path}");
    eprintln!("all bindless texture assertions passed");
}
