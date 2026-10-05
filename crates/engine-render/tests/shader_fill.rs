//! 0.5.1 (#67): fill shaders through the app's own frame sequence
//! (`WindowRenderer`): a node with a `mode="fill"` shader paints its box from
//! the app's `shade`, clipped to its rounded corners; a pass runs only when
//! something changed, and only for a node that is drawn.

mod support;

use engine_core::{
    Animated, NodeId, NodeKind, PaintProperties, Shader, ShaderMode, Tree, UniformValue,
};
use engine_render::{Damage, WindowRenderer};
use peniko::Color;
use std::sync::Arc;
use taffy::prelude::{AvailableSpace, Position, Rect as TaffyRect, Size, Style, auto, length};

const W: u16 = 120;
const H: u16 = 100;
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const BACKGROUND: [u8; 4] = [0x11, 0x11, 0x11, 0xFF];

const TINT: &str =
    "fn shade(p: Pixel) -> vec4<f32> {\n    return vec4<f32>(u.tint.rgb, u.tint.a);\n}\n";
const UV: &str =
    "fn shade(p: Pixel) -> vec4<f32> {\n    return vec4<f32>(p.uv.x, p.uv.y, 0.0, 1.0);\n}\n";

fn tint(r: f32, g: f32, b: f32, a: f32) -> Arc<Shader> {
    Shader::new(
        TINT.to_owned(),
        vec![("tint".to_owned(), UniformValue::Vec4([r, g, b, a]))],
        vec![],
        ShaderMode::Fill,
        false,
    )
    .expect("a valid shader")
}

struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
}

struct Scene {
    gpu: Gpu,
    renderer: WindowRenderer,
    surface: wgpu::Texture,
    surface_view: wgpu::TextureView,
    tree: Tree,
    root: NodeId,
}

impl Scene {
    fn new() -> Self {
        let (device, queue) = pollster::block_on(support::device("shader fill test device"));
        let surface = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("stand-in swapchain image"),
            size: wgpu::Extent3d {
                width: u32::from(W),
                height: u32::from(H),
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
        let renderer = WindowRenderer::new(&device, FORMAT, u32::from(W), u32::from(H), true);
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
            PaintProperties::new(Color::from_rgba8(0x11, 0x11, 0x11, 0xFF), 0.0, 1.0),
        );
        Self {
            gpu: Gpu { device, queue },
            renderer,
            surface,
            surface_view,
            tree,
            root,
        }
    }

    /// A transparent box at (x, y), w x h, with `shader`.
    fn node(&mut self, x: f32, y: f32, w: f32, h: f32, shader: Arc<Shader>) -> NodeId {
        let id = self.boxed(x, y, w, h);
        self.tree.add_child(self.root, id);
        self.tree.get_mut(id).unwrap().shader = Some(shader);
        id
    }

    /// A transparent box at (x, y), w x h, in no parent yet.
    fn boxed(&mut self, x: f32, y: f32, w: f32, h: f32) -> NodeId {
        self.boxed_of(NodeKind::Rect, x, y, w, h)
    }

    /// `boxed`, of a given kind (a node's kind is set when it is made: the tree
    /// counts nodes by kind).
    fn boxed_of(&mut self, kind: NodeKind, x: f32, y: f32, w: f32, h: f32) -> NodeId {
        self.tree.insert(
            kind,
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
            PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 1.0),
        )
    }

    /// One frame, as the app runs it; returns the damage.
    fn frame(&mut self) -> Damage {
        self.tree.compute_layout(
            self.root,
            Size {
                width: AvailableSpace::Definite(f32::from(W)),
                height: AvailableSpace::Definite(f32::from(H)),
            },
        );
        let Gpu { device, queue } = &self.gpu;
        let damage = self
            .renderer
            .prepare(&self.tree, self.root, W, H, true, device, queue);
        let mut encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        self.renderer.draw(
            &self.tree,
            self.root,
            W,
            H,
            &damage,
            device,
            queue,
            &mut encoder,
            &self.surface,
            &self.surface_view,
        );
        queue.submit([encoder.finish()]);
        damage
    }

    fn at(&self, x: u32, y: u32) -> [u8; 4] {
        let data = support::read_texture(&self.gpu.device, &self.gpu.queue, &self.surface);
        let row = (u32::from(W) * 4).next_multiple_of(256);
        let i = (y * row + x * 4) as usize;
        [data[i], data[i + 1], data[i + 2], data[i + 3]]
    }

    fn passes(&self) -> usize {
        self.renderer.shader_pass_count()
    }
}

