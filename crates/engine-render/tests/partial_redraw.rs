//! 0.4.0 M5: a partial redraw -- only the damage rects cleared and
//! repainted, over the kept last frame -- gives exactly the pixels a full
//! redraw of the changed tree does. Each test renders a scene in full into
//! a `PersistentTarget`, changes one thing, redraws only the damage into
//! the same target, and compares it byte for byte with a fresh full render.
//! The animation tests (Step 2) do the same every frame of a real
//! animation in flight, for each property the examples animate.

use std::time::{Duration, Instant};

use engine_core::{
    ImageState, MotionCurve, NodeId, NodeKind, PaintProperties, PathData, PathState,
    ScrollViewState, Shadow, Shadows, TextState, Tree,
};
use engine_render::{
    Damage, DamageTracker, FrameRenderer, GeometryCache, PersistentTarget, TextRenderer,
    build_tree_scene, build_tree_scene_in,
};
use peniko::Color;
use taffy::prelude::{AvailableSpace, Position, Rect as TaffyRect, Size, Style, auto, length};
use vello_gpu::{RenderSize, RenderTargetConfig};

const W: u16 = 240;
const H: u16 = 160;
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

mod support;

struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
}

impl Gpu {
    fn new() -> Self {
        let (device, queue) = pollster::block_on(support::device("partial redraw test device"));
        Self { device, queue }
    }
}

/// A window's rendering state: a target that keeps its frame, a renderer,
/// and a damage tracker -- what `engine-py` keeps per window.
struct Window {
    target: PersistentTarget,
    renderer: FrameRenderer,
    text: TextRenderer,
    geometry: GeometryCache,
    tracker: DamageTracker,
}

impl Window {
    fn new(gpu: &Gpu) -> Self {
        Self {
            target: PersistentTarget::new(&gpu.device, FORMAT, u32::from(W), u32::from(H)),
            renderer: FrameRenderer::new(
                &gpu.device,
                &RenderTargetConfig {
                    format: FORMAT,
                    width: W,
                    height: H,
                },
            ),
            text: TextRenderer::new(),
            geometry: GeometryCache::new(),
            tracker: DamageTracker::new(),
        }
    }

    /// One frame: full, partial, or nothing, as the damage says. Returns it.
    fn frame(&mut self, gpu: &Gpu, tree: &Tree, root: NodeId, partial: bool) -> Damage {
        // As the app does each frame: every image's texture uploaded
        // before the scene that draws it.
        self.renderer
            .sync_image_textures(tree, &gpu.device, &gpu.queue);
        let damage = self.tracker.damage(tree, root, W, H, &mut self.text);
        let damage = if partial { damage } else { Damage::Full };
        let rects = match &damage {
            Damage::None => return damage,
            Damage::Full => None,
            Damage::Rects(rects) => Some(rects.as_slice()),
        };
        let scene = match rects {
            Some(rects) => build_tree_scene_in(
                tree,
                root,
                W,
                H,
                rects,
                self.renderer.resources_mut(),
                &mut self.text,
                &mut self.geometry,
            ),
            None => build_tree_scene(
                tree,
                root,
                W,
                H,
                self.renderer.resources_mut(),
                &mut self.text,
                &mut self.geometry,
            ),
        };
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        self.renderer.render_into(
            &scene,
            &gpu.device,
            &gpu.queue,
            &mut encoder,
            &RenderSize {
                width: W,
                height: H,
            },
            self.target.view(),
            rects,
        );
        gpu.queue.submit([encoder.finish()]);
        damage
    }

    /// The target's pixels, read back through a copy -- as the window's
    /// swapchain image receives them.
    fn pixels(&self, gpu: &Gpu) -> Vec<u8> {
        support::copy_out(&gpu.device, &gpu.queue, &self.target)
    }
}

struct Scene {
    tree: Tree,
    root: NodeId,
    card: NodeId,
    tint: NodeId,
    label: NodeId,
    group: NodeId,
}

