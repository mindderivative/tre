//! A baseline of what a frame costs, by stage, across the kinds of scenes an
//! app makes -- research for the issue "Research: baseline measurements and a
//! feature inventory". Not a pass/fail test: it prints a table.
//!
//! Each scenario builds a tree and runs the app's own frame sequence
//! (`WindowRenderer`: `prepare`, then `draw`, then a wait for the GPU) at
//! 1920x1080, timing each stage of every frame: `update` (what the app
//! changes), `tick` (animations), `layout`, `prepare` (images, cache
//! eviction, the damage walk), `draw` (scene building and encoding, on the
//! CPU), and `gpu` (submit and wait for completion).
//!
//! Run, in release, on the default adapter or a software one:
//!
//! ```text
//! cargo test -p engine-render --test baseline --release -- --ignored --nocapture
//! VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.x86_64.json \
//!     cargo test -p engine-render --test baseline --release -- --ignored --nocapture
//! BASELINE_FILTER=text cargo test ...   # only scenarios whose name contains "text"
//! ```

use std::time::{Duration, Instant};

use engine_core::{
    Animated, ImageState, MotionCurve, NodeId, NodeKind, PaintProperties, ScrollViewState, Shader,
    ShaderMode, Shadow, Shadows, TextAlign, TextState, Tree,
};
use engine_render::{Damage, WindowRenderer};
use peniko::Color;
use taffy::prelude::{
    AvailableSpace, FlexDirection, FlexWrap, Position, Rect as TaffyRect, Size, Style, auto, length,
};

mod support;

const W: u16 = 1920;
const H: u16 = 1080;
const WARMUP: usize = 8;
const TIMED: usize = 60;
const BG: Color = Color::from_rgba8(0x1C, 0x1B, 0x1F, 0xFF);
const CARD: Color = Color::from_rgba8(0x67, 0x50, 0xA4, 0xFF);

/// What a scenario holds between frames.
struct Scene {
    tree: Tree,
    root: NodeId,
    /// A node the scenario changes each frame.
    focus: Option<NodeId>,
}

type Update = fn(&mut Scene, usize, &mut WindowRenderer);

struct Scenario {
    name: &'static str,
    build: fn() -> Scene,
    update: Update,
    note: &'static str,
}

fn root_style() -> Style {
    Style {
        display: taffy::Display::Flex,
        flex_wrap: FlexWrap::Wrap,
        size: Size {
            width: length(f32::from(W)),
            height: length(f32::from(H)),
        },
        ..Default::default()
    }
}

fn margin(v: f32) -> TaffyRect<taffy::style::LengthPercentageAuto> {
    TaffyRect {
        left: length(v),
        right: length(v),
        top: length(v),
        bottom: length(v),
    }
}

fn cell_style(cell: f32) -> Style {
    Style {
        size: Size {
            width: length(cell - 4.0),
            height: length(cell - 4.0),
        },
        margin: margin(2.0),
        ..Default::default()
    }
}

/// A window of `cell`-sized cards, filling the window.
fn grid(cell: f32, animate_all: bool) -> Scene {
    let now = Instant::now();
    let mut tree = Tree::new();
    let root = tree.insert(
        NodeKind::Container,
        root_style(),
        PaintProperties::new(BG, 0.0, 1.0),
    );
    let count = (f32::from(W) / cell) as u32 * (f32::from(H) / cell) as u32;
    let mut first = None;
    for _ in 0..count {
        let mut paint = PaintProperties::new(CARD, 6.0, 1.0);
        if animate_all {
            paint.background.animate_to(
                Color::from_rgba8(0x03, 0xDA, 0xC6, 0xFF),
                Duration::from_secs(600),
                MotionCurve::Linear,
                now,
            );
        }
        let id = tree.insert(NodeKind::Rect, cell_style(cell), paint);
        tree.add_child(root, id);
        first.get_or_insert(id);
    }
    Scene {
        tree,
        root,
        focus: first,
    }
}

fn idle(_: &mut Scene, _: usize, _: &mut WindowRenderer) {}

/// One card's colour changes each frame (the app sets it, as a state change would).
fn one_card_changes(scene: &mut Scene, frame: usize, _: &mut WindowRenderer) {
    let id = scene.focus.unwrap();
    let t = (frame % 255) as u8;
    scene.tree.get_mut(id).unwrap().paint.background =
        Animated::new(Color::from_rgba8(t, 0x80, 0xFF - t, 0xFF));
}