fn near(got: [u8; 4], want: [u8; 4], slack: i32) -> bool {
    got.iter()
        .zip(want)
        .all(|(g, w)| (i32::from(*g) - i32::from(w)).abs() <= slack)
}

#[test]
fn a_fill_shader_paints_its_box_and_nothing_else() {
    let mut s = Scene::new();
    s.node(20.0, 20.0, 40.0, 40.0, tint(1.0, 0.0, 0.0, 1.0));
    s.frame();
    assert_eq!(s.passes(), 1);
    assert!(
        near(s.at(40, 40), [255, 0, 0, 255], 1),
        "{:?}",
        s.at(40, 40)
    );
    assert_eq!(s.at(10, 10), BACKGROUND, "outside the box");
    assert_eq!(s.at(70, 40), BACKGROUND, "right of the box");
}

#[test]
fn the_shader_sees_uv_from_the_top_left() {
    let mut s = Scene::new();
    let shader = Shader::new(UV.to_owned(), vec![], vec![], ShaderMode::Fill, false).unwrap();
    s.node(0.0, 0.0, 100.0, 100.0, shader);
    s.frame();
    let (left, right) = (s.at(2, 50), s.at(97, 50));
    let (top, bottom) = (s.at(50, 2), s.at(50, 97));
    assert!(
        left[0] < 15 && right[0] > 240,
        "u grows to the right: {left:?} {right:?}"
    );
    assert!(
        top[1] < 15 && bottom[1] > 240,
        "v grows downward: {top:?} {bottom:?}"
    );
}

#[test]
fn a_translucent_result_blends_over_what_is_behind() {
    let mut s = Scene::new();
    // Straight alpha: half-transparent white over #111.
    s.node(20.0, 20.0, 40.0, 40.0, tint(1.0, 1.0, 1.0, 0.5));
    s.frame();
    let want = [(255 + 17) / 2, (255 + 17) / 2, (255 + 17) / 2, 255];
    assert!(
        near(
            s.at(40, 40),
            [want[0] as u8, want[1] as u8, want[2] as u8, 255],
            3
        ),
        "{:?}",
        s.at(40, 40)
    );
}

#[test]
fn a_new_uniform_value_redraws_and_an_idle_frame_runs_no_pass() {
    let mut s = Scene::new();
    let shader = tint(1.0, 0.0, 0.0, 1.0);
    s.node(20.0, 20.0, 40.0, 40.0, shader.clone());
    s.frame();
    assert_eq!(s.passes(), 1);
    assert_eq!(s.frame(), Damage::None, "nothing changed");
    assert_eq!(s.passes(), 0, "an idle frame runs no pass");

    shader
        .set_uniforms(vec![(
            "tint".to_owned(),
            UniformValue::Vec4([0.0, 0.0, 1.0, 1.0]),
        )])
        .unwrap();
    assert!(matches!(s.frame(), Damage::Rects(_)), "the node is damaged");
    assert_eq!(s.passes(), 1);
    assert!(
        near(s.at(40, 40), [0, 0, 255, 255], 1),
        "{:?}",
        s.at(40, 40)
    );
    assert_eq!(s.at(10, 10), BACKGROUND);
}

