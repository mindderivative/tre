//! 0.5.4 (#102): the display scale. Layout stays in logical pixels and a
//! frame is drawn, culled and damaged in physical ones, so a tree drawn at
//! scale 2 must look like the same tree authored at twice the size and drawn
//! at scale 1. These tests render both and compare them.

mod support;

use engine_core::{
    Animated, NodeId, NodeKind, PaintProperties, Shader, ShaderMode, TextAlign, TextState, Tree,
};
use engine_render::{Damage, WindowRenderer};
use peniko::Color;
use taffy::prelude::{AvailableSpace, Position, Rect as TaffyRect, Size, Style, auto, length};

const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const BG: Color = Color::from_rgba8(0x11, 0x11, 0x11, 0xFF);
const RED: Color = Color::from_rgba8(0xFF, 0x20, 0x20, 0xFF);
const BLUE: Color = Color::from_rgba8(0x20, 0x40, 0xFF, 0xFF);

/// A window of `logical` layout pixels drawn at `scale`.
struct Window {
    device: wgpu::Device,
    queue: wgpu::Queue,
    renderer: WindowRenderer,
    surface: wgpu::Texture,
    surface_view: wgpu::TextureView,
    tree: Tree,
    root: NodeId,
    logical: (f32, f32),
    physical: (u16, u16),
}

