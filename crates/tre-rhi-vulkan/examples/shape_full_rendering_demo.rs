//! Phase 10 Step 10.2 proof: a real GPU render exercising the two new
//! shape-rendering capabilities this step adds -- non-uniform corner
//! radii + a real border on `Rectangle`, and a bordered, partial-arc
//! `Circle` -- both recorded through `ShapeRegistry::flatten_into`
//! (never hand-written RHI calls), driven end to end via the generic
//! `execute_frame`/`PipelineRegistry` path `canvas_combined_scene_demo.
//! rs` already established for a multi-pipeline scene.
//!
//! Real correctness, not just "didn't crash": pixel samples prove (1) a
//! sharp (radius 0) corner stays sharp while the opposite, rounded
//! corner is genuinely excised by its own real radius -- a render using
//! only ONE shared radius could not pass both checks at once; (2) the
//! border band renders in `border_color`, the interior in `fill_color`;
//! (3) a circle's `arc_length` sweep genuinely excludes the wedge beyond
//! it (sampled in the excluded northwest quadrant) while still filling
//! the swept region.

use ash::vk;
use tre_engine::{
    execute_frame, rgba8, submit_frame, BufferBinding, Circle, CornerRadii, FillStyle,
    PipelineKind, PipelineRegistry, PrimitiveCommon, Rectangle, RenderingCanvas, ScissorRect,
    ShapePrimitive, ShapeRegistry,
};
use tre_rhi_vulkan::{HeadlessSwapchain, VulkanDevice};

#[path = "support/pixel_helpers.rs"]
mod pixel_helpers;

const SWAPCHAIN_WIDTH: u32 = 320;
const SWAPCHAIN_HEIGHT: u32 = 180;

// --- The styled rectangle: a sharp top-left/bottom-right corner pair, a
// rounded top-right/bottom-left pair, plus a real border. ---
const RECT_POSITION: [f32; 2] = [20.0, 20.0];
const RECT_SIZE: [f32; 2] = [150.0, 100.0];
const RECT_ROUNDED_RADIUS: f32 = 40.0;
const RECT_BORDER_THICKNESS: f32 = 8.0;

// --- The bordered, 270-degree (three-quarter) circle. Sweeping
// clockwise from 12 o'clock, a 270-degree arc covers 12 -> 3 -> 6 -> 9
// o'clock, excluding the northwest (9 -> 12 o'clock) wedge. ---
const CIRCLE_CENTER: [f32; 2] = [240.0, 100.0];
const CIRCLE_RADIUS: f32 = 50.0;
const CIRCLE_BORDER_THICKNESS: f32 = 6.0;
const CIRCLE_ARC_LENGTH_DEGREES: f32 = 270.0;