#[test]
fn the_shader_is_clipped_to_the_rounded_box() {
    let mut s = Scene::new();
    let id = s.node(20.0, 20.0, 60.0, 60.0, tint(0.0, 1.0, 0.0, 1.0));
    s.tree.get_mut(id).unwrap().paint.corner_radius = Animated::new(30.0);
    s.frame();
    assert!(
        near(s.at(50, 50), [0, 255, 0, 255], 1),
        "the centre is shaded"
    );
    assert_eq!(
        s.at(21, 21),
        BACKGROUND,
        "the corner is cut off by the rounding"
    );
}

#[test]
fn a_size_change_reallocates_the_texture_and_repaints() {
    let mut s = Scene::new();
    let id = s.node(10.0, 10.0, 30.0, 30.0, tint(1.0, 0.0, 0.0, 1.0));
    s.frame();
    assert_eq!(s.at(60, 25), BACKGROUND);
    let mut style = s.tree.get(id).unwrap().layout_style.clone();
    style.size.width = length(80.0);
    s.tree.set_layout_style(id, style);
    s.frame();
    assert_eq!(s.passes(), 1, "the new size runs the pass again");
    assert!(
        near(s.at(60, 25), [255, 0, 0, 255], 1),
        "{:?}",
        s.at(60, 25)
    );
}

#[test]
fn a_hidden_node_and_an_off_screen_node_run_no_pass() {
    let mut s = Scene::new();
    let hidden = s.node(10.0, 10.0, 30.0, 30.0, tint(1.0, 0.0, 0.0, 1.0));
    s.tree.get_mut(hidden).unwrap().visible = false;
    s.node(500.0, 500.0, 30.0, 30.0, tint(0.0, 0.0, 1.0, 1.0));
    s.frame();
    assert_eq!(s.passes(), 0, "neither is drawn, so neither runs");
    assert_eq!(s.at(20, 20), BACKGROUND);

    s.tree.get_mut(hidden).unwrap().visible = true;
    s.frame();
    assert_eq!(s.passes(), 1, "shown, it runs");
    assert!(near(s.at(20, 20), [255, 0, 0, 255], 1));
}

#[test]
fn clearing_the_shader_paints_the_box_as_before() {
    let mut s = Scene::new();
    let id = s.node(20.0, 20.0, 40.0, 40.0, tint(1.0, 0.0, 0.0, 1.0));
    s.frame();
    s.tree.get_mut(id).unwrap().shader = None;
    s.frame();
    assert_eq!(s.at(40, 40), BACKGROUND);
    assert_eq!(s.passes(), 0);
}

#[test]
fn a_shader_and_the_nodes_own_fill_draw_together_fill_on_top() {
    let mut s = Scene::new();
    let id = s.node(20.0, 20.0, 40.0, 40.0, tint(1.0, 0.0, 0.0, 1.0));
    // A half-transparent white fill tints the shader beneath it.
    s.tree.get_mut(id).unwrap().paint.background =
        Animated::new(Color::from_rgba8(255, 255, 255, 128));
    s.frame();
    let got = s.at(40, 40);
    assert!(got[0] > 250 && (120..140).contains(&got[1]), "{got:?}");
}

#[test]
fn a_node_too_large_for_a_texture_paints_without_its_shader() {
    let mut s = Scene::new();
    // Wider than any texture can be: logged once, painted as if it had none.
    let id = s.node(0.0, 0.0, 9000.0, 30.0, tint(1.0, 0.0, 0.0, 1.0));
    s.frame();
    assert_eq!(s.passes(), 0);
    assert_eq!(s.at(40, 10), BACKGROUND);
    // The frame loop is unharmed: a normal shader node still draws.
    s.tree.remove(id);
    s.node(20.0, 20.0, 40.0, 40.0, tint(1.0, 0.0, 0.0, 1.0));
    s.frame();
    assert!(near(s.at(40, 40), [255, 0, 0, 255], 1));
}

