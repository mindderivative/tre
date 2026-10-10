//! 0.5.6 (#164): a node's `mask`, and what a change of it damages.

mod support;

use engine_core::{Mask, NodeKind, PaintProperties, Tree};
use engine_render::{Damage, DamageTracker, TextRenderer};
use peniko::Color;
use support::Frame;
use taffy::prelude::{AvailableSpace, Position, Rect as TaffyRect, Size, Style, auto, length};

const SIZE: u16 = 100;
const BG: Color = Color::from_rgba8(40, 90, 200, 255);
const RED: Color = Color::from_rgba8(255, 0, 0, 255);

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

/// A background with one 40x40 box at 30,30.
fn scene() -> (Tree, engine_core::NodeId, engine_core::NodeId) {
    let mut tree = Tree::new();
    let root = tree.insert(
        NodeKind::Container,
        style(0.0, 0.0, f32::from(SIZE), f32::from(SIZE)),
        PaintProperties::new(BG, 0.0, 1.0),
    );
    let boxed = tree.insert(
        NodeKind::Rect,
        style(30.0, 30.0, 40.0, 40.0),
        PaintProperties::new(RED, 0.0, 1.0),
    );
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

fn rgb_at(frame: &Frame, x: u32, y: u32) -> [u8; 3] {
    let [r, g, b, _] = frame.at(x, y);
    [r, g, b]
}

fn rgb(c: Color) -> [u8; 3] {
    let [r, g, b, _] = c.to_rgba8().to_u8_array();
    [r, g, b]
}

#[test]
fn a_circle_mask_cuts_the_corners_of_the_box() {
    let (mut tree, root, boxed) = scene();
    tree.get_mut(boxed).unwrap().paint.mask = Some(Box::new(Mask::Circle));
    let frame = pollster::block_on(Frame::of(&tree, root));
    assert_eq!(rgb_at(&frame, 50, 50), rgb(RED));
    assert_eq!(rgb_at(&frame, 31, 31), rgb(BG), "the corner is cut");
    assert_eq!(rgb_at(&frame, 68, 68), rgb(BG));
    assert_eq!(
        rgb_at(&frame, 50, 31),
        rgb(RED),
        "the top of the circle stays"
    );
}

#[test]
fn setting_or_changing_a_mask_damages_the_node() {
    let (mut tree, root, boxed) = scene();
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
    tree.get_mut(boxed).unwrap().paint.mask = Some(Box::new(Mask::Circle));
    assert_ne!(
        tracker.damage(&tree, root, SIZE, SIZE, &mut text),
        Damage::None,
        "setting a mask repaints"
    );
    assert_eq!(
        tracker.damage(&tree, root, SIZE, SIZE, &mut text),
        Damage::None
    );
    tree.get_mut(boxed).unwrap().paint.mask =
        Some(Box::new(Mask::Rounded(engine_core::CornerRadii([5.0; 4]))));
    assert_ne!(
        tracker.damage(&tree, root, SIZE, SIZE, &mut text),
        Damage::None,
        "changing the shape repaints"
    );
    tree.get_mut(boxed).unwrap().paint.mask = None;
    assert_ne!(
        tracker.damage(&tree, root, SIZE, SIZE, &mut text),
        Damage::None,
        "clearing it repaints"
    );
}