fn placed(x: f32, y: f32, w: f32, h: f32) -> Style {
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

fn add(tree: &mut Tree, parent: NodeId, kind: NodeKind, style: Style, color: Color) -> NodeId {
    let id = tree.insert(kind, style, PaintProperties::new(color, 6.0, 1.0));
    tree.add_child(parent, id);
    id
}

/// A background, a shadowed rounded card, a translucent box overlapping
/// it, a text label, and a group holding a child.
fn scene() -> Scene {
    let mut tree = Tree::new();
    let root = tree.insert(
        NodeKind::Rect,
        Style {
            size: Size {
                width: length(f32::from(W)),
                height: length(f32::from(H)),
            },
            ..Default::default()
        },
        PaintProperties::new(Color::from_rgba8(0x22, 0x22, 0x28, 0xFF), 0.0, 1.0),
    );
    let card = add(
        &mut tree,
        root,
        NodeKind::Rect,
        placed(20.0, 20.0, 80.0, 50.0),
        Color::from_rgba8(0xF0, 0xF0, 0xF0, 0xFF),
    );
    tree.get_mut(card).unwrap().paint.shadows.current = Shadows(vec![Shadow {
        color: Color::from_rgba8(0, 0, 0, 0xA0),
        offset_x: 0.0,
        offset_y: 6.0,
        blur: 8.0,
        spread: 1.0,
    }]);
    let tint = add(
        &mut tree,
        root,
        NodeKind::Rect,
        placed(70.0, 45.0, 60.0, 40.0),
        Color::from_rgba8(0x30, 0x90, 0xFF, 0x90),
    );
    let label = add(
        &mut tree,
        root,
        NodeKind::Text(TextState {
            content: "Partial".into(),
            font_family: "Roboto".into(),
            font_weight: 400.0,
            font_size: 16.0,
            align: Default::default(),
            line_height: None,
            options: Default::default(),
        }),
        placed(150.0, 20.0, 80.0, 20.0),
        Color::from_rgba8(0xFF, 0xFF, 0xFF, 0xFF),
    );
    let group = add(
        &mut tree,
        root,
        NodeKind::Rect,
        placed(140.0, 90.0, 80.0, 50.0),
        Color::from_rgba8(0x60, 0x40, 0x90, 0xFF),
    );
    add(
        &mut tree,
        group,
        NodeKind::Rect,
        placed(10.0, 10.0, 30.0, 30.0),
        Color::from_rgba8(0xFF, 0xC0, 0x40, 0xFF),
    );
    layout(&mut tree, root);
    Scene {
        tree,
        root,
        card,
        tint,
        label,
        group,
    }
}

fn layout(tree: &mut Tree, root: NodeId) {
    tree.compute_layout(
        root,
        Size {
            width: AvailableSpace::Definite(f32::from(W)),
            height: AvailableSpace::Definite(f32::from(H)),
        },
    );
}

/// Renders the scene in full, applies `change`, redraws partially, and
/// checks the result against a fresh full render of the changed tree.
fn partial_matches_full(change: impl FnOnce(&mut Scene)) {
    let gpu = Gpu::new();
    let mut s = scene();
    let mut window = Window::new(&gpu);
    assert_eq!(window.frame(&gpu, &s.tree, s.root, true), Damage::Full);

    change(&mut s);
    layout(&mut s.tree, s.root);
    let damage = window.frame(&gpu, &s.tree, s.root, true);
    assert!(
        matches!(damage, Damage::Rects(_)),
        "a small change redraws partially, got {damage:?}"
    );
    let partial = window.pixels(&gpu);

    let mut reference = Window::new(&gpu);
    reference.frame(&gpu, &s.tree, s.root, false);
    let full = reference.pixels(&gpu);

    let differing = partial.iter().zip(&full).filter(|(a, b)| a != b).count();
    assert_eq!(
        differing, 0,
        "{differing} bytes differ between partial and full redraw ({damage:?})"
    );
}

#[test]
fn a_color_change_under_an_overlapping_translucent_box() {
    partial_matches_full(|s| {
        s.tree.get_mut(s.card).unwrap().paint.background.current =
            Color::from_rgba8(0xE0, 0x50, 0x50, 0xFF);
    });
}

#[test]
fn a_move_that_uncovers_what_was_underneath() {
    partial_matches_full(|s| {
        let mut style = s.tree.get(s.tint).unwrap().layout_style.clone();
        style.inset.left = length(100.0);
        s.tree.set_layout_style(s.tint, style);
    });
}

#[test]
fn a_shadow_change() {
    partial_matches_full(|s| {
        s.tree.get_mut(s.card).unwrap().paint.shadows.current.0[0].offset_y = 14.0;
    });
}

#[test]
fn a_text_change() {
    partial_matches_full(|s| {
        if let NodeKind::Text(state) = &mut s.tree.get_mut(s.label).unwrap().kind {
            state.content = "Redrawn".into();
        }
    });
}

#[test]
fn a_group_opacity_change() {
    partial_matches_full(|s| {
        s.tree.get_mut(s.group).unwrap().paint.opacity.current = 0.4;
    });
}

#[test]
fn a_node_removed() {
    partial_matches_full(|s| {
        s.tree.detach(s.root, s.tint);
    });
}

#[test]
fn nothing_changed_renders_nothing_and_keeps_the_frame() {
    let gpu = Gpu::new();
    let s = scene();
    let mut window = Window::new(&gpu);
    window.frame(&gpu, &s.tree, s.root, true);
    let before = window.pixels(&gpu);
    assert_eq!(window.frame(&gpu, &s.tree, s.root, true), Damage::None);
    assert!(before == window.pixels(&gpu));
}

// ---- Step 2: every frame of the examples' animations -----------------

const FRAMES: u32 = 10;
const DURATION: Duration = Duration::from_millis(400);
/// The M3 standard easing curve, as `animation.py` passes it.
const STANDARD: MotionCurve = MotionCurve::Bezier(0.2, 0.0, 0.0, 1.0);

/// Runs an animation frame by frame: `step(scene, frame, now)` starts
/// (frame 0) or retargets it, the tree ticks to `now`, and one window
/// redraws only the damage while another redraws in full. Every frame's
/// pixels must match, and at least one frame must have been partial.
fn animation_matches_full(mut step: impl FnMut(&mut Scene, u32, Instant)) {
    let gpu = Gpu::new();
    let mut s = scene();
    let mut partial = Window::new(&gpu);
    let mut full = Window::new(&gpu);
    partial.frame(&gpu, &s.tree, s.root, true);
    full.frame(&gpu, &s.tree, s.root, false);

    let start = Instant::now();
    let mut partial_frames = 0;
    // One frame past the end, where the animation settles on its target.
    for frame in 0..=FRAMES + 1 {
        let now = start + DURATION * frame / FRAMES;
        step(&mut s, frame, now);
        s.tree.tick_all(now);
        layout(&mut s.tree, s.root);
        let damage = partial.frame(&gpu, &s.tree, s.root, true);
        if matches!(damage, Damage::Rects(_)) {
            partial_frames += 1;
        }
        full.frame(&gpu, &s.tree, s.root, false);
        let (a, b) = (partial.pixels(&gpu), full.pixels(&gpu));
        let differing = a.iter().zip(&b).filter(|(a, b)| a != b).count();
        assert_eq!(
            differing, 0,
            "frame {frame}: {differing} bytes differ between partial and full redraw ({damage:?})"
        );
    }
    assert!(partial_frames > 0, "no frame was redrawn partially");
}

#[test]
fn animated_opacity_and_corner_radius() {
    // animation.py's first card: a linear fade that rounds its corners.
    animation_matches_full(|s, frame, now| {
        if frame == 0 {
            let paint = &mut s.tree.get_mut(s.card).unwrap().paint;
            paint
                .opacity
                .animate_to(0.2, DURATION, MotionCurve::Linear, now);
            paint
                .corner_radius
                .animate_to(24.0, DURATION, MotionCurve::Linear, now);
        }
    });
}

#[test]
fn animated_fill_retargeted_halfway() {
    // animation.py's second card: an eased fill that turns back midway.
    animation_matches_full(|s, frame, now| {
        let fill = &mut s.tree.get_mut(s.tint).unwrap().paint.background;
        match frame {
            0 => fill.animate_to(
                Color::from_rgba8(0x03, 0xDA, 0xC6, 0xC0),
                DURATION,
                STANDARD,
                now,
            ),
            5 => fill.animate_to(
                Color::from_rgba8(0x30, 0x90, 0xFF, 0x90),
                DURATION,
                STANDARD,
                now,
            ),
            _ => {}
        }
    });
}

#[test]
fn animated_scale_grows_the_shadow_with_it() {
    // animation.py's chained card, switch.py's thumb, ripple.py's press.
    animation_matches_full(|s, frame, now| {
        let scale = &mut s.tree.get_mut(s.card).unwrap().paint.node_transform.scale;
        match frame {
            0 => scale.animate_to(1.2, DURATION / 2, STANDARD, now),
            5 => scale.animate_to(1.0, DURATION / 2, STANDARD, now),
            _ => {}
        }
    });
}

#[test]
fn animated_shadows() {
    // shadows.py: a card lifting to a higher elevation.
    animation_matches_full(|s, frame, now| {
        if frame == 0 {
            s.tree.get_mut(s.card).unwrap().paint.shadows.animate_to(
                Shadows(vec![Shadow {
                    color: Color::from_rgba8(0, 0, 0, 0x60),
                    offset_x: 2.0,
                    offset_y: 14.0,
                    blur: 16.0,
                    spread: 3.0,
                }]),
                DURATION,
                STANDARD,
                now,
            );
        }
    });
}

#[test]
fn animated_translation_across_other_nodes() {
    // reorder.py's rows and switch.py's thumb sliding over what's beneath.
    animation_matches_full(|s, frame, now| {
        if frame == 0 {
            let t = &mut s.tree.get_mut(s.tint).unwrap().paint.node_transform;
            t.translate_x.animate_to(60.0, DURATION, STANDARD, now);
            t.translate_y.animate_to(-25.0, DURATION, STANDARD, now);
        }
    });
}

#[test]
fn animated_rotation() {
    animation_matches_full(|s, frame, now| {
        if frame == 0 {
            s.tree
                .get_mut(s.tint)
                .unwrap()
                .paint
                .node_transform
                .rotation_deg
                .animate_to(35.0, DURATION, MotionCurve::Linear, now);
        }
    });
}

#[test]
fn animated_border_on_a_group() {
    // stroke_color / stroke_width on a node with a child.
    animation_matches_full(|s, frame, now| {
        if frame == 0 {
            let paint = &mut s.tree.get_mut(s.group).unwrap().paint;
            paint.border_color.animate_to(
                Color::from_rgba8(0xFF, 0xFF, 0xFF, 0xFF),
                DURATION,
                STANDARD,
                now,
            );
            paint.border_width.animate_to(4.0, DURATION, STANDARD, now);
        }
    });
}

#[test]
fn animated_path_morph_and_trim() {
    // path_morph.py: a shape morphing into another while its stroke trims.
    let mut path = None;
    animation_matches_full(|s, frame, now| {
        if frame == 0 {
            let id = s.tree.insert(
                NodeKind::Path(PathState::new(
                    PathData::from_svg("M0,0 L40,0 L40,40 L0,40 Z").unwrap(),
                )),
                placed(20.0, 95.0, 40.0, 40.0),
                PaintProperties::new(Color::from_rgba8(0x80, 0xE0, 0x80, 0xFF), 0.0, 1.0),
            );
            s.tree.add_child(s.root, id);
            path = Some(id);
        }
        if frame == 1 {
            let node = s.tree.get_mut(path.unwrap()).unwrap();
            node.paint.border_color.animate_to(
                Color::from_rgba8(0xFF, 0xFF, 0xFF, 0xFF),
                DURATION,
                STANDARD,
                now,
            );
            node.paint
                .border_width
                .animate_to(2.0, DURATION, STANDARD, now);
            let NodeKind::Path(state) = &mut node.kind else {
                unreachable!()
            };
            state.data.animate_to(
                PathData::from_svg("M20,0 L40,20 L20,40 L0,20 Z").unwrap(),
                DURATION,
                STANDARD,
                now,
            );
            state.trim_end.animate_to(0.6, DURATION, STANDARD, now);
        }
    });
}

#[test]
fn animated_scroll_offset() {
    // A carousel snapping: a scroll view's content moving under its clip.
    let mut view = None;
    animation_matches_full(|s, frame, now| {
        if frame == 0 {
            let id = s.tree.insert(
                NodeKind::ScrollView(ScrollViewState::new(false)),
                placed(20.0, 95.0, 60.0, 50.0),
                PaintProperties::new(Color::from_rgba8(0x10, 0x10, 0x10, 0xFF), 0.0, 1.0),
            );
            s.tree.add_child(s.root, id);
            let content = s.tree.insert(
                NodeKind::Container,
                Style {
                    size: Size {
                        width: length(60.0),
                        height: length(200.0),
                    },
                    ..Default::default()
                },
                PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 1.0),
            );
            s.tree.add_child(id, content);
            for (i, y) in [10.0, 60.0, 110.0, 160.0].into_iter().enumerate() {
                let shade = 0x40 + 0x30 * i as u8;
                add(
                    &mut s.tree,
                    content,
                    NodeKind::Rect,
                    placed(5.0, y, 50.0, 30.0),
                    Color::from_rgba8(shade, 0x60, 0xC0, 0xFF),
                );
            }
            view = Some(id);
        }
        if frame == 1 {
            let NodeKind::ScrollView(state) = &mut s.tree.get_mut(view.unwrap()).unwrap().kind
            else {
                unreachable!()
            };
            state.scroll.animate_to(120.0, DURATION, STANDARD, now);
        }
    });
}

