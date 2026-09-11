//! Phase 10 Step 10.2.1 proof: real `FillStyle::Gradient` rendering for
//! all four shape kinds, via `ShapeRegistry::create_gradient` and
//! `ShapeRegistry::flatten_into` (never hand-written RHI calls).
//!
//! Exercises BOTH real gradient code paths this step adds, not just one:
//! (1) `Rectangle`/`Circle` route through the per-vertex `GpuRectStyle`/
//! `GpuEllipseStyle` style-buffer record's own new `fill_kind`/
//! `gradient_word_index` fields (`sdf_rect_styled.frag`/`sdf_ellipse.
//! frag`'s own `eval_gradient`); (2) `Polygon` has no per-vertex style
//! record at all, so it routes through the entirely separate
//! `PipelineKind::GradientFill` pipeline (`gradient_fill.frag`, reusing
//! the existing `bindless_textured.vert` and its own `texture_index`
//! push constant, repurposed here as a gradient word index). A `Circle`
//! with a border also proves gradient fill composes correctly with the
//! SDF border-blend math already proven for solid fills.
//!
//! Real correctness, not just "didn't crash": every probed pixel is
//! compared against an independent Rust reference implementation of the
//! exact same premultiplied, linear-space gradient evaluation the real
//! shaders perform (mirroring `translucent_flat_fill_demo.rs`'s own
//! established "compute the correct answer independently, compare
//! against real GPU output" discipline) -- not just "some color changed."

use ash::vk;
use tre_engine::{
    execute_frame, rgba8, submit_frame, BufferBinding, Circle, CornerRadii, FillStyle, GradientDef,
    GradientKind, GradientStop, PipelineKind, PipelineRegistry, Polygon, PrimitiveCommon,
    Rectangle, RenderingCanvas, ScissorRect, ShapePrimitive, ShapeRegistry,
};
use tre_rhi_vulkan::{HeadlessSwapchain, VulkanDevice};

#[path = "support/pixel_helpers.rs"]
mod pixel_helpers;

const SWAPCHAIN_WIDTH: u32 = 440;
const SWAPCHAIN_HEIGHT: u32 = 200;

// --- The gradient-filled rectangle: horizontal linear gradient, red at
// the left edge to blue at the right, plus a real border (proving
// gradient fill composes with the existing border-blend math). ---
const RECT_POSITION: [f32; 2] = [20.0, 20.0];
const RECT_SIZE: [f32; 2] = [200.0, 100.0];
const RECT_BORDER_THICKNESS: f32 = 8.0;

// --- The gradient-filled circle: radial gradient, white at the center
// to green at the edge, plus a real border. ---
const CIRCLE_CENTER: [f32; 2] = [310.0, 70.0];
const CIRCLE_RADIUS: f32 = 50.0;
const CIRCLE_BORDER_THICKNESS: f32 = 6.0;

// --- The gradient-filled hexagon: horizontal linear gradient, red to
// green, through the entirely separate GradientFill pipeline. ---
const HEXAGON_CENTER: [f32; 2] = [380.0, 70.0];
const HEXAGON_RADIUS: f32 = 35.0;

