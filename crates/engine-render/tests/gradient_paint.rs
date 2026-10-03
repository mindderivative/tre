//! 0.5.4 (#109): a gradient fill paints a ramp across the node's box, the
//! damage tracker sees it change, and it follows rounded corners.

mod support;

use engine_core::{
    Animated, Gradient, GradientShape, GradientStop, NodeKind, PaintProperties, Tree,
};
use engine_render::{Damage, DamageTracker, TextRenderer};
use peniko::Color;
use support::Frame;
use taffy::prelude::{AvailableSpace, Size, Style, length};

const SIZE: u16 = 100;
const BASE: Color = Color::from_rgba8(0, 0, 255, 255);

fn stop(offset: f32, r: u8, g: u8) -> GradientStop {
    GradientStop {
        offset,
        color: Color::from_rgba8(r, g, 0, 255),
    }
}

fn tree_with(gradient: Gradient, radius: f64) -> (Tree, engine_core::NodeId) {
    let mut tree = Tree::new();
    let mut paint = PaintProperties::new(BASE, radius, 1.0);
    paint.gradient = Some(Box::new(Animated::new(gradient)));
    let root = tree.insert(
        NodeKind::Container,
        Style {
            size: Size {
                width: length(f32::from(SIZE)),
                height: length(f32::from(SIZE)),
            },
            ..Default::default()
        },
        paint,
    );
    tree.compute_layout(
        root,
        Size {
            width: AvailableSpace::Definite(f32::from(SIZE)),
            height: AvailableSpace::Definite(f32::from(SIZE)),
        },
    );
    (tree, root)
}

fn left_to_right() -> Gradient {
    Gradient::new(
        GradientShape::Linear { angle_deg: 90.0 },
        vec![stop(0.0, 0, 0), stop(1.0, 255, 0)],
    )
    .unwrap()
}

fn close(a: u8, b: u8, slack: u8) -> bool {
    a.abs_diff(b) <= slack
}

#[test]
fn a_linear_gradient_ramps_across_the_box() {
    let (tree, root) = tree_with(left_to_right(), 0.0);
    let frame = pollster::block_on(Frame::of(&tree, root));
    let (left, mid, right) = (frame.at(2, 50), frame.at(50, 50), frame.at(97, 50));
    assert!(left[0] < 20, "left is dark: {left:?}");
    assert!(close(mid[0], 128, 12), "middle is half: {mid:?}");
    assert!(right[0] > 235, "right is red: {right:?}");
    // The flat colour is not what is drawn.
    assert!(
        mid[2] < 10,
        "no base-colour blue under the gradient: {mid:?}"
    );
    // And it does not vary down the box.
    assert!(close(frame.at(50, 5)[0], frame.at(50, 95)[0], 3));
}

#[test]
fn a_vertical_gradient_runs_top_to_bottom() {
    let g = Gradient::new(
        GradientShape::Linear { angle_deg: 180.0 },
        vec![stop(0.0, 0, 0), stop(1.0, 255, 0)],
    )
    .unwrap();
    let (tree, root) = tree_with(g, 0.0);
    let frame = pollster::block_on(Frame::of(&tree, root));
    assert!(frame.at(50, 3)[0] < 20);
    assert!(frame.at(50, 96)[0] > 235);
    assert!(close(frame.at(5, 50)[0], frame.at(95, 50)[0], 3));
}

#[test]
fn a_radial_gradient_is_brightest_at_its_centre() {
    let g = Gradient::new(
        GradientShape::Radial {
            center: (0.5, 0.5),
            radius: 1.0,
        },
        vec![stop(0.0, 255, 0), stop(1.0, 0, 0)],
    )
    .unwrap();
    let (tree, root) = tree_with(g, 0.0);
    let frame = pollster::block_on(Frame::of(&tree, root));
    assert!(frame.at(50, 50)[0] > 240);
    assert!(
        frame.at(2, 2)[0] < 30,
        "corner is dark: {:?}",
        frame.at(2, 2)
    );
    assert!(
        close(frame.at(20, 50)[0], frame.at(80, 50)[0], 6),
        "symmetric"
    );
}

