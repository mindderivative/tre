//! Phase 10 Step 10.2.2 proof: real `FillStyle::Texture` rendering for
//! all four shape kinds, via `ShapeRegistry::flatten_into` (never
//! hand-written RHI calls).
//!
//! Exercises BOTH real texture-fill code paths this step adds: (1)
//! `Rectangle`/`Circle` sample a bindless texture directly inside their
//! own existing SDF shaders (`sdf_rect_styled.frag`/`sdf_ellipse.frag`'s
//! own new texture branch), mapping `frag_uv` onto the shape's own
//! bounding box; (2) `Polygon`/`Path` have no per-vertex style record at
//! all, so they reuse the EXISTING `PipelineKind::TexturedQuad`/
//! `bindless_textured.frag` pipeline directly -- no new shader, since
//! sampling a texture is not new math the way gradient evaluation was --
//! with real, bounding-box-normalized UVs computed once at flatten time
//! (`shapes::bounding_box_uvs`).
//!
//! Real correctness, not just "didn't crash": the real texture is a
//! four-quadrant flag (red/green/blue/yellow, pure 0/255 channel values
//! so sRGB round-trips exactly, matching `bindless_textures_demo.rs`'s
//! own established precedent) -- probing each shape's own upper-left and
//! lower-right quadrant proves the real UV mapping is correct, not just
//! "some texture appeared."

use ash::vk;
use tre_engine::{
    execute_frame, submit_frame, BufferBinding, Circle, CornerRadii, FillStyle, Path, PathCommand,
    PipelineKind, PipelineRegistry, Polygon, PrimitiveCommon, Rectangle, RenderingCanvas,
    RhiDevice, ScissorRect, ShapePrimitive, ShapeRegistry, TextureFormat,
};
use tre_rhi_vulkan::{HeadlessSwapchain, VulkanDevice};

#[path = "support/pixel_helpers.rs"]
mod pixel_helpers;

const SWAPCHAIN_WIDTH: u32 = 430;
const SWAPCHAIN_HEIGHT: u32 = 230;
const TEXTURE_SIZE: u32 = 8;

const RECT_POSITION: [f32; 2] = [20.0, 20.0];
const RECT_SIZE: [f32; 2] = [120.0, 80.0];

const CIRCLE_CENTER: [f32; 2] = [260.0, 60.0];
const CIRCLE_RADIUS: f32 = 40.0;

const HEXAGON_CENTER: [f32; 2] = [380.0, 70.0];
const HEXAGON_RADIUS: f32 = 35.0;

const PATH_POSITION: [f32; 2] = [20.0, 140.0];
const PATH_SIZE: f32 = 80.0;

/// A real four-quadrant flag texture, in `Bgra8Srgb`'s own in-memory
/// byte order -- red (top-left), green (top-right), blue (bottom-left),
/// yellow (bottom-right). Pure `0`/`255` channel values throughout, the
/// same precedent `bindless_textures_demo.rs` already established, so
/// this format's sRGB encode/decode round-trips exactly rather than
/// approximately.
fn four_quadrant_bgra8(size: u32) -> Vec<u8> {
    let half = size / 2;
    let mut pixels = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let (b, g, r) = match (x < half, y < half) {
                (true, true) => (0u8, 0u8, 255u8), // red
                (false, true) => (0, 255, 0),      // green
                (true, false) => (255, 0, 0),      // blue
                (false, false) => (0, 255, 255),   // yellow
            };
            pixels.extend_from_slice(&[b, g, r, 255]);
        }
    }
    pixels
}

fn assert_color(label: &str, got: [u8; 4], want: [u8; 3]) {
    assert_eq!(
        [got[0], got[1], got[2]],
        want,
        "{label}: expected {want:?}, got {:?} (full pixel {got:?})",
        [got[0], got[1], got[2]]
    );
}