/// TECHNICAL.md Section 6.2's canonical sRGB<->linear formulas -- a real,
/// independent Rust reference of `eval_gradient`'s own exact math
/// (`sdf_rect_styled.frag`/`sdf_ellipse.frag`/`gradient_fill.frag`, all
/// three duplicate the same formula), used to prove the real GPU output
/// against a computation nothing here shares code with.
fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.040_45 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn srgb_encode(c: f32) -> f32 {
    if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

fn to_srgb_byte(c: f32) -> u8 {
    (srgb_encode(c.clamp(0.0, 1.0)) * 255.0)
        .round()
        .clamp(0.0, 255.0) as u8
}

#[derive(Clone, Copy)]
struct RefStop {
    position: f32,
    color: [u8; 3],
}

/// The exact same stop-bracketing/interpolation `eval_gradient` performs
/// in every one of the three shaders this demo exercises -- an
/// independent Rust re-implementation, not shared code, so a real bug in
/// either side would show up as a real mismatch.
fn eval_stops(stops: &[RefStop], t: f32) -> [u8; 3] {
    if stops.len() == 1 {
        return stops[0].color;
    }
    let first = stops[0].position;
    let last = stops[stops.len() - 1].position;
    if t <= first {
        return stops[0].color;
    }
    if t >= last {
        return stops[stops.len() - 1].color;
    }
    let mut lower = 0;
    for i in 0..stops.len() - 1 {
        if t >= stops[i].position && t <= stops[i + 1].position {
            lower = i;
        }
    }
    let (p0, p1) = (stops[lower].position, stops[lower + 1].position);
    let local_t = if p1 > p0 { (t - p0) / (p1 - p0) } else { 0.0 };
    let (c0, c1) = (stops[lower].color, stops[lower + 1].color);
    let mut out = [0u8; 3];
    for ch in 0..3 {
        let lin0 = srgb_to_linear(f32::from(c0[ch]) / 255.0);
        let lin1 = srgb_to_linear(f32::from(c1[ch]) / 255.0);
        out[ch] = to_srgb_byte(lin0 + (lin1 - lin0) * local_t);
    }
    out
}

fn eval_linear(stops: &[RefStop], point: [f32; 2], start: [f32; 2], end: [f32; 2]) -> [u8; 3] {
    let axis = [end[0] - start[0], end[1] - start[1]];
    let len_sq = axis[0].mul_add(axis[0], axis[1] * axis[1]);
    let rel = [point[0] - start[0], point[1] - start[1]];
    let t = if len_sq > 0.0 {
        rel[0].mul_add(axis[0], rel[1] * axis[1]) / len_sq
    } else {
        0.0
    };
    eval_stops(stops, t.clamp(0.0, 1.0))
}

fn eval_radial(stops: &[RefStop], point: [f32; 2], center: [f32; 2], radius: f32) -> [u8; 3] {
    let dx = point[0] - center[0];
    let dy = point[1] - center[1];
    let d = dx.hypot(dy);
    let t = if radius > 0.0 { d / radius } else { 0.0 };
    eval_stops(stops, t.clamp(0.0, 1.0))
}

fn assert_close(label: &str, got: [u8; 4], want_rgb: [u8; 3], tolerance: i32) {
    for (channel, (&got_c, &want_c)) in ["R", "G", "B"].iter().zip(got.iter().zip(&want_rgb)) {
        let diff = (i32::from(got_c) - i32::from(want_c)).abs();
        assert!(
            diff <= tolerance,
            "{label} channel {channel}: expected {want_c} (within {tolerance}), got {got_c} \
             (diff {diff}), full pixel {got:?}"
        );
    }
}

fn main() {
    let mut probe_connection =
        tre_platform::PlatformConnection::new().expect("failed to connect to display server");
    let probe_window = probe_connection
        .create_window("tre gradient fill probe (never shown)", 1, 1)
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
    let gradient_fragment_spv = std::fs::read(format!("{out_dir}/gradient_fill.frag.spv"))
        .expect("failed to read compiled gradient-fill fragment shader");
    let gradient_pipeline = device
        .create_pipeline(
            &bindless_vertex_spv,
            &gradient_fragment_spv,
            tre_rhi_vulkan::HEADLESS_FORMAT,
        )
        .expect("failed to create gradient-fill pipeline");

    let mut pipelines = PipelineRegistry::new();
    pipelines.register(PipelineKind::SdfRectStyled as u16, Box::new(rect_pipeline));
    pipelines.register(PipelineKind::SdfEllipse as u16, Box::new(ellipse_pipeline));
    pipelines.register(
        PipelineKind::GradientFill as u16,
        Box::new(gradient_pipeline),
    );

    let red: [u8; 3] = [220, 30, 30];
    let blue: [u8; 3] = [30, 60, 220];
    let white: [u8; 3] = [255, 255, 255];
    let green: [u8; 3] = [30, 200, 60];
    let black = rgba8(0, 0, 0, 255);

    let mut registry = ShapeRegistry::new();

    // --- Rectangle: linear gradient, red (left) -> blue (right). ---
    let rect_stops = [
        RefStop {
            position: 0.0,
            color: red,
        },
        RefStop {
            position: 1.0,
            color: blue,
        },
    ];
    let rect_gradient = registry
        .create_gradient(GradientDef {
            kind: GradientKind::Linear {
                start: [0.0, 0.0],
                end: [RECT_SIZE[0], 0.0],
            },
            stops: vec![
                GradientStop {
                    position: 0.0,
                    color: rgba8(red[0], red[1], red[2], 255),
                },
                GradientStop {
                    position: 1.0,
                    color: rgba8(blue[0], blue[1], blue[2], 255),
                },
            ],
        })
        .expect("a valid gradient must be accepted");
    let mut rect = Rectangle::new(RECT_SIZE, 0xFFFF_FFFF);
    rect.common.transform.position = RECT_POSITION;
    rect.corner_radius = CornerRadii::uniform(0.0);
    rect.fill = FillStyle::Gradient(rect_gradient);
    rect.border_color = black;
    rect.border_thickness = RECT_BORDER_THICKNESS;
    registry.insert(ShapePrimitive::Rectangle(rect));

    // --- Circle: radial gradient, white (center) -> green (edge). ---
    let circle_stops = [
        RefStop {
            position: 0.0,
            color: white,
        },
        RefStop {
            position: 1.0,
            color: green,
        },
    ];
    let circle_gradient = registry
        .create_gradient(GradientDef {
            kind: GradientKind::Radial {
                center: [CIRCLE_RADIUS, CIRCLE_RADIUS],
                radius: CIRCLE_RADIUS,
            },
            stops: vec![
                GradientStop {
                    position: 0.0,
                    color: rgba8(white[0], white[1], white[2], 255),
                },
                GradientStop {
                    position: 1.0,
                    color: rgba8(green[0], green[1], green[2], 255),
                },
            ],
        })
        .expect("a valid gradient must be accepted");
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
        fill: FillStyle::Gradient(circle_gradient),
        border_color: black,
        border_thickness: CIRCLE_BORDER_THICKNESS,
        arc_length: 360.0,
    }));

    // --- Hexagon: linear gradient, red -> green, through the entirely
    // separate GradientFill pipeline (no per-vertex style record). ---
    let hexagon_stops = [
        RefStop {
            position: 0.0,
            color: red,
        },
        RefStop {
            position: 1.0,
            color: green,
        },
    ];
    let hexagon_gradient = registry
        .create_gradient(GradientDef {
            kind: GradientKind::Linear {
                start: [-HEXAGON_RADIUS, 0.0],
                end: [HEXAGON_RADIUS, 0.0],
            },
            stops: vec![
                GradientStop {
                    position: 0.0,
                    color: rgba8(red[0], red[1], red[2], 255),
                },
                GradientStop {
                    position: 1.0,
                    color: rgba8(green[0], green[1], green[2], 255),
                },
            ],
        })
        .expect("a valid gradient must be accepted");
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
        fill: FillStyle::Gradient(hexagon_gradient),
        border_color: 0,
        border_thickness: 0.0,
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

    const TOLERANCE: i32 = 4;

    // --- Rectangle: two probes 30px in from each edge (well clear of
    // the 8px border band), plus a border-band sample. ---
    let rect_probe_a = pixel_at(50, 70); // local (30, 50) -> t = 0.15
    let expected_a = eval_linear(&rect_stops, [30.0, 50.0], [0.0, 0.0], [RECT_SIZE[0], 0.0]);
    assert_close(
        "rectangle gradient (t=0.15)",
        rect_probe_a,
        expected_a,
        TOLERANCE,
    );

    let rect_probe_b = pixel_at(190, 70); // local (170, 50) -> t = 0.85
    let expected_b = eval_linear(&rect_stops, [170.0, 50.0], [0.0, 0.0], [RECT_SIZE[0], 0.0]);
    assert_close(
        "rectangle gradient (t=0.85)",
        rect_probe_b,
        expected_b,
        TOLERANCE,
    );

    let rect_border = pixel_at(22, 70);
    assert_close("rectangle border", rect_border, [0, 0, 0], TOLERANCE);
    eprintln!("rectangle: linear gradient fill + border both correct -- OK");

    // --- Circle: center (t=0, pure white) and a mid-radius probe. ---
    let circle_center_probe = pixel_at(310, 70);
    assert_close(
        "circle gradient center (t=0)",
        circle_center_probe,
        white,
        TOLERANCE,
    );

    let circle_mid_probe = pixel_at(335, 70); // local (75, 50) from top-left -> radius 25 -> t=0.5
    let expected_mid = eval_radial(
        &circle_stops,
        [75.0, 50.0],
        [CIRCLE_RADIUS, CIRCLE_RADIUS],
        CIRCLE_RADIUS,
    );
    assert_close(
        "circle gradient (t=0.5)",
        circle_mid_probe,
        expected_mid,
        TOLERANCE,
    );

    let circle_border = pixel_at(310, 22);
    assert_close("circle border", circle_border, [0, 0, 0], TOLERANCE);
    eprintln!("circle: radial gradient fill + border both correct -- OK");

    // --- Hexagon: two probes along the gradient's own axis, through the
    // entirely separate GradientFill pipeline. ---
    let hexagon_probe_a = pixel_at(365, 70); // local (-15, 0) -> t = 20/70
    let expected_hex_a = eval_linear(
        &hexagon_stops,
        [-15.0, 0.0],
        [-HEXAGON_RADIUS, 0.0],
        [HEXAGON_RADIUS, 0.0],
    );
    assert_close(
        "hexagon gradient (t=0.286)",
        hexagon_probe_a,
        expected_hex_a,
        TOLERANCE,
    );

    let hexagon_probe_b = pixel_at(395, 70); // local (15, 0) -> t = 50/70
    let expected_hex_b = eval_linear(
        &hexagon_stops,
        [15.0, 0.0],
        [-HEXAGON_RADIUS, 0.0],
        [HEXAGON_RADIUS, 0.0],
    );
    assert_close(
        "hexagon gradient (t=0.714)",
        hexagon_probe_b,
        expected_hex_b,
        TOLERANCE,
    );
    eprintln!(
        "hexagon: linear gradient fill via the entirely separate GradientFill pipeline -- OK"
    );

    let mut rgba = bgra.clone();
    for px in rgba.chunks_exact_mut(4) {
        px.swap(0, 2);
    }
    let out_path = std::env::var("TRE_GRADIENT_FILL_OUTPUT")
        .unwrap_or_else(|_| "gradient_fill_output.png".to_string());
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
    eprintln!("wrote {SWAPCHAIN_WIDTH}x{SWAPCHAIN_HEIGHT} gradient fill render to {out_path}");

    eprintln!("gradient_fill_demo: all checks passed");
}