// ---- review: what paints into a damage rect from outside it ------------

/// Like `partial_matches_full`, with extra nodes `build` adds to the scene
/// before its first frame; `change` gets whatever `build` returned.
fn built_partial_matches_full<T>(
    build: impl FnOnce(&mut Tree, NodeId) -> T,
    change: impl FnOnce(&mut Tree, &T),
) {
    let gpu = Gpu::new();
    let mut s = scene();
    let ids = build(&mut s.tree, s.root);
    layout(&mut s.tree, s.root);
    let mut window = Window::new(&gpu);
    assert_eq!(window.frame(&gpu, &s.tree, s.root, true), Damage::Full);

    change(&mut s.tree, &ids);
    layout(&mut s.tree, s.root);
    let damage = window.frame(&gpu, &s.tree, s.root, true);
    assert!(
        matches!(damage, Damage::Rects(_)),
        "a small change redraws partially, got {damage:?}"
    );
    let partial = window.pixels(&gpu);
    let mut reference = Window::new(&gpu);
    reference.frame(&gpu, &s.tree, s.root, false);
    let full = reference.pixels(&gpu);
    let differing = partial.iter().zip(&full).filter(|(a, b)| a != b).count();
    assert_eq!(
        differing, 0,
        "{differing} bytes differ between partial and full redraw ({damage:?})"
    );
}