#[test]
fn removing_a_shader_node_frees_it_and_the_frame_still_draws() {
    let mut s = Scene::new();
    let a = s.node(10.0, 10.0, 30.0, 30.0, tint(1.0, 0.0, 0.0, 1.0));
    s.node(60.0, 10.0, 30.0, 30.0, tint(0.0, 0.0, 1.0, 1.0));
    s.frame();
    s.tree.remove(a);
    s.frame();
    assert_eq!(s.at(20, 20), BACKGROUND);
    assert!(near(s.at(70, 20), [0, 0, 255, 255], 1));
}

// --- 0.5.1 (#68): inputs --------------------------------------------------------

fn solid(rgba: [u8; 4]) -> peniko::ImageData {
    let mut bytes = Vec::new();
    for _ in 0..4 {
        bytes.extend_from_slice(&rgba);
    }
    peniko::ImageData {
        data: peniko::Blob::from(bytes),
        format: peniko::ImageFormat::Rgba8,
        alpha_type: peniko::ImageAlphaType::Alpha,
        width: 2,
        height: 2,
    }
}

impl Scene {
    /// An image node (a 2x2 solid colour) at (x, y), w x h.
    fn image(&mut self, x: f32, y: f32, w: f32, h: f32, rgba: [u8; 4]) -> NodeId {
        let id = self.boxed_of(
            NodeKind::Image(engine_core::ImageState::new(solid(rgba))),
            x,
            y,
            w,
            h,
        );
        self.tree.add_child(self.root, id);
        id
    }
}

fn with_inputs(wgsl: &str, inputs: Vec<(&str, NodeId)>) -> Arc<Shader> {
    Shader::new(
        wgsl.to_owned(),
        vec![],
        inputs
            .into_iter()
            .map(|(n, id)| (n.to_owned(), id))
            .collect(),
        ShaderMode::Fill,
        false,
    )
    .expect("a valid shader")
}

const INVERT: &str = "fn shade(p: Pixel) -> vec4<f32> {\n    let c = input_photo(p.uv);\n    return vec4<f32>(1.0 - c.rgb, c.a);\n}\n";

#[test]
fn a_shader_samples_an_image_node_and_follows_its_frames() {
    let mut s = Scene::new();
    let photo = s.image(0.0, 0.0, 20.0, 20.0, [255, 0, 0, 255]);
    s.node(
        40.0,
        40.0,
        40.0,
        40.0,
        with_inputs(INVERT, vec![("photo", photo)]),
    );
    s.frame();
    assert_eq!(s.passes(), 1);
    assert!(
        near(s.at(60, 60), [0, 255, 255, 255], 2),
        "red inverts to cyan: {:?}",
        s.at(60, 60)
    );
    assert!(
        near(s.at(10, 10), [255, 0, 0, 255], 1),
        "the image itself still draws"
    );

    assert_eq!(s.frame(), Damage::None, "idle");
    assert_eq!(s.passes(), 0);

    // A new frame of the image (what `push_frame` does).
    match &mut s.tree.get_mut(photo).unwrap().kind {
        NodeKind::Image(state) => state.image = solid([0, 0, 255, 255]),
        _ => unreachable!(),
    }
    let damage = s.frame();
    assert!(matches!(damage, Damage::Rects(_)), "{damage:?}");
    assert_eq!(s.passes(), 1, "a new frame reruns the shader");
    assert!(
        near(s.at(60, 60), [255, 255, 0, 255], 2),
        "blue inverts to yellow: {:?}",
        s.at(60, 60)
    );
}

const SWAP_RB: &str = "fn shade(p: Pixel) -> vec4<f32> {\n    let c = input_up(p.uv);\n    return vec4<f32>(c.b, c.g, c.r, c.a);\n}\n";

