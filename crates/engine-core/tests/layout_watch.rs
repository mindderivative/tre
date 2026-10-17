//! 0.5.7 (#178): `Tree::watch_layout` and `take_layout_changes`, which a node's
//! `layout` event is built on: a watched node reports its laid-out size when it
//! first has one and whenever it changes, by comparison, so every cause is seen.

use engine_core::{CalcLen, NodeId, NodeKind, PaintProperties, Tree};
use peniko::Color;
use taffy::prelude::{AvailableSpace, Size, Style, auto, length};

fn paint() -> PaintProperties {
    PaintProperties::new(Color::from_rgba8(0, 0, 0, 255), 0.0, 1.0)
}

fn lay_out(tree: &mut Tree, root: NodeId) {
    tree.compute_layout(
        root,
        Size {
            width: AvailableSpace::Definite(500.0),
            height: AvailableSpace::Definite(300.0),
        },
    );
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

/// A 300 x 200 parent holding one 50 x 20 child.
fn scene() -> (Tree, NodeId, NodeId) {
    let mut tree = Tree::new();
    let parent = tree.insert(NodeKind::Container, sized(300.0, 200.0), paint());
    let child = tree.insert(NodeKind::Rect, sized(50.0, 20.0), paint());
    tree.add_child(parent, child);
    (tree, parent, child)
}

#[test]
fn the_first_layout_is_reported_once_then_nothing_until_it_changes() {
    let (mut tree, parent, child) = scene();
    tree.watch_layout(child, true);
    assert!(
        tree.take_layout_changes().is_empty(),
        "no layout yet: nothing to report"
    );
    lay_out(&mut tree, parent);
    assert_eq!(
        tree.take_layout_changes(),
        vec![(child, None, (50.0, 20.0))]
    );
    assert!(tree.take_layout_changes().is_empty());
    lay_out(&mut tree, parent);
    assert!(
        tree.take_layout_changes().is_empty(),
        "laid out again, same size"
    );
}

#[test]
fn a_change_of_size_is_reported_with_the_old_and_new_sizes() {
    let (mut tree, parent, child) = scene();
    tree.watch_layout(child, true);
    lay_out(&mut tree, parent);
    tree.take_layout_changes();
    tree.set_layout_style(child, sized(80.0, 20.0));
    lay_out(&mut tree, parent);
    assert_eq!(
        tree.take_layout_changes(),
        vec![(child, Some((50.0, 20.0)), (80.0, 20.0))]
    );
}

#[test]
fn a_parent_resizing_a_percentage_child_is_reported_for_the_child() {
    let (mut tree, parent, child) = scene();
    tree.set_layout_style(
        child,
        Style {
            size: Size {
                width: taffy::prelude::percent(0.5),
                height: length(20.0),
            },
            ..Default::default()
        },
    );
    tree.watch_layout(child, true);
    lay_out(&mut tree, parent);
    assert_eq!(
        tree.take_layout_changes(),
        vec![(child, None, (150.0, 20.0))]
    );
    tree.set_layout_style(parent, sized(400.0, 200.0));
    lay_out(&mut tree, parent);
    assert_eq!(
        tree.take_layout_changes(),
        vec![(child, Some((150.0, 20.0)), (200.0, 20.0))]
    );
}

#[test]
fn a_calc_size_reports_where_it_settles_not_the_passes_on_the_way() {
    let (mut tree, parent, child) = scene();
    tree.set_layout_style(
        child,
        Style {
            size: Size {
                width: auto(),
                height: length(20.0),
            },
            ..Default::default()
        },
    );
    tree.set_calc_size(
        child,
        false,
        Some(CalcLen {
            percent: 1.0,
            px: -16.0,
        }),
    );
    tree.watch_layout(child, true);
    lay_out(&mut tree, parent);
    assert_eq!(
        tree.take_layout_changes(),
        vec![(child, None, (284.0, 20.0))]
    );
}

#[test]
fn unwatching_and_removal_stop_the_reports() {
    let (mut tree, parent, child) = scene();
    tree.watch_layout(child, true);
    lay_out(&mut tree, parent);
    tree.take_layout_changes();
    tree.watch_layout(child, false);
    tree.set_layout_style(child, sized(80.0, 20.0));
    lay_out(&mut tree, parent);
    assert!(tree.take_layout_changes().is_empty());

    let other = tree.insert(NodeKind::Rect, sized(10.0, 10.0), paint());
    tree.add_child(parent, other);
    tree.watch_layout(other, true);
    assert!(tree.remove(other));
    lay_out(&mut tree, parent);
    assert!(
        tree.take_layout_changes().is_empty(),
        "a removed node is forgotten"
    );
}

#[test]
fn nodes_that_changed_together_report_in_a_fixed_order() {
    let (mut tree, parent, first) = scene();
    let second = tree.insert(NodeKind::Rect, sized(30.0, 30.0), paint());
    tree.add_child(parent, second);
    tree.watch_layout(second, true);
    tree.watch_layout(first, true);
    lay_out(&mut tree, parent);
    let reported: Vec<NodeId> = tree.take_layout_changes().iter().map(|c| c.0).collect();
    let mut sorted = reported.clone();
    sorted.sort_by_key(|&id| engine_core::node_id_as_u64(id));
    assert_eq!(reported, sorted);
    assert_eq!(reported.len(), 2);
}