fn grid_small() -> Scene {
    grid(60.0, false)
}
fn grid_medium() -> Scene {
    grid(30.0, false)
}
fn grid_large() -> Scene {
    grid(15.0, false)
}
fn grid_all_animating() -> Scene {
    grid(30.0, true)
}

fn label_style() -> Style {
    Style {
        size: Size {
            width: length(120.0),
            height: length(20.0),
        },
        margin: TaffyRect {
            left: length(4.0),
            right: length(4.0),
            top: length(2.0),
            bottom: length(2.0),
        },
        ..Default::default()
    }
}

fn label(content: String) -> NodeKind {
    NodeKind::Text(TextState {
        content,
        font_family: "Roboto".to_string(),
        font_weight: 400.0,
        font_size: 14.0,
        align: TextAlign::Start,
        line_height: None,
        options: Default::default(),
    })
}

/// 1,200 text labels; the first one's text changes every frame.
fn text_labels() -> Scene {
    let mut tree = Tree::new();
    let root = tree.insert(
        NodeKind::Container,
        root_style(),
        PaintProperties::new(BG, 0.0, 1.0),
    );
    let mut first = None;
    for i in 0..1200 {
        let id = tree.insert(
            label(format!("List item number {i} with some words")),
            label_style(),
            PaintProperties::new(Color::from_rgba8(0xE6, 0xE1, 0xE5, 0xFF), 0.0, 1.0),
        );
        tree.add_child(root, id);
        first.get_or_insert(id);
    }
    Scene {
        tree,
        root,
        focus: first,
    }
}

fn text_changes(scene: &mut Scene, frame: usize, _: &mut WindowRenderer) {
    let id = scene.focus.unwrap();
    if let NodeKind::Text(state) = &mut scene.tree.get_mut(id).unwrap().kind {
        state.content = format!("Changing label, frame {frame}");
    }
}

/// A scroll view over 3,000 rows, scrolled a few pixels every frame.
fn scroll_list() -> Scene {
    let mut tree = Tree::new();
    let root = tree.insert(
        NodeKind::Container,
        Style {
            size: Size {
                width: length(f32::from(W)),
                height: length(f32::from(H)),
            },
            ..Default::default()
        },
        PaintProperties::new(BG, 0.0, 1.0),
    );
    let view = tree.insert(
        NodeKind::ScrollView(ScrollViewState::new(false)),
        Style {
            size: Size {
                width: length(600.0),
                height: length(f32::from(H)),
            },
            ..Default::default()
        },
        PaintProperties::new(BG, 0.0, 1.0),
    );
    tree.add_child(root, view);
    let content = tree.insert(
        NodeKind::Container,
        Style {
            flex_direction: FlexDirection::Column,
            ..Default::default()
        },
        PaintProperties::new(BG, 0.0, 1.0),
    );
    tree.add_child(view, content);
    for i in 0..3000 {
        let colour = if i % 2 == 0 {
            CARD
        } else {
            Color::from_rgba8(0x4A, 0x44, 0x58, 0xFF)
        };
        let row = tree.insert(
            NodeKind::Rect,
            Style {
                size: Size {
                    width: length(560.0),
                    height: length(32.0),
                },
                margin: TaffyRect {
                    left: length(8.0),
                    right: length(8.0),
                    top: length(2.0),
                    bottom: length(2.0),
                },
                ..Default::default()
            },
            PaintProperties::new(colour, 4.0, 1.0),
        );
        tree.add_child(content, row);
    }
    Scene {
        tree,
        root,
        focus: Some(view),
    }
}

fn scroll_step(scene: &mut Scene, frame: usize, _: &mut WindowRenderer) {
    let id = scene.focus.unwrap();
    if let NodeKind::ScrollView(state) = &mut scene.tree.get_mut(id).unwrap().kind {
        state.scroll = Animated::new((frame * 7) as f64);
    }
}

const FILL_WGSL: &str = "fn shade(p: Pixel) -> vec4<f32> {\n    let t = frame.time;\n    return vec4<f32>(0.5 + 0.5 * sin(p.uv.x * 6.0 + t), 0.5 + 0.5 * cos(p.uv.y * 6.0 - t), 0.6, 1.0);\n}\n";
const FX_WGSL: &str = "fn shade(p: Pixel) -> vec4<f32> {\n    let c = content(p.uv);\n    return vec4<f32>(c.rgb * (0.8 + 0.2 * sin(frame.time + p.uv.x * 8.0)), c.a);\n}\n";