#[test]
fn a_shader_reads_another_shader_which_runs_first_even_off_screen() {
    let mut s = Scene::new();
    let upstream = tint(1.0, 0.0, 0.0, 1.0);
    // Off-screen: nothing draws it, but it is needed.
    let up = s.node(500.0, 500.0, 20.0, 20.0, upstream.clone());
    s.node(
        40.0,
        40.0,
        40.0,
        40.0,
        with_inputs(SWAP_RB, vec![("up", up)]),
    );
    s.frame();
    assert_eq!(s.passes(), 2, "the dependency, then the shader");
    assert!(
        near(s.at(60, 60), [0, 0, 255, 255], 2),
        "{:?}",
        s.at(60, 60)
    );

    upstream
        .set_uniforms(vec![(
            "tint".to_owned(),
            UniformValue::Vec4([0.0, 1.0, 0.0, 1.0]),
        )])
        .unwrap();
    let damage = s.frame();
    assert!(
        matches!(damage, Damage::Rects(_)),
        "a change upstream repaints: {damage:?}"
    );
    assert_eq!(s.passes(), 2);
    assert!(
        near(s.at(60, 60), [0, 255, 0, 255], 2),
        "{:?}",
        s.at(60, 60)
    );
}

#[test]
fn an_image_node_can_sample_its_own_pixels() {
    let mut s = Scene::new();
    let photo = s.image(20.0, 20.0, 40.0, 40.0, [255, 0, 0, 255]);
    let shader = with_inputs(INVERT, vec![("photo", photo)]);
    assert!(
        !s.tree.shader_cycle(photo, &shader),
        "an image input is its pixels, not a cycle"
    );
    s.tree.get_mut(photo).unwrap().shader = Some(shader);
    s.frame();
    // The shader paints behind the image, which covers it: the image wins.
    assert!(
        near(s.at(40, 40), [255, 0, 0, 255], 1),
        "{:?}",
        s.at(40, 40)
    );
    assert_eq!(s.passes(), 1);
}

#[test]
fn a_shader_cycle_is_found_and_drawing_one_does_not_hang() {
    let mut s = Scene::new();
    let a = s.node(10.0, 10.0, 30.0, 30.0, tint(1.0, 0.0, 0.0, 1.0));
    let b = s.node(60.0, 10.0, 30.0, 30.0, tint(0.0, 1.0, 0.0, 1.0));
    let src = "fn shade(p: Pixel) -> vec4<f32> {\n    return input_x(p.uv);\n}\n";
    let a_reads_b = with_inputs(src, vec![("x", b)]);
    let b_reads_a = with_inputs(src, vec![("x", a)]);
    assert!(!s.tree.shader_cycle(a, &a_reads_b));
    s.tree.get_mut(a).unwrap().shader = Some(a_reads_b);
    assert!(
        s.tree.shader_cycle(b, &b_reads_a),
        "b reading a, which reads b"
    );
    assert!(
        s.tree.shader_cycle(a, &with_inputs(src, vec![("x", a)])),
        "itself"
    );
    // Forced in anyway, the frame still draws: neither node gets its shader.
    s.tree.get_mut(b).unwrap().shader = Some(b_reads_a);
    s.frame();
    assert_eq!(s.at(20, 20), BACKGROUND);
    assert_eq!(s.at(70, 20), BACKGROUND);
}

#[test]
fn a_node_that_is_not_an_image_and_has_no_shader_is_no_input() {
    let mut s = Scene::new();
    let plain = s.node(10.0, 10.0, 30.0, 30.0, tint(1.0, 0.0, 0.0, 1.0));
    s.tree.get_mut(plain).unwrap().shader = None;
    s.node(
        60.0,
        10.0,
        30.0,
        30.0,
        with_inputs(INVERT, vec![("photo", plain)]),
    );
    s.frame();
    assert_eq!(s.at(70, 20), BACKGROUND, "painted as if it had no shader");
}

// --- 0.5.1 (#69): effects -------------------------------------------------------

const INVERT_CONTENT: &str = "fn shade(p: Pixel) -> vec4<f32> {\n    let c = content(p.uv);\n    return vec4<f32>(1.0 - c.rgb, c.a);\n}\n";
const PASS: &str = "fn shade(p: Pixel) -> vec4<f32> {\n    return content(p.uv);\n}\n";
const SWAP: &str = "fn shade(p: Pixel) -> vec4<f32> {\n    let c = content(p.uv);\n    return vec4<f32>(c.b, c.g, c.r, c.a);\n}\n";

