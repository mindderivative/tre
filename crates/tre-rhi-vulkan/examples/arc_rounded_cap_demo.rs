//! Phase 10 Step 10.2.5 proof: real analytic rounded stroke caps at a
//! partial arc's own two cut angles (`sdf_ellipse.frag`'s `cap_sdf`,
//! unioned into the sector-clipped ellipse SDF via `min()`), proven on
//! real GPU hardware with real pixel samples -- a direct visual/pixel
//! proof, not a CPU-side math check (there is no independent CPU
//! formula to test the actual antialiased 2D render against; the point
//! IS the real per-pixel shader behavior).
//!
//! What's being proven, precisely: a bordered, partial-arc `Circle`
//! (a quarter circle, 12 o'clock to 3 o'clock) used to have a hard,
//! flat cutoff exactly at each of its two sweep boundaries -- a pixel a
//! few degrees PAST the cut, even well within the border band's own
//! radius, was pure background. With the real rounded cap, a pixel a
//! few degrees past the cut (but still within the cap circle's own
//! radius) is border-colored -- while a pixel well past that (outside
//! the cap's own footprint) is still correctly excluded as background,
//! proving this is a real, BOUNDED rounding, not simply "the cutoff
//! stopped working."

use ash::vk;
use tre_engine::{
    execute_frame, rgba8, submit_frame, BufferBinding, Circle, PipelineKind, PipelineRegistry,
    PrimitiveCommon, RenderingCanvas, ScissorRect, ShapePrimitive, ShapeRegistry,
};
use tre_rhi_vulkan::{HeadlessSwapchain, VulkanDevice};

#[path = "support/pixel_helpers.rs"]
mod pixel_helpers;

const CANVAS_WIDTH: u32 = 300;
const CANVAS_HEIGHT: u32 = 260;
const CENTER: [f32; 2] = [150.0, 130.0];
const RADIUS: f32 = 90.0;
const BORDER_THICKNESS: f32 = 24.0;
/// 12 o'clock to 3 o'clock -- two real, distinct cut angles to check,
/// each swept a different direction relative to their own excluded
/// wedge (the START cut excludes CCW of it, the END cut excludes CW).
const ARC_LENGTH_DEGREES: f32 = 90.0;

/// Raw shader-space angle (radians, `atan2(y,x)` convention, NOT the
/// `Circle::arc_length`'s "12 o'clock clockwise" convention) for each
/// cut, mirroring `flatten_circle`'s own `TWELVE_OCLOCK = -FRAC_PI_2`
/// constant.
const TWELVE_OCLOCK_RADIANS: f32 = -std::f32::consts::FRAC_PI_2;

/// A point at `radius` from `CENTER`, at raw angle `angle_radians`.
fn point_at(angle_radians: f32, radius: f32) -> (u32, u32) {
    let x = CENTER[0] + radius * angle_radians.cos();
    let y = CENTER[1] + radius * angle_radians.sin();
    (x.round() as u32, y.round() as u32)
}

