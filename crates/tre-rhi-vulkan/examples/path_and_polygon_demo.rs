//! Phase 10 Step 10.2 follow-up proof: real `Path` fill (including a
//! genuine compound shape with a hole -- impossible for this project's
//! previous hand-rolled ear-clipper, which only ever handled a single
//! simple contour) and real `Path`/`Polygon` border/stroke rendering
//! (previously entirely unbuilt for either shape kind), both via
//! `lyon`'s real fill/stroke tessellators, recorded through
//! `ShapeRegistry::flatten_into` -- never a hand-written RHI call.
//!
//! Real correctness, not just "didn't crash": pixel samples prove (1) a
//! `Path` with two subpaths (an outer square, an inner "hole" square)
//! renders as a real ring -- filled between the two boundaries, but
//! genuinely NOT filled inside the hole; (2) a `Path`'s own stroke
//! renders in `border_color` along its boundary; (3) a `Polygon`
//! (hexagon)'s own stroke renders in its own `border_color` too.

use ash::vk;
use tre_engine::{
    execute_frame, rgba8, submit_frame, BufferBinding, FillStyle, LineCap, LineJoin, Path,
    PathCommand, PipelineKind, PipelineRegistry, Polygon, PrimitiveCommon, RenderingCanvas,
    ScissorRect, ShapePrimitive, ShapeRegistry,
};
use tre_rhi_vulkan::{HeadlessSwapchain, VulkanDevice};

#[path = "support/pixel_helpers.rs"]
mod pixel_helpers;

const SWAPCHAIN_WIDTH: u32 = 320;
const SWAPCHAIN_HEIGHT: u32 = 200;

// --- The donut: an outer 100x100 square with a 40x40 "hole" square cut
// out of its center, positioned at (20, 20). Real area-wise: this is
// exactly the compound-shape-with-a-hole case ear-clipping could never
// express -- one Path, two subpaths, wound oppositely so NonZero
// resolves the inner one as a real hole. ---
const DONUT_POSITION: [f32; 2] = [20.0, 20.0];
const DONUT_OUTER: f32 = 100.0;
const DONUT_HOLE: f32 = 40.0;
const DONUT_BORDER_THICKNESS: f32 = 6.0;

// --- The hexagon: a real Polygon with its own border/stroke. ---
const HEXAGON_CENTER: [f32; 2] = [220.0, 70.0];
const HEXAGON_RADIUS: f32 = 45.0;
const HEXAGON_BORDER_THICKNESS: f32 = 8.0;

fn square_path(top_left: [f32; 2], size: f32, clockwise: bool) -> Vec<PathCommand> {
    let [x, y] = top_left;
    let corners = if clockwise {
        [[x, y], [x + size, y], [x + size, y + size], [x, y + size]]
    } else {
        [[x, y], [x, y + size], [x + size, y + size], [x + size, y]]
    };
    vec![
        PathCommand::MoveTo(corners[0]),
        PathCommand::LineTo(corners[1]),
        PathCommand::LineTo(corners[2]),
        PathCommand::LineTo(corners[3]),
        PathCommand::Close,
    ]
}