fn effect(wgsl: &str) -> Arc<Shader> {
    Shader::new(wgsl.to_owned(), vec![], vec![], ShaderMode::Effect, false).expect("a valid effect")
}

impl Scene {
    /// A child box of `parent` at (x, y), w x h, painted `rgba`.
    fn child(&mut self, parent: NodeId, x: f32, y: f32, w: f32, h: f32, rgba: [u8; 4]) -> NodeId {
        let id = self.boxed(x, y, w, h);
        self.tree.add_child(parent, id);
        let node = self.tree.get_mut(id).unwrap();
        node.paint.background =
            Animated::new(Color::from_rgba8(rgba[0], rgba[1], rgba[2], rgba[3]));
        id
    }
}

#[test]
fn an_effect_transforms_its_nodes_own_content_and_children() {
    let mut s = Scene::new();
    let e = s.node(20.0, 20.0, 40.0, 40.0, effect(INVERT_CONTENT));
    s.tree.get_mut(e).unwrap().paint.background = Animated::new(Color::from_rgba8(255, 0, 0, 255));
    s.child(e, 10.0, 10.0, 20.0, 20.0, [255, 255, 255, 255]);
    s.frame();
    assert!(
        near(s.at(25, 25), [0, 255, 255, 255], 2),
        "own paint, inverted: {:?}",
        s.at(25, 25)
    );
    assert!(
        near(s.at(40, 40), [0, 0, 0, 255], 2),
        "the child, inverted: {:?}",
        s.at(40, 40)
    );
    assert_eq!(s.at(10, 10), BACKGROUND, "outside the node");
}

#[test]
fn an_effect_passing_content_through_matches_drawing_it_directly() {
    let mut s = Scene::new();
    let e = s.node(20.0, 20.0, 40.0, 40.0, effect(PASS));
    // Half-transparent red over the window's dark background.
    s.child(e, 0.0, 0.0, 40.0, 40.0, [255, 0, 0, 128]);
    s.frame();
    let got = s.at(40, 40);
    assert!(
        near(got, [136, 8, 8, 255], 4),
        "alpha survives the round trip: {got:?}"
    );
}

#[test]
fn a_change_in_the_subtree_redraws_the_whole_effect_box_and_an_idle_frame_costs_nothing() {
    let mut s = Scene::new();
    let e = s.node(20.0, 20.0, 40.0, 40.0, effect(INVERT_CONTENT));
    let child = s.child(e, 0.0, 0.0, 10.0, 10.0, [255, 0, 0, 255]);
    s.frame();
    assert!(near(s.at(22, 22), [0, 255, 255, 255], 2));
    assert_eq!(s.frame(), Damage::None);
    assert_eq!(s.passes(), 0, "an idle frame renders and runs nothing");

    s.tree.get_mut(child).unwrap().paint.background =
        Animated::new(Color::from_rgba8(0, 0, 255, 255));
    let damage = s.frame();
    match &damage {
        Damage::Rects(rects) => assert!(
            rects
                .iter()
                .any(|r| r.contains_rect(peniko::kurbo::Rect::new(20.0, 20.0, 60.0, 60.0))),
            "the whole effect box is damaged, not just the child: {damage:?}"
        ),
        other => panic!("expected rects, got {other:?}"),
    }
    assert_eq!(s.passes(), 1);
    assert!(
        near(s.at(22, 22), [255, 255, 0, 255], 2),
        "{:?}",
        s.at(22, 22)
    );
}

