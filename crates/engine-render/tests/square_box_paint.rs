//! 0.5.4 (#107): a square box is filled with `fill_rect` and its border drawn
//! as four rects, because vello does that far faster than a path fill and a
//! stroke. They must cover the same pixels the path fill and the stroke did.

mod support;

use engine_core::{NodeKind, PaintProperties, Tree};
use engine_render::{FrameRenderer, GeometryCache, TextRenderer, build_tree_scene};
use peniko::Color;
use peniko::kurbo::{Affine, Rect, Stroke};
use taffy::prelude::{AvailableSpace, Position, Rect as TaffyRect, Size, Style, auto, length};
use vello_gpu::{RenderSize, RenderTargetConfig, Scene};

const S: u16 = 64;
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const RED: Color = Color::from_rgba8(255, 0, 0, 255);
const WHITE: Color = Color::from_rgba8(255, 255, 255, 255);
const BG: Color = Color::from_rgba8(0, 0, 0, 255);

fn render(device: &wgpu::Device, queue: &wgpu::Queue, scene: &Scene) -> Vec<u8> {
    let mut renderer = FrameRenderer::new(
        device,
        &RenderTargetConfig {
            format: FORMAT,
            width: S,
            height: S,
        },
    );
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: u32::from(S),
            height: u32::from(S),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = target.create_view(&wgpu::TextureViewDescriptor::default());
    let mut encoder = device.create_command_encoder(&Default::default());
    renderer.render_into(
        scene,
        device,
        queue,
        &mut encoder,
        &RenderSize {
            width: S,
            height: S,
        },
        &view,
        None,
    );
    queue.submit([encoder.finish()]);
    support::read_texture(device, queue, &target)
}

/// What the engine draws for a square box of `w` x `h` at `x`, `y` with a
/// `border` wide border, against what the old fill-path-and-stroke drew.
fn compare(x: f32, y: f32, w: f32, h: f32, border: f64) {
    let (device, queue) = pollster::block_on(support::device("square box"));
    let mut tree = Tree::new();
    let root = tree.insert(
        NodeKind::Container,
        Style {
            size: Size {
                width: length(f32::from(S)),
                height: length(f32::from(S)),
            },
            ..Default::default()
        },
        PaintProperties::new(BG, 0.0, 1.0),
    );
    let mut paint = PaintProperties::new(RED, 0.0, 1.0);
    paint.border_width.current = border;
    paint.border_color.current = WHITE;
    let id = tree.insert(
        NodeKind::Rect,
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
        },
        paint,
    );
    tree.add_child(root, id);
    tree.compute_layout(
        root,
        Size {
            width: AvailableSpace::Definite(f32::from(S)),
            height: AvailableSpace::Definite(f32::from(S)),
        },
    );
    let mut renderer = FrameRenderer::new(
        &device,
        &RenderTargetConfig {
            format: FORMAT,
            width: S,
            height: S,
        },
    );
    let scene = build_tree_scene(
        &tree,
        root,
        S,
        S,
        renderer.resources_mut(),
        &mut TextRenderer::new(),
        &mut GeometryCache::new(),
    );
    let got = render(&device, &queue, &scene);

    // The old way: fill the rect as a path, stroke its border inset by half.
    let (lw, lh) = (
        f64::from(tree.layout(id).size.width),
        f64::from(tree.layout(id).size.height),
    );
    let mut old = Scene::new(S, S);
    old.set_transform(Affine::IDENTITY);
    old.set_paint(BG);
    old.fill_rect(&Rect::new(0.0, 0.0, f64::from(S), f64::from(S)));
    old.set_transform(Affine::translate((
        f64::from(tree.layout(id).location.x),
        f64::from(tree.layout(id).location.y),
    )));
    old.set_paint(RED);
    old.fill_path(&Rect::new(0.0, 0.0, lw, lh).to_path_for_test());
    if border > 0.0 {
        old.set_paint(WHITE);
        old.set_stroke(Stroke::new(border));
        let i = border / 2.0;
        old.stroke_path(&Rect::new(i, i, lw - i, lh - i).to_path_for_test());
    }
    let want = render(&device, &queue, &old);

    let worst = got
        .iter()
        .zip(&want)
        .map(|(a, b)| (i32::from(*a) - i32::from(*b)).abs())
        .max()
        .unwrap();
    assert!(
        worst <= 2,
        "{w}x{h} at {x},{y} border {border}: worst channel diff {worst}"
    );
}

trait ToPath {
    fn to_path_for_test(&self) -> peniko::kurbo::BezPath;
}
impl ToPath for Rect {
    fn to_path_for_test(&self) -> peniko::kurbo::BezPath {
        peniko::kurbo::Shape::to_path(self, 0.1)
    }
}

#[test]
fn a_square_box_fills_as_the_path_did() {
    compare(10.0, 10.0, 30.0, 20.0, 0.0);
    compare(7.0, 9.0, 21.0, 13.0, 0.0);
}

#[test]
fn a_square_border_covers_what_the_stroke_did() {
    compare(10.0, 10.0, 30.0, 20.0, 1.0);
    compare(7.0, 9.0, 21.0, 13.0, 1.0);
    compare(7.5, 9.25, 21.0, 13.0, 1.0);
}

#[test]
fn a_border_wider_than_half_the_box_still_matches() {
    compare(10.0, 10.0, 6.0, 4.0, 1.0);
}

/// Wider borders keep the stroke, whose round joins round the outer corners
/// by half the width; four rects would square them. Unchanged output.
#[test]
fn wide_borders_keep_the_strokes_look() {
    compare(10.0, 10.0, 30.0, 20.0, 3.0);
    compare(7.0, 9.0, 21.0, 13.0, 2.0);
}