fn main() {
    let mut probe_connection =
        tre_platform::PlatformConnection::new().expect("failed to connect to display server");
    let probe_window = probe_connection
        .create_window("tre shape full rendering probe (never shown)", 1, 1)
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

    // --- Two new pipelines, both paired with the existing
    // sdf_rounded_rect.vert (see each shader's own doc comment). ---
    let out_dir = env!("OUT_DIR");
    let vertex_spv = std::fs::read(format!("{out_dir}/sdf_rounded_rect.vert.spv"))
        .expect("failed to read compiled vertex shader");
    let rect_fragment_spv = std::fs::read(format!("{out_dir}/sdf_rect_styled.frag.spv"))
        .expect("failed to read compiled styled-rect fragment shader");
    let rect_pipeline = device
        .create_pipeline(
            &vertex_spv,
            &rect_fragment_spv,
            tre_rhi_vulkan::HEADLESS_FORMAT,
        )
        .expect("failed to create styled-rect pipeline");
    let ellipse_fragment_spv = std::fs::read(format!("{out_dir}/sdf_ellipse.frag.spv"))
        .expect("failed to read compiled ellipse fragment shader");
    let ellipse_pipeline = device
        .create_pipeline(
            &vertex_spv,
            &ellipse_fragment_spv,
            tre_rhi_vulkan::HEADLESS_FORMAT,
        )
        .expect("failed to create ellipse pipeline");

    let mut pipelines = PipelineRegistry::new();
    pipelines.register(PipelineKind::SdfRectStyled as u16, Box::new(rect_pipeline));
    pipelines.register(PipelineKind::SdfEllipse as u16, Box::new(ellipse_pipeline));

    // --- The real retained-mode scene, described once and flattened
    // through ShapeRegistry -- no hand-written RHI calls. ---
    let white = rgba8(255, 255, 255, 255);
    let red = rgba8(255, 0, 0, 255);
    let blue = rgba8(0, 0, 255, 255);
    let yellow = rgba8(255, 255, 0, 255);

    let mut registry = ShapeRegistry::new();

    let mut rect = Rectangle::new(RECT_SIZE, white);
    rect.common.transform.position = RECT_POSITION;
    rect.corner_radius = CornerRadii {
        top_left: 0.0,
        top_right: RECT_ROUNDED_RADIUS,
        bottom_right: 0.0,
        bottom_left: RECT_ROUNDED_RADIUS,
    };
    rect.fill = FillStyle::Solid(white);
    rect.border_color = red;
    rect.border_thickness = RECT_BORDER_THICKNESS;
    registry.insert(ShapePrimitive::Rectangle(rect));

    registry.insert(ShapePrimitive::Circle(Circle {
        common: {
            let mut common = PrimitiveCommon::new();
            common.transform.position = [
                CIRCLE_CENTER[0] - CIRCLE_RADIUS,
                CIRCLE_CENTER[1] - CIRCLE_RADIUS,
            ];
            common
        },
        radius: [CIRCLE_RADIUS, CIRCLE_RADIUS],
        fill: FillStyle::Solid(blue),
        border_color: yellow,
        border_thickness: CIRCLE_BORDER_THICKNESS,
        arc_length: CIRCLE_ARC_LENGTH_DEGREES,
    }));

    let mut canvas = RenderingCanvas::new();
    registry.flatten_into(&mut canvas, &device, None);
    let frame = canvas.flatten();
    assert_eq!(
        frame.commands.len(),
        2,
        "one rectangle + one circle command"
    );

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

    let full_window = ScissorRect {
        x: 0,
        y: 0,
        width: SWAPCHAIN_WIDTH,
        height: SWAPCHAIN_HEIGHT,
    };
    submit_frame(&device, &swapchain, |cmd_buffer| {
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
            cmd_buffer,
        );
    })
    .expect("submit_frame failed");

    let bgra = swapchain
        .read_pixels_bgra8()
        .expect("failed to read back pixels");
    let pixel_at =
        |x: u32, y: u32| -> [u8; 4] { pixel_helpers::bgra_pixel_at(&bgra, SWAPCHAIN_WIDTH, x, y) };

    let background = pixel_at(0, 0);
    eprintln!("background: {background:?}");

    // --- Rectangle: sharp corner stays sharp. ---
    // 2px inside the top-left corner (radius 0.0) -- must be border
    // (red), not background: a single shared radius could not make this
    // corner sharp while also rounding the opposite one below.
    let sharp_corner_interior = pixel_at(22, 22);
    assert_eq!(
        sharp_corner_interior,
        [255, 0, 0, 255],
        "the sharp (radius 0.0) top-left corner must render border color right up to the \
         corner, got {sharp_corner_interior:?}"
    );
    eprintln!("rectangle sharp top-left corner: border color right to the edge -- OK");

    // --- Rectangle: the OPPOSITE corner's own real 40px radius excises
    // a point that sits inside the raw bounding box but outside the
    // rounded arc. ---
    let rounded_corner_excised = pixel_at(168, 22);
    assert_eq!(
        rounded_corner_excised, background,
        "a point near the rounded (radius 40.0) top-right corner, outside that corner's own \
         arc, must be background -- got {rounded_corner_excised:?} (this would only pass by \
         accident if every corner shared one radius)"
    );
    eprintln!("rectangle rounded top-right corner: excised outside its own 40px arc -- OK");

    // --- Rectangle: border vs. fill. ---
    let rect_border = pixel_at(95, 22); // top edge, clear of any corner
    assert_eq!(
        rect_border,
        [255, 0, 0, 255],
        "the rectangle's border band must render border_color, got {rect_border:?}"
    );
    let rect_fill = pixel_at(95, 70); // interior center
    assert_eq!(
        rect_fill,
        [255, 255, 255, 255],
        "the rectangle's interior must render fill_color, got {rect_fill:?}"
    );
    eprintln!("rectangle border/fill: border_color at the edge, fill_color at the center -- OK");

    // --- Circle: the swept region fills; the excluded wedge does not. ---
    let circle_fill = pixel_at(240, 100); // dead center
    assert_eq!(
        circle_fill,
        [0, 0, 255, 255],
        "the circle's center must render fill_color, got {circle_fill:?}"
    );
    let circle_border = pixel_at(287, 100); // due east, inside the 6px border band
    assert_eq!(
        circle_border,
        [255, 255, 0, 255],
        "the circle's border band must render border_color, got {circle_border:?}"
    );
    let circle_excluded_wedge = pixel_at(215, 75); // northwest, outside the 270-degree sweep
    assert_eq!(
        circle_excluded_wedge, background,
        "a point in the circle's own excluded (northwest) wedge must be background -- got \
         {circle_excluded_wedge:?}"
    );
    eprintln!(
        "circle: swept region filled (center, border) and excluded wedge left background -- OK"
    );

    let mut rgba_out = bgra.clone();
    for px in rgba_out.chunks_exact_mut(4) {
        px.swap(0, 2);
    }
    let out_path = std::env::var("TRE_SHAPE_FULL_RENDERING_OUTPUT")
        .unwrap_or_else(|_| "shape_full_rendering_output.png".to_string());
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
        .write_image_data(&rgba_out)
        .expect("failed to write PNG image data");
    eprintln!(
        "wrote {SWAPCHAIN_WIDTH}x{SWAPCHAIN_HEIGHT} shape full rendering render to {out_path}"
    );

    eprintln!("shape_full_rendering_demo: all checks passed");
}