#[test]
fn an_effect_inside_an_effect_runs_inner_first() {
    let mut s = Scene::new();
    let outer = s.node(10.0, 10.0, 60.0, 60.0, effect(INVERT_CONTENT));
    let inner = s.child(outer, 10.0, 10.0, 30.0, 30.0, [0, 0, 0, 0]);
    s.tree.get_mut(inner).unwrap().shader = Some(effect(SWAP));
    s.child(inner, 0.0, 0.0, 30.0, 30.0, [255, 0, 0, 255]);
    s.frame();
    assert_eq!(s.passes(), 2);
    // red -> swapped to blue -> inverted to yellow.
    assert!(
        near(s.at(30, 30), [255, 255, 0, 255], 2),
        "{:?}",
        s.at(30, 30)
    );
}

#[test]
fn a_fill_shader_inside_an_effect_paints_into_its_content() {
    let mut s = Scene::new();
    let e = s.node(20.0, 20.0, 40.0, 40.0, effect(PASS));
    let f = s.child(e, 0.0, 0.0, 40.0, 40.0, [0, 0, 0, 0]);
    s.tree.get_mut(f).unwrap().shader = Some(tint(0.0, 1.0, 0.0, 1.0));
    s.frame();
    assert!(
        near(s.at(40, 40), [0, 255, 0, 255], 2),
        "{:?}",
        s.at(40, 40)
    );
}

#[test]
fn an_effect_composes_under_its_opacity_and_transform() {
    let mut s = Scene::new();
    let e = s.node(10.0, 10.0, 40.0, 40.0, effect(PASS));
    s.child(e, 0.0, 0.0, 40.0, 40.0, [255, 0, 0, 255]);
    s.tree.get_mut(e).unwrap().paint.opacity = Animated::new(0.5);
    s.tree.get_mut(e).unwrap().paint.node_transform.translate_x = Animated::new(30.0);
    s.frame();
    assert_eq!(
        s.at(20, 30),
        BACKGROUND,
        "moved away from where it was laid out"
    );
    let got = s.at(60, 30);
    assert!(
        near(got, [136, 8, 8, 255], 4),
        "half-opaque red, moved 30px: {got:?}"
    );
}

#[test]
fn a_hidden_effect_runs_nothing() {
    let mut s = Scene::new();
    let e = s.node(20.0, 20.0, 40.0, 40.0, effect(PASS));
    s.child(e, 0.0, 0.0, 40.0, 40.0, [255, 0, 0, 255]);
    s.tree.get_mut(e).unwrap().visible = false;
    s.frame();
    assert_eq!(s.passes(), 0);
    assert_eq!(s.at(40, 40), BACKGROUND);
}

#[test]
fn an_effect_over_text_transforms_its_glyphs() {
    let mut s = Scene::new();
    let id = s.boxed(10.0, 10.0, 100.0, 40.0);
    s.tree.add_child(s.root, id);
    let node = s.tree.get_mut(id).unwrap();
    node.kind = NodeKind::Text(engine_core::TextState {
        content: "HHHH".to_string(),
        font_family: "Roboto".to_string(),
        font_weight: 700.0,
        font_size: 32.0,
        align: engine_core::TextAlign::Start,
        line_height: None,
        options: Default::default(),
    });
    // The glyph colour is white; the effect inverts it to black.
    node.paint.background = Animated::new(Color::from_rgba8(255, 255, 255, 255));
    node.shader = Some(effect(INVERT_CONTENT));
    s.frame();
    let dark = |p: [u8; 4]| p[0] < 0x10 && p[1] < 0x10 && p[2] < 0x10 && p[3] == 255;
    let white = |p: [u8; 4]| p[0] > 0xE0 && p[1] > 0xE0 && p[2] > 0xE0;
    let any =
        |test: &dyn Fn([u8; 4]) -> bool| (10..110).any(|x| (10..50).any(|y| test(s.at(x, y))));
    assert!(
        any(&|p| p[0] < 0x08 && p[3] == 255) || any(&dark),
        "inverted glyphs are black"
    );
    assert!(!any(&white), "no white glyph pixels survive the inversion");
}

// --- 0.5.1 (#70): time and animation --------------------------------------------

