//! 0.5.6.2 (#179): a node placed against another node, anywhere in the tree.
//! After each layout the tree puts the node's box at `placement` of its anchor's
//! box, so it follows the anchor with no lag and stays an ordinary child of its
//! own parent. These are the tree-level proofs; `tests/test_anchored_layout.py`
//! covers the Python surface.

use engine_core::{Anchored, NodeId, NodeKind, PaintProperties, Placement, Tree};
use peniko::Color;
use taffy::prelude::{AvailableSpace, FlexDirection, Size, Style, length};
use taffy::style::Position;

fn paint() -> PaintProperties {
    PaintProperties::new(Color::from_rgba8(0, 0, 0, 255), 0.0, 1.0)
}

fn sized(width: f32, height: f32) -> Style {
    Style {
        size: Size {
            width: length(width),
            height: length(height),
        },
        ..Default::default()
    }
}

fn lay_out(tree: &mut Tree, root: NodeId) {
    tree.compute_layout(
        root,
        Size {
            width: AvailableSpace::Definite(500.0),
            height: AvailableSpace::Definite(400.0),
        },
    );
}

fn absolute_at(left: f32, top: f32, width: f32, height: f32) -> Style {
    Style {
        position: Position::Absolute,
        inset: taffy::Rect {
            left: length(left),
            top: length(top),
            right: taffy::prelude::auto(),
            bottom: taffy::prelude::auto(),
        },
        ..sized(width, height)
    }
}

/// Two sibling boxes: the left holds the anchor (60 x 30, 30 px in and 40 down),
/// the right, 200 px over, holds a 20 x 10 node that is not the anchor's child.
fn scene() -> (Tree, NodeId, NodeId, NodeId, NodeId) {
    let mut tree = Tree::new();
    let root = tree.insert(
        NodeKind::Container,
        Style {
            flex_direction: FlexDirection::Row,
            ..sized(500.0, 400.0)
        },
        paint(),
    );
    let left = tree.insert(NodeKind::Container, sized(200.0, 300.0), paint());
    let right = tree.insert(NodeKind::Container, sized(200.0, 300.0), paint());
    tree.add_child(root, left);
    tree.add_child(root, right);
    let anchor = tree.insert(NodeKind::Rect, absolute_at(30.0, 40.0, 60.0, 30.0), paint());
    tree.add_child(left, anchor);
    let node = tree.insert(NodeKind::Rect, sized(20.0, 10.0), paint());
    tree.add_child(right, node);
    (tree, root, anchor, node, right)
}

fn anchored(anchor: NodeId, placement: Placement) -> Anchored {
    Anchored {
        anchor,
        placement,
        offset: (0.0, 0.0),
    }
}

fn place(placement: Placement, offset: (f64, f64)) -> (f64, f64) {
    let (mut tree, root, anchor, node, _) = scene();
    assert!(tree.set_anchor(
        node,
        Some(Anchored {
            anchor,
            placement,
            offset
        })
    ));
    lay_out(&mut tree, root);
    tree.absolute_position(node)
}

#[test]
fn every_placement_puts_the_node_against_an_anchor_in_another_subtree() {
    // The anchor is 60 x 30 at window (30, 40); the node is 20 x 10.
    for (placement, expected) in [
        (Placement::Below, (30.0, 70.0)),
        (Placement::Above, (30.0, 30.0)),
        (Placement::Start, (10.0, 40.0)),
        (Placement::End, (90.0, 40.0)),
        // A point: the node's centre on it, so its top-left is (10, 5) less.
        (Placement::TopStart, (20.0, 35.0)),
        (Placement::TopEnd, (80.0, 35.0)),
        (Placement::BottomStart, (20.0, 65.0)),
        (Placement::BottomEnd, (80.0, 65.0)),
        (Placement::Center, (50.0, 50.0)),
    ] {
        assert_eq!(place(placement, (0.0, 0.0)), expected, "{placement:?}");
    }
}

#[test]
fn the_offset_moves_the_attachment_point() {
    assert_eq!(place(Placement::TopEnd, (-8.0, 6.0)), (72.0, 41.0));
}

#[test]
fn the_node_stays_a_child_of_its_own_parent_and_is_made_absolute() {
    let (mut tree, root, anchor, node, right) = scene();
    assert!(tree.set_anchor(node, Some(anchored(anchor, Placement::Below))));
    lay_out(&mut tree, root);
    assert_eq!(tree.get(node).unwrap().parent, Some(right));
    assert_eq!(
        tree.get(node).unwrap().layout_style.position,
        Position::Absolute
    );
}

