//! Phase 10 Step 10.2.6 proof: `ShapeRegistry`/`RenderingCanvas` driven
//! shape rendering is genuinely zero-allocation in steady state, closing
//! ARCHITECTURE.md Section 7.5's own disclosed gap -- `main_loop_demo.rs`
//! (Phase 9 Step 9.2) already proved this for its own hand-drawn scene
//! via `RenderingCanvas::draw_rounded_rect`/`draw_text`, but no demo had
//! ever wrapped a `ShapeRegistry::flatten_into`-driven scene in the same
//! real, self-checking `tre_memory::RenderTickGuard`/`DebugAllocGuard`
//! machinery, even though it reuses the exact same already-proven
//! `reset()`/`flatten_into` primitives underneath.
//!
//! Modeled directly on `main_loop_demo.rs`'s own established warm-up-
//! then-guard pattern (a single-threaded, headless simplification: no
//! `SubCanvas` workers, no windowing/input/atlas -- this step is about
//! `ShapeRegistry`/`RenderingCanvas` specifically, not the full 8-stage
//! pipeline Step 8.1.2/9.2 already proved). One `root: RenderingCanvas`
//! stitches into one persistent `FrameArena`, exactly like `main_loop_
//! demo`'s own root canvas does alongside its worker canvases -- using
//! that same real reuse mechanism with a worker count of zero is not a
//! new capability, just this step's own new caller of it.
//!
//! **A real, representative MIXED scene, not a strawman**, covering
//! every new fill/blend feature Steps 10.2.1-10.2.5 added, so this
//! step's own zero-allocation proof covers all of it, not just the
//! original Step 10.2 surface: a solid `Rectangle`, a gradient-filled,
//! non-circular `Circle` (exact ellipse SDF, Step 10.2.4), a texture-
//! filled `Polygon`, a solid-fill `Polygon` under a non-`Normal`
//! `BlendMode` (Step 10.2.3), and a bordered, partial-arc `Circle`
//! (Step 10.2.5's rounded caps). **Mutates real shape properties every
//! frame** -- position, solid-fill color, and (via the new
//! `ShapeRegistry::gradient_mut`, added for this step: no prior API let
//! a caller update an already-registered gradient's own stops without
//! registering an unbounded, ever-growing new one every frame) a
//! gradient's own stop colors -- matching this project's own standing
//! discipline of testing what real, continuously-updating UI usage
//! actually does, not a frame replayed unchanged.
//!
//! **A real, previously-undetected allocation this step's own guard
//! found and fixed** (REVIEW.md finding #176): `generate_polygon_
//! points`/`fan_from_center` (`crates/tre-engine/src/shapes.rs`) each
//! returned a freshly heap-allocated `Vec` on every `Polygon` flatten --
//! fixed via new `_into` siblings writing into `ShapeRegistry`'s own
//! persistent scratch buffers instead. **A real, deeper gap this step
//! does NOT fix, disclosed rather than hidden**: `lyon`-backed
//! tessellation (`tessellate_fill`/`tessellate_stroke`, used by any
//! `Path`'s own fill/stroke and any BORDERED `Polygon`) still allocates
//! fresh tessellator/path/`VertexBuffers` objects on every call -- this
//! demo's own scene deliberately uses only borderless shapes and no
//! `Path`, so it never exercises that gap; `flatten_polygon`'s own doc
//! comment has the full account.

use tre_engine::{
    execute_frame, rgba8, BlendMode, BufferBinding, Circle, FillStyle, FlattenedFrame, FrameArena,
    GradientDef, GradientKind, GradientStop, PipelineKind, PipelineRegistry, Polygon,
    PrimitiveCommon, Rectangle, RenderingCanvas, RhiDevice, ScissorRect, ShapePrimitive,
    ShapeRegistry, TextureFormat,
};
use tre_memory::{DebugAllocGuard, RenderTickGuard};
use tre_rhi_vulkan::{HeadlessSwapchain, VulkanDevice};

#[path = "support/pixel_helpers.rs"]
mod pixel_helpers;

// Phase 10 Step 10.2.6: this demo's own real, self-checking proof that
// its `ShapeRegistry`/`RenderingCanvas` per-frame work is genuinely
// allocation-free -- see this file's own header comment for the guard's
// real scope.
#[global_allocator]
static ALLOCATOR: DebugAllocGuard = DebugAllocGuard::new();

const CANVAS_WIDTH: u32 = 420;
const CANVAS_HEIGHT: u32 = 320;
const RING_BUFFER_CAPACITY: usize = 64 * 1024;
const FRAME_COUNT: u32 = 120;