#[test]
fn a_change_inside_another_nodes_shadow_repaints_that_shadow() {
    // The shadow reaches past its node's box, over the changed node; the
    // shadowed node's box itself is nowhere near the damage.
    built_partial_matches_full(
        |tree, root| {
            let lit = add(
                tree,
                root,
                NodeKind::Rect,
                placed(20.0, 100.0, 40.0, 30.0),
                Color::from_rgba8(0xE0, 0xE0, 0xE0, 0xFF),
            );
            tree.get_mut(lit).unwrap().paint.shadows.current = Shadows(vec![Shadow {
                color: Color::from_rgba8(0xFF, 0x40, 0x40, 0xFF),
                offset_x: 0.0,
                offset_y: 0.0,
                blur: 12.0,
                spread: 4.0,
            }]);
            add(
                tree,
                root,
                NodeKind::Rect,
                placed(70.0, 110.0, 8.0, 8.0),
                Color::from_rgba8(0x40, 0x40, 0xFF, 0x80),
            )
        },
        |tree, dot| {
            tree.get_mut(*dot).unwrap().paint.background.current =
                Color::from_rgba8(0x40, 0xFF, 0x40, 0x80);
        },
    );
}

#[test]
fn a_child_overflowing_its_parent_is_repainted() {
    // The parent doesn't clip, and its own box is far from the change.
    built_partial_matches_full(
        |tree, root| {
            let parent = add(
                tree,
                root,
                NodeKind::Rect,
                placed(10.0, 100.0, 20.0, 20.0),
                Color::from_rgba8(0x80, 0x80, 0x80, 0xFF),
            );
            add(
                tree,
                parent,
                NodeKind::Rect,
                placed(60.0, 20.0, 20.0, 20.0),
                Color::from_rgba8(0xFF, 0xC0, 0x40, 0xFF),
            )
        },
        |tree, child| {
            tree.get_mut(*child).unwrap().paint.background.current =
                Color::from_rgba8(0x40, 0xC0, 0xFF, 0xFF);
        },
    );
}

/// A 4x4 RGBA8 image of one colour.
fn solid_image(rgba: [u8; 4]) -> peniko::ImageData {
    peniko::ImageData {
        data: peniko::Blob::from(rgba.repeat(16)),
        format: peniko::ImageFormat::Rgba8,
        alpha_type: peniko::ImageAlphaType::Alpha,
        width: 4,
        height: 4,
    }
}

#[test]
fn a_new_image_frame_at_the_same_size_shows_in_place() {
    // A video's next frame: written into the existing texture, not a new
    // one, and it must show exactly as a fresh upload does.
    built_partial_matches_full(
        |tree, root| {
            add(
                tree,
                root,
                NodeKind::Image(ImageState::new(solid_image([0x00, 0xFF, 0x00, 0xFF]))),
                placed(20.0, 100.0, 40.0, 40.0),
                Color::from_rgba8(0, 0, 0, 0),
            )
        },
        |tree, image| {
            let NodeKind::Image(state) = &mut tree.get_mut(*image).unwrap().kind else {
                unreachable!()
            };
            state.image = solid_image([0xFF, 0x20, 0x80, 0xFF]);
        },
    );
}