fn main() {
    let mut probe_connection =
        tre_platform::PlatformConnection::new().expect("failed to connect to display server");
    let probe_window = probe_connection
        .create_window("tre arc rounded cap probe (never shown)", 1, 1)
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
    let swapchain = HeadlessSwapchain::new(&device, CANVAS_WIDTH, CANVAS_HEIGHT)
        .expect("failed to create HeadlessSwapchain");

    let out_dir = env!("OUT_DIR");
    let vertex_spv = std::fs::read(format!("{out_dir}/sdf_rounded_rect.vert.spv"))
        .expect("failed to read compiled vertex shader");
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
    pipelines.register(PipelineKind::SdfEllipse as u16, Box::new(ellipse_pipeline));

    let black = rgba8(0, 0, 0, 255);
    let white = rgba8(255, 255, 255, 255);

    let mut registry = ShapeRegistry::new();
    registry.insert(ShapePrimitive::Circle(Circle {
        common: {
            let mut common = PrimitiveCommon::new();
            common.transform.position = [CENTER[0] - RADIUS, CENTER[1] - RADIUS];
            common
        },
        radius: [RADIUS, RADIUS],
        fill: tre_engine::FillStyle::Solid(white),
        border_color: black,
        border_thickness: BORDER_THICKNESS,
        arc_length: ARC_LENGTH_DEGREES,
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
        width: CANVAS_WIDTH,
        height: CANVAS_HEIGHT,
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
        |x: u32, y: u32| -> [u8; 4] { pixel_helpers::bgra_pixel_at(&bgra, CANVAS_WIDTH, x, y) };
    let gray_level = |rgba: [u8; 4]| -> f32 {
        (f32::from(rgba[0]) + f32::from(rgba[1]) + f32::from(rgba[2])) / 3.0
    };
    let is_border = |rgba: [u8; 4]| -> bool { gray_level(rgba) < 40.0 };
    let is_background = |rgba: [u8; 4]| -> bool {
        let g = gray_level(rgba);
        g > 40.0 && g < 200.0
    };

    // The border band's own centerline radius -- where the cap circles
    // themselves are centered, so probing here sits at the deepest,
    // most robust point within each cap's own footprint.
    let probe_radius = RADIUS - BORDER_THICKNESS / 2.0;

    let start_angle = TWELVE_OCLOCK_RADIANS;
    let end_angle = TWELVE_OCLOCK_RADIANS + ARC_LENGTH_DEGREES.to_radians();

    // Each cut's own excluded side: the START cut excludes the
    // COUNTERCLOCKWISE side (more negative angle, toward 11/10/9
    // o'clock); the END cut excludes the CLOCKWISE side (more positive
    // angle, toward 4/5/6 o'clock) -- `execute_frame`'s own sector-
    // cutoff math (`relative > arc_sweep_angle` is excluded) confirms
    // this: rotating a cut angle further INTO the swept sector's own
    // direction stays included, so the excluded side is the opposite.
    let cases = [
        ("start (12 o'clock)", start_angle, -1.0_f32),
        ("end (3 o'clock)", end_angle, 1.0_f32),
    ];

    for (label, cut_angle, excluded_direction) in cases {
        let near_angle = cut_angle + excluded_direction * 4.0_f32.to_radians();
        let far_angle = cut_angle + excluded_direction * 25.0_f32.to_radians();

        let (near_x, near_y) = point_at(near_angle, probe_radius);
        let (far_x, far_y) = point_at(far_angle, probe_radius);
        let near_pixel = pixel_at(near_x, near_y);
        let far_pixel = pixel_at(far_x, far_y);

        eprintln!(
            "{label}: 4deg past the cut (into the excluded side) = {near_pixel:?} (gray \
             {:.1}); 25deg past the cut = {far_pixel:?} (gray {:.1})",
            gray_level(near_pixel),
            gray_level(far_pixel)
        );

        assert!(
            is_border(near_pixel),
            "{label}: a pixel only 4deg past the cut, on its own excluded side, at the border \
             band's own centerline radius, must be border-colored (the real rounded cap) -- got \
             {near_pixel:?}. A flat, unrounded cutoff would have made this pure background."
        );
        assert!(
            is_background(far_pixel),
            "{label}: a pixel 25deg past the cut must still be excluded background -- the \
             rounded cap is a real, BOUNDED rounding (radius {}px), not simply the cutoff no \
             longer working. Got {far_pixel:?}",
            BORDER_THICKNESS / 2.0
        );
    }
    eprintln!(
        "both cut angles: the real GPU shader rounds the border's own stroke ends within a \
         bounded radius, and still correctly excludes the rest of the wedge -- proving Step \
         10.2.5's fix on real hardware"
    );

    let mut rgba = bgra.clone();
    for px in rgba.chunks_exact_mut(4) {
        px.swap(0, 2);
    }
    let out_path = std::env::var("TRE_ARC_ROUNDED_CAP_OUTPUT")
        .unwrap_or_else(|_| "arc_rounded_cap_output.png".to_string());
    let file = std::fs::File::create(&out_path).expect("failed to create output PNG file");
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), CANVAS_WIDTH, CANVAS_HEIGHT);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().expect("failed to write PNG header");
    writer
        .write_image_data(&rgba)
        .expect("failed to write PNG image data");

    eprintln!("wrote {CANVAS_WIDTH}x{CANVAS_HEIGHT} arc rounded cap render to {out_path}");
}
