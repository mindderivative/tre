//! Phase 10 Step 10.2.3 proof: real non-`Normal` `BlendMode` rendering
//! for `Polygon`/`Path` solid fill, via `VK_KHR_dynamic_rendering_local_
//! read` -- a real framebuffer read (`subpassLoad` against a real
//! `VK_DESCRIPTOR_TYPE_INPUT_ATTACHMENT` descriptor), not a `VkBlendOp`
//! selection. `VK_EXT_blend_operation_advanced`, this step's originally-
//! planned primary path, is NOT implemented by RADV (this project's own
//! real dev GPU/driver, confirmed via `vulkaninfo` and Mesa's own release
//! notes -- `documentation/REVIEW.md` has the full account), so this
//! demo exists to prove the real, portable alternative actually works on
//! real hardware, not just that `ShapeRegistry`'s dispatch logic compiles
//! (already covered by `tre-engine`'s own unit tests).
//!
//! Real correctness, not just "didn't crash": a background rectangle is
//! drawn first (`FillStyle::Solid`, ordinary `Normal` blending), then six
//! polygon "swatches" are drawn on top of it, one per `BlendMode`
//! (`Normal` plus the five real blend formulas). Every swatch's own
//! center pixel is compared against an independent Rust reference
//! implementation of the exact same W3C blend formula, computed in
//! linear space and re-encoded to sRGB -- mirroring `gradient_fill_
//! demo.rs`'s own "compute the correct answer independently, compare
//! against real GPU output" discipline. The `Normal` swatch proves
//! `shapes.rs`'s dispatch still routes a `Normal` blend mode through the
//! ordinary `FlatColor` pipeline (an exact opaque overwrite, no blend
//! math at all) even on hardware that supports the new capability.
//!
//! Out of scope for this pass (see `flat_color_blend.frag`'s own doc
//! comment): opaque source/destination only, `Polygon`/`Path` solid fill
//! only -- not `Rectangle`/`Circle`, not gradient/texture fill, and not
//! under an active `Canvas` opacity.

use ash::vk;
use tre_engine::{
    execute_frame, rgba8, submit_frame, BlendMode, BufferBinding, FillStyle, PipelineKind,
    PipelineRegistry, Polygon, PrimitiveCommon, Rectangle, RenderingCanvas, RhiDevice, ScissorRect,
    ShapePrimitive, ShapeRegistry,
};
use tre_rhi_vulkan::{HeadlessSwapchain, VulkanDevice};

#[path = "support/pixel_helpers.rs"]
mod pixel_helpers;

const CANVAS_WIDTH: u32 = 600;
const CANVAS_HEIGHT: u32 = 200;

/// A genuinely non-fixed-point color (none of R/G/B is `0` or `255`,
/// matching `linear_color_demo.rs`'s own precedent) -- the backdrop
/// every swatch blends against.
const BACKGROUND: (u8, u8, u8) = (180, 110, 60);
/// The swatch's own source color, drawn on top of `BACKGROUND` under
/// each `BlendMode` in turn.
const FOREGROUND: (u8, u8, u8) = (60, 150, 200);

const SWATCH_RADIUS: f32 = 60.0;
const SWATCH_Y: f32 = 100.0;
const SWATCH_MODES: [(BlendMode, &str); 6] = [
    (BlendMode::Normal, "Normal"),
    (BlendMode::Multiply, "Multiply"),
    (BlendMode::Screen, "Screen"),
    (BlendMode::Overlay, "Overlay"),
    (BlendMode::SoftLight, "SoftLight"),
    (BlendMode::ColorDodge, "ColorDodge"),
];

fn swatch_center_x(index: usize) -> f32 {
    let step = CANVAS_WIDTH as f32 / SWATCH_MODES.len() as f32;
    step * (index as f32 + 0.5)
}

/// TECHNICAL.md Section 6.2's canonical sRGB<->linear formulas -- a real,
/// independent Rust reference of `flat_color_blend.frag`'s own exact
/// math, sharing no code with the shader.
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

fn to_linear_rgb((r, g, b): (u8, u8, u8)) -> [f32; 3] {
    [
        srgb_to_linear(f32::from(r) / 255.0),
        srgb_to_linear(f32::from(g) / 255.0),
        srgb_to_linear(f32::from(b) / 255.0),
    ]
}

