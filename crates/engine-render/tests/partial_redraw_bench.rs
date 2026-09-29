//! 0.4.0 M5 Step 3: what partial redraw saves, measured. A 1920x1080
//! window of 576 rounded cards, timed frame by frame through the whole
//! pipeline -- tick, layout, damage, scene, render, copy to the stand-in
//! swapchain image, submit -- and then waiting for the GPU to finish, since
//! partial redraw saves GPU work above all. Two workloads:
//!
//! - **small**: one 48x48 card animates its fill over the static grid, the
//!   case partial redraw is for (a spinner, a caret, a progress bar);
//! - **full**: every card animates, so the damage is the whole window and
//!   partial redraw must fall back without costing anything.
//!
//! Each runs with partial redraw on and off. The same workloads on
//! `v0.3.5.1` (which always redraws in full, straight into the image) were
//! measured with a port of this file in a worktree; `BUILD_TRACKER.md` M5
//! has both sets of figures.
//!
//! `#[ignore]`d, run explicitly with `--release`:
//! `cargo test -p engine-render --test partial_redraw_bench --release -- --ignored --nocapture`.

use std::time::{Duration, Instant};

use engine_core::{MotionCurve, NodeId, NodeKind, PaintProperties, Tree};
use engine_render::{
    Damage, DamageTracker, FrameRenderer, GeometryCache, PersistentTarget, TextRenderer,
    build_tree_scene, build_tree_scene_in,
};
use peniko::Color;
use taffy::prelude::{
    AvailableSpace, FlexWrap, Position, Rect as TaffyRect, Size, Style, auto, length,
};
use vello_gpu::{RenderSize, RenderTargetConfig};

mod support;

const WIDTH: u16 = 1920;
const HEIGHT: u16 = 1080;
const CELL: f32 = 60.0;
const WARMUP: usize = 5;
const TIMED: usize = 60;

/// The grid, plus one card placed over it; with `all_animate`, every grid
/// card animates too.
fn build(start: Instant, all_animate: bool) -> (Tree, NodeId) {
    let mut tree = Tree::new();
    let root = tree.insert(
        NodeKind::Rect,
        Style {
            display: taffy::Display::Flex,
            flex_wrap: FlexWrap::Wrap,
            size: Size {
                width: length(f32::from(WIDTH)),
                height: length(f32::from(HEIGHT)),
            },
            ..Default::default()
        },
        PaintProperties::new(Color::from_rgba8(0x1C, 0x1B, 0x1F, 0xFF), 0.0, 1.0),
    );
    let cells = (f32::from(WIDTH) / CELL) as u32 * (f32::from(HEIGHT) / CELL) as u32;
    for _ in 0..cells {
        let mut paint = PaintProperties::new(Color::from_rgba8(0x67, 0x50, 0xA4, 0xFF), 8.0, 1.0);
        if all_animate {
            paint.background.animate_to(
                Color::from_rgba8(0x03, 0xDA, 0xC6, 0xFF),
                Duration::from_secs(60),
                MotionCurve::Linear,
                start,
            );
        }
        let card = tree.insert(
            NodeKind::Rect,
            Style {
                size: Size {
                    width: length(CELL - 6.0),
                    height: length(CELL - 6.0),
                },
                margin: TaffyRect {
                    left: length(3.0),
                    right: length(3.0),
                    top: length(3.0),
                    bottom: length(3.0),
                },
                ..Default::default()
            },
            paint,
        );
        tree.add_child(root, card);
    }
    let mut paint = PaintProperties::new(Color::from_rgba8(0xFF, 0xB0, 0x40, 0xFF), 24.0, 1.0);
    paint.background.animate_to(
        Color::from_rgba8(0x40, 0xC0, 0xFF, 0xFF),
        Duration::from_secs(60),
        MotionCurve::Linear,
        start,
    );
    let spinner = tree.insert(
        NodeKind::Rect,
        Style {
            position: Position::Absolute,
            inset: TaffyRect {
                left: length(900.0),
                top: length(500.0),
                right: auto(),
                bottom: auto(),
            },
            size: Size {
                width: length(48.0),
                height: length(48.0),
            },
            ..Default::default()
        },
        paint,
    );
    tree.add_child(root, spinner);
    (tree, root)
}

