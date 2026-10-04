//! 0.5.4 (#110): a node's `blur` and `blend`, and the damage reach a blur adds.

mod support;

use engine_core::{Animated, Blend, NodeKind, PaintProperties, Tree};
use engine_render::{Damage, DamageTracker, TextRenderer};
use peniko::Color;
use support::Frame;
use taffy::prelude::{AvailableSpace, Position, Rect as TaffyRect, Size, Style, auto, length};

const SIZE: u16 = 100;
const BG: Color = Color::from_rgba8(40, 90, 200, 255);
const RED: Color = Color::from_rgba8(255, 0, 0, 255);
const YELLOW: Color = Color::from_rgba8(255, 200, 0, 255);

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

/// A background with one 40x40 box at 30,30, painted by `tweak`.
fn scene(
    color: Color,
    tweak: impl Fn(&mut PaintProperties),
) -> (Tree, engine_core::NodeId, engine_core::NodeId) {
    let mut tree = Tree::new();
    let root = tree.insert(
        NodeKind::Container,
        style(0.0, 0.0, f32::from(SIZE), f32::from(SIZE)),
        PaintProperties::new(BG, 0.0, 1.0),
    );
    let mut paint = PaintProperties::new(color, 0.0, 1.0);
    tweak(&mut paint);
    let boxed = tree.insert(NodeKind::Rect, style(30.0, 30.0, 40.0, 40.0), paint);
    tree.add_child(root, boxed);
    tree.compute_layout(
        root,
        Size {
            width: AvailableSpace::Definite(f32::from(SIZE)),
            height: AvailableSpace::Definite(f32::from(SIZE)),
        },
    );
    (tree, root, boxed)
}

fn frame(tree: &Tree, root: engine_core::NodeId) -> Frame {
    pollster::block_on(Frame::of(tree, root))
}

fn rgb(c: Color) -> [u8; 3] {
    let [r, g, b, _] = c.to_rgba8().to_u8_array();
    [r, g, b]
}

#[test]
fn a_blurred_box_softens_its_edges_and_keeps_its_middle() {
    let (tree, root, _) = scene(RED, |p| p.blur = Animated::new(4.0));
    let f = frame(&tree, root);
    assert_eq!(&f.at(50, 50)[..3], &[255, 0, 0], "the middle is untouched");
    let edge = f.at(30, 50);
    assert!(
        edge[0] > 90 && edge[0] < 220,
        "the edge is half way between box and background: {edge:?}"
    );
    let outside = f.at(25, 50);
    assert!(
        outside[0] > 40,
        "the blur spreads past the box: {outside:?}"
    );
    assert_eq!(&f.at(5, 5)[..3], &rgb(BG), "far from the box is untouched");
}

#[test]
fn no_blur_is_the_box_as_before() {
    let (tree, root, _) = scene(RED, |_| {});
    let f = frame(&tree, root);
    assert_eq!(&f.at(30, 50)[..3], &[255, 0, 0]);
    assert_eq!(&f.at(29, 50)[..3], &rgb(BG));
}

#[test]
fn a_blur_covers_the_nodes_children_too() {
    let (mut tree, root, parent) = scene(RED, |p| p.blur = Animated::new(5.0));
    let child = tree.insert(
        NodeKind::Rect,
        style(10.0, 10.0, 20.0, 20.0),
        PaintProperties::new(YELLOW, 0.0, 1.0),
    );
    tree.add_child(parent, child);
    tree.compute_layout(
        root,
        Size {
            width: AvailableSpace::Definite(f32::from(SIZE)),
            height: AvailableSpace::Definite(f32::from(SIZE)),
        },
    );
    let f = frame(&tree, root);
    // The child's edge at 40,40..60,60 is soft, not a hard yellow/red step.
    let a = f.at(39, 50);
    let b = f.at(41, 50);
    assert!(
        a[1].abs_diff(b[1]) < 60,
        "the child's edge is blurred with its parent: {a:?} {b:?}"
    );
}

#[test]
fn multiply_mixes_with_the_backdrop() {
    let (tree, root, _) = scene(YELLOW, |p| p.blend = Blend::Multiply);
    let f = frame(&tree, root);
    // (255,200,0) x (40,90,200) / 255, per channel.
    let got = f.at(50, 50);
    for (g, w) in got[..3].iter().zip([40u8, 71, 0]) {
        assert!(g.abs_diff(w) <= 2, "multiply: {got:?}");
    }
    assert_eq!(&f.at(5, 5)[..3], &rgb(BG), "outside the box is untouched");
}

#[test]
fn screen_and_difference_differ_from_multiply() {
    let screen = {
        let (t, r, _) = scene(YELLOW, |p| p.blend = Blend::Screen);
        frame(&t, r).at(50, 50)
    };
    let difference = {
        let (t, r, _) = scene(YELLOW, |p| p.blend = Blend::Difference);
        frame(&t, r).at(50, 50)
    };
    assert!(
        screen[0] == 255 && screen[2] >= 195,
        "screen lightens: {screen:?}"
    );
    assert!(
        difference[0].abs_diff(215) <= 2,
        "difference: {difference:?}"
    );
}

#[test]
fn a_blend_names_round_trip() {
    for (name, blend) in Blend::ALL {
        assert_eq!(Blend::from_name(name), Some(blend));
        assert_eq!(blend.name(), name);
    }
    assert_eq!(Blend::from_name("plus"), None);
}