const CLOCK: &str =
    "fn shade(p: Pixel) -> vec4<f32> {\n    return vec4<f32>(frame.time, 0.0, 0.0, 1.0);\n}\n";

fn clock(animated: bool) -> Arc<Shader> {
    Shader::new(CLOCK.to_owned(), vec![], vec![], ShaderMode::Fill, animated).expect("valid")
}

impl Scene {
    fn set_time(&mut self, seconds: f32) {
        self.renderer.set_time(seconds);
    }
}

#[test]
fn an_animated_shader_sees_the_window_clock_and_repaints_exactly_its_box() {
    let mut s = Scene::new();
    s.node(20.0, 20.0, 40.0, 40.0, clock(true));
    s.set_time(0.25);
    s.frame();
    assert!(near(s.at(40, 40), [64, 0, 0, 255], 2), "{:?}", s.at(40, 40));

    s.set_time(0.5);
    let damage = s.frame();
    assert!(
        near(s.at(40, 40), [128, 0, 0, 255], 2),
        "{:?}",
        s.at(40, 40)
    );
    match &damage {
        Damage::Rects(rects) => {
            let area = peniko::kurbo::Rect::new(20.0, 20.0, 60.0, 60.0).inflate(4.0, 4.0);
            assert!(
                rects.iter().all(|r| r.union(area) == area),
                "only the node's box is repainted: {damage:?}"
            );
        }
        other => panic!("expected the node's rect, got {other:?}"),
    }
    assert_eq!(s.passes(), 1);

    s.set_time(0.5);
    assert_eq!(s.frame(), Damage::None, "the same moment paints nothing");
    assert_eq!(s.passes(), 0);
}

#[test]
fn a_still_shader_ignores_the_clock() {
    let mut s = Scene::new();
    s.node(20.0, 20.0, 40.0, 40.0, clock(false));
    s.set_time(0.25);
    s.frame();
    s.set_time(0.75);
    assert_eq!(
        s.frame(),
        Damage::None,
        "not animated: time changes nothing"
    );
    assert_eq!(s.passes(), 0);
}

#[test]
fn only_a_drawn_animated_shader_keeps_the_loop_awake() {
    let mut s = Scene::new();
    s.node(20.0, 20.0, 40.0, 40.0, clock(false));
    s.frame();
    assert!(
        !s.renderer.has_animated_shader(),
        "a still shader lets the loop sleep"
    );

    let moving = s.node(70.0, 20.0, 30.0, 30.0, clock(true));
    s.frame();
    assert!(s.renderer.has_animated_shader());

    s.tree.get_mut(moving).unwrap().visible = false;
    s.frame();
    assert!(
        !s.renderer.has_animated_shader(),
        "hidden, it no longer does"
    );

    s.tree.get_mut(moving).unwrap().visible = true;
    let mut style = s.tree.get(moving).unwrap().layout_style.clone();
    style.inset.left = length(900.0);
    s.tree.set_layout_style(moving, style);
    s.frame();
    assert!(
        !s.renderer.has_animated_shader(),
        "off-screen, it doesn't either"
    );
}

#[test]
fn an_animated_fill_inside_an_effect_repaints_the_effect_each_frame() {
    let mut s = Scene::new();
    let e = s.node(20.0, 20.0, 40.0, 40.0, effect(PASS));
    let f = s.child(e, 0.0, 0.0, 40.0, 40.0, [0, 0, 0, 0]);
    s.tree.get_mut(f).unwrap().shader = Some(clock(true));
    s.set_time(0.25);
    s.frame();
    assert!(near(s.at(40, 40), [64, 0, 0, 255], 2), "{:?}", s.at(40, 40));
    assert!(s.renderer.has_animated_shader(), "found through the effect");
    s.set_time(0.5);
    s.frame();
    assert!(
        near(s.at(40, 40), [128, 0, 0, 255], 2),
        "{:?}",
        s.at(40, 40)
    );
}