fn absolute_style(x: f32, y: f32, w: f32, h: f32) -> Style {
    Style {
        position: Position::Absolute,
        inset: TaffyRect {
            left: length(x),
            top: length(y),
            right: auto(),
            bottom: auto(),
        },
        size: Size {
            width: length(w),
            height: length(h),
        },
        ..Default::default()
    }
}

fn shader_fill() -> Scene {
    let mut tree = Tree::new();
    let root = tree.insert(
        NodeKind::Container,
        root_style(),
        PaintProperties::new(BG, 0.0, 1.0),
    );
    let id = tree.insert(
        NodeKind::Rect,
        absolute_style(100.0, 100.0, 800.0, 600.0),
        PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 24.0, 1.0),
    );
    tree.add_child(root, id);
    tree.get_mut(id).unwrap().shader =
        Some(Shader::new(FILL_WGSL.into(), vec![], vec![], ShaderMode::Fill, true).expect("valid"));
    Scene {
        tree,
        root,
        focus: Some(id),
    }
}

/// An effect over a panel of 60 children, animated.
fn shader_effect() -> Scene {
    let mut tree = Tree::new();
    let root = tree.insert(
        NodeKind::Container,
        root_style(),
        PaintProperties::new(BG, 0.0, 1.0),
    );
    let panel = tree.insert(
        NodeKind::Rect,
        absolute_style(100.0, 100.0, 800.0, 600.0),
        PaintProperties::new(Color::from_rgba8(0x30, 0x2C, 0x38, 0xFF), 16.0, 1.0),
    );
    tree.add_child(root, panel);
    for i in 0..60 {
        let child = tree.insert(
            NodeKind::Rect,
            absolute_style(
                20.0 + (i % 10) as f32 * 76.0,
                20.0 + (i / 10) as f32 * 90.0,
                64.0,
                70.0,
            ),
            PaintProperties::new(CARD, 8.0, 1.0),
        );
        tree.add_child(panel, child);
    }
    tree.get_mut(panel).unwrap().shader =
        Some(Shader::new(FX_WGSL.into(), vec![], vec![], ShaderMode::Effect, true).expect("valid"));
    Scene {
        tree,
        root,
        focus: Some(panel),
    }
}

fn shader_tick(_: &mut Scene, frame: usize, renderer: &mut WindowRenderer) {
    renderer.set_time(frame as f32 / 60.0);
}

fn video_frame_data(seed: u8) -> peniko::ImageData {
    let (w, h) = (1280u32, 720u32);
    let mut bytes = vec![seed; (w * h * 4) as usize];
    for px in bytes.as_chunks_mut::<4>().0.iter_mut() {
        px[3] = 0xFF;
    }
    peniko::ImageData {
        data: peniko::Blob::from(bytes),
        format: peniko::ImageFormat::Rgba8,
        alpha_type: peniko::ImageAlphaType::Alpha,
        width: w,
        height: h,
    }
}

/// A 1280x720 video, a new frame every tick.
fn video() -> Scene {
    let mut tree = Tree::new();
    let root = tree.insert(
        NodeKind::Container,
        root_style(),
        PaintProperties::new(BG, 0.0, 1.0),
    );
    let id = tree.insert(
        NodeKind::Image(ImageState::new(video_frame_data(0))),
        absolute_style(100.0, 100.0, 1280.0, 720.0),
        PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 1.0),
    );
    tree.add_child(root, id);
    Scene {
        tree,
        root,
        focus: Some(id),
    }
}

fn video_frame(scene: &mut Scene, frame: usize, _: &mut WindowRenderer) {
    let id = scene.focus.unwrap();
    if let NodeKind::Image(state) = &mut scene.tree.get_mut(id).unwrap().kind {
        state.image = video_frame_data((frame % 250) as u8);
    }
}

/// 300 cards with a soft shadow each, one of them changing.
fn shadows() -> Scene {
    let mut scene = grid(80.0, false);
    let ids: Vec<NodeId> = scene.tree.get(scene.root).unwrap().children.clone();
    for id in ids {
        scene.tree.get_mut(id).unwrap().paint.shadows = Animated::new(Shadows(vec![Shadow {
            color: Color::from_rgba8(0, 0, 0, 90),
            offset_x: 0.0,
            offset_y: 4.0,
            blur: 12.0,
            spread: 0.0,
        }]));
    }
    scene
}