#[test]
fn a_blurs_reach_is_part_of_what_a_node_damages() {
    let (mut tree, root, boxed) = scene(RED, |_| {});
    let mut tracker = DamageTracker::new();
    let mut text = TextRenderer::new();
    assert_eq!(
        tracker.damage(&tree, root, SIZE, SIZE, &mut text),
        Damage::Full
    );
    assert_eq!(
        tracker.damage(&tree, root, SIZE, SIZE, &mut text),
        Damage::None
    );
    tree.get_mut(boxed).unwrap().paint.blur = Animated::new(4.0);
    let Damage::Rects(rects) = tracker.damage(&tree, root, SIZE, SIZE, &mut text) else {
        panic!("a blur change damages part of the window");
    };
    // Box 30..70, blur reach 12 each side.
    assert!(
        rects
            .iter()
            .any(|r| r.x0 <= 18.0 && r.y0 <= 18.0 && r.x1 >= 82.0 && r.y1 >= 82.0),
        "{rects:?}"
    );
    tree.get_mut(boxed).unwrap().paint.blend = Blend::Screen;
    assert_ne!(
        tracker.damage(&tree, root, SIZE, SIZE, &mut text),
        Damage::None
    );
}

/// Stripes behind, and a 40x40 panel at 30,30 with a backdrop blur.
fn frosted(
    sigma: f64,
    panel_fill: Color,
) -> (
    Tree,
    engine_core::NodeId,
    engine_core::NodeId,
    engine_core::NodeId,
) {
    let mut tree = Tree::new();
    let root = tree.insert(
        NodeKind::Container,
        style(0.0, 0.0, f32::from(SIZE), f32::from(SIZE)),
        PaintProperties::new(BG, 0.0, 1.0),
    );
    // A vertical red bar crossing the panel, 10 wide at x=45..55.
    let bar = tree.insert(
        NodeKind::Rect,
        style(45.0, 0.0, 10.0, f32::from(SIZE)),
        PaintProperties::new(RED, 0.0, 1.0),
    );
    tree.add_child(root, bar);
    let mut paint = PaintProperties::new(panel_fill, 0.0, 1.0);
    paint.backdrop_blur = Animated::new(sigma);
    let panel = tree.insert(NodeKind::Rect, style(30.0, 30.0, 40.0, 40.0), paint);
    tree.add_child(root, panel);
    tree.compute_layout(
        root,
        Size {
            width: AvailableSpace::Definite(f32::from(SIZE)),
            height: AvailableSpace::Definite(f32::from(SIZE)),
        },
    );
    (tree, root, bar, panel)
}

const CLEAR: Color = Color::from_rgba8(0, 0, 0, 0);

#[test]
fn a_backdrop_blur_blurs_what_is_behind_inside_the_box_only() {
    let (tree, root, _, _) = frosted(4.0, CLEAR);
    let f = frame(&tree, root);
    // Outside the panel the bar is a hard red column.
    assert_eq!(&f.at(50, 10)[..3], &[255, 0, 0]);
    assert_eq!(&f.at(44, 10)[..3], &rgb(BG));
    // Inside, its edges are soft: pixels just outside the bar pick up red,
    // and the bar's middle is less than full red.
    let beside = f.at(42, 50);
    assert!(
        beside[0] > 70,
        "the bar bleeds sideways inside the panel: {beside:?}"
    );
    let middle = f.at(50, 50);
    assert!(middle[0] < 250, "and thins out: {middle:?}");
    // The panel's box edge is crisp: its left edge shows a clean step.
    assert_eq!(&f.at(29, 50)[..3], &rgb(BG));
}

#[test]
fn no_backdrop_blur_leaves_what_is_behind_alone() {
    let (tree, root, _, _) = frosted(0.0, CLEAR);
    let f = frame(&tree, root);
    assert_eq!(&f.at(50, 50)[..3], &[255, 0, 0]);
    assert_eq!(&f.at(44, 50)[..3], &rgb(BG));
}

#[test]
fn a_tinted_panel_draws_over_its_blurred_backdrop() {
    let (tree, root, _, _) = frosted(4.0, Color::from_rgba8(255, 255, 255, 128));
    let f = frame(&tree, root);
    let beside = f.at(42, 50);
    assert!(beside[1] > 150, "the white tint lifts it: {beside:?}");
}

#[test]
fn something_behind_a_backdrop_blur_damages_it_even_when_it_is_beside_the_box() {
    let (mut tree, root, bar, panel) = frosted(4.0, CLEAR);
    let mut tracker = DamageTracker::new();
    let mut text = TextRenderer::new();
    assert_eq!(
        tracker.damage(&tree, root, SIZE, SIZE, &mut text),
        Damage::Full
    );
    assert_eq!(
        tracker.damage(&tree, root, SIZE, SIZE, &mut text),
        Damage::None
    );
    // The bar changes colour: its pixels inside the panel change as the blur
    // shows them, so the panel's box must be redrawn too.
    tree.get_mut(bar).unwrap().paint.background = Animated::new(YELLOW);
    let Damage::Rects(rects) = tracker.damage(&tree, root, SIZE, SIZE, &mut text) else {
        panic!("part of the window");
    };
    let _ = panel;
    assert!(
        rects
            .iter()
            .any(|r| r.x0 <= 30.0 && r.x1 >= 70.0 && r.y0 <= 30.0 && r.y1 >= 70.0),
        "the panel's box is damaged: {rects:?}"
    );
}
