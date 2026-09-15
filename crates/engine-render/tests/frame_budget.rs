//! The frame-time CI benchmark §6 Locked Decisions promises no later than
//! this build-order step (§14 step 3): "a stated-but-unenforced number is
//! exactly TRE's MSRV mistake restated in a new form" -- the 16.6ms
//! (60Hz)/8.3ms (120Hz) frame budget stated in ARCHITECTURE.md §6 was
//! fiction until a real render+layout+tick pipeline existed to measure
//! it against. This is that pipeline's first measurement.
//!
//! Builds a `Tree` of 300 nodes (20x15 grid, each 20x20) wrapped by
//! `taffy`'s own flex-wrap layout -- enough nodes for the timing to mean
//! something, not a token single-node smoke check -- gives every node an
//! active color animation so `tick_all` does real interpolation work
//! every iteration (not a fast no-op), and times the actual steady-state
//! per-frame sequence: tick -> compute_layout -> build_tree_scene ->
//! encode -> submit. Device/adapter/texture setup and one warm-up
//! iteration (first-use GPU pipeline compilation inside `vello_hybrid`
//! is a real but one-time cost, not a per-frame one) happen before the
//! timed loop starts.
//!
//! 16.6ms is the enforced target -- this test fails if the median
//! exceeds it. 8.3ms is reported, not enforced (§6 states it as a
//! "stretch goal", not a hard requirement).
//!
//! **`#[ignore]`d, run explicitly with `--release`:** a debug build's
//! unoptimized codegen (no inlining, bounds checks everywhere, no
//! vectorization) measured this exact pipeline at ~36ms median for 300
//! nodes -- a real finding, not a fluke of this one run, and not this
//! test's fault: nobody ships a debug build, and §6's budget is a claim
//! about the product's real runtime performance, which only an
//! optimized build represents. The release build measures ~0.6ms for
//! the same 300 nodes, comfortably inside both budgets. Rather than
//! shrink the node count until a debug build happens to pass -- which
//! would produce a number meaning nothing about real performance, the
//! same mistake as a benchmark tuned to its own outcome -- this test is
//! `#[ignore]`d from the default `cargo test --workspace` run (which
//! stays debug-mode and fast) and must be run explicitly:
//! `cargo test -p engine-render --test frame_budget --release -- --ignored --nocapture`.
//! This is the command CI's frame-budget job runs.

use std::time::{Duration, Instant};

use engine_core::{MotionCurve, NodeKind, PaintProperties, Tree};
use engine_render::{FrameRenderer, build_tree_scene};
use peniko::Color;
use taffy::prelude::{AvailableSpace, FlexWrap, Size, Style, length};
use vello_hybrid::{RenderSize, RenderTargetConfig};

const GRID_COLS: u32 = 20;
const GRID_ROWS: u32 = 15;
const CELL: f32 = 20.0;
const WIDTH: u16 = (GRID_COLS as u16) * (CELL as u16);
const HEIGHT: u16 = (GRID_ROWS as u16) * (CELL as u16);

const FRAME_BUDGET_MS: f64 = 16.6;
const STRETCH_BUDGET_MS: f64 = 8.3;
const WARMUP_ITERATIONS: usize = 3;
const TIMED_ITERATIONS: usize = 30;

fn build_grid_tree(start: Instant) -> (Tree, engine_core::NodeId) {
    let mut tree = Tree::new();
    let root = tree.insert(
        NodeKind::Container,
        Style {
            display: taffy::Display::Flex,
            flex_wrap: FlexWrap::Wrap,
            size: Size {
                width: length(f32::from(WIDTH)),
                height: length(f32::from(HEIGHT)),
            },
            ..Default::default()
        },
        PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 0.0, 1.0),
    );

    for _ in 0..(GRID_COLS * GRID_ROWS) {
        let mut paint =
            PaintProperties::new(Color::from_rgba8(0x67, 0x50, 0xA4, 0xFF), 4.0, 0.0, 1.0);
        // Every node has a genuinely active animation, several seconds
        // long, so every timed iteration's tick_all does real per-node
        // interpolation work rather than an early-out no-op check.
        paint.background.animate_to(
            Color::from_rgba8(0x03, 0xDA, 0xC6, 0xFF),
            Duration::from_secs(10),
            MotionCurve::Linear,
            start,
        );
        let child = tree.insert(
            NodeKind::Rect,
            Style {
                size: Size {
                    width: length(CELL),
                    height: length(CELL),
                },
                ..Default::default()
            },
            paint,
        );
        tree.add_child(root, child);
    }

    (tree, root)
}