struct Timings {
    median_ms: f64,
    p90_ms: f64,
    partial_frames: usize,
}

fn run(device: &wgpu::Device, queue: &wgpu::Queue, all_animate: bool, partial: bool) -> Timings {
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let image = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("stand-in swapchain image"),
        size: wgpu::Extent3d {
            width: u32::from(WIDTH),
            height: u32::from(HEIGHT),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let target = PersistentTarget::new(device, format, u32::from(WIDTH), u32::from(HEIGHT));
    let mut renderer = FrameRenderer::new(
        device,
        &RenderTargetConfig {
            format,
            width: WIDTH,
            height: HEIGHT,
        },
    );
    let mut text = TextRenderer::new();
    let mut geometry = GeometryCache::new();
    let mut tracker = DamageTracker::new();
    let render_size = RenderSize {
        width: WIDTH,
        height: HEIGHT,
    };
    let space = Size {
        width: AvailableSpace::Definite(f32::from(WIDTH)),
        height: AvailableSpace::Definite(f32::from(HEIGHT)),
    };

    let (mut tree, root) = build(Instant::now(), all_animate);
    let mut samples = Vec::with_capacity(TIMED);
    let mut partial_frames = 0;
    for i in 0..WARMUP + TIMED {
        let begin = Instant::now();
        tree.tick_all(Instant::now());
        tree.compute_layout(root, space);
        let damage = tracker.damage(&tree, root, WIDTH, HEIGHT, &mut text);
        let damage = if partial { damage } else { Damage::Full };
        let mut encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        let rects = match &damage {
            Damage::Rects(rects) => Some(rects.as_slice()),
            _ => None,
        };
        let scene = match &damage {
            Damage::None => None,
            Damage::Full => Some(build_tree_scene(
                &tree,
                root,
                WIDTH,
                HEIGHT,
                renderer.resources_mut(),
                &mut text,
                &mut geometry,
            )),
            Damage::Rects(rects) => Some(build_tree_scene_in(
                &tree,
                root,
                WIDTH,
                HEIGHT,
                rects,
                renderer.resources_mut(),
                &mut text,
                &mut geometry,
            )),
        };
        if let Some(scene) = &scene {
            renderer.render_into(
                scene,
                device,
                queue,
                &mut encoder,
                &render_size,
                target.view(),
                rects,
            );
        }
        target.copy_to(&mut encoder, &image);
        queue.submit([encoder.finish()]);
        device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("device poll failed");
        if i >= WARMUP {
            samples.push(begin.elapsed().as_secs_f64() * 1000.0);
            if matches!(damage, Damage::Rects(_)) {
                partial_frames += 1;
            }
        }
    }
    samples.sort_by(f64::total_cmp);
    Timings {
        median_ms: samples[samples.len() / 2],
        p90_ms: samples[samples.len() * 9 / 10],
        partial_frames,
    }
}

#[test]
#[ignore = "perf benchmark -- run with: cargo test -p engine-render --test partial_redraw_bench \
            --release -- --ignored --nocapture"]
fn partial_redraw_frame_times() {
    let (device, queue) = pollster::block_on(support::device("partial redraw bench device"));
    let info = device.adapter_info();
    println!(
        "adapter: {} ({:?}, {:?}), {WIDTH}x{HEIGHT}, {TIMED} timed frames",
        info.name, info.device_type, info.backend
    );
    for (workload, all_animate) in [("small", false), ("full", true)] {
        for partial in [true, false] {
            let t = run(&device, &queue, all_animate, partial);
            println!(
                "{workload:5} partial={partial:5}: median {:6.3} ms, p90 {:6.3} ms, {} partial frames",
                t.median_ms, t.p90_ms, t.partial_frames
            );
            if partial && !all_animate {
                assert_eq!(
                    t.partial_frames, TIMED,
                    "the small workload redraws partially"
                );
            }
            if all_animate {
                assert_eq!(t.partial_frames, 0, "a whole-window change redraws in full");
            }
        }
    }
}
