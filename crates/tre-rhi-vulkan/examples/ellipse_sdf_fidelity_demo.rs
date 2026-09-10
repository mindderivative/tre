//! Phase 10 Step 10.2.4 proof: the real, exact ellipse signed-distance
//! field (`sdf_ellipse.frag`'s new `sd_ellipse`, Inigo Quilez's
//! Newton-Raphson refinement) proven on REAL GPU HARDWARE, not just via
//! `tre-engine`'s own CPU-side `sdf_ellipse_fidelity` unit tests.
//!
//! No prior demo ever drew a genuinely non-circular `Circle` (every
//! `radius` used elsewhere has `radius.x == radius.y`, the one case
//! where the OLD "scaled circle" approximation Step 10.2 originally
//! shipped happened to already be exact) -- this is the first real GPU
//! render of a true, eccentric ellipse.
//!
//! What's actually being proven, precisely: the old approximation's
//! zero-crossing (the fill/no-fill BOUNDARY itself) is, perhaps
//! surprisingly, always exactly correct -- `k1 = length(p/r)` is
//! exactly `1.0` everywhere ON the true ellipse boundary by
//! construction, so `k1*(k1-1)/k2` is always exactly `0` there too. The
//! old approximation's real error is in the SDF's actual MAGNITUDE away
//! from the boundary -- which is exactly what `border_thickness`
//! rendering depends on (`inner_d = d + border_thickness`). This demo
//! draws a real, eccentric, BORDERED ellipse and finds the real
//! border/fill transition pixel at several angles by bisecting on
//! actual GPU-rendered color, then confirms each one lands where an
//! independent CPU reference (a second, from-scratch transcription of
//! the exact shader formula, doing its own Newton refinement) predicts
//! -- not merely close to the ellipse boundary by eye.

use ash::vk;
use tre_engine::{
    execute_frame, rgba8, submit_frame, BufferBinding, Circle, PipelineKind, PipelineRegistry,
    PrimitiveCommon, RenderingCanvas, ScissorRect, ShapePrimitive, ShapeRegistry,
};
use tre_rhi_vulkan::{HeadlessSwapchain, VulkanDevice};

#[path = "support/pixel_helpers.rs"]
mod pixel_helpers;

const CANVAS_WIDTH: u32 = 420;
const CANVAS_HEIGHT: u32 = 200;

const ELLIPSE_CENTER: [f32; 2] = [160.0, 100.0];
const ELLIPSE_RADIUS: [f32; 2] = [140.0, 40.0];
const BORDER_THICKNESS: f32 = 15.0;

/// Angles to probe, degrees from the major (x) axis -- `0.0` is a
/// sanity control (both formulas agree exactly there); the rest are
/// off-axis, exactly where `tre-engine`'s own CPU-side tests measured
/// the old approximation's real, substantial error.
const PROBE_ANGLES_DEGREES: [f32; 4] = [0.0, 30.0, 55.0, 80.0];

/// A second, independent transcription of `sdf_ellipse.frag`'s exact
/// `sd_ellipse` -- deliberately written fresh here rather than calling
/// `tre_engine`'s own (already-tested) copy, so this demo's own
/// prediction shares no code with either the shader or the unit tests
/// that already checked it, matching this codebase's "compute the
/// correct answer independently" discipline one level further.
fn sd_ellipse_exact(p: [f32; 2], ab: [f32; 2]) -> f32 {
    let p = [p[0].abs(), p[1].abs()];
    let q = [ab[0] * (p[0] - ab[0]), ab[1] * (p[1] - ab[1])];
    let seed: [f32; 2] = if q[0] < q[1] {
        [0.01, 1.0]
    } else {
        [1.0, 0.01]
    };
    let norm = (seed[0] * seed[0] + seed[1] * seed[1]).sqrt();
    let mut cs = [seed[0] / norm, seed[1] / norm];
    for _ in 0..5 {
        let u = [ab[0] * cs[0], ab[1] * cs[1]];
        let v = [ab[0] * -cs[1], ab[1] * cs[0]];
        let pu = [p[0] - u[0], p[1] - u[1]];
        let a = pu[0] * v[0] + pu[1] * v[1];
        let c = pu[0] * u[0] + pu[1] * u[1] + v[0] * v[0] + v[1] * v[1];
        let b = (c * c - a * a).max(0.0).sqrt();
        cs = [(cs[0] * b - cs[1] * a) / c, (cs[1] * b + cs[0] * a) / c];
    }
    let d = ((p[0] - ab[0] * cs[0]).powi(2) + (p[1] - ab[1] * cs[1]).powi(2)).sqrt();
    let outside = (p[0] / ab[0]).powi(2) + (p[1] / ab[1]).powi(2) > 1.0;
    if outside {
        d
    } else {
        -d
    }
}