#[test]
#[ignore = "perf benchmark -- meaningless in a debug build, run with: \
            cargo test -p engine-render --test frame_budget --release -- --ignored --nocapture"]
fn frame_pipeline_fits_the_16_6ms_budget() {
    pollster::block_on(async {
        let instance = wgpu::Instance::default();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                force_fallback_adapter: false,
                compatible_surface: None,
            })
            .await
            .expect("no wgpu adapter available in this environment");
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("engine-render frame-budget test device"),
                required_features: wgpu::Features::empty(),
                ..Default::default()
            })
            .await
            .expect("failed to create wgpu device");

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("frame-budget test target"),
            size: wgpu::Extent3d {
                width: u32::from(WIDTH),
                height: u32::from(HEIGHT),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut frame_renderer = FrameRenderer::new(
            &device,
            &RenderTargetConfig {
                format: texture.format(),
                width: u32::from(WIDTH),
                height: u32::from(HEIGHT),
            },
        );
        let render_size = RenderSize {
            width: u32::from(WIDTH),
            height: u32::from(HEIGHT),
        };

        let start = Instant::now();
        let (mut tree, root) = build_grid_tree(start);
        let available_space = Size {
            width: AvailableSpace::Definite(f32::from(WIDTH)),
            height: AvailableSpace::Definite(f32::from(HEIGHT)),
        };

        let mut run_one_frame = |tree: &mut Tree| {
            let now = Instant::now();
            tree.tick_all(now);
            tree.compute_layout(root, available_space);
            let scene = build_tree_scene(tree, root, WIDTH, HEIGHT);
            let mut encoder =
                device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
            frame_renderer.render(&scene, &device, &queue, &mut encoder, &render_size, &view);
            queue.submit([encoder.finish()]);
        };

        // Warm-up: first real use of vello_hybrid's renderer compiles
        // GPU pipelines lazily -- a real, one-time cost that would
        // otherwise dominate iteration 0's timing and make the budget
        // check measure pipeline compilation, not steady-state frame
        // cost.
        for _ in 0..WARMUP_ITERATIONS {
            run_one_frame(&mut tree);
        }

        let mut samples_ms = Vec::with_capacity(TIMED_ITERATIONS);
        for _ in 0..TIMED_ITERATIONS {
            let frame_start = Instant::now();
            run_one_frame(&mut tree);
            samples_ms.push(frame_start.elapsed().as_secs_f64() * 1000.0);
        }

        samples_ms.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let median_ms = samples_ms[samples_ms.len() / 2];
        let max_ms = *samples_ms.last().unwrap();

        eprintln!(
            "engine-render §14 step 3 frame budget: {} nodes, median {:.3}ms, max {:.3}ms \
             (60Hz target {FRAME_BUDGET_MS}ms, 120Hz stretch {STRETCH_BUDGET_MS}ms)",
            GRID_COLS * GRID_ROWS,
            median_ms,
            max_ms,
        );
        if median_ms > STRETCH_BUDGET_MS {
            eprintln!(
                "engine-render §14 step 3: median {median_ms:.3}ms misses the 120Hz stretch \
                 goal ({STRETCH_BUDGET_MS}ms) -- not a failure, informational per §6"
            );
        }

        assert!(
            median_ms < FRAME_BUDGET_MS,
            "median frame time {median_ms:.3}ms exceeds the enforced 16.6ms/60Hz budget \
             (§6 Locked Decisions) across {TIMED_ITERATIONS} iterations, samples: {samples_ms:?}"
        );
    });
}
