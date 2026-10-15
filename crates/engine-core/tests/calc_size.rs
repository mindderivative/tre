//! 0.5.6.2 (#178): a size that mixes units -- a percentage plus pixels, as in
//! `width="calc(100% - 16)"` -- is resolved by the tree against its parent's box
//! after a layout pass, because taffy cannot lay such a size out itself.

use engine_core::{CalcLen, NodeId, NodeKind, PaintProperties, Tree};
use peniko::Color;
use taffy::prelude::{AvailableSpace, Dimension, Size, Style, auto, length};
use taffy::style::Position;

fn paint() -> PaintProperties {
    PaintProperties::new(Color::from_rgba8(0, 0, 0, 255), 0.0, 1.0)
}

fn parent_style(width: f32, padding: f32) -> Style {
    Style {
        size: Size {
            width: length(width),
            height: length(200.0),
        },
        padding: taffy::Rect {
            left: length(padding),
            right: length(padding),
            top: length(padding),
            bottom: length(padding),
        },
        ..Default::default()
    }
}

fn lay_out(tree: &mut Tree, root: NodeId) {
    tree.compute_layout(
        root,
        Size {
            width: AvailableSpace::Definite(400.0),
            height: AvailableSpace::Definite(300.0),
        },
    );
}

/// `percent` of the parent plus `px`.
fn calc(percent: f32, px: f32) -> Option<CalcLen> {
    Some(CalcLen { percent, px })
}

/// A parent of 300 x 200 with `padding`, holding one child laid out with the
/// width and height given as calc sizes (`None`: not calc, 10 px).
fn scene(
    padding: f32,
    absolute: bool,
    width: Option<CalcLen>,
    height: Option<CalcLen>,
) -> (Tree, NodeId, NodeId) {
    let mut tree = Tree::new();
    let parent = tree.insert(NodeKind::Container, parent_style(300.0, padding), paint());
    let child = tree.insert(
        NodeKind::Rect,
        Style {
            position: if absolute {
                Position::Absolute
            } else {
                Position::Relative
            },
            size: Size {
                width: if width.is_some() {
                    auto()
                } else {
                    length(10.0)
                },
                height: if height.is_some() {
                    auto()
                } else {
                    length(10.0)
                },
            },
            ..Default::default()
        },
        paint(),
    );
    tree.add_child(parent, child);
    tree.set_calc_size(child, false, width);
    tree.set_calc_size(child, true, height);
    lay_out(&mut tree, parent);
    (tree, parent, child)
}

fn size(tree: &Tree, id: NodeId) -> (f32, f32) {
    let layout = tree.layout(id);
    (layout.size.width, layout.size.height)
}

#[test]
fn a_hundred_percent_less_a_strip() {
    let (tree, _, child) = scene(0.0, false, calc(1.0, -16.0), None);
    assert_eq!(size(&tree, child), (284.0, 10.0));
}

#[test]
fn a_percentage_plus_pixels() {
    // 33% of 300 is 99, plus 10.
    let (tree, _, child) = scene(0.0, false, calc(0.33, 10.0), None);
    assert_eq!(size(&tree, child), (109.0, 10.0));
}

#[test]
fn height_is_resolved_against_the_parents_height() {
    let (tree, _, child) = scene(0.0, false, None, calc(0.5, 20.0));
    assert_eq!(size(&tree, child), (10.0, 120.0));
}

#[test]
fn both_sides_at_once() {
    let (tree, _, child) = scene(0.0, false, calc(1.0, -16.0), calc(1.0, -50.0));
    assert_eq!(size(&tree, child), (284.0, 150.0));
}

#[test]
fn a_node_in_the_flow_takes_the_parents_content_box_as_a_plain_percentage_does() {
    // 300 wide with padding=10: 280 inside, so 100% - 16 is 264.
    let (tree, _, child) = scene(10.0, false, calc(1.0, -16.0), calc(1.0, 0.0));
    assert_eq!(size(&tree, child), (264.0, 180.0));
}

#[test]
fn an_absolute_node_takes_the_parents_padding_box() {
    let (tree, _, child) = scene(10.0, true, calc(1.0, -16.0), calc(1.0, 0.0));
    assert_eq!(size(&tree, child), (284.0, 200.0));
}

#[test]
fn the_size_follows_the_parent_when_it_is_resized() {
    let (mut tree, parent, child) = scene(0.0, false, calc(1.0, -16.0), None);
    assert_eq!(size(&tree, child).0, 284.0);
    tree.set_layout_style(parent, parent_style(400.0, 0.0));
    lay_out(&mut tree, parent);
    assert_eq!(size(&tree, child).0, 384.0);
}

#[test]
fn a_side_that_is_no_longer_calc_keeps_the_size_it_had_until_it_is_set() {
    let (mut tree, parent, child) = scene(0.0, false, calc(1.0, -16.0), None);
    tree.set_calc_size(child, false, None);
    tree.set_layout_style(parent, parent_style(400.0, 0.0));
    lay_out(&mut tree, parent);
    // Not resolved again: the pixels baked at 300 wide stay.
    assert_eq!(size(&tree, child).0, 284.0);
}

#[test]
fn a_removed_calc_node_is_forgotten() {
    let (mut tree, parent, child) = scene(0.0, false, calc(1.0, -16.0), None);
    assert!(tree.remove(child));
    lay_out(&mut tree, parent);
    assert!(tree.take_calc_conflicts().is_empty());
}

#[test]
fn a_size_with_no_fixed_point_is_given_up_on() {
    // A row as wide as its two children: a 100 px block and a child that is the
    // row's width less 16. child = row - 16, row = 100 + child: no solution.
    let mut tree = Tree::new();
    let outer = tree.insert(
        NodeKind::Container,
        Style {
            size: Size {
                width: length(400.0),
                height: length(200.0),
            },
            align_items: Some(taffy::style::AlignItems::FLEX_START),
            ..Default::default()
        },
        paint(),
    );
    let row = tree.insert(
        NodeKind::Container,
        Style {
            size: Size {
                width: auto(),
                height: length(50.0),
            },
            ..Default::default()
        },
        paint(),
    );
    let block = tree.insert(
        NodeKind::Rect,
        Style {
            size: Size {
                width: length(100.0),
                height: length(10.0),
            },
            ..Default::default()
        },
        paint(),
    );
    let child = tree.insert(
        NodeKind::Rect,
        Style {
            size: Size {
                width: auto(),
                height: length(10.0),
            },
            ..Default::default()
        },
        paint(),
    );
    tree.add_child(outer, row);
    tree.add_child(row, block);
    tree.add_child(row, child);
    tree.set_calc_size(child, false, calc(1.0, -16.0));
    lay_out(&mut tree, outer);
    assert_eq!(tree.take_calc_conflicts(), vec![(child, false)]);
    // Back to `auto`, and it stays there: the next layout does not move it.
    let before = size(&tree, child);
    tree.set_layout_style(outer, tree.get(outer).unwrap().layout_style.clone());
    lay_out(&mut tree, outer);
    assert_eq!(size(&tree, child), before);
    assert_eq!(
        tree.get(child).unwrap().layout_style.size.width,
        Dimension::auto()
    );
    assert!(tree.take_calc_conflicts().is_empty());
}