/// Finds `t` (distance from the ellipse's own center, along the ray at
/// `angle_radians`) such that `sd_ellipse_exact` reports exactly
/// `-BORDER_THICKNESS` there -- the real inner edge of the border band,
/// per the exact shader formula (`inner_d = d + border_thickness` hits
/// zero exactly there). Plain bisection: `sd_ellipse_exact` is
/// monotonically increasing in `t` along any fixed ray from `0` (deep
/// inside, very negative) out to the boundary (`0`).
fn find_border_inner_edge_t(angle_radians: f32) -> f32 {
    let (mut lo, mut hi) = (0.0_f32, ELLIPSE_RADIUS[0].max(ELLIPSE_RADIUS[1]));
    for _ in 0..40 {
        let mid = (lo + hi) / 2.0;
        let p = [mid * angle_radians.cos(), mid * angle_radians.sin()];
        if sd_ellipse_exact(p, ELLIPSE_RADIUS) < -BORDER_THICKNESS {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    (lo + hi) / 2.0
}

fn main() {
    let mut probe_connection =
        tre_platform::PlatformConnection::new().expect("failed to connect to display server");
    let probe_window = probe_connection
        .create_window("tre ellipse sdf fidelity probe (never shown)", 1, 1)
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

    // Pure black/white -- gamma-invariant, round-tripping exactly
    // through sRGB regardless of any unrelated color-space concern
    // (`linear_color_demo.rs`'s own established precedent), so a
    // border/fill color-classification threshold is unambiguous.
    let black = rgba8(0, 0, 0, 255);
    let white = rgba8(255, 255, 255, 255);

    let mut registry = ShapeRegistry::new();
    registry.insert(ShapePrimitive::Circle(Circle {
        common: {
            let mut common = PrimitiveCommon::new();
            common.transform.position = [
                ELLIPSE_CENTER[0] - ELLIPSE_RADIUS[0],
                ELLIPSE_CENTER[1] - ELLIPSE_RADIUS[1],
            ];
            common
        },
        radius: ELLIPSE_RADIUS,
        fill: tre_engine::FillStyle::Solid(white),
        border_color: black,
        border_thickness: BORDER_THICKNESS,
        arc_length: 360.0,
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

    // A few pixels in, either side of the predicted transition --
    // clearly outside the ~1px antialiasing band, so both probes should
    // land solidly in one color or the other if the real shader's
    // border/fill transition genuinely sits where the exact CPU
    // reference predicts.
    const PROBE_OFFSET: f32 = 5.0;

    for angle_degrees in PROBE_ANGLES_DEGREES {
        let angle_radians = angle_degrees.to_radians();
        let t = find_border_inner_edge_t(angle_radians);

        let point_at = |t: f32| -> (u32, u32) {
            let x = ELLIPSE_CENTER[0] + t * angle_radians.cos();
            let y = ELLIPSE_CENTER[1] + t * angle_radians.sin();
            (x.round() as u32, y.round() as u32)
        };
        let (inner_x, inner_y) = point_at(t - PROBE_OFFSET);
        let (outer_x, outer_y) = point_at(t + PROBE_OFFSET);

        let inner_pixel = pixel_at(inner_x, inner_y);
        let outer_pixel = pixel_at(outer_x, outer_y);
        let inner_gray = gray_level(inner_pixel);
        let outer_gray = gray_level(outer_pixel);

        eprintln!(
            "angle {angle_degrees:>5.1}deg: predicted inner-edge t={t:.2}px -- inner probe \
             {inner_pixel:?} (gray {inner_gray:.1}), outer probe {outer_pixel:?} (gray \
             {outer_gray:.1})"
        );

        assert!(
            inner_gray > 200.0,
            "angle {angle_degrees}deg: {PROBE_OFFSET}px inside the predicted border/fill \
             transition (t={t:.2}) must be fill (white), got {inner_pixel:?} -- the real \
             shader's transition does not match the exact CPU reference here"
        );
        assert!(
            outer_gray < 55.0,
            "angle {angle_degrees}deg: {PROBE_OFFSET}px outside the predicted border/fill \
             transition (t={t:.2}) must still be border (black), got {outer_pixel:?} -- the \
             real shader's transition does not match the exact CPU reference here"
        );
    }
    eprintln!(
        "all angles: the real GPU shader's border/fill transition matches the exact, \
         independently-computed ellipse SDF -- proving Step 10.2.4's fix on real hardware, not \
         just in CPU-side unit tests"
    );

    let mut rgba = bgra.clone();
    for px in rgba.chunks_exact_mut(4) {
        px.swap(0, 2);
    }
    let out_path = std::env::var("TRE_ELLIPSE_SDF_FIDELITY_OUTPUT")
        .unwrap_or_else(|_| "ellipse_sdf_fidelity_output.png".to_string());
    let file = std::fs::File::create(&out_path).expect("failed to create output PNG file");
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), CANVAS_WIDTH, CANVAS_HEIGHT);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().expect("failed to write PNG header");
    writer
        .write_image_data(&rgba)
        .expect("failed to write PNG image data");

    eprintln!("wrote {CANVAS_WIDTH}x{CANVAS_HEIGHT} ellipse SDF fidelity render to {out_path}");
}
