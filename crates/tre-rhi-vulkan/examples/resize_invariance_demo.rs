//! Real, GPU-verified proof of a real user-stated requirement: resizing
//! a window must never squash or stretch already-placed content -- an
//! absolutely-positioned shape's own pixel position and size must stay
//! fixed, with a resize only changing how much canvas is revealed around
//! it. Renders the identical `Rectangle` (same `common.transform.
//! position`/size, untouched) into two `HeadlessSwapchain`s of different
//! widths and directly compares pixel samples at the same absolute
//! coordinates in both, rather than reasoning about the vertex shader's
//! `ndc = (in_position / screen_size) * 2.0 - 1.0` transform from code
//! alone (`sdf_rounded_rect.vert`) -- this project's own "verify
//! empirically" standard applies here too. `screen_size` is re-derived
//! from the swapchain's own current extent every frame
//! (`VulkanDevice::begin_frame`'s `let (width, height) =
//! swapchain.extent();`), which is what keeps a shape's own absolute
//! pixel coordinates mapped 1:1 onto the real framebuffer regardless of
//! its current size -- confirmed by pixel readback here, not assumed.

use ash::vk;
use tre_engine::{
    rgba8, submit_frame, CornerRadii, FillStyle, Rectangle, RenderingCanvas, ShapePrimitive,
    ShapeRegistry,
};
use tre_rhi_vulkan::{HeadlessSwapchain, VulkanDevice};

#[path = "support/pixel_helpers.rs"]
mod pixel_helpers;
use pixel_helpers::bgra_pixel_at;

const HEIGHT: u32 = 200;
const RECT_X: u32 = 60;
const RECT_Y: u32 = 40;
const RECT_W: u32 = 80;
const RECT_H: u32 = 50;

fn render_at(
    device: &VulkanDevice,
    pipelines: &tre_engine::PipelineRegistry,
    width: u32,
) -> Vec<u8> {
    let swapchain =
        HeadlessSwapchain::new(device, width, HEIGHT).expect("failed to create HeadlessSwapchain");

    let mut registry = ShapeRegistry::new();
    let mut rect = Rectangle::new([RECT_W as f32, RECT_H as f32], rgba8(255, 0, 0, 255));
    rect.common.transform.position = [RECT_X as f32, RECT_Y as f32];
    rect.fill = FillStyle::Solid(rgba8(255, 0, 0, 255));
    rect.corner_radius = CornerRadii::uniform(0.0);
    registry.insert(ShapePrimitive::Rectangle(rect));

    let mut canvas = RenderingCanvas::new();
    registry.flatten_into(&mut canvas, device, None);
    let frame = canvas.flatten();

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
    let full_window = tre_engine::ScissorRect {
        x: 0,
        y: 0,
        width,
        height: HEIGHT,
    };
    let mut clip_stack = Vec::new();
    submit_frame(device, &swapchain, |cmd_buffer| {
        tre_engine::execute_frame(
            &frame,
            pipelines,
            tre_engine::BufferBinding {
                buffer: &vertex_buffer,
                offset: 0,
            },
            tre_engine::BufferBinding {
                buffer: &index_buffer,
                offset: 0,
            },
            &full_window,
            device,
            cmd_buffer,
            &mut clip_stack,
        );
    })
    .expect("submit_frame failed");

    swapchain
        .read_pixels_bgra8()
        .expect("failed to read back pixels")
}

fn main() {
    let mut probe_connection =
        tre_platform::PlatformConnection::new().expect("failed to connect to display server");
    let probe_window = probe_connection
        .create_window("resize squash/stretch probe (never shown)", 1, 1)
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

    let mut pipelines = tre_engine::PipelineRegistry::new();
    tre_rhi_vulkan::register_shape_pipelines(
        &device,
        &mut pipelines,
        tre_rhi_vulkan::HEADLESS_FORMAT,
    )
    .expect("failed to register shape pipelines");

    let narrow = render_at(&device, &pipelines, 300);
    let wide = render_at(&device, &pipelines, 600);

    let check = |label: &str, buf: &[u8], w: u32, x: u32, y: u32, expect_red: bool| {
        let px = bgra_pixel_at(buf, w, x, y);
        let is_red = px[0] > 200 && px[1] < 60 && px[2] < 60;
        let status = if is_red == expect_red { "OK" } else { "FAIL" };
        println!(
            "{label} [{status}] ({x},{y}) in {w}px-wide image: {px:?} (expected red={expect_red})"
        );
        assert_eq!(is_red, expect_red, "{label} at ({x},{y}) in {w}px image");
    };

    println!("=== narrow (300px wide) ===");
    check("inside rect", &narrow, 300, 100, 65, true);
    check("left of rect", &narrow, 300, 30, 65, false);
    check(
        "right of rect (within 300px canvas)",
        &narrow,
        300,
        250,
        65,
        false,
    );

    println!("=== wide (600px wide, same shape, same absolute coordinates) ===");
    check(
        "inside rect (must be identical position/size)",
        &wide,
        600,
        100,
        65,
        true,
    );
    check("left of rect", &wide, 600, 30, 65, false);
    check(
        "far right -- only exists in the wider canvas, must be background, not stretched red",
        &wide,
        600,
        500,
        65,
        false,
    );
    check(
        "just past the rect's real right edge (60+80=140) -- must NOT be red in either size",
        &wide,
        600,
        160,
        65,
        false,
    );

    println!(
        "\nCONCLUSION: the rectangle occupies the identical absolute pixel region in both \
         framebuffer sizes -- resizing reveals more canvas, it does not squash/stretch \
         existing content."
    );
}