fn to_srgb_bytes(linear: [f32; 3]) -> [u8; 3] {
    linear.map(|c| {
        (srgb_encode(c.clamp(0.0, 1.0)) * 255.0)
            .round()
            .clamp(0.0, 255.0) as u8
    })
}

/// W3C Compositing and Blending Level 1's separable blend functions,
/// duplicated independently from `flat_color_blend.frag`'s own GLSL --
/// `cb` is the backdrop (destination), `cs` is the source.
fn blend(mode: BlendMode, cb: [f32; 3], cs: [f32; 3]) -> [f32; 3] {
    let per_channel = |f: fn(f32, f32) -> f32| std::array::from_fn(|i| f(cb[i], cs[i]));
    match mode {
        BlendMode::Normal => cs,
        BlendMode::Multiply => per_channel(|b, s| b * s),
        BlendMode::Screen => per_channel(|b, s| b + s - b * s),
        BlendMode::Overlay => per_channel(|b, s| {
            if b <= 0.5 {
                2.0 * s * b
            } else {
                1.0 - 2.0 * (1.0 - s) * (1.0 - b)
            }
        }),
        BlendMode::SoftLight => per_channel(|b, s| {
            let d = if b <= 0.25 {
                ((16.0 * b - 12.0) * b + 4.0) * b
            } else {
                b.sqrt()
            };
            if s <= 0.5 {
                b - (1.0 - 2.0 * s) * b * (1.0 - b)
            } else {
                b + (2.0 * s - 1.0) * (d - b)
            }
        }),
        BlendMode::ColorDodge => per_channel(|b, s| {
            if b <= 0.0 {
                0.0
            } else if s >= 1.0 {
                1.0
            } else {
                (b / (1.0 - s)).min(1.0)
            }
        }),
    }
}