#[test]
fn it_follows_the_anchor_in_the_same_layout() {
    let (mut tree, root, anchor, node, _) = scene();
    tree.set_anchor(node, Some(anchored(anchor, Placement::Below)));
    lay_out(&mut tree, root);
    assert_eq!(tree.absolute_position(node), (30.0, 70.0));
    tree.set_layout_style(anchor, absolute_at(100.0, 120.0, 60.0, 30.0));
    lay_out(&mut tree, root);
    assert_eq!(tree.absolute_position(node), (100.0, 150.0));
    // And when the anchor changes size.
    tree.set_layout_style(anchor, absolute_at(100.0, 120.0, 60.0, 50.0));
    lay_out(&mut tree, root);
    assert_eq!(tree.absolute_position(node), (100.0, 170.0));
}

#[test]
fn a_node_anchored_to_a_node_that_is_anchored_follows_the_chain() {
    let (mut tree, root, anchor, node, right) = scene();
    let third = tree.insert(NodeKind::Rect, sized(8.0, 8.0), paint());
    tree.add_child(right, third);
    tree.set_anchor(node, Some(anchored(anchor, Placement::Below)));
    tree.set_anchor(third, Some(anchored(node, Placement::End)));
    lay_out(&mut tree, root);
    // node is at (30, 70), 20 x 10; third sits after it.
    assert_eq!(tree.absolute_position(third), (50.0, 70.0));
    tree.set_layout_style(anchor, absolute_at(40.0, 40.0, 60.0, 30.0));
    lay_out(&mut tree, root);
    assert_eq!(tree.absolute_position(third), (60.0, 70.0));
}

#[test]
fn an_anchor_that_would_be_a_cycle_is_refused() {
    let (mut tree, _, anchor, node, right) = scene();
    assert!(tree.anchor_cycle(node, node), "itself");
    assert!(tree.anchor_cycle(right, node), "a node inside it");
    assert!(!tree.anchor_cycle(node, anchor));
    assert!(!tree.set_anchor(node, Some(anchored(node, Placement::Below))));
    assert!(!tree.set_anchor(right, Some(anchored(node, Placement::Below))));
    assert!(tree.get(right).unwrap().anchored.is_none());
}

#[test]
fn two_nodes_anchored_to_each_other_are_given_up_on() {
    let (mut tree, root, _, node, _) = scene();
    let other = tree.insert(NodeKind::Rect, sized(20.0, 10.0), paint());
    tree.add_child(root, other);
    tree.set_anchor(node, Some(anchored(other, Placement::End)));
    tree.set_anchor(other, Some(anchored(node, Placement::End)));
    lay_out(&mut tree, root);
    let conflicts = tree.take_anchor_conflicts();
    assert!(
        !conflicts.is_empty(),
        "no fixed point: each is always 20 px past the other"
    );
    // Given up on, they are back in the flow, so a later layout is stable.
    lay_out(&mut tree, root);
    assert!(tree.take_anchor_conflicts().is_empty());
}

#[test]
fn clearing_the_anchor_puts_the_node_back_in_the_flow() {
    let (mut tree, root, anchor, node, _) = scene();
    lay_out(&mut tree, root);
    let flowed = tree.absolute_position(node);
    tree.set_anchor(node, Some(anchored(anchor, Placement::Below)));
    lay_out(&mut tree, root);
    assert_ne!(tree.absolute_position(node), flowed);
    assert!(tree.set_anchor(node, None));
    lay_out(&mut tree, root);
    assert_eq!(tree.absolute_position(node), flowed);
    assert_eq!(
        tree.get(node).unwrap().layout_style.position,
        Position::Relative
    );
}

#[test]
fn a_removed_anchor_leaves_the_node_where_it_was_and_removal_is_forgotten() {
    let (mut tree, root, anchor, node, _) = scene();
    tree.set_anchor(node, Some(anchored(anchor, Placement::Below)));
    lay_out(&mut tree, root);
    let at = tree.absolute_position(node);
    assert!(tree.remove(anchor));
    lay_out(&mut tree, root);
    assert_eq!(tree.absolute_position(node), at);
    assert!(tree.remove(node));
    lay_out(&mut tree, root);
    assert!(tree.take_anchor_conflicts().is_empty());
}