fn scenarios() -> Vec<Scenario> {
    vec![
        Scenario {
            name: "idle_576_nodes",
            build: grid_small,
            update: idle,
            note: "nothing changes: the damage walk and a copy",
        },
        Scenario {
            name: "idle_2304_nodes",
            build: grid_medium,
            update: idle,
            note: "same, 4x the nodes",
        },
        Scenario {
            name: "idle_9216_nodes",
            build: grid_large,
            update: idle,
            note: "same, 16x the nodes",
        },
        Scenario {
            name: "one_card_576",
            build: grid_small,
            update: one_card_changes,
            note: "one card's colour changes (partial redraw)",
        },
        Scenario {
            name: "one_card_2304",
            build: grid_medium,
            update: one_card_changes,
            note: "same, 2,304 nodes",
        },
        Scenario {
            name: "one_card_9216",
            build: grid_large,
            update: one_card_changes,
            note: "same, 9,216 nodes",
        },
        Scenario {
            name: "all_animating_2304",
            build: grid_all_animating,
            update: idle,
            note: "every card animates (full redraw)",
        },
        Scenario {
            name: "text_1200_idle",
            build: text_labels,
            update: idle,
            note: "1,200 labels, nothing changes",
        },
        Scenario {
            name: "text_1200_one_changes",
            build: text_labels,
            update: text_changes,
            note: "one label's text changes",
        },
        Scenario {
            name: "scroll_3000_rows",
            build: scroll_list,
            update: scroll_step,
            note: "scroll view, 3,000 rows, scrolled each frame",
        },
        Scenario {
            name: "shadows_300",
            build: shadows,
            update: one_card_changes,
            note: "300 cards with blur-12 shadows, one changes",
        },
        Scenario {
            name: "shader_fill_anim",
            build: shader_fill,
            update: shader_tick,
            note: "800x600 animated fill shader",
        },
        Scenario {
            name: "shader_effect_anim",
            build: shader_effect,
            update: shader_tick,
            note: "animated effect over 60 children",
        },
        Scenario {
            name: "video_720p",
            build: video,
            update: video_frame,
            note: "1280x720 RGBA frame replaced each frame",
        },
    ]
}

#[derive(Default)]
struct Stages {
    total: Vec<f64>,
    update: Vec<f64>,
    tick: Vec<f64>,
    layout: Vec<f64>,
    prepare: Vec<f64>,
    draw: Vec<f64>,
    gpu: Vec<f64>,
    partial: usize,
    none: usize,
    full: usize,
}