fn main() {
    let mut probe_connection =
        tre_platform::PlatformConnection::new().expect("failed to connect to display server");
    let probe_window = probe_connection
        .create_window("tre texture fill probe (never shown)", 1, 1)
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
    let sdf_vertex_spv = std::fs::read(format!("{out_dir}/sdf_rounded_rect.vert.spv"))
        .expect("failed to read compiled vertex shader");
    let rect_fragment_spv = std::fs::read(format!("{out_dir}/sdf_rect_styled.frag.spv"))
        .expect("failed to read compiled styled-rect fragment shader");
    let rect_pipeline = device
        .create_pipeline(
            &sdf_vertex_spv,
            &rect_fragment_spv,
            tre_rhi_vulkan::HEADLESS_FORMAT,
        )
        .expect("failed to create styled-rect pipeline");
    let ellipse_fragment_spv = std::fs::read(format!("{out_dir}/sdf_ellipse.frag.spv"))
        .expect("failed to read compiled ellipse fragment shader");
    let ellipse_pipeline = device
        .create_pipeline(
            &sdf_vertex_spv,
            &ellipse_fragment_spv,
            tre_rhi_vulkan::HEADLESS_FORMAT,
        )
        .expect("failed to create ellipse pipeline");
    let bindless_vertex_spv = std::fs::read(format!("{out_dir}/bindless_textured.vert.spv"))
        .expect("failed to read compiled bindless vertex shader");
    let bindless_fragment_spv = std::fs::read(format!("{out_dir}/bindless_textured.frag.spv"))
        .expect("failed to read compiled bindless fragment shader");
    let textured_quad_pipeline = device
        .create_pipeline(
            &bindless_vertex_spv,
            &bindless_fragment_spv,
            tre_rhi_vulkan::HEADLESS_FORMAT,
        )
        .expect("failed to create textured-quad pipeline");

    let mut pipelines = PipelineRegistry::new();
    pipelines.register(PipelineKind::SdfRectStyled as u16, Box::new(rect_pipeline));
    pipelines.register(PipelineKind::SdfEllipse as u16, Box::new(ellipse_pipeline));
    pipelines.register(
        PipelineKind::TexturedQuad as u16,
        Box::new(textured_quad_pipeline),
    );

    let flag_texture = device
        .create_texture(
            TEXTURE_SIZE,
            TEXTURE_SIZE,
            TextureFormat::Bgra8Srgb,
            &four_quadrant_bgra8(TEXTURE_SIZE),
        )
        .expect("failed to create flag texture");
    let texture_index = flag_texture
        .bindless_index()
        .expect("create_texture always registers a real bindless index");

    const RED: [u8; 3] = [255, 0, 0];
    const YELLOW: [u8; 3] = [255, 255, 0];

    let mut registry = ShapeRegistry::new();

    let mut rect = Rectangle::new(RECT_SIZE, 0xFFFF_FFFF);
    rect.common.transform.position = RECT_POSITION;
    rect.corner_radius = CornerRadii::uniform(0.0);
    rect.fill = FillStyle::Texture(texture_index);
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
        fill: FillStyle::Texture(texture_index),
        border_color: 0,
        border_thickness: 0.0,
        arc_length: 360.0,
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
        fill: FillStyle::Texture(texture_index),
        border_color: 0,
        border_thickness: 0.0,
    }));

    registry.insert(ShapePrimitive::Path(Path {
        common: {
            let mut common = PrimitiveCommon::new();
            common.transform.position = PATH_POSITION;
            common
        },
        commands: vec![
            PathCommand::MoveTo([0.0, 0.0]),
            PathCommand::LineTo([PATH_SIZE, 0.0]),
            PathCommand::LineTo([PATH_SIZE, PATH_SIZE]),
            PathCommand::LineTo([0.0, PATH_SIZE]),
            PathCommand::Close,
        ],
        fill: FillStyle::Texture(texture_index),
        border_color: 0,
        border_thickness: 0.0,
        stroke_line_cap: tre_engine::LineCap::Butt,
        stroke_line_join: tre_engine::LineJoin::Miter,
    }));

    let mut canvas = RenderingCanvas::new();
    registry.flatten_into(&mut canvas, &device);
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

    // --- Rectangle: upper-left quadrant (red) and lower-right (yellow). ---
    assert_color("rectangle upper-left", pixel_at(50, 40), RED);
    assert_color("rectangle lower-right", pixel_at(110, 80), YELLOW);
    eprintln!("rectangle: texture fill UV mapping correct -- OK");

    // --- Circle: same quadrant convention via its own bounding box. ---
    assert_color("circle upper-left", pixel_at(240, 40), RED);
    assert_color("circle lower-right", pixel_at(280, 80), YELLOW);
    eprintln!("circle: texture fill UV mapping correct -- OK");

    // --- Hexagon: through the entirely separate TexturedQuad pipeline. ---
    assert_color("hexagon upper-left", pixel_at(365, 53), RED);
    assert_color("hexagon lower-right", pixel_at(395, 88), YELLOW);
    eprintln!("hexagon: texture fill via the reused TexturedQuad pipeline -- OK");

    // --- Path: a plain square, exact bounding box == its own corners. ---
    assert_color("path upper-left", pixel_at(40, 160), RED);
    assert_color("path lower-right", pixel_at(80, 200), YELLOW);
    eprintln!("path: texture fill UV mapping correct -- OK");

    let mut rgba = bgra.clone();
    for px in rgba.chunks_exact_mut(4) {
        px.swap(0, 2);
    }
    let out_path = std::env::var("TRE_TEXTURE_FILL_OUTPUT")
        .unwrap_or_else(|_| "texture_fill_output.png".to_string());
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
    eprintln!("wrote {SWAPCHAIN_WIDTH}x{SWAPCHAIN_HEIGHT} texture fill render to {out_path}");

    eprintln!("texture_fill_demo: all checks passed");
}