fn main() {
    let mut probe_connection =
        tre_platform::PlatformConnection::new().expect("failed to connect to display server");
    let probe_window = probe_connection
        .create_window("tre path and polygon probe (never shown)", 1, 1)
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
    let vertex_spv = std::fs::read(format!("{out_dir}/walking_skeleton.vert.spv"))
        .expect("failed to read compiled vertex shader");
    let fragment_spv = std::fs::read(format!("{out_dir}/walking_skeleton.frag.spv"))
        .expect("failed to read compiled fragment shader");
    let pipeline = device
        .create_pipeline(&vertex_spv, &fragment_spv, tre_rhi_vulkan::HEADLESS_FORMAT)
        .expect("failed to create pipeline");
    let mut pipelines = PipelineRegistry::new();
    pipelines.register(PipelineKind::FlatColor as u16, Box::new(pipeline));

    let white = rgba8(255, 255, 255, 255);
    let red = rgba8(255, 0, 0, 255);
    let green = rgba8(0, 255, 0, 255);
    let yellow = rgba8(255, 255, 0, 255);

    let mut registry = ShapeRegistry::new();

    // The donut: outer square wound clockwise, inner "hole" square wound
    // counter-clockwise -- opposite winding is what makes NonZero
    // resolve the inner contour as a real subtractive hole.
    let hole_offset = (DONUT_OUTER - DONUT_HOLE) / 2.0;
    let mut donut_commands = square_path([0.0, 0.0], DONUT_OUTER, true);
    donut_commands.extend(square_path([hole_offset, hole_offset], DONUT_HOLE, false));
    registry.insert(ShapePrimitive::Path(Path {
        common: {
            let mut common = PrimitiveCommon::new();
            common.transform.position = DONUT_POSITION;
            common
        },
        commands: donut_commands,
        fill: FillStyle::Solid(white),
        border_color: red,
        border_thickness: DONUT_BORDER_THICKNESS,
        border_enabled: true,
        stroke_line_cap: LineCap::Butt,
        stroke_line_join: LineJoin::Miter,
    }));

    registry.insert(ShapePrimitive::Polygon(Polygon {
        common: {
            let mut common = PrimitiveCommon::new();
            common.transform.position = HEXAGON_CENTER;
            common
        },
        sides: 6,
        radius: HEXAGON_RADIUS,
        vertex_radius: 0.0,
        star_points: None,
        fill: FillStyle::Solid(green),
        border_color: yellow,
        border_thickness: HEXAGON_BORDER_THICKNESS,
        border_enabled: true,
    }));

    let mut canvas = RenderingCanvas::new();
    registry.flatten_into(&mut canvas, &device, None);
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

    // --- The donut: filled ring, real hole, real border. ---
    // Deep in the ring (between outer edge and hole), clear of the
    // border band.
    let ring_fill = pixel_at(30, 70);
    assert_eq!(
        ring_fill,
        [255, 255, 255, 255],
        "the donut's own ring must render fill_color, got {ring_fill:?}"
    );
    // Dead center: inside the hole -- must be background, not fill. This
    // is the real proof a compound Path with a hole renders correctly.
    let hole_center = pixel_at(70, 70);
    assert_eq!(
        hole_center, background,
        "the donut's own hole must be background (a real subtractive hole, not filled), got \
         {hole_center:?}"
    );
    // The outer edge's own border band.
    let outer_border = pixel_at(22, 70);
    assert_eq!(
        outer_border,
        [255, 0, 0, 255],
        "the donut's own outer border must render border_color, got {outer_border:?}"
    );
    eprintln!("donut: ring filled, hole is real (background), outer border rendered -- OK");

    // --- The hexagon: filled interior, real border. ---
    let hexagon_center = pixel_at(220, 70);
    assert_eq!(
        hexagon_center,
        [0, 255, 0, 255],
        "the hexagon's own center must render fill_color, got {hexagon_center:?}"
    );
    // Just inside the top edge (12 o'clock direction), within the 8px
    // border band.
    let hexagon_border = pixel_at(220, 70 - 45 + 4);
    assert_eq!(
        hexagon_border,
        [255, 255, 0, 255],
        "the hexagon's own border must render border_color, got {hexagon_border:?}"
    );
    eprintln!("hexagon: fill and border both rendered correctly -- OK");

    let mut rgba_out = bgra.clone();
    for px in rgba_out.chunks_exact_mut(4) {
        px.swap(0, 2);
    }
    let out_path = std::env::var("TRE_PATH_AND_POLYGON_OUTPUT")
        .unwrap_or_else(|_| "path_and_polygon_output.png".to_string());
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
    eprintln!("wrote {SWAPCHAIN_WIDTH}x{SWAPCHAIN_HEIGHT} path and polygon render to {out_path}");

    eprintln!("path_and_polygon_demo: all checks passed");
}