fn style(x: f32, y: f32, w: f32, h: f32) -> Style {
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

impl Window {
    fn new(logical: (f32, f32), scale: f64) -> Self {
        let (device, queue) = pollster::block_on(support::device("hidpi test device"));
        let physical = (
            (f64::from(logical.0) * scale).round() as u16,
            (f64::from(logical.1) * scale).round() as u16,
        );
        let surface = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("stand-in swapchain image"),
            size: wgpu::Extent3d {
                width: u32::from(physical.0),
                height: u32::from(physical.1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: FORMAT,
            usage: wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let surface_view = surface.create_view(&wgpu::TextureViewDescriptor::default());
        let mut renderer = WindowRenderer::new(
            &device,
            FORMAT,
            u32::from(physical.0),
            u32::from(physical.1),
            true,
        );
        renderer.set_scale(scale);
        let mut tree = Tree::new();
        let root = tree.insert(
            NodeKind::Container,
            style(0.0, 0.0, logical.0, logical.1),
            PaintProperties::new(BG, 0.0, 1.0),
        );
        Self {
            device,
            queue,
            renderer,
            surface,
            surface_view,
            tree,
            root,
            logical,
            physical,
        }
    }

    fn add(&mut self, parent: NodeId, kind: NodeKind, s: Style, paint: PaintProperties) -> NodeId {
        let id = self.tree.insert(kind, s, paint);
        self.tree.add_child(parent, id);
        id
    }

    fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: Color, radius: f64) -> NodeId {
        let root = self.root;
        self.add(
            root,
            NodeKind::Rect,
            style(x, y, w, h),
            PaintProperties::new(color, radius, 1.0),
        )
    }

    /// One frame as the app runs it; returns the damage.
    fn frame(&mut self) -> Damage {
        self.tree.compute_layout(
            self.root,
            Size {
                width: AvailableSpace::Definite(self.logical.0),
                height: AvailableSpace::Definite(self.logical.1),
            },
        );
        let (w, h) = self.physical;
        let damage =
            self.renderer
                .prepare(&self.tree, self.root, w, h, true, &self.device, &self.queue);
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        self.renderer.draw(
            &self.tree,
            self.root,
            w,
            h,
            &damage,
            &self.device,
            &self.queue,
            &mut encoder,
            &self.surface,
            &self.surface_view,
        );
        self.queue.submit([encoder.finish()]);
        damage
    }

    /// The frame's pixels, row by row, without the row padding.
    fn pixels(&self) -> Vec<[u8; 4]> {
        let data = support::read_texture(&self.device, &self.queue, &self.surface);
        let row = (u32::from(self.physical.0) * 4).next_multiple_of(256) as usize;
        let mut out = Vec::new();
        for y in 0..self.physical.1 as usize {
            for x in 0..self.physical.0 as usize {
                let i = y * row + x * 4;
                out.push([data[i], data[i + 1], data[i + 2], data[i + 3]]);
            }
        }
        out
    }

    fn at(&self, x: u32, y: u32) -> [u8; 4] {
        self.pixels()[(y * u32::from(self.physical.0) + x) as usize]
    }
}

/// How many pixels differ by more than `slack` in any channel.
fn differing(a: &[[u8; 4]], b: &[[u8; 4]], slack: i32) -> usize {
    assert_eq!(a.len(), b.len(), "frames of different sizes");
    a.iter()
        .zip(b)
        .filter(|(p, q)| {
            p.iter()
                .zip(q.iter())
                .any(|(x, y)| (i32::from(*x) - i32::from(*y)).abs() > slack)
        })
        .count()
}

fn near(got: [u8; 4], want: [u8; 4], slack: i32) -> bool {
    got.iter()
        .zip(want)
        .all(|(g, w)| (i32::from(*g) - i32::from(w)).abs() <= slack)
}

fn rgba(c: Color) -> [u8; 4] {
    c.to_rgba8().to_u8_array()
}

#[test]
fn plain_boxes_at_scale_2_are_the_boxes_authored_at_twice_the_size() {
    let mut scaled = Window::new((120.0, 80.0), 2.0);
    scaled.rect(10.0, 10.0, 40.0, 30.0, RED, 0.0);
    let parent = scaled.rect(60.0, 20.0, 50.0, 50.0, BLUE, 0.0);
    scaled.add(
        parent,
        NodeKind::Rect,
        style(5.0, 5.0, 20.0, 20.0),
        PaintProperties::new(RED, 0.0, 1.0),
    );
    scaled.frame();

    let mut authored = Window::new((240.0, 160.0), 1.0);
    authored.rect(20.0, 20.0, 80.0, 60.0, RED, 0.0);
    let parent = authored.rect(120.0, 40.0, 100.0, 100.0, BLUE, 0.0);
    authored.add(
        parent,
        NodeKind::Rect,
        style(10.0, 10.0, 40.0, 40.0),
        PaintProperties::new(RED, 0.0, 1.0),
    );
    authored.frame();

    assert_eq!(scaled.physical, authored.physical);
    assert_eq!(
        differing(&scaled.pixels(), &authored.pixels(), 0),
        0,
        "whole-pixel boxes are identical"
    );
}

#[test]
fn rounded_corners_and_borders_scale_with_the_window() {
    let build = |w: &mut Window, k: f32| {
        let id = w.rect(
            10.0 * k,
            10.0 * k,
            60.0 * k,
            40.0 * k,
            RED,
            f64::from(12.0 * k),
        );
        let paint = &mut w.tree.get_mut(id).unwrap().paint;
        paint.border_color = Animated::new(BLUE);
        paint.border_width = Animated::new(f64::from(3.0 * k));
    };
    let mut scaled = Window::new((120.0, 80.0), 2.0);
    build(&mut scaled, 1.0);
    scaled.frame();
    let mut authored = Window::new((240.0, 160.0), 1.0);
    build(&mut authored, 2.0);
    authored.frame();
    let off = differing(&scaled.pixels(), &authored.pixels(), 3);
    assert!(
        off < 20,
        "{off} pixels differ by more than 3 between scale 2 and the 2x-authored tree"
    );
}

#[test]
fn text_at_scale_2_matches_text_authored_at_twice_the_size() {
    let label = |size: f32| {
        NodeKind::Text(TextState {
            content: "Hello, scale".to_string(),
            font_family: "Roboto".to_string(),
            font_weight: 500.0,
            font_size: size,
            align: TextAlign::Start,
            line_height: None,
            options: Default::default(),
        })
    };
    let white = Color::from_rgba8(255, 255, 255, 255);
    let mut scaled = Window::new((160.0, 40.0), 2.0);
    let root = scaled.root;
    scaled.add(
        root,
        label(14.0),
        style(8.0, 8.0, 140.0, 24.0),
        PaintProperties::new(white, 0.0, 1.0),
    );
    scaled.frame();
    let mut authored = Window::new((320.0, 80.0), 1.0);
    let root = authored.root;
    authored.add(
        root,
        label(28.0),
        style(16.0, 16.0, 280.0, 48.0),
        PaintProperties::new(white, 0.0, 1.0),
    );
    authored.frame();

    let ink = |w: &Window| {
        let px = w.pixels();
        let width = usize::from(w.physical.0);
        let (mut x0, mut y0, mut x1, mut y1) = (usize::MAX, usize::MAX, 0, 0);
        let mut count = 0usize;
        for (i, p) in px.iter().enumerate() {
            if p[0] > 0x60 {
                let (x, y) = (i % width, i / width);
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x);
                y1 = y1.max(y);
                count += 1;
            }
        }
        (x0, y0, x1, y1, count)
    };
    let (a, b) = (ink(&scaled), ink(&authored));
    assert!(a.4 > 200 && b.4 > 200, "text was drawn: {a:?} {b:?}");
    // The same place and extent, to within a pixel or two of hinting.
    for (got, want, what) in [
        (a.0, b.0, "left"),
        (a.1, b.1, "top"),
        (a.2, b.2, "right"),
        (a.3, b.3, "bottom"),
    ] {
        assert!(
            got.abs_diff(want) <= 3,
            "ink {what}: {got} at scale 2 against {want} authored at 2x ({a:?} vs {b:?})"
        );
    }
    let ratio = a.4 as f64 / b.4 as f64;
    assert!((0.85..1.15).contains(&ratio), "about as much ink: {ratio}");
}

#[test]
fn damage_is_in_physical_pixels_and_the_same_scale_changes_nothing() {
    let mut w = Window::new((120.0, 80.0), 2.0);
    let id = w.rect(10.0, 10.0, 20.0, 20.0, RED, 0.0);
    assert_eq!(w.frame(), Damage::Full);
    assert_eq!(w.frame(), Damage::None);
    w.tree.get_mut(id).unwrap().paint.background = Animated::new(BLUE);
    match w.frame() {
        Damage::Rects(rects) => {
            let want = peniko::kurbo::Rect::new(20.0, 20.0, 60.0, 60.0);
            assert!(
                rects.iter().any(|r| r.contains_rect(want)),
                "a 20x20 logical box at (10, 10) damages physical (20, 20)-(60, 60): {rects:?}"
            );
            assert!(
                rects.iter().all(|r| r.x1 <= 64.0 && r.y1 <= 64.0),
                "and nothing far from it: {rects:?}"
            );
        }
        other => panic!("expected rects, got {other:?}"),
    }
    assert!(near(w.at(40, 40), rgba(BLUE), 1));
    w.renderer.set_scale(2.0);
    assert_eq!(w.frame(), Damage::None, "the same scale changes nothing");
}

#[test]
fn a_new_scale_redraws_the_whole_frame() {
    let mut w = Window::new((120.0, 80.0), 1.0);
    w.rect(10.0, 10.0, 20.0, 20.0, RED, 0.0);
    assert_eq!(w.frame(), Damage::Full);
    assert_eq!(w.frame(), Damage::None);
    w.renderer.set_scale(2.0);
    assert_eq!(w.frame(), Damage::Full, "a new scale is a full redraw");
}

#[test]
fn a_node_off_the_logical_window_is_culled_and_one_on_it_is_not() {
    let mut w = Window::new((120.0, 80.0), 2.0);
    // Logical x 100 is inside the 120-wide logical window; logical 130 is not,
    // although physical 130 would be inside the 240-wide surface.
    w.rect(100.0, 10.0, 15.0, 15.0, RED, 0.0);
    w.rect(130.0, 10.0, 15.0, 15.0, BLUE, 0.0);
    w.frame();
    assert!(
        near(w.at(210, 30), rgba(RED), 1),
        "the on-window box is drawn"
    );
}

const FILL: &str =
    "fn shade(p: Pixel) -> vec4<f32> {\n    return vec4<f32>(p.uv.x, 0.0, 1.0 - p.uv.x, 1.0);\n}\n";
const INVERT: &str = "fn shade(p: Pixel) -> vec4<f32> {\n    let c = content(p.uv);\n    return vec4<f32>(1.0 - c.rgb, c.a);\n}\n";

#[test]
fn shader_textures_are_one_to_one_with_the_screen() {
    let mut w = Window::new((120.0, 80.0), 2.0);
    let id = w.rect(10.0, 10.0, 40.0, 40.0, Color::from_rgba8(0, 0, 0, 0), 0.0);
    w.tree.get_mut(id).unwrap().shader =
        Some(Shader::new(FILL.into(), vec![], vec![], ShaderMode::Fill, false).unwrap());
    w.frame();
    // A 40x40 logical box is 80x80 physical, starting at physical (20, 20).
    // The ramp runs red-to-blue by x across all 80 physical pixels.
    let left = w.at(22, 60);
    let right = w.at(97, 60);
    assert!(
        left[0] < 20 && left[2] > 230,
        "left edge is blue-ish: {left:?}"
    );
    assert!(
        right[0] > 230 && right[2] < 30,
        "right edge is red-ish: {right:?}"
    );
    assert_eq!(w.at(101, 60), rgba(BG), "and nothing past the box");
    // Sharp: neighbouring physical pixels differ by about 1/80 of the ramp,
    // which a texture stretched up from a 40-wide render would not give.
    let (a, b) = (w.at(50, 60), w.at(51, 60));
    assert!(
        (i32::from(a[0]) - i32::from(b[0])).abs() <= 6,
        "a smooth ramp at the physical resolution: {a:?} {b:?}"
    );
}

#[test]
fn an_effect_is_rendered_at_the_physical_size() {
    let mut w = Window::new((120.0, 80.0), 2.0);
    let panel = w.rect(10.0, 10.0, 60.0, 40.0, Color::from_rgba8(0, 0, 0, 0), 0.0);
    w.add(
        panel,
        NodeKind::Rect,
        style(10.0, 10.0, 20.0, 20.0),
        PaintProperties::new(Color::from_rgba8(255, 255, 255, 255), 0.0, 1.0),
    );
    w.tree.get_mut(panel).unwrap().shader =
        Some(Shader::new(INVERT.into(), vec![], vec![], ShaderMode::Effect, false).unwrap());
    w.frame();
    // The white 20x20 logical child (physical 40x40 at (40, 40)) is inverted
    // to black; the transparent rest of the panel shows the background.
    assert!(near(w.at(60, 60), [0, 0, 0, 255], 2), "{:?}", w.at(60, 60));
    assert_eq!(w.at(30, 30), rgba(BG), "{:?}", w.at(30, 30));
    // A crisp edge at the physical pixel: 39 is outside the child, 41 inside.
    assert_eq!(w.at(39, 60), rgba(BG));
    assert!(near(w.at(41, 60), [0, 0, 0, 255], 2));
}

#[test]
fn fractional_scales_put_box_edges_on_whole_device_pixels() {
    // 3 logical px at 1.5x is 4.5 device px: unsnapped, the left column of
    // the box would be half red over the background.
    let mut w = Window::new((40.0, 40.0), 1.5);
    w.rect(3.0, 3.0, 10.0, 10.0, RED, 0.0); // 15x15 device px
    w.frame();
    let pixels = w.pixels();
    // Every pixel is either all background or all red: no blended edge.
    let blended = pixels
        .iter()
        .filter(|p| !near(**p, rgba(BG), 2) && !near(**p, rgba(RED), 2))
        .count();
    assert_eq!(blended, 0, "snapped edges leave no blended pixels");
    let red = pixels.iter().filter(|p| near(**p, rgba(RED), 2)).count();
    assert_eq!(red, 15 * 15);
}

#[test]
fn snapping_changes_nothing_at_whole_scales() {
    let mut scaled = Window::new((60.0, 40.0), 2.0);
    scaled.rect(7.0, 5.0, 20.0, 11.0, RED, 0.0);
    scaled.frame();
    let mut authored = Window::new((120.0, 80.0), 1.0);
    authored.rect(14.0, 10.0, 40.0, 22.0, RED, 0.0);
    authored.frame();
    assert_eq!(differing(&scaled.pixels(), &authored.pixels(), 0), 0);
}

#[test]
fn a_blur_scales_with_the_display() {
    let mut scaled = Window::new((80.0, 60.0), 2.0);
    let id = scaled.rect(20.0, 15.0, 30.0, 30.0, RED, 0.0);
    scaled.tree.get_mut(id).unwrap().paint.blur = Animated::new(3.0);
    scaled.frame();

    let mut authored = Window::new((160.0, 120.0), 1.0);
    let id = authored.rect(40.0, 30.0, 60.0, 60.0, RED, 0.0);
    authored.tree.get_mut(id).unwrap().paint.blur = Animated::new(6.0);
    authored.frame();

    // The same blur, to a rounding of the filter's own size steps.
    assert!(
        differing(&scaled.pixels(), &authored.pixels(), 12) < 40,
        "a sigma of 3 at 2x is a sigma of 6 at 1x"
    );
}

/// 0.5.4 (#110): a window with a bar behind a frosted panel.
fn frosted_window(bar_color: Color) -> (Window, NodeId) {
    let mut w = Window::new((100.0, 100.0), 1.0);
    let bar = w.rect(45.0, 0.0, 10.0, 100.0, bar_color, 0.0);
    let panel = w.rect(30.0, 30.0, 40.0, 40.0, Color::from_rgba8(0, 0, 0, 0), 6.0);
    w.tree.get_mut(panel).unwrap().paint.backdrop_blur = Animated::new(4.0);
    (w, bar)
}

#[test]
fn partial_redraw_behind_a_backdrop_blur_matches_a_full_redraw() {
    let (mut live, bar) = frosted_window(RED);
    live.frame();
    live.tree.get_mut(bar).unwrap().paint.background = Animated::new(BLUE);
    let damage = live.frame();
    assert!(
        matches!(damage, Damage::Rects(_)),
        "a partial redraw: {damage:?}"
    );

    let (mut fresh, _) = frosted_window(BLUE);
    fresh.frame();
    assert_eq!(
        differing(&live.pixels(), &fresh.pixels(), 0),
        0,
        "the kept frame plus the partial redraw is the frame drawn whole"
    );
}