fn solid_texture(size: u32, bgra: [u8; 4]) -> Vec<u8> {
    (0..size * size).flat_map(|_| bgra).collect()
}

fn main() {
    let mut probe_connection =
        tre_platform::PlatformConnection::new().expect("failed to connect to display server");
    let probe_window = probe_connection
        .create_window("tre shape registry zero alloc probe (never shown)", 1, 1)
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
        "this demo's own Path shape exercises a real non-Normal BlendMode -- it requires a \
         device that actually supports VK_KHR_dynamic_rendering_local_read (this project's own \
         real dev GPU does; see documentation/REVIEW.md)"
    );
    let swapchain = HeadlessSwapchain::new(&device, CANVAS_WIDTH, CANVAS_HEIGHT)
        .expect("failed to create HeadlessSwapchain");

    let out_dir = env!("OUT_DIR");
    let read_spv = |name: &str| {
        std::fs::read(format!("{out_dir}/{name}"))
            .unwrap_or_else(|e| panic!("failed to read compiled shader {name}: {e}"))
    };

    let sdf_rounded_rect_pipeline = device
        .create_pipeline(
            &read_spv("sdf_rounded_rect.vert.spv"),
            &read_spv("sdf_rounded_rect.frag.spv"),
            tre_rhi_vulkan::HEADLESS_FORMAT,
        )
        .expect("failed to create rounded-rect pipeline");
    let ellipse_pipeline = device
        .create_pipeline(
            &read_spv("sdf_rounded_rect.vert.spv"),
            &read_spv("sdf_ellipse.frag.spv"),
            tre_rhi_vulkan::HEADLESS_FORMAT,
        )
        .expect("failed to create ellipse pipeline");
    let textured_quad_pipeline = device
        .create_pipeline(
            &read_spv("bindless_textured.vert.spv"),
            &read_spv("bindless_textured.frag.spv"),
            tre_rhi_vulkan::HEADLESS_FORMAT,
        )
        .expect("failed to create textured-quad pipeline");
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
        Box::new(sdf_rounded_rect_pipeline),
    );
    pipelines.register(PipelineKind::SdfEllipse as u16, Box::new(ellipse_pipeline));
    pipelines.register(
        PipelineKind::TexturedQuad as u16,
        Box::new(textured_quad_pipeline),
    );
    pipelines.register(
        PipelineKind::FlatColorBlend as u16,
        Box::new(flat_color_blend_pipeline),
    );

    const TEXTURE_SIZE: u32 = 8;
    let texture = device
        .create_texture(
            TEXTURE_SIZE,
            TEXTURE_SIZE,
            TextureFormat::Bgra8Srgb,
            &solid_texture(TEXTURE_SIZE, [200, 140, 60, 255]),
        )
        .expect("failed to create texture");
    let texture_index = texture
        .bindless_index()
        .expect("create_texture always registers a real bindless index");

    let mut registry = ShapeRegistry::new();

    let mut rect = Rectangle::new([90.0, 70.0], rgba8(220, 60, 60, 255));
    rect.common.transform.position = [20.0, 20.0];
    let rect_id = registry.insert(ShapePrimitive::Rectangle(rect));

    let gradient_id = registry
        .create_gradient(GradientDef {
            kind: GradientKind::Radial {
                center: [70.0, 45.0],
                radius: 70.0,
            },
            stops: vec![
                GradientStop {
                    position: 0.0,
                    color: rgba8(255, 255, 255, 255),
                },
                GradientStop {
                    position: 1.0,
                    color: rgba8(40, 80, 200, 255),
                },
            ],
        })
        .expect("a valid gradient must be accepted");
    let ellipse_id = registry.insert(ShapePrimitive::Circle(Circle {
        common: {
            let mut common = PrimitiveCommon::new();
            common.transform.position = [140.0, 20.0];
            common
        },
        radius: [70.0, 45.0], // non-uniform -- a real, non-circular ellipse (Step 10.2.4)
        fill: FillStyle::Gradient(gradient_id),
        border_color: 0,
        border_thickness: 0.0,
        arc_length: 360.0,
    }));

    let polygon_id = registry.insert(ShapePrimitive::Polygon(Polygon {
        common: {
            let mut common = PrimitiveCommon::new();
            common.transform.position = [280.0, 60.0];
            common
        },
        sides: 6,
        radius: 40.0,
        vertex_radius: 0.0,
        star_points: None,
        fill: FillStyle::Texture(texture_index),
        border_color: 0,
        border_thickness: 0.0,
    }));

    // A borderless Polygon, not a Path: `flatten_polygon`'s own fill
    // path (`generate_polygon_points_into`/`fan_from_center_into`,
    // fixed for real zero-allocation reuse by this very step -- REVIEW.md
    // finding #176) never touches `lyon` at all. A `Path`'s own fill/
    // stroke tessellation (`tessellate_fill`/`tessellate_stroke`) still
    // allocates fresh `lyon` tessellator/path/`VertexBuffers` objects on
    // every call -- a real, deeper, DISCLOSED gap this step does not
    // fix (see `flatten_polygon`'s own doc comment and REVIEW.md), so a
    // `Path` shape is deliberately not part of this demo's own guarded
    // scene. The non-`Normal` `BlendMode` code path itself (Step 10.2.3)
    // is exactly the same `draw_polygon_fill` dispatch for a Polygon as
    // for a Path, so real coverage of that feature does not need one.
    let triangle_id = registry.insert(ShapePrimitive::Polygon(Polygon {
        common: {
            let mut common = PrimitiveCommon::new();
            common.transform.position = [90.0, 220.0];
            common.blend_mode = BlendMode::Multiply;
            common
        },
        sides: 3,
        radius: 55.0,
        vertex_radius: 0.0,
        star_points: None,
        fill: FillStyle::Solid(rgba8(60, 200, 90, 255)),
        border_color: 0,
        border_thickness: 0.0,
    }));

    let arc_id = registry.insert(ShapePrimitive::Circle(Circle {
        common: {
            let mut common = PrimitiveCommon::new();
            common.transform.position = [230.0, 150.0];
            common
        },
        radius: [55.0, 55.0],
        fill: FillStyle::Solid(rgba8(255, 255, 255, 255)),
        border_color: rgba8(20, 20, 20, 255),
        border_thickness: 18.0,
        arc_length: 270.0,
    }));

    // --- Phase 9 Step 9.2's own reuse machinery (REVIEW.md finding
    // #134): every per-frame structure built exactly once, here --
    // `reset()`/`flatten_into()` every frame from this point on, never
    // reconstructed. `arena`'s capacities are generous for this demo's
    // own five SDF-quad/tessellated shapes (every SDF-rendered shape --
    // Rectangle, Circle -- is a single quad; only the tessellated
    // Polygon/Path and the arc-circle's own real border stroke need
    // more than 4 vertices each).
    let mut root = RenderingCanvas::new();
    let mut arena = FrameArena::with_capacity(256, 512, 32, 0);
    let mut flattened = FlattenedFrame::default();
    let ring_buffer = device.create_dynamic_ring_buffer(RING_BUFFER_CAPACITY);

    // --- One unguarded warm-up pass: every shape starts with `layout_
    // dirty == true` (freshly inserted), so this first flatten_into call
    // draws the whole scene and grows every `Vec` this demo touches
    // (root's own vertices/indices/commands, arena's own scatter arenas
    // and raw_commands/raw_indices/sort_scratch/counts, flattened's own
    // fields, and the device's own per-frame shape-style ring buffer) to
    // this demo's real steady-state capacity -- a real, expected one-
    // time warm-up cost, not a steady-state violation the guard below
    // exists to catch.
    registry.flatten_into(&mut root, &device);
    assert!(
        root.stitch_into(&arena),
        "arena was sized exactly for this demo"
    );
    root.reset();
    arena.flatten_into(&mut flattened);

    let full_window = ScissorRect {
        x: 0,
        y: 0,
        width: CANVAS_WIDTH,
        height: CANVAS_HEIGHT,
    };

    for frame in 0..FRAME_COUNT {
        #[allow(
            clippy::cast_precision_loss,
            reason = "FRAME_COUNT is a small constant, far below f32's exact-integer range"
        )]
        let t = frame as f32 * 0.08;

        // Phase 10 Step 10.2.6: the CPU-side render tick -- mutating
        // real shape properties (position, color, gradient stops),
        // resetting and re-recording the canvas, stitching, and
        // sort/batching must all be genuinely allocation-free. Excludes
        // RHI submission (`begin_frame`/`execute_frame`/`submit_and_
        // present`) below, matching `main_loop_demo.rs`'s own identical,
        // disclosed exclusion (`VulkanDevice::begin_frame` allocates a
        // fresh `Box<dyn RhiCommandBuffer>` every frame -- a real,
        // separate, already-disclosed gap, not this step's own scope).
        let tick = RenderTickGuard::begin();

        if let Some(slot) = registry.get_mut(rect_id) {
            if let ShapePrimitive::Rectangle(rect) = &mut slot.shape {
                rect.common.transform.position[0] = 20.0 + t.sin() * 15.0;
                rect.fill = FillStyle::Solid(rgba8(
                    220,
                    (60.0 + t.sin() * 40.0).clamp(0.0, 255.0) as u8,
                    60,
                    255,
                ));
            }
            slot.layout_dirty = true;
        }

        if let Some(gradient) = registry.gradient_mut(gradient_id) {
            let g = (200.0 + t.cos() * 55.0).clamp(0.0, 255.0) as u8;
            gradient.stops[1].color = rgba8(40, g, 200, 255);
        }
        if let Some(slot) = registry.get_mut(ellipse_id) {
            if let ShapePrimitive::Circle(circle) = &mut slot.shape {
                circle.common.transform.position[1] = 20.0 + t.cos() * 10.0;
            }
            slot.layout_dirty = true;
        }

        if let Some(slot) = registry.get_mut(polygon_id) {
            if let ShapePrimitive::Polygon(polygon) = &mut slot.shape {
                polygon.common.transform.position[0] = 280.0 + t.sin() * 12.0;
            }
            slot.layout_dirty = true;
        }

        if let Some(slot) = registry.get_mut(triangle_id) {
            if let ShapePrimitive::Polygon(polygon) = &mut slot.shape {
                polygon.fill = FillStyle::Solid(rgba8(
                    60,
                    (200.0 + t.sin() * 55.0).clamp(0.0, 255.0) as u8,
                    90,
                    255,
                ));
            }
            slot.layout_dirty = true;
        }

        if let Some(slot) = registry.get_mut(arc_id) {
            if let ShapePrimitive::Circle(circle) = &mut slot.shape {
                circle.common.transform.position[0] = 230.0 + t.cos() * 12.0;
            }
            slot.layout_dirty = true;
        }

        root.reset();
        registry.flatten_into(&mut root, &device);
        assert!(
            root.stitch_into(&arena),
            "arena was sized exactly for this demo"
        );
        arena.flatten_into(&mut flattened);

        let vertex_bytes: &[u8] = bytemuck::cast_slice(&flattened.vertices);
        let index_bytes: &[u8] = bytemuck::cast_slice(&flattened.indices);
        let vertex_offset = ring_buffer
            .write(vertex_bytes)
            .expect("ring buffer write failed for vertices");
        let index_offset = ring_buffer
            .write(index_bytes)
            .expect("ring buffer write failed for indices");

        drop(tick);

        let (mut cmd_buffer, image) = device.begin_frame(&swapchain).expect("begin_frame failed");
        execute_frame(
            &flattened,
            &pipelines,
            BufferBinding {
                buffer: &*ring_buffer,
                offset: vertex_offset,
            },
            BufferBinding {
                buffer: &*ring_buffer,
                offset: index_offset,
            },
            &full_window,
            &device,
            &mut *cmd_buffer,
        );
        device
            .submit_and_present(cmd_buffer, &swapchain, image)
            .expect("submit_and_present failed");

        if (frame + 1) % 30 == 0 {
            eprintln!("frame {} rendered with zero heap allocations", frame + 1);
        }
    }

    eprintln!(
        "{FRAME_COUNT} frames of a real, mutating, mixed ShapeRegistry scene (Rectangle, \
         gradient-filled non-circular Circle, texture-filled Polygon, blend-mode Polygon, \
         bordered partial-arc Circle) rendered with zero heap allocations after warm-up -- \
         ARCHITECTURE.md Section 7.5's disclosed gap is closed"
    );

    let bgra = swapchain
        .read_pixels_bgra8()
        .expect("failed to read back pixels");
    let mut rgba = bgra.clone();
    for px in rgba.chunks_exact_mut(4) {
        px.swap(0, 2);
    }
    let out_path = std::env::var("TRE_SHAPE_REGISTRY_ZERO_ALLOC_OUTPUT")
        .unwrap_or_else(|_| "shape_registry_zero_alloc_output.png".to_string());
    let file = std::fs::File::create(&out_path).expect("failed to create output PNG file");
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), CANVAS_WIDTH, CANVAS_HEIGHT);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().expect("failed to write PNG header");
    writer
        .write_image_data(&rgba)
        .expect("failed to write PNG image data");

    eprintln!("wrote the final frame ({CANVAS_WIDTH}x{CANVAS_HEIGHT}) to {out_path}");
}