fn pct(samples: &[f64], p: f64) -> f64 {
    let mut s = samples.to_vec();
    s.sort_by(f64::total_cmp);
    s[((s.len() - 1) as f64 * p).round() as usize]
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

fn run(device: &wgpu::Device, queue: &wgpu::Queue, scenario: &Scenario) -> Stages {
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let image = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("baseline surface"),
        size: wgpu::Extent3d {
            width: u32::from(W),
            height: u32::from(H),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = image.create_view(&wgpu::TextureViewDescriptor::default());
    let mut renderer = WindowRenderer::new(device, format, u32::from(W), u32::from(H), true);
    let space = Size {
        width: AvailableSpace::Definite(f32::from(W)),
        height: AvailableSpace::Definite(f32::from(H)),
    };
    let mut scene = (scenario.build)();
    let mut out = Stages::default();
    for i in 0..WARMUP + TIMED {
        let begin = Instant::now();
        (scenario.update)(&mut scene, i, &mut renderer);
        let t_update = Instant::now();
        scene.tree.tick_all(Instant::now());
        let t_tick = Instant::now();
        scene.tree.compute_layout(scene.root, space);
        let t_layout = Instant::now();
        let damage = renderer.prepare(&scene.tree, scene.root, W, H, true, device, queue);
        let t_prepare = Instant::now();
        let mut encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        renderer.draw(
            &scene.tree,
            scene.root,
            W,
            H,
            &damage,
            device,
            queue,
            &mut encoder,
            &image,
            &view,
        );
        let t_draw = Instant::now();
        queue.submit([encoder.finish()]);
        device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("device poll failed");
        let end = Instant::now();
        if i >= WARMUP {
            out.total.push(ms(end - begin));
            out.update.push(ms(t_update - begin));
            out.tick.push(ms(t_tick - t_update));
            out.layout.push(ms(t_layout - t_tick));
            out.prepare.push(ms(t_prepare - t_layout));
            out.draw.push(ms(t_draw - t_prepare));
            out.gpu.push(ms(end - t_draw));
            match damage {
                Damage::None => out.none += 1,
                Damage::Rects(_) => out.partial += 1,
                Damage::Full => out.full += 1,
            }
        }
    }
    out
}

#[test]
#[ignore = "baseline measurements -- run with: cargo test -p engine-render --test baseline \
            --release -- --ignored --nocapture"]
fn frame_cost_by_stage() {
    let (device, queue) = pollster::block_on(support::device("baseline device"));
    let adapter = std::env::var("VK_ICD_FILENAMES").unwrap_or_else(|_| "default adapter".into());
    let filter = std::env::var("BASELINE_FILTER").unwrap_or_default();
    println!("\nadapter: {adapter}");
    println!(
        "{:<22} {:>7} {:>7} | {:>6} {:>6} {:>7} {:>8} {:>7} {:>7} | {:>4} {:>4} {:>4}  note",
        "scenario",
        "median",
        "p95",
        "update",
        "tick",
        "layout",
        "prepare",
        "draw",
        "gpu",
        "none",
        "part",
        "full"
    );
    for scenario in scenarios().iter().filter(|s| s.name.contains(&filter)) {
        let s = run(&device, &queue, scenario);
        let m = |v: &Vec<f64>| pct(v, 0.5);
        println!(
            "{:<22} {:>7.2} {:>7.2} | {:>6.2} {:>6.2} {:>7.2} {:>8.2} {:>7.2} {:>7.2} | {:>4} {:>4} {:>4}  {}",
            scenario.name,
            m(&s.total),
            pct(&s.total, 0.95),
            m(&s.update),
            m(&s.tick),
            m(&s.layout),
            m(&s.prepare),
            m(&s.draw),
            m(&s.gpu),
            s.none,
            s.partial,
            s.full,
            scenario.note
        );
    }
}

/// Resident memory, from `/proc/self/statm` (pages), in bytes; Linux only.
fn rss() -> usize {
    let statm = std::fs::read_to_string("/proc/self/statm").unwrap_or_default();
    statm
        .split_whitespace()
        .nth(1)
        .and_then(|p| p.parse::<usize>().ok())
        .unwrap_or(0)
        * 4096
}

#[test]
#[ignore = "baseline measurements -- run with: cargo test -p engine-render --test baseline \
            --release -- --ignored --nocapture"]
fn memory_per_node() {
    println!(
        "\nsize_of Node            = {} bytes",
        std::mem::size_of::<engine_core::Node>()
    );
    println!(
        "size_of NodeKind        = {} bytes",
        std::mem::size_of::<NodeKind>()
    );
    println!(
        "size_of Style (taffy)   = {} bytes",
        std::mem::size_of::<Style>()
    );
    println!(
        "size_of PaintProperties = {} bytes",
        std::mem::size_of::<PaintProperties>()
    );
    println!(
        "size_of Layout (taffy)  = {} bytes",
        std::mem::size_of::<taffy::Layout>()
    );
    for count in [10_000usize, 50_000] {
        let before = rss();
        let mut tree = Tree::new();
        let root = tree.insert(
            NodeKind::Container,
            root_style(),
            PaintProperties::new(BG, 0.0, 1.0),
        );
        for _ in 0..count {
            let id = tree.insert(
                NodeKind::Rect,
                cell_style(20.0),
                PaintProperties::new(CARD, 4.0, 1.0),
            );
            tree.add_child(root, id);
        }
        let built = rss();
        tree.compute_layout(
            root,
            Size {
                width: AvailableSpace::Definite(f32::from(W)),
                height: AvailableSpace::Definite(f32::from(H)),
            },
        );
        let laid_out = rss();
        println!(
            "{count:>6} rect nodes: {:>6.0} bytes/node built, {:>6.0} bytes/node after layout",
            (built.saturating_sub(before)) as f64 / count as f64,
            (laid_out.saturating_sub(before)) as f64 / count as f64
        );
        drop(tree);
    }
    let before = rss();
    let mut tree = Tree::new();
    let root = tree.insert(
        NodeKind::Container,
        root_style(),
        PaintProperties::new(BG, 0.0, 1.0),
    );
    for i in 0..10_000 {
        let id = tree.insert(
            label(format!("List item number {i} with some words")),
            label_style(),
            PaintProperties::new(CARD, 0.0, 1.0),
        );
        tree.add_child(root, id);
    }
    println!(
        "{:>6} text nodes: {:>6.0} bytes/node built (text content included)",
        10_000,
        rss().saturating_sub(before) as f64 / 10_000.0
    );
}
