//! 0.5.4 (#107): what building a frame's scene costs per node, split into the
//! parts. `cargo test -p engine-render --release --test scene_cost -- --ignored --nocapture`

mod support;

use std::time::{Duration, Instant};

use engine_core::{NodeId, NodeKind, PaintProperties, Tree};
use engine_render::{FrameRenderer, GeometryCache, TextRenderer, build_tree_scene};
use peniko::Color;
use taffy::prelude::{AvailableSpace, FlexWrap, Size, Style, length};
use vello_gpu::{RenderSize, RenderTargetConfig};

const W: u16 = 960;
const H: u16 = 480;
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// A window of `cell`-sized cards; `tweak` adjusts each card's paint.
fn grid(cell: f32, tweak: impl Fn(&mut PaintProperties)) -> (Tree, NodeId, usize) {
    let mut tree = Tree::new();
    let root = tree.insert(
        NodeKind::Container,
        Style {
            flex_wrap: FlexWrap::Wrap,
            size: Size {
                width: length(f32::from(W)),
                height: length(f32::from(H)),
            },
            ..Default::default()
        },
        PaintProperties::new(Color::from_rgba8(0x11, 0x11, 0x11, 255), 0.0, 1.0),
    );
    let count = (f32::from(W) / cell) as usize * (f32::from(H) / cell) as usize;
    for _ in 0..count {
        let mut paint = PaintProperties::new(Color::from_rgba8(0x44, 0x55, 0x99, 255), 6.0, 1.0);
        tweak(&mut paint);
        let id = tree.insert(
            NodeKind::Rect,
            Style {
                size: Size {
                    width: length(cell - 4.0),
                    height: length(cell - 4.0),
                },
                margin: taffy::Rect {
                    left: length(2.0),
                    right: length(2.0),
                    top: length(2.0),
                    bottom: length(2.0),
                },
                ..Default::default()
            },
            paint,
        );
        tree.add_child(root, id);
    }
    tree.compute_layout(
        root,
        Size {
            width: AvailableSpace::Definite(f32::from(W)),
            height: AvailableSpace::Definite(f32::from(H)),
        },
    );
    (tree, root, count)
}

fn measure(name: &str, tweak: impl Fn(&mut PaintProperties)) {
    let (tree, root, count) = grid(20.0, tweak);
    time_tree(name, tree, root, count);
}

/// 1,200 single-line labels in rows.
fn labels() -> (Tree, NodeId, usize) {
    let mut tree = Tree::new();
    let root = tree.insert(
        NodeKind::Container,
        Style {
            flex_wrap: FlexWrap::Wrap,
            size: Size {
                width: length(f32::from(W)),
                height: length(f32::from(H)),
            },
            ..Default::default()
        },
        PaintProperties::new(Color::from_rgba8(0x11, 0x11, 0x11, 255), 0.0, 1.0),
    );
    for i in 0..1200 {
        let id = tree.insert(
            NodeKind::Text(engine_core::TextState {
                content: format!("List item number {i} with some words"),
                font_family: "Roboto".to_string(),
                font_weight: 400.0,
                font_size: 14.0,
                align: engine_core::TextAlign::Start,
                line_height: None,
                options: Default::default(),
            }),
            Style {
                size: Size {
                    width: length(120.0),
                    height: length(20.0),
                },
                ..Default::default()
            },
            PaintProperties::new(Color::from_rgba8(0xE6, 0xE1, 0xE5, 255), 0.0, 1.0),
        );
        tree.add_child(root, id);
    }
    tree.compute_layout(
        root,
        Size {
            width: AvailableSpace::Definite(f32::from(W)),
            height: AvailableSpace::Definite(f32::from(H)),
        },
    );
    (tree, root, 1200)
}

fn time_tree(name: &str, tree: Tree, root: NodeId, count: usize) {
    let (device, queue) = pollster::block_on(support::device("scene cost"));
    let mut renderer = FrameRenderer::new(
        &device,
        &RenderTargetConfig {
            format: FORMAT,
            width: W,
            height: H,
        },
    );
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: u32::from(W),
            height: u32::from(H),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::STORAGE_BINDING,
        view_formats: &[],
    });
    let view = target.create_view(&wgpu::TextureViewDescriptor::default());
    let mut text = TextRenderer::new();
    let mut geometry = GeometryCache::new();
    let runs = 30;
    let (mut build, mut encode) = (Duration::ZERO, Duration::ZERO);
    for i in 0..runs + 3 {
        let t0 = Instant::now();
        let scene = build_tree_scene(
            &tree,
            root,
            W,
            H,
            renderer.resources_mut(),
            &mut text,
            &mut geometry,
        );
        let t1 = Instant::now();
        let mut encoder = device.create_command_encoder(&Default::default());
        renderer.render_into(
            &scene,
            &device,
            &queue,
            &mut encoder,
            &RenderSize {
                width: W,
                height: H,
            },
            &view,
            None,
        );
        queue.submit([encoder.finish()]);
        let t2 = Instant::now();
        if i >= 3 {
            build += t1 - t0;
            encode += t2 - t1;
        }
    }
    let (b, e) = (build / runs, encode / runs);
    println!(
        "{name:28} {count} nodes  build {:>7.1?} ({:>4.0} ns/node)  render_into+submit {:>7.1?} ({:>4.0} ns/node)",
        b,
        b.as_nanos() as f64 / count as f64,
        e,
        e.as_nanos() as f64 / count as f64
    );
}

#[test]
#[ignore = "timing, not correctness"]
fn scene_cost() {
    let (t, r, n) = labels();
    time_tree("1,200 text labels", t, r, n);
    measure("rounded cards", |_| {});
    measure("square cards (radius 0)", |p| p.corner_radius.current = 0.0);
    measure("square cards, 1px border", |p| {
        p.corner_radius.current = 0.0;
        p.border_width.current = 1.0;
        p.border_color.current = Color::from_rgba8(255, 255, 255, 255);
    });
    measure("cards with 1px border", |p| {
        p.border_width.current = 1.0;
        p.border_color.current = Color::from_rgba8(255, 255, 255, 255);
    });
    measure("cards at opacity 0.9", |p| p.opacity.current = 0.9);
}

#[test]
#[ignore = "timing, not correctness"]
fn scene_allocation() {
    use vello_gpu::Scene;
    let runs = 200u32;
    let start = Instant::now();
    for _ in 0..runs {
        std::hint::black_box(Scene::new(1920, 1080));
    }
    println!("Scene::new(1920x1080): {:?}", start.elapsed() / runs);
}