#[test]
fn a_sweep_gradient_goes_round_from_up() {
    // Starts at 12 o'clock: just after it, dark; just before it, red.
    let g = Gradient::new(
        GradientShape::Sweep {
            center: (0.5, 0.5),
            start_deg: 0.0,
        },
        vec![stop(0.0, 0, 0), stop(1.0, 255, 0)],
    )
    .unwrap();
    let (tree, root) = tree_with(g, 0.0);
    let frame = pollster::block_on(Frame::of(&tree, root));
    let after = frame.at(55, 10)[0];
    let before = frame.at(45, 10)[0];
    assert!(
        after < 40,
        "just clockwise of up is the first stop: {after}"
    );
    assert!(
        before > 215,
        "just anticlockwise of up is the last: {before}"
    );
    let right = frame.at(90, 50)[0];
    assert!(close(right, 64, 16), "a quarter turn is a quarter: {right}");
}

#[test]
fn a_gradient_follows_the_rounded_corners() {
    let (tree, root) = tree_with(left_to_right(), 30.0);
    let frame = pollster::block_on(Frame::of(&tree, root));
    assert_eq!(frame.at(1, 1)[3], 0, "the corner is outside the box");
    assert_eq!(frame.at(50, 50)[3], 255);
}

#[test]
fn no_gradient_is_the_flat_colour_as_before() {
    let mut tree = Tree::new();
    let root = tree.insert(
        NodeKind::Container,
        Style {
            size: Size {
                width: length(f32::from(SIZE)),
                height: length(f32::from(SIZE)),
            },
            ..Default::default()
        },
        PaintProperties::new(BASE, 0.0, 1.0),
    );
    tree.compute_layout(
        root,
        Size {
            width: AvailableSpace::Definite(f32::from(SIZE)),
            height: AvailableSpace::Definite(f32::from(SIZE)),
        },
    );
    let frame = pollster::block_on(Frame::of(&tree, root));
    assert_eq!(frame.at(50, 50), support::rgba(BASE));
}

#[test]
fn a_changed_gradient_damages_the_node_and_an_unchanged_one_does_not() {
    let (mut tree, root) = tree_with(left_to_right(), 0.0);
    let mut tracker = DamageTracker::new();
    let mut text = TextRenderer::new();
    let run = |tracker: &mut DamageTracker, tree: &Tree, text: &mut TextRenderer| {
        tracker.damage(tree, root, SIZE, SIZE, text)
    };
    assert_eq!(run(&mut tracker, &tree, &mut text), Damage::Full);
    assert_eq!(run(&mut tracker, &tree, &mut text), Damage::None);

    let other = Gradient::new(
        GradientShape::Linear { angle_deg: 90.0 },
        vec![stop(0.0, 0, 0), stop(1.0, 255, 128)],
    )
    .unwrap();
    tree.get_mut(root).unwrap().paint.gradient = Some(Box::new(Animated::new(other)));
    assert_ne!(run(&mut tracker, &tree, &mut text), Damage::None);
    assert_eq!(run(&mut tracker, &tree, &mut text), Damage::None);

    tree.get_mut(root).unwrap().paint.gradient = None;
    assert_ne!(run(&mut tracker, &tree, &mut text), Damage::None);
}

#[test]
fn a_sweep_starts_where_start_deg_says() {
    // Starting at 90 degrees, the first stop is at 3 o'clock.
    let g = Gradient::new(
        GradientShape::Sweep {
            center: (0.5, 0.5),
            start_deg: 90.0,
        },
        vec![stop(0.0, 0, 0), stop(1.0, 255, 0)],
    )
    .unwrap();
    let (tree, root) = tree_with(g, 0.0);
    let frame = pollster::block_on(Frame::of(&tree, root));
    assert!(
        frame.at(90, 55)[0] < 40,
        "just clockwise of right is the first stop"
    );
    assert!(
        close(frame.at(50, 90)[0], 64, 16),
        "down is a quarter round"
    );
    assert!(close(frame.at(10, 50)[0], 128, 16), "left is half");
}