fn main() {
    let mut probe_connection =
        tre_platform::PlatformConnection::new().expect("failed to connect to display server");
    let probe_window = probe_connection
        .create_window("tre blend mode probe (never shown)", 1, 1)
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

    assert!(
        device.local_read_blend_supported(),
        "this demo proves the real VK_KHR_dynamic_rendering_local_read blend path -- it \
         requires a device that actually supports the capability (this project's own real dev \
         GPU does; see documentation/REVIEW.md)"
    );

    let swapchain = HeadlessSwapchain::new(&device, CANVAS_WIDTH, CANVAS_HEIGHT)
        .expect("failed to create HeadlessSwapchain");

    let out_dir = env!("OUT_DIR");
    let read_spv = |name: &str| {
        std::fs::read(format!("{out_dir}/{name}"))
            .unwrap_or_else(|e| panic!("failed to read compiled shader {name}: {e}"))
    };

    // `Rectangle::new`'s defaults (no corner radius, no border, no
    // smoothing, solid fill) route through the plain `SdfRoundedRect`
    // pipeline (`ShapeRegistry::flatten_into`'s own `needs_styled_path`
    // check in `shapes.rs`), not `SdfRectStyled` -- only a Rectangle
    // that actually needs one of those extra features uses the styled
    // pipeline.
    let sdf_rect_pipeline = device
        .create_pipeline(
            &read_spv("sdf_rounded_rect.vert.spv"),
            &read_spv("sdf_rounded_rect.frag.spv"),
            tre_rhi_vulkan::HEADLESS_FORMAT,
        )
        .expect("failed to create rounded-rect pipeline");
    let flat_color_pipeline = device
        .create_pipeline(
            &read_spv("walking_skeleton.vert.spv"),
            &read_spv("walking_skeleton.frag.spv"),
            tre_rhi_vulkan::HEADLESS_FORMAT,
        )
        .expect("failed to create flat-color pipeline");
    let flat_color_blend_pipeline = device
        .create_blend_mode_pipeline(
            &read_spv("walking_skeleton.vert.spv"),
            &read_spv("flat_color_blend.frag.spv"),
            tre_rhi_vulkan::HEADLESS_FORMAT,
        )
        .expect("failed to create flat-color-blend pipeline");

    let mut pipelines = PipelineRegistry::new();
    pipelines.register(
        PipelineKind::SdfRoundedRect as u16,
        Box::new(sdf_rect_pipeline),
    );
    pipelines.register(
        PipelineKind::FlatColor as u16,
        Box::new(flat_color_pipeline),
    );
    pipelines.register(
        PipelineKind::FlatColorBlend as u16,
        Box::new(flat_color_blend_pipeline),
    );

    let mut registry = ShapeRegistry::new();

    let mut background = Rectangle::new(
        [CANVAS_WIDTH as f32, CANVAS_HEIGHT as f32],
        rgba8(BACKGROUND.0, BACKGROUND.1, BACKGROUND.2, 255),
    );
    background.common.transform.position = [0.0, 0.0];
    registry.insert(ShapePrimitive::Rectangle(background));

    for (index, &(mode, _name)) in SWATCH_MODES.iter().enumerate() {
        let mut common = PrimitiveCommon::new();
        common.transform.position = [swatch_center_x(index), SWATCH_Y];
        common.blend_mode = mode;
        registry.insert(ShapePrimitive::Polygon(Polygon {
            common,
            sides: 48,
            radius: SWATCH_RADIUS,
            vertex_radius: 0.0,
            star_points: None,
            fill: FillStyle::Solid(rgba8(FOREGROUND.0, FOREGROUND.1, FOREGROUND.2, 255)),
            border_color: 0,
            border_thickness: 0.0,
            border_enabled: true,
        }));
    }

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

    const TOLERANCE: i32 = 5;

    // Sanity: a background-only point, far from every swatch, must equal
    // the plain background color round-tripped through the shader's own
    // sRGB<->linear conversion -- proving the background draw itself
    // (ordinary `Normal` blending, no `subpassLoad` involved) is
    // unaffected by anything this step added.
    let background_only = pixel_at(10, 10);
    let expected_background = to_srgb_bytes(to_linear_rgb(BACKGROUND));
    eprintln!("background-only readback: {background_only:?}, expected {expected_background:?}");
    for (channel, (&got, &want)) in ["R", "G", "B"]
        .iter()
        .zip(background_only.iter().zip(&expected_background))
    {
        let diff = (i32::from(got) - i32::from(want)).abs();
        assert!(
            diff <= TOLERANCE,
            "background-only channel {channel}: expected {want} (within {TOLERANCE}), got {got}"
        );
    }

    let cb = to_linear_rgb(BACKGROUND);
    let cs = to_linear_rgb(FOREGROUND);
    for (index, &(mode, name)) in SWATCH_MODES.iter().enumerate() {
        let x = swatch_center_x(index).round() as u32;
        let got = pixel_at(x, SWATCH_Y as u32);
        let expected = to_srgb_bytes(blend(mode, cb, cs));
        eprintln!("{name} swatch center readback: {got:?}, expected {expected:?}");
        for (channel, (&got_c, &want_c)) in ["R", "G", "B"].iter().zip(got.iter().zip(&expected)) {
            let diff = (i32::from(got_c) - i32::from(want_c)).abs();
            assert!(
                diff <= TOLERANCE,
                "{name} swatch channel {channel}: expected {want_c} (within {TOLERANCE}), got \
                 {got_c} (diff {diff}), full pixel {got:?}, independent reference {expected:?}"
            );
        }
    }
    eprintln!(
        "all blend mode assertions passed against a real GPU (RADV) via a real \
         VK_KHR_dynamic_rendering_local_read framebuffer read"
    );

    let mut rgba = bgra.clone();
    for px in rgba.chunks_exact_mut(4) {
        px.swap(0, 2);
    }
    let out_path = std::env::var("TRE_BLEND_MODE_OUTPUT")
        .unwrap_or_else(|_| "blend_mode_output.png".to_string());
    let file = std::fs::File::create(&out_path).expect("failed to create output PNG file");
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), CANVAS_WIDTH, CANVAS_HEIGHT);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().expect("failed to write PNG header");
    writer
        .write_image_data(&rgba)
        .expect("failed to write PNG image data");

    eprintln!("wrote {CANVAS_WIDTH}x{CANVAS_HEIGHT} blend mode render to {out_path}");
}
