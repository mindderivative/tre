use super::*;
use crate::animation::MotionCurve;
use crate::node::ScrollViewState;
use peniko::Color;
use std::time::Duration;
use taffy::prelude::{FlexDirection, length};

fn leaf(width: f32, height: f32) -> (NodeKind, Style, PaintProperties) {
    (
        NodeKind::Rect,
        Style {
            size: Size {
                width: length(width),
                height: length(height),
            },
            ..Default::default()
        },
        PaintProperties::new(Color::from_rgba8(255, 0, 0, 255), 0.0, 1.0),
    )
}

/// M29 Phase 1 (§5, §6): real regression coverage for the
/// centralized dirty flag -- `take_dirty()` must report `true` after
/// each real category of mutation (structural, paint-property via
/// the `get_mut` chokepoint, animation-tick) and
/// `false` on a `Tree` touched only by read-only calls in between.
#[test]
fn take_dirty_reports_true_after_each_real_mutation_category_and_false_between() {
    let mut tree = Tree::new();
    assert!(
        tree.take_dirty(),
        "a brand new Tree must report dirty once, so the very first frame always paints"
    );
    assert!(
        !tree.take_dirty(),
        "a second read with no mutation in between must report clean"
    );

    // Structural: insert.
    let (kind, style, paint) = leaf(10.0, 10.0);
    let id = tree.insert(kind, style, paint);
    assert!(tree.take_dirty(), "insert must mark the tree dirty");
    assert!(!tree.take_dirty(), "a plain read must not");
    let _ = tree.get(id);
    let _ = tree.focused();
    assert!(
        !tree.take_dirty(),
        "read-only accessors must never mark the tree dirty"
    );

    // Paint-property, via the get_mut chokepoint every raw Node
    // mutation (`animate`, `node.set`, etc.) goes through.
    tree.get_mut(id).unwrap().paint.opacity.current = 0.5;
    assert!(tree.take_dirty(), "get_mut must mark the tree dirty");
    assert!(!tree.take_dirty());

    // Animation-tick: a real mid-flight animation must report dirty
    // via tick_all's own already-computed `any_active`, without a
    // fresh mutation happening inside tick_all's own caller code.
    tree.get_mut(id).unwrap().paint.opacity.animate_to(
        0.0,
        Duration::from_millis(100),
        MotionCurve::Linear,
        Instant::now(),
    );
    tree.take_dirty(); // consume the dirty bit the animate_to's own get_mut just set
    let (any_active, _) = tree.tick_all(Instant::now() + Duration::from_millis(50));
    assert!(
        any_active,
        "the animation must genuinely still be mid-flight"
    );
    assert!(
        tree.take_dirty(),
        "a mid-flight tick_all must mark the tree dirty"
    );

    let (any_active, _) = tree.tick_all(Instant::now() + Duration::from_millis(500));
    assert!(!any_active, "the animation must have completed by now");
    assert!(
        !tree.take_dirty(),
        "tick_all with nothing active must not mark the tree dirty"
    );
}

#[test]
fn insert_creates_a_parentless_node() {
    let mut tree = Tree::new();
    let (kind, style, paint) = leaf(10.0, 10.0);
    let id = tree.insert(kind, style, paint);

    let node = tree.get(id).expect("just-inserted node must be present");
    assert_eq!(node.id, id);
    assert_eq!(node.parent, None);
    assert!(node.children.is_empty());
}

#[test]
fn remove_drops_a_whole_subtree_and_detaches_from_its_parent() {
    let mut tree = Tree::new();
    let (k, s, p) = leaf(10.0, 10.0);
    let root = tree.insert(k, s, p);
    let (k, s, p) = leaf(10.0, 10.0);
    let child = tree.insert(k, s, p);
    tree.add_child(root, child);
    let (k, s, p) = leaf(5.0, 5.0);
    let grandchild = tree.insert(k, s, p);
    tree.add_child(child, grandchild);

    // A sibling that must survive the removal, to prove this isn't
    // just clearing the whole tree.
    let (k, s, p) = leaf(10.0, 10.0);
    let sibling = tree.insert(k, s, p);
    tree.add_child(root, sibling);

    let removed = tree.remove(child);
    assert!(removed, "remove must report true for a real, present id");

    assert!(
        tree.get(child).is_none(),
        "the removed node itself must be gone"
    );
    assert!(
        tree.get(grandchild).is_none(),
        "the whole subtree must be gone, not just its root"
    );
    assert!(
        tree.get(sibling).is_some(),
        "an unrelated sibling must survive"
    );
    assert_eq!(
        tree.get(root).unwrap().children,
        vec![sibling],
        "the parent's own children list must no longer mention the removed node"
    );
}

/// M6 Phase 1 (§8): the trivial cycle case -- a node can't become
/// its own child.
/// Issue #14: `add_child` of a node attached elsewhere moves it --
/// one parent, in both `children` lists and taffy -- rather than
/// leaving it under two parents.
#[test]
fn add_child_moves_an_already_attached_child() {
    let mut tree = Tree::new();
    let (k, s, p) = leaf(10.0, 10.0);
    let first = tree.insert(k, s, p);
    let (k, s, p) = leaf(10.0, 10.0);
    let second = tree.insert(k, s, p);
    let (k, s, p) = leaf(10.0, 10.0);
    let child = tree.insert(k, s, p);

    tree.add_child(first, child);
    tree.add_child(second, child);
    assert_eq!(tree.get(child).unwrap().parent, Some(second));
    assert!(tree.get(first).unwrap().children.is_empty());
    assert_eq!(tree.get(second).unwrap().children, vec![child]);
    let taffy_children = |tree: &Tree, id| tree.taffy.children(tree.taffy_nodes[id]).unwrap();
    assert!(taffy_children(&tree, first).is_empty());
    assert_eq!(taffy_children(&tree, second).len(), 1);

    tree.add_child(second, child); // onto its own parent: no duplicate
    assert_eq!(tree.get(second).unwrap().children, vec![child]);
}

#[test]
fn try_add_child_rejects_a_node_as_its_own_child() {
    let mut tree = Tree::new();
    let (k, s, p) = leaf(10.0, 10.0);
    let a = tree.insert(k, s, p);

    assert!(
        !tree.try_add_child(a, a),
        "attaching a node under itself must be rejected"
    );
    assert_eq!(
        tree.get(a).unwrap().children,
        Vec::<NodeId>::new(),
        "a rejected attach must leave the tree completely untouched"
    );
}

/// The real, multi-level case: attaching an ancestor as a child of
/// its own descendant would corrupt the tree into a cycle no
/// traversal could ever terminate on.
#[test]
fn try_add_child_rejects_an_ancestor_as_a_descendants_child() {
    let mut tree = Tree::new();
    let (k, s, p) = leaf(10.0, 10.0);
    let grandparent = tree.insert(k, s, p);
    let (k, s, p) = leaf(10.0, 10.0);
    let parent = tree.insert(k, s, p);
    let (k, s, p) = leaf(10.0, 10.0);
    let child = tree.insert(k, s, p);
    tree.add_child(grandparent, parent);
    tree.add_child(parent, child);

    assert!(
        !tree.try_add_child(child, grandparent),
        "attaching grandparent as child's own child must be rejected -- grandparent is \
         already child's ancestor"
    );
    assert_eq!(
        tree.get(child).unwrap().children,
        Vec::<NodeId>::new(),
        "a rejected attach must leave the tree completely untouched"
    );
    assert_eq!(
        tree.get(grandparent).unwrap().parent,
        None,
        "grandparent's own real parent (none) must be unchanged"
    );
}

/// The real, closely-related corruption risk found by reading
/// `add_child` closely (PLAN.md): re-parenting an already-attached
/// node must move it, not duplicate its parent pointer -- the same
/// "`add_child` has no dedup" bug class M4 Phase 7/9 each already
/// found once, now guarded against for `try_add_child`'s own
/// general-purpose reparenting.
#[test]
fn try_add_child_moves_an_already_attached_node_instead_of_duplicating_it() {
    let mut tree = Tree::new();
    let (k, s, p) = leaf(10.0, 10.0);
    let old_parent = tree.insert(k, s, p);
    let (k, s, p) = leaf(10.0, 10.0);
    let new_parent = tree.insert(k, s, p);
    let (k, s, p) = leaf(10.0, 10.0);
    let child = tree.insert(k, s, p);
    tree.add_child(old_parent, child);

    assert!(tree.try_add_child(new_parent, child));

    assert_eq!(
        tree.get(old_parent).unwrap().children,
        Vec::<NodeId>::new(),
        "the old parent must no longer list the moved child"
    );
    assert_eq!(
        tree.get(new_parent).unwrap().children,
        vec![child],
        "the new parent must list the moved child exactly once"
    );
    assert_eq!(
        tree.get(child).unwrap().parent,
        Some(new_parent),
        "the child's own parent pointer must point at its new parent"
    );
}

/// M96: `insert_child` reorders siblings by final index, and a move
/// within one tree keeps the node's identity and focus.
#[test]
fn insert_child_reorders_and_keeps_focus_within_a_tree() {
    let mut tree = Tree::new();
    let (k, s, p) = leaf(10.0, 10.0);
    let parent = tree.insert(k, s, p);
    let kids: Vec<NodeId> = (0..3)
        .map(|_| {
            let (k, s, p) = leaf(10.0, 10.0);
            let id = tree.insert(k, s, p);
            tree.add_child(parent, id);
            id
        })
        .collect();
    tree.focused = Some(kids[0]);

    assert!(tree.insert_child(parent, 2, kids[0]));
    assert_eq!(
        tree.get(parent).unwrap().children,
        vec![kids[1], kids[2], kids[0]]
    );
    assert!(tree.insert_child(parent, 0, kids[2]));
    assert_eq!(
        tree.get(parent).unwrap().children,
        vec![kids[2], kids[1], kids[0]]
    );
    assert_eq!(
        tree.focused,
        Some(kids[0]),
        "a move within one tree keeps focus"
    );
    let taffy_order: Vec<taffy::NodeId> = tree.taffy.children(tree.taffy_nodes[parent]).unwrap();
    let expected: Vec<taffy::NodeId> = [kids[2], kids[1], kids[0]]
        .iter()
        .map(|&id| tree.taffy_nodes[id])
        .collect();
    assert_eq!(
        taffy_order, expected,
        "taffy's child order follows the tree's"
    );
}

/// M96: moving a subtree into a different, detached tree clears focus
/// held inside it, since it's no longer reachable where it was.
#[test]
fn insert_child_into_another_root_forgets_focus() {
    let mut tree = Tree::new();
    let (k, s, p) = leaf(10.0, 10.0);
    let root = tree.insert(k, s, p);
    let (k, s, p) = leaf(10.0, 10.0);
    let detached = tree.insert(k, s, p);
    let (k, s, p) = leaf(10.0, 10.0);
    let child = tree.insert(k, s, p);
    tree.add_child(root, child);
    tree.focused = Some(child);

    assert!(tree.insert_child(detached, 0, child));
    assert_eq!(tree.focused, None);
    assert_eq!(tree.root_of(child), detached);
    assert!(
        !tree.insert_child(child, 0, detached),
        "an ancestor can't become a child"
    );
}

/// M96: a collectible subtree lives while anything references any node
/// in it, and is freed once nothing does.
#[test]
fn a_collectible_subtree_is_freed_once_unreferenced() {
    let mut tree = Tree::new();
    let (k, s, p) = leaf(10.0, 10.0);
    let root = tree.insert(k, s, p);
    let (k, s, p) = leaf(10.0, 10.0);
    let child = tree.insert(k, s, p);
    tree.add_child(root, child);

    tree.detach_collectible(root);
    assert!(tree.collect_unreferenced(root, |id| id == child).is_empty());
    let freed = tree.collect_unreferenced(child, |_| false);
    assert_eq!(freed.len(), 2);
    assert!(tree.get(root).is_none() && tree.get(child).is_none());
}

/// M96: attaching a collectible root makes it ordinary again, and a
/// node detached the legacy way is never collected.
#[test]
fn attached_and_legacy_detached_nodes_are_never_collected() {
    let mut tree = Tree::new();
    let (k, s, p) = leaf(10.0, 10.0);
    let root = tree.insert(k, s, p);
    let (k, s, p) = leaf(10.0, 10.0);
    let made = tree.insert(k, s, p);
    tree.detach_collectible(made);
    assert!(tree.try_add_child(root, made));
    assert!(tree.collect_unreferenced(made, |_| false).is_empty());
    assert!(tree.get(made).is_some(), "attached: kept");

    let (k, s, p) = leaf(10.0, 10.0);
    let legacy = tree.insert(k, s, p);
    tree.add_child(root, legacy);
    tree.detach(root, legacy);
    assert!(tree.collect_unreferenced(legacy, |_| false).is_empty());
    assert!(tree.get(legacy).is_some(), "legacy-detached: kept");
}

/// M96: a virtual list shows exactly the rows its viewport meets, adopts
/// a caller-built row, and releases rows scrolled away without freeing
/// them.
#[test]
fn a_virtual_list_adopts_visible_rows_and_releases_the_rest() {
    let mut tree = Tree::new();
    let list = tree.insert(
        NodeKind::VirtualList(VirtualListState::new(100, ItemExtent::Fixed(10.0))),
        Style {
            size: Size {
                width: length(50.0),
                height: length(35.0),
            },
            ..Default::default()
        },
        PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 1.0),
    );
    let available = Size {
        width: AvailableSpace::Definite(50.0),
        height: AvailableSpace::Definite(35.0),
    };
    tree.compute_layout(list, available);
    assert_eq!(tree.virtual_list_visible(list), 0..4, "rows 0-3 meet 0..35");

    let rows: Vec<NodeId> = (0..4)
        .map(|idx| {
            let (k, s, p) = leaf(1.0, 1.0);
            let row = tree.insert(k, s, p);
            assert!(tree.virtual_list_adopt(list, idx, row));
            row
        })
        .collect();
    tree.compute_layout(list, available);
    assert_eq!(tree.layout(rows[3]).location.y, 30.0);
    assert_eq!(tree.layout(rows[3]).size.width, 50.0, "full width");

    tree.scroll_virtual_list_by(list, 25.0);
    let visible = tree.virtual_list_visible(list);
    assert_eq!(visible, 2..6);
    let released = tree.virtual_list_release_outside(list, visible);
    assert_eq!(released, vec![rows[0], rows[1]]);
    assert!(
        tree.get(rows[0]).is_some_and(|n| n.parent.is_none()),
        "detached, alive"
    );
}

/// M96: content added while a layer is open goes beneath it, and an
/// anchored layer with no room on its preferred side flips, then shifts
/// to stay inside the root.
#[test]
fn layers_stay_on_top_and_flip_and_shift_to_fit() {
    let mut tree = Tree::new();
    let (k, s, p) = leaf(200.0, 100.0);
    let root = tree.insert(k, s, p);
    let (k, mut s, p) = leaf(20.0, 10.0);
    s.position = Position::Absolute;
    s.inset = TaffyRect {
        left: length(190.0),
        top: length(85.0),
        right: auto(),
        bottom: auto(),
    };
    let anchor = tree.insert(k, s, p);
    tree.add_child(root, anchor);
    let (k, s, p) = leaf(50.0, 40.0);
    let layer = tree.insert(k, s, p);
    let meta = OverlayMeta {
        anchor: Some(anchor),
        placement: Some(Placement::Below),
        ..Default::default()
    };
    assert!(tree.show_layer(root, layer, meta));
    let (k, s, p) = leaf(5.0, 5.0);
    let later = tree.insert(k, s, p);
    tree.add_child(root, later);
    assert_eq!(tree.get(root).unwrap().children, vec![anchor, later, layer]);
    assert_eq!(tree.content_children(root), &[anchor, later]);

    tree.compute_layout(
        root,
        Size {
            width: AvailableSpace::Definite(200.0),
            height: AvailableSpace::Definite(100.0),
        },
    );
    assert_eq!(
        tree.overlay_meta(layer).unwrap().placed,
        Some(Placement::Above)
    );
    let at = tree.layout(layer).location;
    assert_eq!(
        (at.x, at.y),
        (150.0, 45.0),
        "above the anchor, shifted inside"
    );
}

#[test]
fn remove_of_an_unknown_id_is_a_harmless_no_op() {
    let mut tree = Tree::new();
    let (k, s, p) = leaf(10.0, 10.0);
    let real = tree.insert(k, s, p);
    tree.remove(real); // consume the only real id so it's now stale

    assert!(
        !tree.remove(real),
        "removing an already-removed id must report false, not panic"
    );
}

#[test]
fn absolute_position_accumulates_through_nested_parents() {
    let mut tree = Tree::new();
    let root_style = Style {
        display: taffy::Display::Flex,
        padding: taffy::prelude::Rect {
            left: length(5.0),
            top: length(5.0),
            right: length(5.0),
            bottom: length(5.0),
        },
        size: Size {
            width: length(300.0),
            height: length(300.0),
        },
        ..Default::default()
    };
    let (_, _, root_paint) = leaf(0.0, 0.0);
    let root = tree.insert(NodeKind::Container, root_style, root_paint);

    let inner_style = Style {
        display: taffy::Display::Flex,
        flex_direction: FlexDirection::Row,
        size: Size {
            width: length(200.0),
            height: length(200.0),
        },
        ..Default::default()
    };
    let (_, _, inner_paint) = leaf(0.0, 0.0);
    let inner = tree.insert(NodeKind::Container, inner_style, inner_paint);
    tree.add_child(root, inner);

    // A spacer sibling before the real target, so the target's own
    // parent-relative x is nonzero -- proving real accumulation
    // through two levels, not a coincidence of both being at (0,0).
    let (k, s, p) = leaf(40.0, 40.0);
    let spacer = tree.insert(k, s, p);
    tree.add_child(inner, spacer);
    let (k, s, p) = leaf(40.0, 40.0);
    let target = tree.insert(k, s, p);
    tree.add_child(inner, target);

    tree.compute_layout(
        root,
        Size {
            width: AvailableSpace::Definite(300.0),
            height: AvailableSpace::Definite(300.0),
        },
    );

    // root padding (5,5) + inner's own location (0,0 within root's
    // content box) + spacer's width (40) = target's absolute x.
    let (x, y) = tree.absolute_position(target);
    assert_eq!(x, 5.0 + 40.0);
    assert_eq!(y, 5.0);
}

/// M6 Phase 4 (§8): the real, post-M5-review gap this phase closes --
/// `absolute_position` must report a node's *actual* transformed
/// canvas position, not its plain untransformed layout position, for
/// a node inside an ancestor with a real `transform` -- the same
/// claim `hit_test`/`draw_own` already correctly make (M5 Phase
/// 1/2), now true here too.
#[test]
fn absolute_position_follows_an_ancestor_transform() {
    let mut tree = Tree::new();
    let root_style = Style {
        size: Size {
            width: length(200.0),
            height: length(200.0),
        },
        ..Default::default()
    };
    let (_, _, root_paint) = leaf(0.0, 0.0);
    let root = tree.insert(NodeKind::Container, root_style, root_paint);

    let (_, camera_style, camera_paint) = leaf(200.0, 200.0);
    let camera = tree.insert(NodeKind::Container, camera_style, camera_paint);
    tree.add_child(root, camera);

    let (k, s, p) = leaf(40.0, 40.0);
    let chip = tree.insert(k, s, p);
    tree.add_child(camera, chip);

    tree.compute_layout(
        root,
        Size {
            width: AvailableSpace::Definite(200.0),
            height: AvailableSpace::Definite(200.0),
        },
    );

    // Before any transform: chip sits at its plain local (0,0) --
    // default block layout places the sole child at its parent's
    // origin, matching every other transform test's own established
    // baseline (M5 Phase 1/2).
    assert_eq!(tree.absolute_position(chip), (0.0, 0.0));

    // camera's own transform: scale(2.0) about the origin, then
    // translate by (20, 20) -- chip's local (0,0) maps to canvas
    // (20, 20), not (0, 0).
    tree.get_mut(camera).unwrap().paint.transform.current =
        Affine::translate((20.0, 20.0)) * Affine::scale(2.0);

    assert_eq!(
        tree.absolute_position(chip),
        (20.0, 20.0),
        "absolute_position must report chip's real, transformed canvas position, not \
         its plain untransformed layout position"
    );
}

#[test]
fn open_overlay_positions_below_its_anchor_and_appends_to_root() {
    let mut tree = Tree::new();
    let root_style = Style {
        display: taffy::Display::Flex,
        size: Size {
            width: length(300.0),
            height: length(300.0),
        },
        ..Default::default()
    };
    let (_, _, root_paint) = leaf(0.0, 0.0);
    let root = tree.insert(NodeKind::Container, root_style, root_paint);

    let (k, s, p) = leaf(80.0, 20.0); // the trigger button, e.g. a menu bar item
    let anchor = tree.insert(k, s, p);
    tree.add_child(root, anchor);

    tree.compute_layout(
        root,
        Size {
            width: AvailableSpace::Definite(300.0),
            height: AvailableSpace::Definite(300.0),
        },
    );
    let (anchor_x, anchor_y) = tree.absolute_position(anchor);
    assert_eq!((anchor_x, anchor_y), (0.0, 0.0));

    let (k, s, p) = leaf(120.0, 60.0); // the dropdown menu surface
    let menu = tree.insert(k, s, p);
    tree.open_overlay(
        root,
        anchor,
        menu,
        OverlayMeta {
            anchor: Some(anchor),
            dismiss_on_outside_click: true,
            dismiss_on_escape: true,
            modal: false,
            ..Default::default()
        },
    );

    // A second compute_layout is required after open_overlay, per
    // its own doc comment, for the new absolutely-positioned child
    // to actually resolve.
    tree.compute_layout(
        root,
        Size {
            width: AvailableSpace::Definite(300.0),
            height: AvailableSpace::Definite(300.0),
        },
    );

    let menu_layout = tree.layout(menu);
    assert_eq!(
        (menu_layout.location.x, menu_layout.location.y),
        (anchor_x as f32, anchor_y as f32 + 20.0),
        "the menu must land directly below the anchor's own bottom edge"
    );

    assert_eq!(
        tree.get(root).unwrap().children,
        vec![anchor, menu],
        "the overlay must be appended to root's children, after the anchor"
    );

    let meta = tree
        .overlay_meta(menu)
        .expect("the overlay's metadata must be stored, keyed by its own NodeId");
    assert_eq!(meta.anchor, Some(anchor));
    assert!(meta.dismiss_on_outside_click);
    assert!(meta.dismiss_on_escape);
}

#[test]
fn position_overlay_over_covers_an_arbitrary_rect_independent_of_any_anchor() {
    let mut tree = Tree::new();
    let root_style = Style {
        size: Size {
            width: length(300.0),
            height: length(300.0),
        },
        ..Default::default()
    };
    let (_, _, root_paint) = leaf(0.0, 0.0);
    let root = tree.insert(NodeKind::Container, root_style, root_paint);
    tree.compute_layout(
        root,
        Size {
            width: AvailableSpace::Definite(300.0),
            height: AvailableSpace::Definite(300.0),
        },
    );

    // A dock zone's own container -- some arbitrary rect nowhere
    // near the origin, proving this isn't secretly anchor-relative.
    let (k, s, p) = leaf(120.0, 80.0);
    let highlight = tree.insert(k, s, p);
    tree.position_overlay_over(highlight, Rect::new(140.0, 60.0, 260.0, 140.0));

    // Not attached anywhere -- `position_overlay_over` only sets
    // layout_style, matching its own doc comment ("deliberately
    // does not attach content anywhere").
    assert!(tree.get(highlight).unwrap().parent.is_none());

    tree.add_child(root, highlight);
    tree.compute_layout(
        root,
        Size {
            width: AvailableSpace::Definite(300.0),
            height: AvailableSpace::Definite(300.0),
        },
    );

    let (x, y) = tree.absolute_position(highlight);
    let size = tree.layout(highlight).size;
    assert_eq!(
        (x, y),
        (140.0, 60.0),
        "must land exactly at the rect's own origin"
    );
    assert_eq!(
        (size.width, size.height),
        (120.0, 80.0),
        "must be resized to exactly cover the rect, not keep its own prior size"
    );
}

#[test]
fn close_overlay_detaches_the_node_and_drops_its_metadata() {
    let mut tree = Tree::new();
    let root_style = Style {
        size: Size {
            width: length(300.0),
            height: length(300.0),
        },
        ..Default::default()
    };
    let (_, _, root_paint) = leaf(0.0, 0.0);
    let root = tree.insert(NodeKind::Container, root_style, root_paint);
    let (k, s, p) = leaf(80.0, 20.0);
    let anchor = tree.insert(k, s, p);
    tree.add_child(root, anchor);
    tree.compute_layout(
        root,
        Size {
            width: AvailableSpace::Definite(300.0),
            height: AvailableSpace::Definite(300.0),
        },
    );

    let (k, s, p) = leaf(120.0, 60.0);
    let menu = tree.insert(k, s, p);
    tree.open_overlay(
        root,
        anchor,
        menu,
        OverlayMeta {
            anchor: Some(anchor),
            dismiss_on_outside_click: true,
            dismiss_on_escape: true,
            modal: false,
            ..Default::default()
        },
    );

    assert!(tree.close_overlay(menu));
    assert!(
        tree.get(menu).is_some(),
        "M10 Phase 1: the overlay node itself must survive closing -- detached, not \
         destroyed, so `set_context_menu`'s own registered content can be reopened"
    );
    assert!(
        tree.get(menu).unwrap().parent.is_none(),
        "a closed overlay must be genuinely detached from its former parent"
    );
    assert!(
        tree.overlay_meta(menu).is_none(),
        "its metadata must be gone too"
    );
    assert_eq!(
        tree.get(root).unwrap().children,
        vec![anchor],
        "root's children must no longer mention the closed overlay"
    );
    assert!(
        !tree.close_overlay(menu),
        "closing an already-closed overlay must report false"
    );
}

#[test]
fn remove_drops_overlay_metadata_for_removed_overlay_content() {
    // Real regression coverage for the review-found gap: unlike
    // `close_overlay`, `Tree::remove` never cleared `self.overlays`
    // for a removed node that happened to be open overlay content.
    let mut tree = Tree::new();
    let root_style = Style {
        size: Size {
            width: length(300.0),
            height: length(300.0),
        },
        ..Default::default()
    };
    let (_, _, root_paint) = leaf(0.0, 0.0);
    let root = tree.insert(NodeKind::Container, root_style, root_paint);
    let (k, s, p) = leaf(80.0, 20.0);
    let anchor = tree.insert(k, s, p);
    tree.add_child(root, anchor);
    tree.compute_layout(
        root,
        Size {
            width: AvailableSpace::Definite(300.0),
            height: AvailableSpace::Definite(300.0),
        },
    );

    let (k, s, p) = leaf(120.0, 60.0);
    let menu = tree.insert(k, s, p);
    tree.open_overlay(
        root,
        anchor,
        menu,
        OverlayMeta {
            anchor: Some(anchor),
            dismiss_on_outside_click: true,
            dismiss_on_escape: true,
            modal: false,
            ..Default::default()
        },
    );
    assert!(tree.overlay_meta(menu).is_some());

    assert!(tree.remove(menu));
    assert!(
        tree.overlay_meta(menu).is_none(),
        "removing overlay content directly (not through close_overlay) must still drop \
         its metadata, or a future NodeId reusing that slot would inherit stale overlay \
         bookkeeping"
    );

    // Same real gap, one level up: removing an *ancestor* of open
    // overlay content must reach it too, since `remove` recurses
    // into `root`'s own children first.
    let (k, s, p) = leaf(120.0, 60.0);
    let menu2 = tree.insert(k, s, p);
    tree.open_overlay(
        root,
        anchor,
        menu2,
        OverlayMeta {
            anchor: Some(anchor),
            dismiss_on_outside_click: true,
            dismiss_on_escape: true,
            modal: false,
            ..Default::default()
        },
    );
    assert!(tree.overlay_meta(menu2).is_some());

    assert!(tree.remove(root));
    assert!(
        tree.overlay_meta(menu2).is_none(),
        "removing an ancestor of open overlay content must drop its metadata too"
    );
}

/// M10 Phase 1 (§11.3): a root + anchor (0,0)-(80,20) + a real open
/// overlay `menu`, positioned by `open_overlay` itself directly
/// below the anchor -- (0,20)-(120,80) -- with both `dismiss_on_*`
/// flags set as given, ready for a real dispatch.
fn overlay_scene(
    dismiss_on_outside_click: bool,
    dismiss_on_escape: bool,
) -> (Tree, NodeId, NodeId, NodeId) {
    let mut tree = Tree::new();
    let root_style = Style {
        size: Size {
            width: length(300.0),
            height: length(300.0),
        },
        ..Default::default()
    };
    let (_, _, root_paint) = leaf(0.0, 0.0);
    let root = tree.insert(NodeKind::Container, root_style, root_paint);
    let (k, s, p) = leaf(80.0, 20.0);
    let anchor = tree.insert(k, s, p);
    tree.add_child(root, anchor);
    tree.compute_layout(
        root,
        Size {
            width: AvailableSpace::Definite(300.0),
            height: AvailableSpace::Definite(300.0),
        },
    );

    let (k, s, p) = leaf(120.0, 60.0);
    let menu = tree.insert(k, s, p);
    tree.open_overlay(
        root,
        anchor,
        menu,
        OverlayMeta {
            anchor: Some(anchor),
            dismiss_on_outside_click,
            dismiss_on_escape,
            modal: false,
            ..Default::default()
        },
    );
    tree.compute_layout(
        root,
        Size {
            width: AvailableSpace::Definite(300.0),
            height: AvailableSpace::Definite(300.0),
        },
    );
    (tree, root, anchor, menu)
}

/// M94: a root with one 50x50 child at (20, 30), laid out.
fn one_child_scene() -> (Tree, NodeId, NodeId) {
    let mut tree = Tree::new();
    let (k, s, p) = leaf(200.0, 200.0);
    let root = tree.insert(k, s, p);
    let (k, mut s, p) = leaf(50.0, 50.0);
    s.position = taffy::style::Position::Absolute;
    s.inset = taffy::geometry::Rect {
        left: taffy::style::LengthPercentageAuto::length(20.0),
        top: taffy::style::LengthPercentageAuto::length(30.0),
        right: taffy::style::LengthPercentageAuto::auto(),
        bottom: taffy::style::LengthPercentageAuto::auto(),
    };
    let child = tree.insert(k, s, p);
    tree.add_child(root, child);
    tree.compute_layout(
        root,
        Size {
            width: AvailableSpace::Definite(200.0),
            height: AvailableSpace::Definite(200.0),
        },
    );
    (tree, root, child)
}

#[test]
fn window_to_local_and_local_to_window_are_inverses() {
    let (tree, _, child) = one_child_scene();
    let local = tree.window_to_local(child, Point::new(25.0, 40.0));
    assert!((local.x - 5.0).abs() < 1e-9 && (local.y - 10.0).abs() < 1e-9);
    let back = tree.local_to_window(child, local);
    assert!((back.x - 25.0).abs() < 1e-9 && (back.y - 40.0).abs() < 1e-9);
}

#[test]
fn pointer_capture_is_released_when_its_node_is_removed() {
    let (mut tree, _, child) = one_child_scene();
    tree.set_pointer_capture(Some(child));
    assert_eq!(tree.pointer_capture(), Some(child));
    tree.remove(child);
    assert_eq!(tree.pointer_capture(), None);
    // A stale id can't take capture.
    tree.set_pointer_capture(Some(child));
    assert_eq!(tree.pointer_capture(), None);
}

#[test]
fn pointer_left_clears_hover_with_a_hover_changed_outcome() {
    let (mut tree, root, child) = one_child_scene();
    tree.dispatch(
        root,
        InputEvent::PointerMoved {
            position: Point::new(25.0, 40.0),
        },
        Instant::now(),
    );
    let outcome = tree.dispatch(root, InputEvent::PointerLeft, Instant::now());
    assert_eq!(
        outcome,
        DispatchOutcome::HoverChanged {
            old: Some(child),
            new: None,
        }
    );
    // Nothing hovered any more, so a second leave reports nothing.
    let outcome = tree.dispatch(root, InputEvent::PointerLeft, Instant::now());
    assert_eq!(outcome, DispatchOutcome::None);
}

#[test]
fn named_keys_modifiers_and_scale_changes_are_plumbing_only() {
    let (mut tree, root, _) = one_child_scene();
    for event in [
        InputEvent::Key {
            name: "f5".into(),
            pressed: true,
            repeat: false,
        },
        InputEvent::ModifiersChanged(crate::Modifiers {
            shift: true,
            ..Default::default()
        }),
        InputEvent::ScaleFactorChanged { scale_factor: 2.0 },
    ] {
        assert_eq!(
            tree.dispatch(root, event, Instant::now()),
            DispatchOutcome::None
        );
    }
}

/// M94: three 40x40 focusable children of a root, laid out.
fn three_focusable() -> (Tree, NodeId, [NodeId; 3]) {
    let mut tree = Tree::new();
    let (k, s, p) = leaf(300.0, 300.0);
    let root = tree.insert(k, s, p);
    let mut ids = Vec::new();
    for _ in 0..3 {
        let (k, s, p) = leaf(40.0, 40.0);
        let id = tree.insert(k, s, p);
        tree.get_mut(id).unwrap().access.focusable = Some(true);
        tree.add_child(root, id);
        ids.push(id);
    }
    tree.compute_layout(
        root,
        Size {
            width: AvailableSpace::Definite(300.0),
            height: AvailableSpace::Definite(300.0),
        },
    );
    (tree, root, [ids[0], ids[1], ids[2]])
}

#[test]
fn tab_index_orders_positive_first_then_tree_order_and_skips_negative() {
    let (mut tree, root, [a, b, c]) = three_focusable();
    tree.get_mut(c).unwrap().access.tab_index = 1;
    tree.get_mut(b).unwrap().access.tab_index = -1;
    let mut visited = Vec::new();
    for _ in 0..3 {
        tree.move_focus(root, FocusDirection::Next);
        visited.push(tree.focused().unwrap());
    }
    assert_eq!(visited, vec![c, a, c]);
}

#[test]
fn a_press_focuses_the_nearest_focusable_ancestor() {
    let (mut tree, root, [a, ..]) = three_focusable();
    let (k, s, p) = leaf(10.0, 10.0);
    let inner = tree.insert(k, s, p);
    tree.add_child(a, inner);
    tree.compute_layout(
        root,
        Size {
            width: AvailableSpace::Definite(300.0),
            height: AvailableSpace::Definite(300.0),
        },
    );
    let (x, y) = tree.absolute_position(inner);
    let outcome = tree.dispatch(
        root,
        InputEvent::PointerPressed {
            position: Point::new(x + 5.0, y + 5.0),
            button: PointerButton::Primary,
        },
        Instant::now(),
    );
    assert_eq!(
        outcome,
        DispatchOutcome::FocusChanged {
            old: None,
            new: Some(a),
        }
    );
}

#[test]
fn explicit_accessibility_fields_reach_the_access_tree() {
    use crate::access::{AccessValue, Action, Live, Role};
    let (mut tree, root, [a, ..]) = three_focusable();
    {
        let access = &mut tree.get_mut(a).unwrap().access;
        access.role = Role::Slider;
        access.value = Some(AccessValue::Number(3.0));
        access.value_min = Some(0.0);
        access.value_max = Some(10.0);
        access.value_step = Some(1.0);
        access.expanded = Some(false);
        access.level = Some(2);
        access.live = Some(Live::Polite);
        access.hidden = true;
    }
    let update = tree.build_access_update(root);
    let (_, node) = update
        .nodes
        .iter()
        .find(|(id, _)| *id == to_access_id(a))
        .unwrap();
    assert_eq!(node.numeric_value(), Some(3.0));
    assert_eq!(node.min_numeric_value(), Some(0.0));
    assert_eq!(node.max_numeric_value(), Some(10.0));
    assert_eq!(node.numeric_value_step(), Some(1.0));
    assert_eq!(node.is_expanded(), Some(false));
    assert_eq!(node.level(), Some(2));
    assert_eq!(node.live(), Some(Live::Polite));
    assert!(node.is_hidden());
    for action in [
        Action::Focus,
        Action::Increment,
        Action::Decrement,
        Action::SetValue,
        Action::Expand,
        Action::Collapse,
    ] {
        assert!(node.supports_action(action), "{action:?} offered");
    }
}

#[test]
fn button_roles_offer_click_and_focusable_offers_focus() {
    use crate::access::{AccessNodeData, Action, Role};
    let mut access = AccessNodeData::new(Role::Button);
    assert_eq!(access.offered_actions(), vec![Action::Click]);
    access.focusable = Some(true);
    assert!(access.offered_actions().contains(&Action::Focus));
    assert!(access.in_tab_order());
    access.tab_index = -1;
    assert!(!access.in_tab_order());
    let group = AccessNodeData::new(Role::Group);
    assert!(group.offered_actions().is_empty());
    assert!(!group.in_tab_order());
}

#[test]
fn clear_focus_reports_the_transition_once() {
    let (mut tree, _, child) = one_child_scene();
    tree.set_focus_to(child);
    assert_eq!(tree.clear_focus(), Some((Some(child), None)));
    assert_eq!(tree.clear_focus(), None);
}

#[test]
fn a_real_press_outside_a_dismiss_on_outside_click_overlay_closes_it() {
    let (mut tree, root, _, menu) = overlay_scene(true, true);
    let outcome = tree.dispatch(
        root,
        InputEvent::PointerPressed {
            position: Point::new(250.0, 250.0), // well outside the menu's own (0,20)-(120,80)
            button: PointerButton::Primary,
        },
        Instant::now(),
    );
    assert_eq!(outcome, DispatchOutcome::None);
    assert!(
        tree.overlay_meta(menu).is_none(),
        "a real outside press must genuinely close the overlay, not just hide it"
    );
    assert!(
        !tree.get(root).unwrap().children.contains(&menu),
        "the closed overlay must no longer be displayed, detached from root"
    );
}

#[test]
fn a_real_press_inside_the_overlay_leaves_it_open_and_dispatches_normally() {
    let (mut tree, root, _, menu) = overlay_scene(true, true);
    let outcome = tree.dispatch(
        root,
        InputEvent::PointerPressed {
            position: Point::new(50.0, 50.0), // inside the menu's own (0,20)-(120,80)
            button: PointerButton::Primary,
        },
        Instant::now(),
    );
    assert_eq!(outcome, DispatchOutcome::None);
    assert!(
        tree.get(menu).is_some(),
        "a press genuinely inside the overlay must never dismiss it"
    );
    assert_eq!(
        tree.pressed,
        Some((PointerButton::Primary, menu)),
        "an in-bounds press must still register normally -- dismissal must not swallow it"
    );
}

#[test]
fn a_real_press_back_on_the_anchor_does_not_dismiss_its_own_overlay() {
    // A real regression found while verifying Phase 1 end to end
    // (Python's own `test_right_clicking_the_same_anchor_twice_
    // does_not_crash`): right-clicking the same anchor a second
    // time must not have its own press treated as an "outside
    // click" against the overlay it's about to (safely, no-op)
    // reopen -- the anchor itself is genuinely excluded.
    let (mut tree, root, anchor, menu) = overlay_scene(true, true);
    let anchor_center = {
        let (x, y) = tree.absolute_position(anchor);
        let layout = tree.layout(anchor);
        Point::new(
            x + f64::from(layout.size.width) / 2.0,
            y + f64::from(layout.size.height) / 2.0,
        )
    };
    tree.dispatch(
        root,
        InputEvent::PointerPressed {
            position: anchor_center,
            button: PointerButton::Secondary,
        },
        Instant::now(),
    );
    assert!(
        tree.get(menu).is_some(),
        "a press back on the overlay's own anchor must not dismiss it"
    );
}

#[test]
fn dismiss_on_outside_click_false_survives_a_real_outside_press() {
    let (mut tree, root, _, menu) = overlay_scene(false, true);
    tree.dispatch(
        root,
        InputEvent::PointerPressed {
            position: Point::new(250.0, 250.0),
            button: PointerButton::Primary,
        },
        Instant::now(),
    );
    assert!(
        tree.get(menu).is_some(),
        "dismiss_on_outside_click: false must never be dismissed by an outside press"
    );
}

/// M30 Phase 4 Step 1 (§11.3): the real, confirmed gap this
/// milestone's own scoping text named, now proven fixed -- before
/// `OverlayMeta.modal` existed, `dismiss_on_outside_click: false`
/// meant an outside press fell all the way through to `hit_test`
/// on the background, exactly like no overlay were open at all.
/// A real modal dialog needs the opposite: never dismiss on an
/// outside press, but never let it reach the background either.
#[test]
fn a_press_outside_a_modal_overlay_is_consumed_without_dismissing_it() {
    let mut tree = Tree::new();
    let root_style = Style {
        size: Size {
            width: length(300.0),
            height: length(300.0),
        },
        ..Default::default()
    };
    let (_, _, root_paint) = leaf(0.0, 0.0);
    let root = tree.insert(NodeKind::Container, root_style, root_paint);

    // A real background button, positioned squarely at the point
    // this test presses -- if the press reached it, `tree.pressed`
    // would be `Some((_, background))`, the real, observable proof
    // a modal overlay failed to block it.
    let (k, s, p) = leaf(40.0, 40.0);
    let background = tree.insert(k, s, p);
    tree.add_child(root, background);

    let (k, s, p) = leaf(80.0, 20.0);
    let anchor = tree.insert(k, s, p);
    tree.add_child(root, anchor);
    tree.compute_layout(
        root,
        Size {
            width: AvailableSpace::Definite(300.0),
            height: AvailableSpace::Definite(300.0),
        },
    );

    let (k, s, p) = leaf(120.0, 60.0);
    let dialog = tree.insert(k, s, p);
    tree.open_overlay(
        root,
        anchor,
        dialog,
        OverlayMeta {
            anchor: Some(anchor),
            dismiss_on_outside_click: false,
            dismiss_on_escape: true,
            modal: true,
            ..Default::default()
        },
    );
    tree.compute_layout(
        root,
        Size {
            width: AvailableSpace::Definite(300.0),
            height: AvailableSpace::Definite(300.0),
        },
    );

    // The background button's own real center -- well outside the
    // dialog's own real bounds (anchored near the top-left).
    let outcome = tree.dispatch(
        root,
        InputEvent::PointerPressed {
            position: Point::new(20.0, 20.0),
            button: PointerButton::Primary,
        },
        Instant::now(),
    );

    assert_eq!(
        outcome,
        DispatchOutcome::None,
        "a press outside a modal overlay must report no outcome, not reach the background"
    );
    assert!(
        tree.pressed.is_none(),
        "a press outside a modal overlay must never register as a real press on the \
         background node underneath it"
    );
    assert!(
        tree.overlay_meta(dialog).is_some(),
        "modal: true with dismiss_on_outside_click: false must not dismiss the overlay"
    );
}

/// `modal`'s own real no-op default, proven directly -- every
/// overlay before this step (context menus, dropdown menus,
/// tooltips) must keep letting an outside press reach the
/// background exactly as it always did, when `dismiss_on_outside_
/// click` is also `false`.
#[test]
fn modal_false_still_lets_an_outside_press_reach_the_background() {
    let mut tree = Tree::new();
    let root_style = Style {
        size: Size {
            width: length(300.0),
            height: length(300.0),
        },
        ..Default::default()
    };
    let (_, _, root_paint) = leaf(0.0, 0.0);
    let root = tree.insert(NodeKind::Container, root_style, root_paint);

    let (k, s, p) = leaf(40.0, 40.0);
    let background = tree.insert(k, s, p);
    tree.add_child(root, background);

    let (k, s, p) = leaf(80.0, 20.0);
    let anchor = tree.insert(k, s, p);
    tree.add_child(root, anchor);
    tree.compute_layout(
        root,
        Size {
            width: AvailableSpace::Definite(300.0),
            height: AvailableSpace::Definite(300.0),
        },
    );

    let (k, s, p) = leaf(120.0, 60.0);
    let menu = tree.insert(k, s, p);
    tree.open_overlay(
        root,
        anchor,
        menu,
        OverlayMeta {
            anchor: Some(anchor),
            dismiss_on_outside_click: false,
            dismiss_on_escape: true,
            modal: false,
            ..Default::default()
        },
    );
    tree.compute_layout(
        root,
        Size {
            width: AvailableSpace::Definite(300.0),
            height: AvailableSpace::Definite(300.0),
        },
    );

    tree.dispatch(
        root,
        InputEvent::PointerPressed {
            position: Point::new(20.0, 20.0),
            button: PointerButton::Primary,
        },
        Instant::now(),
    );

    assert_eq!(
        tree.pressed,
        Some((PointerButton::Primary, background)),
        "modal: false must still let an outside press reach the real background node, \
         the exact real behavior every overlay had before this step"
    );
}

#[test]
fn a_real_escape_dispatch_closes_every_dismiss_on_escape_overlay() {
    let (mut tree, root, _, menu) = overlay_scene(true, true);
    let outcome = tree.dispatch(
        root,
        InputEvent::KeyPressed {
            key: Key::Escape,
            shift: false,
        },
        Instant::now(),
    );
    assert_eq!(outcome, DispatchOutcome::None);
    assert!(
        tree.overlay_meta(menu).is_none(),
        "a real Escape dispatch must genuinely close a dismiss_on_escape overlay"
    );
    assert!(
        !tree.get(root).unwrap().children.contains(&menu),
        "the closed overlay must no longer be displayed, detached from root"
    );
}

#[test]
fn dismiss_on_escape_false_survives_a_real_escape_dispatch() {
    let (mut tree, root, _, menu) = overlay_scene(true, false);
    tree.dispatch(
        root,
        InputEvent::KeyPressed {
            key: Key::Escape,
            shift: false,
        },
        Instant::now(),
    );
    assert!(
        tree.get(menu).is_some(),
        "dismiss_on_escape: false must never be dismissed by an Escape dispatch"
    );
}

/// M10 Phase 1 (§11.3): the exact real scenario that motivated
/// `close_overlay`'s own detach-not-destroy fix -- close an
/// overlay via a real Escape dispatch, then reopen the *same*
/// content node again via `open_overlay`. Must not panic (a
/// destroyed content `NodeId` would make `open_overlay`'s own
/// `self.get(content).expect(...)` fail), and the reopened overlay
/// must be for real (attached to root again).
#[test]
fn a_dismissed_overlays_own_content_can_be_reopened() {
    let (mut tree, root, anchor, menu) = overlay_scene(true, true);
    tree.dispatch(
        root,
        InputEvent::KeyPressed {
            key: Key::Escape,
            shift: false,
        },
        Instant::now(),
    );
    assert!(
        tree.overlay_meta(menu).is_none(),
        "sanity: must be closed first"
    );

    tree.open_overlay(
        root,
        anchor,
        menu,
        OverlayMeta {
            anchor: Some(anchor),
            dismiss_on_outside_click: true,
            dismiss_on_escape: true,
            modal: false,
            ..Default::default()
        },
    );

    assert!(
        tree.overlay_meta(menu).is_some(),
        "the same content NodeId must be reopenable after a real dismissal"
    );
    assert!(
        tree.get(root).unwrap().children.contains(&menu),
        "the reopened overlay must be genuinely attached to root again"
    );
}

/// M30 Phase 8 Step 6 (§11.3): direct, dedicated coverage for the
/// real, confirmed bug this step's own `Main Menu` submenus found
/// live (not assumed in advance, via a genuinely failing example
/// run) -- `open_overlay` always positions content anchor-relative-
/// *below*, so a submenu anchored to an item inside an already-open
/// parent menu genuinely sits outside the parent menu's own bounds.
/// Before the fix, `dismiss_overlays_outside`'s own filter only
/// checked a *candidate* overlay's own bounds, so a press genuinely
/// inside the submenu (but outside the parent menu) was wrongly
/// treated as "outside" the parent menu -- dismissing it and
/// consuming the press via `dispatch`'s own early-return, before the
/// submenu's own content ever got a chance to register the press.
///
/// Builds a two-overlay scene deliberately, not reusing
/// `overlay_scene` (which only ever opens one overlay): a
/// `trigger` (0,0)-(80,20) anchors `parent_menu` (0,20)-(120,40);
/// `parent_item`, a real child of `parent_menu` sized to fill it
/// exactly, anchors `submenu` (0,40)-(120,100) -- so `submenu`
/// genuinely starts exactly where `parent_menu` ends, definitively
/// outside it. A press at `submenu`'s own center (60,70) is inside
/// `submenu` but outside `parent_menu`'s own (0,20)-(120,40) bounds.
#[test]
fn a_press_inside_a_nested_submenu_does_not_dismiss_its_own_parent_menu() {
    let mut tree = Tree::new();
    let root_style = Style {
        size: Size {
            width: length(300.0),
            height: length(300.0),
        },
        ..Default::default()
    };
    let (_, _, root_paint) = leaf(0.0, 0.0);
    let root = tree.insert(NodeKind::Container, root_style, root_paint);
    let (k, s, p) = leaf(80.0, 20.0);
    let trigger = tree.insert(k, s, p);
    tree.add_child(root, trigger);
    tree.compute_layout(
        root,
        Size {
            width: AvailableSpace::Definite(300.0),
            height: AvailableSpace::Definite(300.0),
        },
    );

    let (k, s, p) = leaf(120.0, 20.0);
    let parent_menu = tree.insert(k, s, p);
    tree.open_overlay(
        root,
        trigger,
        parent_menu,
        OverlayMeta {
            anchor: Some(trigger),
            dismiss_on_outside_click: true,
            dismiss_on_escape: true,
            modal: false,
            ..Default::default()
        },
    );
    tree.compute_layout(
        root,
        Size {
            width: AvailableSpace::Definite(300.0),
            height: AvailableSpace::Definite(300.0),
        },
    );

    // A real child of parent_menu, sized to fill it exactly, the
    // way a menu item fills its menu -- its own resolved absolute
    // position is therefore identical to parent_menu's, (0,20).
    let (k, s, p) = leaf(120.0, 20.0);
    let parent_item = tree.insert(k, s, p);
    tree.add_child(parent_menu, parent_item);
    tree.compute_layout(
        root,
        Size {
            width: AvailableSpace::Definite(300.0),
            height: AvailableSpace::Definite(300.0),
        },
    );

    let (k, s, p) = leaf(120.0, 60.0);
    let submenu = tree.insert(k, s, p);
    tree.open_overlay(
        root,
        parent_item,
        submenu,
        OverlayMeta {
            anchor: Some(parent_item),
            dismiss_on_outside_click: true,
            dismiss_on_escape: true,
            modal: false,
            ..Default::default()
        },
    );
    tree.compute_layout(
        root,
        Size {
            width: AvailableSpace::Definite(300.0),
            height: AvailableSpace::Definite(300.0),
        },
    );

    let outcome = tree.dispatch(
        root,
        InputEvent::PointerPressed {
            position: Point::new(60.0, 70.0), // inside submenu's (0,40)-(120,100), outside parent_menu's (0,20)-(120,40)
            button: PointerButton::Primary,
        },
        Instant::now(),
    );

    assert_eq!(outcome, DispatchOutcome::None);
    assert!(
        tree.overlay_meta(parent_menu).is_some(),
        "a real press inside a nested submenu must never dismiss its own parent menu, \
         even though the submenu itself sits outside the parent menu's own bounds"
    );
    assert!(
        tree.overlay_meta(submenu).is_some(),
        "a real press genuinely inside the submenu must never dismiss the submenu itself"
    );
    assert_eq!(
        tree.pressed,
        Some((PointerButton::Primary, submenu)),
        "the press must reach the submenu's own content, not be swallowed by a wrongful \
         dismissal of the parent menu"
    );
}

#[test]
fn add_child_links_both_structures() {
    let mut tree = Tree::new();
    let (k, s, p) = leaf(10.0, 10.0);
    let parent = tree.insert(k, s, p);
    let (k, s, p) = leaf(10.0, 10.0);
    let child = tree.insert(k, s, p);

    tree.add_child(parent, child);

    assert_eq!(tree.get(parent).unwrap().children, vec![child]);
    assert_eq!(tree.get(child).unwrap().parent, Some(parent));
}

#[test]
fn detach_removes_attachment_without_deleting_the_node() {
    let mut tree = Tree::new();
    let (k, s, p) = leaf(10.0, 10.0);
    let parent = tree.insert(k, s, p);
    let (k, s, p) = leaf(10.0, 10.0);
    let child = tree.insert(k, s, p);
    tree.add_child(parent, child);

    tree.detach(parent, child);

    assert_eq!(
        tree.get(parent).unwrap().children,
        Vec::<NodeId>::new(),
        "the parent must no longer list the detached child"
    );
    assert_eq!(
        tree.get(child).unwrap().parent,
        None,
        "the detached child must no longer point back at its old parent"
    );
    assert!(
        tree.get(child).is_some(),
        "unlike remove(), the child node itself must still exist"
    );

    // A detached, reparented node must still work with every
    // ordinary Tree operation -- proving it's a real, live node,
    // not a half-removed one.
    let (k, s, p) = leaf(10.0, 10.0);
    let new_parent = tree.insert(k, s, p);
    tree.add_child(new_parent, child);
    assert_eq!(tree.get(child).unwrap().parent, Some(new_parent));
}

#[test]
fn apply_active_tab_attaches_exactly_one_panel_at_a_time() {
    use crate::dock::DockZone;

    let mut tree = Tree::new();
    let (k, s, p) = leaf(100.0, 100.0);
    let container = tree.insert(k, s, p);

    let (k, s, p) = leaf(50.0, 50.0);
    let tab_a = tree.insert(k, s, p);
    let (k, s, p) = leaf(50.0, 50.0);
    let tab_b = tree.insert(k, s, p);
    let (k, s, p) = leaf(50.0, 50.0);
    let tab_c = tree.insert(k, s, p);

    let mut zone = DockZone::new(100.0);
    zone.panels = smallvec::smallvec![tab_a, tab_b, tab_c];
    zone.active_tab = 0;

    tree.apply_active_tab(container, &zone);
    assert_eq!(
        tree.get(container).unwrap().children,
        vec![tab_a],
        "only the active tab should be attached"
    );

    zone.active_tab = 2;
    tree.apply_active_tab(container, &zone);
    assert_eq!(
        tree.get(container).unwrap().children,
        vec![tab_c],
        "switching tabs must detach the old one and attach the new one"
    );
    assert!(
        tree.get(tab_a).is_some(),
        "a detached (not removed) tab must still exist, ready to be switched back to"
    );

    zone.active_tab = 0;
    tree.apply_active_tab(container, &zone);
    assert_eq!(
        tree.get(container).unwrap().children,
        vec![tab_a],
        "switching back to a previously-detached tab must reattach the same node"
    );
}

fn virtual_list_materializer(idx: usize) -> (NodeKind, Style, PaintProperties) {
    (
        NodeKind::Rect,
        Style {
            size: Size {
                width: length(200.0),
                height: length(20.0),
            },
            ..Default::default()
        },
        // Encode the logical index into the color so a rendering
        // test elsewhere could tell items apart -- not exercised by
        // these engine-core unit tests, which only check `NodeId`/
        // position bookkeeping, not paint.
        PaintProperties::new(Color::from_rgba8((idx % 256) as u8, 0, 0, 255), 0.0, 1.0),
    )
}

#[test]
fn set_virtual_list_window_only_ever_materializes_the_requested_range() {
    // §11.7's own claim: a 100,000-row list never needs 100,000 real
    // `Node`s -- this is the concrete, provable version of that,
    // not just an architectural assertion.
    let mut tree = Tree::new();
    let list = tree.insert(
        NodeKind::VirtualList(VirtualListState::new(100_000, ItemExtent::Fixed(20.0))),
        Style::default(),
        PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 1.0),
    );

    tree.set_virtual_list_window(list, 0..5, virtual_list_materializer);

    let NodeKind::VirtualList(state) = &tree.get(list).unwrap().kind else {
        panic!("expected a VirtualList");
    };
    assert_eq!(
        state.materialized.keys().copied().collect::<Vec<_>>(),
        vec![0, 1, 2, 3, 4],
        "only the requested window should be materialized, not item_count"
    );
    assert_eq!(
        tree.get(list).unwrap().children.len(),
        5,
        "materialized nodes must be real children of the list, not tracked separately"
    );

    tree.compute_layout(
        list,
        Size {
            width: AvailableSpace::Definite(200.0),
            height: AvailableSpace::Definite(400.0),
        },
    );
    for idx in 0..5 {
        let id = state_materialized_id(&tree, list, idx);
        let (_, y) = tree.absolute_position(id);
        assert_eq!(
            y,
            idx as f64 * 20.0,
            "item {idx} must be positioned at its own logical offset, index * item_extent"
        );
    }
}

fn state_materialized_id(tree: &Tree, list: NodeId, idx: usize) -> NodeId {
    let NodeKind::VirtualList(state) = &tree.get(list).unwrap().kind else {
        panic!("expected a VirtualList");
    };
    *state
        .materialized
        .get(&idx)
        .unwrap_or_else(|| panic!("index {idx} expected to be materialized"))
}

#[test]
fn set_virtual_list_window_recycles_scrolled_out_indices_and_invalidates_their_old_node_ids() {
    let mut tree = Tree::new();
    let list = tree.insert(
        NodeKind::VirtualList(VirtualListState::new(100_000, ItemExtent::Fixed(20.0))),
        Style::default(),
        PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 1.0),
    );

    tree.set_virtual_list_window(list, 0..5, virtual_list_materializer);
    let scrolled_away_ids: Vec<NodeId> = (0..3)
        .map(|idx| state_materialized_id(&tree, list, idx))
        .collect();

    // Scroll down: 0..5 -> 3..8. Indices 0,1,2 leave the window,
    // indices 5,6,7 newly enter it.
    tree.set_virtual_list_window(list, 3..8, virtual_list_materializer);

    let NodeKind::VirtualList(state) = &tree.get(list).unwrap().kind else {
        panic!("expected a VirtualList");
    };
    assert_eq!(
        state.materialized.keys().copied().collect::<Vec<_>>(),
        vec![3, 4, 5, 6, 7],
        "the window must shift to exactly the newly-visible range"
    );
    assert_eq!(
        tree.get(list).unwrap().children.len(),
        5,
        "still only 5 real children after scrolling, never growing with item_count"
    );

    // The real generational-safety claim (§5, §11.7): a stray
    // reference to a scrolled-away item's old `NodeId` must fail
    // safely, not silently resolve to whatever a reused slot now
    // holds.
    for old_id in scrolled_away_ids {
        assert!(
            tree.get(old_id).is_none(),
            "a scrolled-away item's old NodeId must no longer resolve to anything"
        );
    }
}

/// A list with a real, definite viewport height, much shorter than
/// its own real content extent (`item_extent * item_count`) -- the
/// exact shape `scroll_virtual_list_by`'s own clamping needs to be
/// real, not vacuous (`Style::default()`'s own auto-sizing, used by
/// this module's other `VirtualList` tests, doesn't give a
/// meaningful "viewport" to clamp against).
fn scrollable_list(item_count: usize) -> (Tree, NodeId) {
    let mut tree = Tree::new();
    let list = tree.insert(
        NodeKind::VirtualList(VirtualListState::new(item_count, ItemExtent::Fixed(20.0))),
        Style {
            size: Size {
                width: length(200.0),
                height: length(100.0),
            },
            ..Default::default()
        },
        PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 1.0),
    );
    tree.compute_layout(
        list,
        Size {
            width: AvailableSpace::Definite(200.0),
            height: AvailableSpace::Definite(100.0),
        },
    );
    (tree, list)
}

#[test]
fn scroll_virtual_list_by_moves_and_clamps_the_offset_at_both_ends() {
    // 20 items * 20px = 400px of real content, in a 100px-tall
    // viewport -- max_offset = 400 - 100 = 300.0.
    let (mut tree, list) = scrollable_list(20);

    tree.scroll_virtual_list_by(list, 50.0);
    let NodeKind::VirtualList(state) = &tree.get(list).unwrap().kind else {
        panic!("expected a VirtualList");
    };
    assert_eq!(state.scroll_offset.current, 50.0);

    // Past the real upper bound -- must clamp to max_offset, not
    // overshoot into content that doesn't exist.
    tree.scroll_virtual_list_by(list, 1000.0);
    let NodeKind::VirtualList(state) = &tree.get(list).unwrap().kind else {
        panic!("expected a VirtualList");
    };
    assert_eq!(
        state.scroll_offset.current, 300.0,
        "must clamp to the real max_offset (content_extent - viewport_height), not overshoot"
    );

    // Past the real lower bound -- must clamp to 0.0, not go
    // negative.
    tree.scroll_virtual_list_by(list, -10_000.0);
    let NodeKind::VirtualList(state) = &tree.get(list).unwrap().kind else {
        panic!("expected a VirtualList");
    };
    assert_eq!(
        state.scroll_offset.current, 0.0,
        "must clamp to 0.0, not go negative"
    );
}

#[test]
fn scroll_virtual_list_by_cannot_scroll_a_list_shorter_than_its_own_viewport() {
    // 3 items * 20px = 60px of real content, in a 100px-tall
    // viewport -- there's nothing to reveal, so max_offset must be
    // 0.0, not negative.
    let (mut tree, list) = scrollable_list(3);
    tree.scroll_virtual_list_by(list, 50.0);
    let NodeKind::VirtualList(state) = &tree.get(list).unwrap().kind else {
        panic!("expected a VirtualList");
    };
    assert_eq!(
        state.scroll_offset.current, 0.0,
        "a list shorter than its own viewport must not be scrollable at all"
    );
}

/// M12 Phase 1 (§11.7): a real, non-uniform set of resolved offsets
/// -- rows of height 10, 30, 15, 25, matching neither a uniform
/// spacing nor a simple arithmetic sequence, so a test passing here
/// can't be an accident of `Fixed`-shaped math still secretly being
/// used underneath.
fn variable_offsets() -> Vec<(usize, f64)> {
    // Heights: [10, 30, 15, 25] -> cumulative offsets [0, 10, 40, 55],
    // total extent (index 4, one past the last item) = 80.
    vec![(0, 0.0), (1, 10.0), (2, 40.0), (3, 55.0), (4, 80.0)]
}

#[test]
fn offset_of_and_total_extent_match_the_uniform_formula_for_fixed() {
    let state = VirtualListState::new(10, ItemExtent::Fixed(20.0));
    for idx in 0..10 {
        assert_eq!(
            state.offset_of(idx),
            idx as f64 * 20.0,
            "Fixed must keep computing idx * item_extent exactly as before this phase"
        );
    }
    assert_eq!(
        state.total_extent(),
        200.0,
        "Fixed must keep computing item_count * item_extent exactly as before this phase"
    );
}

#[test]
fn offset_of_and_total_extent_use_real_resolved_offsets_for_variable() {
    let mut tree = Tree::new();
    let list = tree.insert(
        NodeKind::VirtualList(VirtualListState::new(4, ItemExtent::Variable)),
        Style::default(),
        PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 1.0),
    );
    tree.set_virtual_list_resolved_offsets(list, variable_offsets());

    let NodeKind::VirtualList(state) = &tree.get(list).unwrap().kind else {
        panic!("expected a VirtualList");
    };
    assert_eq!(state.offset_of(0), 0.0);
    assert_eq!(state.offset_of(1), 10.0);
    assert_eq!(state.offset_of(2), 40.0);
    assert_eq!(
        state.offset_of(3),
        55.0,
        "must reflect the real, non-uniform spacing"
    );
    assert_eq!(
        state.total_extent(),
        80.0,
        "total_extent must be the real resolved offset one past the last item, not \
         item_count * some average extent"
    );
}

#[test]
fn an_unresolved_variable_list_has_no_extent_and_does_not_panic() {
    let state = VirtualListState::new(4, ItemExtent::Variable);
    // Nothing resolved yet (no layout, or a raising size_hint): every
    // row sits at 0 and the list has no extent to scroll.
    assert_eq!(state.offset_of(2), 0.0);
    assert_eq!(state.total_extent(), 0.0);
}

#[test]
fn set_virtual_list_window_positions_items_at_their_real_non_uniform_offsets() {
    let mut tree = Tree::new();
    let list = tree.insert(
        NodeKind::VirtualList(VirtualListState::new(4, ItemExtent::Variable)),
        Style::default(),
        PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 1.0),
    );
    tree.set_virtual_list_resolved_offsets(list, variable_offsets());

    tree.set_virtual_list_window(list, 0..4, virtual_list_materializer);
    tree.compute_layout(
        list,
        Size {
            width: AvailableSpace::Definite(200.0),
            height: AvailableSpace::Definite(400.0),
        },
    );

    let expected = [0.0, 10.0, 40.0, 55.0];
    for (idx, &expected_y) in expected.iter().enumerate() {
        let id = state_materialized_id(&tree, list, idx);
        let (_, y) = tree.absolute_position(id);
        assert_eq!(
            y, expected_y,
            "item {idx} must be positioned at its own real, non-uniform resolved offset, \
             not idx * some uniform extent"
        );
    }
}

#[test]
fn scroll_virtual_list_by_clamps_against_the_real_non_uniform_total_extent() {
    let mut tree = Tree::new();
    let list = tree.insert(
        NodeKind::VirtualList(VirtualListState::new(4, ItemExtent::Variable)),
        Style {
            size: Size {
                width: length(200.0),
                height: length(30.0),
            },
            ..Default::default()
        },
        PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 1.0),
    );
    tree.compute_layout(
        list,
        Size {
            width: AvailableSpace::Definite(200.0),
            height: AvailableSpace::Definite(30.0),
        },
    );
    tree.set_virtual_list_resolved_offsets(list, variable_offsets());

    // Real content extent 80.0, viewport 30.0 -> max_offset = 50.0,
    // not the wrong 4 * some uniform guess a Fixed-shaped formula
    // would produce.
    tree.scroll_virtual_list_by(list, 1000.0);
    let NodeKind::VirtualList(state) = &tree.get(list).unwrap().kind else {
        panic!("expected a VirtualList");
    };
    assert_eq!(
        state.scroll_offset.current, 50.0,
        "must clamp to the real non-uniform total_extent minus viewport height"
    );
}

// -----------------------------------------------------------------
// M47 (§5, §7, §11.7): VirtualList's own real scrollbar thumb --
// mirrors the ScrollView thumb test cluster above (M38 Phase 6),
// reusing the identical `scrollable_list` fixture (200x100
// viewport, Fixed(20.0) item extent) the scroll-clamping tests
// above already use, so the real numbers match exactly: 20 items *
// 20px = 400px content in a 100px-tall viewport.
// -----------------------------------------------------------------

#[test]
fn virtual_list_thumb_geometry_computes_the_real_track_thumb_and_along_values() {
    // track = 100 - 2*2 (margin) = 96. thumb = max(96*(100/400),
    // 32 (min length)) = max(24, 32) = 32 -- the identical real
    // numbers ScrollView's own equivalent test already proves,
    // confirming VirtualList's `total_extent()`-based geometry
    // agrees with the measured-child-based one for the same shape.
    let (tree, list) = scrollable_list(20);
    let NodeKind::VirtualList(state) = &tree.get(list).unwrap().kind else {
        panic!("expected a VirtualList");
    };
    assert_eq!(
        state.thumb_geometry(100.0),
        (96.0, 32.0, 2.0),
        "at scroll 0, `along` must sit at the real starting margin"
    );
}

#[test]
fn virtual_list_thumb_geometry_along_tracks_real_scroll_progress() {
    let (mut tree, list) = scrollable_list(20);
    tree.scroll_virtual_list_by(list, 150.0); // half of max_scroll (300)
    let NodeKind::VirtualList(state) = &tree.get(list).unwrap().kind else {
        panic!("expected a VirtualList");
    };
    let (track, thumb, along) = state.thumb_geometry(100.0);
    assert_eq!(
        (track, thumb),
        (96.0, 32.0),
        "track/thumb don't depend on scroll position"
    );
    assert!(
        (along - 34.0).abs() < 0.001,
        "at half scroll, `along` must sit halfway across the real (track - thumb) = 64px \
         of travel (2 + 64*0.5 = 34), got {along}"
    );
}

#[test]
fn grabs_virtual_list_thumb_is_true_only_within_the_real_thumb_plus_slop() {
    // 200px-wide viewport: thumb sits on the right edge, x in
    // [200-4-2, 200-2] = [194, 198], y in [2, 34] at scroll 0 (the
    // proof above).
    let (tree, list) = scrollable_list(20);
    assert!(
        tree.grabs_virtual_list_thumb(list, Point::new(196.0, 18.0)),
        "a point squarely inside the real thumb rect must grab it"
    );
    assert!(
        tree.grabs_virtual_list_thumb(list, Point::new(194.0 - 5.0, 18.0)),
        "a point just within the real SCROLLBAR_GRAB_SLOP tolerance must still grab it"
    );
    assert!(
        !tree.grabs_virtual_list_thumb(list, Point::new(10.0, 10.0)),
        "a point nowhere near the real thumb (e.g. over the scrolled content) must not grab it"
    );
}

#[test]
fn virtual_list_thumb_does_not_grab_when_theres_nothing_to_scroll() {
    // 3 items * 20px = 60px of content in a 100px-tall viewport --
    // the identical real "nothing to scroll" shape `scroll_virtual_
    // list_by_cannot_scroll_a_list_shorter_than_its_own_viewport`
    // already proves for wheel scrolling; the thumb must agree.
    let (tree, list) = scrollable_list(3);
    assert!(
        !tree.grabs_virtual_list_thumb(list, Point::new(196.0, 18.0)),
        "no thumb exists to grab when total_extent() <= viewport"
    );
}

#[test]
fn update_virtual_list_thumb_drag_moves_the_scroll_offset_proportionally_to_pointer_travel() {
    let (mut tree, list) = scrollable_list(20);
    // Start a real drag exactly the way `PointerPressed`'s own
    // dispatch arm does: anchor the current pointer coordinate and
    // scroll offset, then mark the list as the live drag target.
    let NodeKind::VirtualList(state) = &mut tree.get_mut(list).unwrap().kind else {
        panic!("expected a VirtualList");
    };
    state.thumb_drag_anchor = Some((18.0, 0.0));
    tree.dragging = Some(list);

    // track - thumb = 96 - 32 = 64px of real thumb travel maps to
    // the real 300px of max_scroll -- moving the pointer down by
    // 32px (half the real travel) must move scroll by half of
    // max_scroll (150).
    tree.update_virtual_list_thumb_drag(list, Point::new(0.0, 50.0));
    let NodeKind::VirtualList(state) = &tree.get(list).unwrap().kind else {
        panic!("expected a VirtualList");
    };
    assert!(
        (state.scroll_offset.current - 150.0).abs() < 0.001,
        "half the real thumb travel must move scroll by half of max_scroll, got {}",
        state.scroll_offset.current
    );
}

#[test]
fn update_virtual_list_thumb_drag_uses_the_real_non_uniform_total_extent_for_variable() {
    // Real proof `total_extent()`, not a Fixed-shaped formula, is
    // what the thumb drag clamps against for a Variable-extent
    // list. Rows of height 40/60/50/50 (non-uniform, cumulative
    // offsets [0, 40, 100, 150], total_extent 200) in a 100px-tall
    // viewport -- deliberately *not* `scroll_virtual_list_by_
    // clamps_against_the_real_non_uniform_total_extent`'s own
    // tiny 30px-viewport fixture: that one leaves zero real thumb
    // travel (`SCROLLBAR_MIN_LENGTH` fills the whole track), a
    // real, correct no-op for *drag* that this test needs to avoid
    // to actually exercise the drag math.
    let mut tree = Tree::new();
    let list = tree.insert(
        NodeKind::VirtualList(VirtualListState::new(4, ItemExtent::Variable)),
        Style {
            size: Size {
                width: length(200.0),
                height: length(100.0),
            },
            ..Default::default()
        },
        PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 1.0),
    );
    tree.compute_layout(
        list,
        Size {
            width: AvailableSpace::Definite(200.0),
            height: AvailableSpace::Definite(100.0),
        },
    );
    tree.set_virtual_list_resolved_offsets(
        list,
        vec![(0, 0.0), (1, 40.0), (2, 100.0), (3, 150.0), (4, 200.0)],
    );

    let NodeKind::VirtualList(state) = &mut tree.get_mut(list).unwrap().kind else {
        panic!("expected a VirtualList");
    };
    state.thumb_drag_anchor = Some((0.0, 0.0));
    tree.dragging = Some(list);

    // track = 100 - 4 = 96, thumb = max(96*(100/200), 32) = 48,
    // travel = 48 -- real room to drag, unlike the tiny-viewport
    // fixture above. A drag far past that travel must clamp to the
    // real max_scroll (200 - 100 = 100.0), not the wrong value a
    // Fixed-shaped formula (4 * some uniform guess) would produce.
    tree.update_virtual_list_thumb_drag(list, Point::new(0.0, 10_000.0));
    let NodeKind::VirtualList(state) = &tree.get(list).unwrap().kind else {
        panic!("expected a VirtualList");
    };
    assert_eq!(
        state.scroll_offset.current, 100.0,
        "must clamp to the real non-uniform total_extent minus viewport height"
    );
}

#[test]
fn dispatch_scroll_over_a_virtual_lists_child_updates_its_real_scroll_offset() {
    let (mut tree, list) = scrollable_list(20);
    tree.set_virtual_list_window(list, 0..5, virtual_list_materializer);
    tree.compute_layout(
        list,
        Size {
            width: AvailableSpace::Definite(200.0),
            height: AvailableSpace::Definite(100.0),
        },
    );

    // Item 1's own real slot (y in [20, 40)) -- a real scroll
    // gesture over a materialized *child*, not the list's own root
    // pixel, must still bubble up to the list's own scroll offset.
    let outcome = tree.dispatch(
        list,
        InputEvent::Scroll {
            delta: ScrollDelta::Lines(0.0, -2.0),
            position: Point::new(100.0, 30.0),
        },
        Instant::now(),
    );

    assert_eq!(outcome, DispatchOutcome::None);
    let NodeKind::VirtualList(state) = &tree.get(list).unwrap().kind else {
        panic!("expected a VirtualList");
    };
    assert_eq!(
        state.scroll_offset.current, 40.0,
        "2.0 lines * this crate's own 20px-per-line convention == 40.0px"
    );
}

/// M37 (§5, §7, §11.7): the real, decisive regression proof this
/// whole fix exists for -- a real point-based hit-test at a
/// materialized item's own genuine post-scroll screen position
/// must now resolve to that exact item, not a stale, wrong one.
/// Mirrors the exact scratch investigation M36's own scoping ran
/// (removed after use there): a real 20-item `VirtualList`,
/// `set_virtual_list_window(0..5, ...)`, scrolled by 40px (two
/// full item-slots) -- item index 2's own real content-relative
/// slot (y 40..60) now paints at real screen y 0..20, and item
/// index 4's own real content-relative slot (y 80..100) now paints
/// at real screen y 40..60. Before this fix, hit-testing still
/// used each item's stale, un-adjusted `layout_style` position, so
/// a real point at item 2's own genuine post-scroll screen
/// position (y=10) would have resolved to item 4 (whose own real,
/// stale, un-adjusted slot -- 80..100 -- has no overlap with y=10
/// at all, so it would actually have resolved to *nothing*, an
/// even more direct proof of the real bug this closes).
#[test]
fn hit_test_after_a_real_scroll_resolves_the_materialized_items_own_genuine_post_scroll_position() {
    let (mut tree, list) = scrollable_list(20);
    tree.set_virtual_list_window(list, 0..5, virtual_list_materializer);
    tree.compute_layout(
        list,
        Size {
            width: AvailableSpace::Definite(200.0),
            height: AvailableSpace::Definite(100.0),
        },
    );
    tree.scroll_virtual_list_by(list, 40.0);
    tree.compute_layout(
        list,
        Size {
            width: AvailableSpace::Definite(200.0),
            height: AvailableSpace::Definite(100.0),
        },
    );

    let item2 = match &tree.get(list).unwrap().kind {
        NodeKind::VirtualList(state) => *state.materialized.get(&2).unwrap(),
        _ => panic!("expected VirtualList"),
    };
    let hit = tree.hit_test(list, Point::new(100.0, 10.0));
    assert_eq!(
        hit,
        Some(item2),
        "a real point at item 2's own genuine post-scroll screen position must hit it, \
         not a stale item or nothing at all"
    );
}

#[test]
fn dispatch_scroll_that_hits_nothing_is_a_true_no_op() {
    let mut tree = Tree::new();
    let (kind, style, paint) = leaf(50.0, 50.0);
    let root = tree.insert(kind, style, paint);
    tree.compute_layout(
        root,
        Size {
            width: AvailableSpace::Definite(50.0),
            height: AvailableSpace::Definite(50.0),
        },
    );

    // Hits `root` itself (a plain Rect, no VirtualList ancestor at
    // all) -- must not panic, and there's nothing real to assert
    // changed, since nothing in this tree can scroll.
    let outcome = tree.dispatch(
        root,
        InputEvent::Scroll {
            delta: ScrollDelta::Lines(0.0, -2.0),
            position: Point::new(25.0, 25.0),
        },
        Instant::now(),
    );
    assert_eq!(outcome, DispatchOutcome::None);
}

/// Three fixed-size children in a row, via `FlexDirection::Row` with
/// no gap -- a deterministic layout with a known, hand-computable
/// answer, so this proves `taffy`'s layout output is actually wired
/// through `Tree`, not just that it doesn't panic.
#[test]
fn compute_layout_positions_row_children_left_to_right() {
    let mut tree = Tree::new();
    let root_style = Style {
        display: taffy::Display::Flex,
        flex_direction: FlexDirection::Row,
        size: Size {
            width: length(300.0),
            height: length(100.0),
        },
        ..Default::default()
    };
    let (_, _, root_paint) = leaf(0.0, 0.0);
    let root = tree.insert(NodeKind::Container, root_style, root_paint);

    let mut children = Vec::new();
    for _ in 0..3 {
        let (kind, style, paint) = leaf(100.0, 100.0);
        let child = tree.insert(kind, style, paint);
        tree.add_child(root, child);
        children.push(child);
    }

    tree.compute_layout(
        root,
        Size {
            width: AvailableSpace::Definite(300.0),
            height: AvailableSpace::Definite(100.0),
        },
    );

    for (i, &child) in children.iter().enumerate() {
        let layout = tree.layout(child);
        assert_eq!(layout.size.width, 100.0);
        assert_eq!(layout.size.height, 100.0);
        assert_eq!(layout.location.x, i as f32 * 100.0);
        assert_eq!(layout.location.y, 0.0);
    }
}

#[test]
fn tick_all_reports_active_and_advances_every_node() {
    use std::time::Duration;

    let mut tree = Tree::new();
    let (kind, style, mut paint) = leaf(10.0, 10.0);
    let start = Instant::now();
    paint.opacity.animate_to(
        0.0,
        Duration::from_secs(1),
        crate::animation::MotionCurve::Linear,
        start,
    );
    let id = tree.insert(kind, style, paint);

    let (still_active, _completed) = tree.tick_all(start + Duration::from_millis(500));
    assert!(still_active);
    let node = tree.get(id).unwrap();
    assert!((node.paint.opacity.current - 0.5).abs() < 0.01);

    let (still_active, _completed) = tree.tick_all(start + Duration::from_secs(2));
    assert!(!still_active);
    assert_eq!(tree.get(id).unwrap().paint.opacity.current, 0.0);
}

/// M7 Phase 3 (§7.1): `ThemeChanged` is plumbing only, the identical
/// "true no-op" contract `Scroll` already established -- `engine-py`
/// handles the real color-resolution/tint-push side effect directly
/// on the raw event, not through `Tree::dispatch`'s own return value.
#[test]
fn theme_changed_dispatches_to_a_true_no_op() {
    let mut tree = Tree::new();
    let (kind, style, paint) = leaf(10.0, 10.0);
    let root = tree.insert(kind, style, paint);

    let outcome = tree.dispatch(
        root,
        InputEvent::ThemeChanged { dark: true },
        Instant::now(),
    );

    assert_eq!(outcome, DispatchOutcome::None);
}

/// M32 Phase 2 (§4, §5): the real gap this phase closes -- "nothing
/// resizes any node's box when its window resizes." Unlike
/// `ThemeChanged`, `Tree::dispatch` genuinely mutates `root`'s own
/// `layout_style.size` here (a pure taffy concern `engine-core`
/// fully owns), and a subsequent `compute_layout` call must
/// actually reflect it -- not just that the style field changed,
/// but that a real fresh layout pass produces the new real size.
#[test]
fn resized_grows_the_roots_own_layout_box_and_a_fresh_layout_reflects_it() {
    let mut tree = Tree::new();
    let (kind, style, paint) = leaf(100.0, 100.0);
    let root = tree.insert(kind, style, paint);

    tree.compute_layout(
        root,
        Size {
            width: AvailableSpace::Definite(100.0),
            height: AvailableSpace::Definite(100.0),
        },
    );
    assert_eq!(tree.layout(root).size.width, 100.0);
    assert_eq!(tree.layout(root).size.height, 100.0);

    let outcome = tree.dispatch(
        root,
        InputEvent::Resized {
            width: 300.0,
            height: 250.0,
        },
        Instant::now(),
    );
    assert_eq!(outcome, DispatchOutcome::None);
    assert_eq!(
        tree.get(root).unwrap().layout_style.size.width,
        length(300.0)
    );

    // The real point: a fresh `compute_layout` after the resize
    // must genuinely produce the new size, not the stale one --
    // proving the mutation is real taffy `Style`, not a field
    // `compute_layout` itself ignores.
    tree.compute_layout(
        root,
        Size {
            width: AvailableSpace::Definite(300.0),
            height: AvailableSpace::Definite(250.0),
        },
    );
    assert_eq!(tree.layout(root).size.width, 300.0);
    assert_eq!(tree.layout(root).size.height, 250.0);
}

/// A 100x100 root with two overlapping 60x60 children at the same
/// position -- `second` is added later, so it's `first`'s topmost
/// sibling per §6's own children-list-order-is-paint-order rule, and
/// `hit_test` must pick it.
fn overlapping_siblings() -> (Tree, NodeId, NodeId, NodeId) {
    let mut tree = Tree::new();
    let root_style = Style {
        display: taffy::Display::Flex,
        size: Size {
            width: length(100.0),
            height: length(100.0),
        },
        ..Default::default()
    };
    let (_, _, root_paint) = leaf(0.0, 0.0);
    let root = tree.insert(NodeKind::Container, root_style, root_paint);

    let zero_inset = TaffyRect {
        left: length(0.0),
        top: length(0.0),
        right: auto(),
        bottom: auto(),
    };

    let (k, s, p) = leaf(60.0, 60.0);
    let mut absolute = s;
    absolute.position = Position::Absolute;
    absolute.inset = zero_inset;
    let first = tree.insert(k, absolute, p);
    tree.add_child(root, first);

    let (k, s, p) = leaf(60.0, 60.0);
    let mut absolute = s;
    absolute.position = Position::Absolute;
    absolute.inset = zero_inset;
    let second = tree.insert(k, absolute, p);
    tree.add_child(root, second);

    tree.compute_layout(
        root,
        Size {
            width: AvailableSpace::Definite(100.0),
            height: AvailableSpace::Definite(100.0),
        },
    );
    (tree, root, first, second)
}

#[test]
fn hit_test_reaches_the_topmost_of_two_overlapping_siblings() {
    let (tree, root, _first, second) = overlapping_siblings();
    let hit = tree.hit_test(root, Point::new(30.0, 30.0));
    assert_eq!(
        hit,
        Some(second),
        "the later (topmost-painted) sibling must win a point both overlap"
    );
}

#[test]
fn hit_test_misses_entirely_outside_every_nodes_bounds() {
    let (tree, root, _first, _second) = overlapping_siblings();
    // root itself spans exactly [0,100)x[0,100) (it's a real,
    // hit-testable node too, by design -- every node is a valid hit
    // target by its own bounds, matching §11.10's own text), so the
    // only genuinely-outside point is beyond root's own extent.
    assert_eq!(tree.hit_test(root, Point::new(150.0, 150.0)), None);
}

#[test]
fn hit_test_reaches_an_appended_overlay_over_background_content() {
    let mut tree = Tree::new();
    let root_style = Style {
        display: taffy::Display::Flex,
        size: Size {
            width: length(300.0),
            height: length(300.0),
        },
        ..Default::default()
    };
    let (_, _, root_paint) = leaf(0.0, 0.0);
    let root = tree.insert(NodeKind::Container, root_style, root_paint);

    // Background content covering the whole canvas.
    let (k, s, p) = leaf(300.0, 300.0);
    let background = tree.insert(k, s, p);
    tree.add_child(root, background);

    // A small anchor button sitting on top of the background, at
    // the canvas's own top-left -- an ordinary toolbar-button shape,
    // not something that itself covers the whole canvas (unlike
    // `background`), so `open_overlay`'s own "below the anchor's
    // bottom edge" placement lands the menu somewhere still on
    // screen and still overlapping `background`.
    let zero_inset = TaffyRect {
        left: length(0.0),
        top: length(0.0),
        right: auto(),
        bottom: auto(),
    };
    let (k, s, p) = leaf(80.0, 20.0);
    let mut anchor_style = s;
    anchor_style.position = Position::Absolute;
    anchor_style.inset = zero_inset;
    let anchor = tree.insert(k, anchor_style, p);
    tree.add_child(root, anchor);

    tree.compute_layout(
        root,
        Size {
            width: AvailableSpace::Definite(300.0),
            height: AvailableSpace::Definite(300.0),
        },
    );

    let (k, s, p) = leaf(120.0, 60.0);
    let menu = tree.insert(k, s, p);
    tree.open_overlay(
        root,
        anchor,
        menu,
        OverlayMeta {
            anchor: Some(anchor),
            dismiss_on_outside_click: true,
            dismiss_on_escape: true,
            modal: false,
            ..Default::default()
        },
    );
    tree.compute_layout(
        root,
        Size {
            width: AvailableSpace::Definite(300.0),
            height: AvailableSpace::Definite(300.0),
        },
    );

    // (10, 25) sits inside both the full-canvas background AND the
    // overlay menu (positioned just below the anchor button's
    // bottom edge, at y=20, per `open_overlay`) -- the point this
    // test exists to prove: the overlay wins, not the background
    // it's stacked above.
    let hit = tree.hit_test(root, Point::new(10.0, 25.0));
    assert_eq!(
        hit,
        Some(menu),
        "an appended overlay must win hit-testing over the background it covers, \
         the same append-order-is-paint-order convention step 13 already proved for paint"
    );
}

/// M5 Phase 2 (§11.10): a real, non-identity ancestor `transform`
/// must shift where its child is actually hit -- the same claim
/// M5 Phase 1's `engine-render` pixel test proved for painting, one
/// layer down. Uses a pure-translate transform, not scale, so the
/// expected hit point is exact integer arithmetic, not an
/// approximation.
#[test]
fn hit_test_follows_an_ancestor_translate_transform() {
    let mut tree = Tree::new();
    let root_style = Style {
        size: Size {
            width: length(200.0),
            height: length(200.0),
        },
        ..Default::default()
    };
    let (_, _, root_paint) = leaf(0.0, 0.0);
    let root = tree.insert(NodeKind::Container, root_style, root_paint);

    let (_, camera_style, camera_paint) = leaf(200.0, 200.0);
    let camera = tree.insert(NodeKind::Container, camera_style, camera_paint);
    tree.add_child(root, camera);

    let (k, s, p) = leaf(50.0, 50.0);
    let chip = tree.insert(k, s, p);
    tree.add_child(camera, chip);

    tree.compute_layout(
        root,
        Size {
            width: AvailableSpace::Definite(200.0),
            height: AvailableSpace::Definite(200.0),
        },
    );

    // Before any transform: `chip` occupies local/canvas (0,0)-(50,50)
    // (default block layout places the sole child at its parent's
    // origin) -- (25,25) hits it. `camera` itself is a real,
    // full-canvas (200x200) hit-testable node too (every node is a
    // valid hit target by its own bounds, matching the existing
    // `hit_test_misses_entirely_outside_every_nodes_bounds` test's
    // own comment) -- (150,150) misses `chip` but still lands on
    // `camera`'s own bounds, not `None`.
    assert_eq!(tree.hit_test(root, Point::new(25.0, 25.0)), Some(chip));
    assert_eq!(tree.hit_test(root, Point::new(150.0, 150.0)), Some(camera));

    // `camera`'s own transform translates by (100, 100) -- this
    // moves `camera` itself as well as `chip` (§11.9's own "exactly
    // like nested <g transform> in SVG": a node's own transform
    // applies to itself, not just its descendants, proven at the
    // paint level by M5 Phase 1). `chip` now occupies canvas
    // (100,100)-(150,150); `camera`'s own footprint is now entirely
    // off the original (0,0)-(200,200) canvas on its near edges.
    tree.get_mut(camera).unwrap().paint.transform.current = Affine::translate((100.0, 100.0));

    assert_eq!(
        tree.hit_test(root, Point::new(25.0, 25.0)),
        Some(root),
        "chip's and camera's old, untransformed footprint must no longer hit either of \
         them -- the point now falls through to root's own untransformed full-canvas \
         bounds, the real background beneath"
    );
    assert_eq!(
        tree.hit_test(root, Point::new(125.0, 125.0)),
        Some(chip),
        "chip's new, transformed position must hit it"
    );
}

/// The scale half of the same claim: a non-uniform composed
/// transform (translate * scale) must also grow/shrink the
/// hit-testable area, not just move it.
#[test]
fn hit_test_follows_an_ancestor_scale_and_translate_transform() {
    let mut tree = Tree::new();
    let root_style = Style {
        size: Size {
            width: length(200.0),
            height: length(200.0),
        },
        ..Default::default()
    };
    let (_, _, root_paint) = leaf(0.0, 0.0);
    let root = tree.insert(NodeKind::Container, root_style, root_paint);

    let (_, camera_style, camera_paint) = leaf(200.0, 200.0);
    let camera = tree.insert(NodeKind::Container, camera_style, camera_paint);
    tree.add_child(root, camera);

    let (k, s, p) = leaf(40.0, 40.0);
    let chip = tree.insert(k, s, p);
    tree.add_child(camera, chip);

    tree.compute_layout(
        root,
        Size {
            width: AvailableSpace::Definite(200.0),
            height: AvailableSpace::Definite(200.0),
        },
    );

    // scale(2.0) about the origin, then translate by (20, 20):
    // chip's local (0,0)-(40,40) box maps to canvas (20,20)-(100,100).
    // This same transform also moves `camera`'s own 200x200 bounds
    // to canvas (20,20)-(420,420) -- larger and shifted, not the
    // original (0,0)-(200,200) -- so a point that misses both now
    // falls through to `root`'s own untransformed full-canvas
    // bounds, not `None` (mirroring the translate-only test above).
    tree.get_mut(camera).unwrap().paint.transform.current =
        Affine::translate((20.0, 20.0)) * Affine::scale(2.0);

    assert_eq!(
        tree.hit_test(root, Point::new(60.0, 60.0)),
        Some(chip),
        "a point inside the scaled-up transformed box must hit chip"
    );
    assert_eq!(
        tree.hit_test(root, Point::new(10.0, 10.0)),
        Some(root),
        "a point outside both chip's and camera's new transformed footprint (though \
         inside their old untransformed one) must fall through to root's own \
         untransformed bounds, not silently miss"
    );
}

/// M5 Phase 3 (§11.10): a `Canvas` node's `CustomHitTest::Circle`
/// genuinely overrides the default rect test, not just narrows it --
/// a point inside the node's own 100x100 rect but outside the
/// circle must miss entirely (the canvas is this tree's only node,
/// so there's nothing else for it to fall through to).
#[test]
fn canvas_custom_circle_hit_test_overrides_the_default_rect() {
    let mut tree = Tree::new();
    let mut state = CanvasState::new();
    state.hit_test = Some(CustomHitTest::Circle {
        cx: 50.0,
        cy: 50.0,
        radius: 10.0,
    });
    let canvas = tree.insert(
        NodeKind::Canvas(state),
        Style {
            size: Size {
                width: length(100.0),
                height: length(100.0),
            },
            ..Default::default()
        },
        PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 1.0),
    );
    tree.compute_layout(
        canvas,
        Size {
            width: AvailableSpace::Definite(100.0),
            height: AvailableSpace::Definite(100.0),
        },
    );

    assert_eq!(
        tree.hit_test(canvas, Point::new(50.0, 50.0)),
        Some(canvas),
        "the circle's own center must hit"
    );
    assert_eq!(
        tree.hit_test(canvas, Point::new(5.0, 5.0)),
        None,
        "a point well inside the node's 100x100 rect but far outside the 10px-radius \
         circle must miss -- the rect default must not apply as a fallback"
    );
}

/// 0.5.1 (#53): a canvas's painter coordinates start at its padding, so a
/// custom hit shape is in those coordinates: a circle at painter (10, 10) in
/// a canvas padded 30 left and 20 top is at the node's local (40, 30).
#[test]
fn canvas_custom_hit_shape_is_in_painter_coordinates_inside_the_padding() {
    let mut tree = Tree::new();
    let mut state = CanvasState::new();
    state.hit_test = Some(CustomHitTest::Circle {
        cx: 10.0,
        cy: 10.0,
        radius: 5.0,
    });
    let canvas = tree.insert(
        NodeKind::Canvas(state),
        Style {
            size: Size {
                width: length(100.0),
                height: length(100.0),
            },
            padding: taffy::geometry::Rect {
                left: length(30.0),
                top: length(20.0),
                right: length(0.0),
                bottom: length(0.0),
            },
            ..Default::default()
        },
        PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 1.0),
    );
    tree.compute_layout(
        canvas,
        Size {
            width: AvailableSpace::Definite(100.0),
            height: AvailableSpace::Definite(100.0),
        },
    );
    assert_eq!(
        tree.hit_test(canvas, Point::new(40.0, 30.0)),
        Some(canvas),
        "the circle's center, in the content box"
    );
    assert_eq!(
        tree.hit_test(canvas, Point::new(10.0, 10.0)),
        None,
        "the node's corner is not painter (10, 10)"
    );
}

/// The path half of the same claim (§11.10's own "a bezier curve
/// within N pixels of the point" example) -- a straight diagonal
/// `BezPath` (a degenerate, zero-curvature case of the same real
/// `ParamCurveNearest` math a true bezier would use) with a 5px
/// tolerance.
#[test]
fn canvas_custom_path_hit_test_uses_real_distance_to_path() {
    let mut tree = Tree::new();
    let mut path = BezPath::new();
    path.move_to((0.0, 0.0));
    path.line_to((100.0, 100.0));

    let mut state = CanvasState::new();
    state.hit_test = Some(CustomHitTest::Path {
        path,
        tolerance: 5.0,
    });
    let canvas = tree.insert(
        NodeKind::Canvas(state),
        Style {
            size: Size {
                width: length(100.0),
                height: length(100.0),
            },
            ..Default::default()
        },
        PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 1.0),
    );
    tree.compute_layout(
        canvas,
        Size {
            width: AvailableSpace::Definite(100.0),
            height: AvailableSpace::Definite(100.0),
        },
    );

    // Distance from (50, 52) to the line y=x is |50-52|/sqrt(2) ~= 1.41px.
    assert_eq!(
        tree.hit_test(canvas, Point::new(50.0, 52.0)),
        Some(canvas),
        "a point ~1.4px from the diagonal, within the 5px tolerance, must hit"
    );
    // Distance from (10, 90) to the line y=x is |10-90|/sqrt(2) ~= 56.6px.
    assert_eq!(
        tree.hit_test(canvas, Point::new(10.0, 90.0)),
        None,
        "a point ~56.6px from the diagonal, well outside the 5px tolerance (though \
         still inside the node's own rect), must miss"
    );
}

/// M11 Phase 1 (§11.10, §11.11): the non-degenerate case the test
/// above's own doc comment names but doesn't exercise -- a real
/// `quad_to` curve, not a straight line. `move_to(0,0)`, `quad_to`
/// control `(50,100)`, end `(100,0)` bulges to a true midpoint of
/// `(50,50)` (the standard quadratic-bezier weighted-control-point
/// formula), far from the naive chord's own midpoint `(50,0)`.
#[test]
fn canvas_custom_path_hit_test_uses_the_real_curve_not_the_straight_chord_between_its_endpoints() {
    let mut tree = Tree::new();
    let mut path = BezPath::new();
    path.move_to((0.0, 0.0));
    path.quad_to((50.0, 100.0), (100.0, 0.0));

    let mut state = CanvasState::new();
    state.hit_test = Some(CustomHitTest::Path {
        path,
        tolerance: 5.0,
    });
    let canvas = tree.insert(
        NodeKind::Canvas(state),
        Style {
            size: Size {
                width: length(100.0),
                height: length(100.0),
            },
            ..Default::default()
        },
        PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 1.0),
    );
    tree.compute_layout(
        canvas,
        Size {
            width: AvailableSpace::Definite(100.0),
            height: AvailableSpace::Definite(100.0),
        },
    );

    // (50, 50) sits right on the true curve's own real midpoint --
    // 50px from the naive straight chord (0,0)-(100,0), so this
    // would wrongly MISS if hit-testing only ever saw a straight
    // line between the curve's endpoints.
    assert_eq!(
        tree.hit_test(canvas, Point::new(50.0, 50.0)),
        Some(canvas),
        "a point on the real curve's own midpoint, far from the straight chord between \
         its endpoints, must hit -- proving this is real curve-aware distance, not a \
         straight-line approximation"
    );
    // (50, 2) sits ~2px from the naive straight chord -- it would
    // wrongly HIT under a chord-only distance, but the real curve
    // passes through (50, 50) here, ~48px away, well outside the
    // 5px tolerance.
    assert_eq!(
        tree.hit_test(canvas, Point::new(50.0, 2.0)),
        None,
        "a point near the straight chord but far from the real curve must miss -- the \
         adversarial case a chord-only (not curve-aware) distance would get wrong"
    );
}

#[test]
fn move_focus_cycles_only_through_interactive_nodes_in_tree_order_wrapping_at_both_ends() {
    use crate::access::{AccessNodeData, Action, Role};

    let mut tree = Tree::new();
    let (k, s, p) = leaf(0.0, 0.0);
    let root = tree.insert(k, s, p);

    let (k, s, p) = leaf(10.0, 10.0);
    let button_a = tree.insert(k, s, p);
    tree.set_access(
        button_a,
        AccessNodeData::new(Role::Button).with_action(Action::Click),
    );
    tree.add_child(root, button_a);

    // A plain, non-interactive node in between -- must be skipped
    // entirely by Tab, not just left unfocused.
    let (k, s, p) = leaf(10.0, 10.0);
    let decoration = tree.insert(k, s, p);
    tree.add_child(root, decoration);

    let (k, s, p) = leaf(10.0, 10.0);
    let button_b = tree.insert(k, s, p);
    tree.set_access(
        button_b,
        AccessNodeData::new(Role::Button).with_action(Action::Click),
    );
    tree.add_child(root, button_b);

    assert_eq!(tree.focused(), None);

    tree.move_focus(root, FocusDirection::Next);
    assert_eq!(
        tree.focused(),
        Some(button_a),
        "Tab from nothing focused lands on the first interactive node"
    );

    tree.move_focus(root, FocusDirection::Next);
    assert_eq!(
        tree.focused(),
        Some(button_b),
        "Tab skips the non-interactive decoration node entirely"
    );

    tree.move_focus(root, FocusDirection::Next);
    assert_eq!(
        tree.focused(),
        Some(button_a),
        "Tab wraps back to the first interactive node at the end"
    );

    tree.move_focus(root, FocusDirection::Previous);
    assert_eq!(
        tree.focused(),
        Some(button_b),
        "Shift-Tab wraps backward past the first node to the last"
    );
}

#[test]
fn dispatch_activates_only_a_same_node_primary_press_and_release_pair() {
    let mut tree = Tree::new();
    let root_style = Style {
        display: taffy::Display::Flex,
        size: Size {
            width: length(100.0),
            height: length(50.0),
        },
        ..Default::default()
    };
    let (_, _, root_paint) = leaf(0.0, 0.0);
    let root = tree.insert(NodeKind::Container, root_style, root_paint);

    let (k, s, p) = leaf(50.0, 50.0);
    let a = tree.insert(k, s, p);
    tree.add_child(root, a);
    let (k, s, p) = leaf(50.0, 50.0);
    let b = tree.insert(k, s, p);
    tree.add_child(root, b);

    tree.compute_layout(
        root,
        Size {
            width: AvailableSpace::Definite(100.0),
            height: AvailableSpace::Definite(50.0),
        },
    );

    let now = Instant::now();

    // Press and release over the same node (A) -- a real click.
    let outcome = tree.dispatch(
        root,
        InputEvent::PointerPressed {
            position: Point::new(25.0, 25.0),
            button: PointerButton::Primary,
        },
        now,
    );
    assert_eq!(
        outcome,
        DispatchOutcome::None,
        "a press alone never activates"
    );
    let outcome = tree.dispatch(
        root,
        InputEvent::PointerReleased {
            position: Point::new(25.0, 25.0),
            button: PointerButton::Primary,
        },
        now,
    );
    assert_eq!(
        outcome,
        DispatchOutcome::Activated(a),
        "press and release over the same node with the primary button must activate it"
    );

    // Press over A, release over B -- a drag-off, not a click.
    tree.dispatch(
        root,
        InputEvent::PointerPressed {
            position: Point::new(25.0, 25.0),
            button: PointerButton::Primary,
        },
        now,
    );
    let outcome = tree.dispatch(
        root,
        InputEvent::PointerReleased {
            position: Point::new(75.0, 25.0),
            button: PointerButton::Primary,
        },
        now,
    );
    assert_eq!(
        outcome,
        DispatchOutcome::None,
        "releasing over a different node than was pressed must not activate anything"
    );

    // M4 Phase 7 (§11.3): press and release over the same node with
    // the secondary button now produces its own real outcome --
    // this section used to assert `None` here with a comment
    // predicting "reserved for a future context-menu mechanism,"
    // which this phase is.
    tree.dispatch(
        root,
        InputEvent::PointerPressed {
            position: Point::new(25.0, 25.0),
            button: PointerButton::Secondary,
        },
        now,
    );
    let outcome = tree.dispatch(
        root,
        InputEvent::PointerReleased {
            position: Point::new(25.0, 25.0),
            button: PointerButton::Secondary,
        },
        now,
    );
    assert_eq!(
        outcome,
        DispatchOutcome::SecondaryActivated(a),
        "a same-node secondary-button press/release pair must produce SecondaryActivated"
    );

    // Middle-button press/release still has no real meaning --
    // unlike Secondary (this phase), nothing in this codebase names
    // a real use for Middle yet.
    tree.dispatch(
        root,
        InputEvent::PointerPressed {
            position: Point::new(25.0, 25.0),
            button: PointerButton::Middle,
        },
        now,
    );
    let outcome = tree.dispatch(
        root,
        InputEvent::PointerReleased {
            position: Point::new(25.0, 25.0),
            button: PointerButton::Middle,
        },
        now,
    );
    assert_eq!(
        outcome,
        DispatchOutcome::None,
        "a same-node middle-button press/release pair must still produce no real outcome"
    );

    // 0.4.1 (issue #21): the side buttons, like the middle one, have no
    // outcome of their own -- their listeners hear the press and release.
    for button in [PointerButton::Back, PointerButton::Forward] {
        tree.dispatch(
            root,
            InputEvent::PointerPressed {
                position: Point::new(25.0, 25.0),
                button,
            },
            now,
        );
        let outcome = tree.dispatch(
            root,
            InputEvent::PointerReleased {
                position: Point::new(25.0, 25.0),
                button,
            },
            now,
        );
        assert_eq!(
            outcome,
            DispatchOutcome::None,
            "a {button:?} press/release pair activates nothing"
        );
    }

    // Enter/Space on the currently-focused node also activates it.
    tree.set_focus_to(b);
    let outcome = tree.dispatch(
        root,
        InputEvent::KeyPressed {
            key: Key::Enter,
            shift: false,
        },
        now,
    );
    assert_eq!(outcome, DispatchOutcome::Activated(b));
}

#[test]
fn dispatch_reports_hover_changed_only_on_a_real_transition() {
    let mut tree = Tree::new();
    let root_style = Style {
        display: taffy::Display::Flex,
        size: Size {
            width: length(100.0),
            height: length(50.0),
        },
        ..Default::default()
    };
    let (_, _, root_paint) = leaf(0.0, 0.0);
    let root = tree.insert(NodeKind::Container, root_style, root_paint);

    let (k, s, p) = leaf(50.0, 50.0);
    let a = tree.insert(k, s, p);
    tree.add_child(root, a);
    let (k, s, p) = leaf(50.0, 50.0);
    let b = tree.insert(k, s, p);
    tree.add_child(root, b);

    tree.compute_layout(
        root,
        Size {
            width: AvailableSpace::Definite(100.0),
            height: AvailableSpace::Definite(50.0),
        },
    );

    let now = Instant::now();

    // Moving onto A for the first time: None -> Some(a).
    let outcome = tree.dispatch(
        root,
        InputEvent::PointerMoved {
            position: Point::new(25.0, 25.0),
        },
        now,
    );
    assert_eq!(
        outcome,
        DispatchOutcome::HoverChanged {
            old: None,
            new: Some(a)
        },
        "hitting a node for the first time must report a real transition"
    );

    // Moving again within A: no transition, no outcome.
    let outcome = tree.dispatch(
        root,
        InputEvent::PointerMoved {
            position: Point::new(30.0, 30.0),
        },
        now,
    );
    assert_eq!(
        outcome,
        DispatchOutcome::None,
        "a repeated move within the same already-hovered node must not report a transition"
    );

    // Moving from A to B: Some(a) -> Some(b).
    let outcome = tree.dispatch(
        root,
        InputEvent::PointerMoved {
            position: Point::new(75.0, 25.0),
        },
        now,
    );
    assert_eq!(
        outcome,
        DispatchOutcome::HoverChanged {
            old: Some(a),
            new: Some(b)
        },
        "moving directly from one hovered node to another must report both halves"
    );

    // Moving off every node: Some(b) -> None.
    let outcome = tree.dispatch(
        root,
        InputEvent::PointerMoved {
            position: Point::new(500.0, 500.0),
        },
        now,
    );
    assert_eq!(
        outcome,
        DispatchOutcome::HoverChanged {
            old: Some(b),
            new: None
        },
        "leaving every hit-testable node must still report the exit half"
    );
}

/// M55 (§10, §16.2): `HoverChanged`'s own real precedent, mirrored
/// for a real click-to-focus transition -- before this, the
/// `PointerPressed` arm that calls `set_focus_to` (M18/M30/M53)
/// always returned `DispatchOutcome::None`, silently discarding
/// the real transition.
#[test]
fn dispatch_reports_focus_changed_on_a_real_click_to_focus_transition() {
    let mut tree = Tree::new();
    let root_style = Style {
        size: Size {
            width: length(120.0),
            height: length(24.0),
        },
        ..Default::default()
    };
    let (_, _, root_paint) = leaf(0.0, 0.0);
    let root = tree.insert(NodeKind::Container, root_style, root_paint);
    let field = tree.insert(
        NodeKind::TextField(Box::new(TextFieldState::new("hi", "Roboto", 400.0, 16.0))),
        Style {
            size: Size {
                width: length(120.0),
                height: length(24.0),
            },
            ..Default::default()
        },
        PaintProperties::new(Color::from_rgba8(0xEE, 0xEE, 0xEE, 0xFF), 0.0, 1.0),
    );
    tree.add_child(root, field);
    tree.compute_layout(
        root,
        Size {
            width: AvailableSpace::Definite(120.0),
            height: AvailableSpace::Definite(24.0),
        },
    );

    let now = Instant::now();

    let outcome = tree.dispatch(
        root,
        InputEvent::PointerPressed {
            position: Point::new(10.0, 10.0),
            button: PointerButton::Primary,
        },
        now,
    );
    assert_eq!(
        outcome,
        DispatchOutcome::FocusChanged {
            old: None,
            new: Some(field)
        },
        "a real click-to-focus on a previously-unfocused TextField must report the transition"
    );

    // Pressing the already-focused field again: no real transition.
    let outcome = tree.dispatch(
        root,
        InputEvent::PointerPressed {
            position: Point::new(10.0, 10.0),
            button: PointerButton::Primary,
        },
        now,
    );
    assert_eq!(
        outcome,
        DispatchOutcome::None,
        "pressing an already-focused field again must not report a stale transition"
    );
}

/// `dispatch_reports_focus_changed_on_a_real_click_to_focus_
/// transition`'s own real Tab-navigation sibling.
#[test]
fn dispatch_reports_focus_changed_on_real_tab_navigation() {
    use crate::access::{Action, Role};

    let mut tree = Tree::new();
    let root_style = Style {
        display: taffy::Display::Flex,
        size: Size {
            width: length(100.0),
            height: length(50.0),
        },
        ..Default::default()
    };
    let (_, _, root_paint) = leaf(0.0, 0.0);
    let root = tree.insert(NodeKind::Container, root_style, root_paint);

    let (k, s, p) = leaf(50.0, 50.0);
    let a = tree.insert(k, s, p);
    tree.set_access(
        a,
        AccessNodeData::new(Role::Button).with_action(Action::Click),
    );
    tree.add_child(root, a);

    tree.compute_layout(
        root,
        Size {
            width: AvailableSpace::Definite(100.0),
            height: AvailableSpace::Definite(50.0),
        },
    );

    let now = Instant::now();

    let outcome = tree.dispatch(
        root,
        InputEvent::KeyPressed {
            key: Key::Tab,
            shift: false,
        },
        now,
    );
    assert_eq!(
        outcome,
        DispatchOutcome::FocusChanged {
            old: None,
            new: Some(a)
        },
        "a real Tab press onto the one interactive node must report the transition"
    );
}

/// A real `PointerPressed` that never touches `self.focused` at all
/// (a plain, non-`TextField`/`Terminal` node) must not fabricate a
/// `FocusChanged` -- the same "no real fact, no outcome" contract
/// `HoverChanged`'s own no-transition case already established.
#[test]
fn dispatch_does_not_report_focus_changed_for_a_non_focusable_click() {
    let mut tree = Tree::new();
    let root_style = Style {
        size: Size {
            width: length(50.0),
            height: length(50.0),
        },
        ..Default::default()
    };
    let (_, _, root_paint) = leaf(0.0, 0.0);
    let root = tree.insert(NodeKind::Container, root_style, root_paint);
    let (k, s, p) = leaf(50.0, 50.0);
    let rect = tree.insert(k, s, p);
    tree.add_child(root, rect);
    tree.compute_layout(
        root,
        Size {
            width: AvailableSpace::Definite(50.0),
            height: AvailableSpace::Definite(50.0),
        },
    );

    let now = Instant::now();

    let outcome = tree.dispatch(
        root,
        InputEvent::PointerPressed {
            position: Point::new(25.0, 25.0),
            button: PointerButton::Primary,
        },
        now,
    );
    assert_eq!(
        outcome,
        DispatchOutcome::None,
        "clicking a plain Rect must not fabricate a focus transition"
    );
    assert_eq!(tree.focused(), None);
}

#[test]
fn dispatch_scroll_is_a_true_no_op() {
    let mut tree = Tree::new();
    let root_style = Style {
        display: taffy::Display::Flex,
        size: Size {
            width: length(100.0),
            height: length(50.0),
        },
        ..Default::default()
    };
    let (_, _, root_paint) = leaf(0.0, 0.0);
    let root = tree.insert(NodeKind::Container, root_style, root_paint);
    let (k, s, p) = leaf(50.0, 50.0);
    let a = tree.insert(k, s, p);
    tree.add_child(root, a);

    tree.compute_layout(
        root,
        Size {
            width: AvailableSpace::Definite(100.0),
            height: AvailableSpace::Definite(50.0),
        },
    );

    let now = Instant::now();

    // M4 Phase 8 (§11.7/§11.8 groundwork): real translation reaches
    // Tree::dispatch, but a Scroll event must be a genuine no-op --
    // no outcome, and no side effect on any other tracked state --
    // matching this phase's own explicit "plumbing only" scope.
    let before_hovered = tree.hovered;
    let outcome = tree.dispatch(
        root,
        InputEvent::Scroll {
            delta: ScrollDelta::Lines(0.0, -3.0),
            position: Point::new(25.0, 25.0),
        },
        now,
    );
    assert_eq!(
        outcome,
        DispatchOutcome::None,
        "a Scroll event must produce no real outcome yet"
    );
    assert_eq!(
        tree.hovered, before_hovered,
        "a Scroll event must not touch hover state as a side effect"
    );

    let outcome = tree.dispatch(
        root,
        InputEvent::Scroll {
            delta: ScrollDelta::Pixels(0.0, 40.0),
            position: Point::new(25.0, 25.0),
        },
        now,
    );
    assert_eq!(
        outcome,
        DispatchOutcome::None,
        "a pixel-delta Scroll event must also produce no real outcome yet"
    );
}

#[test]
fn access_id_round_trips_through_to_access_id_and_from_access_id() {
    let mut tree = Tree::new();
    let (k, s, p) = leaf(10.0, 10.0);
    let node = tree.insert(k, s, p);

    let access_id = to_access_id(node);
    let round_tripped = from_access_id(access_id);
    assert_eq!(
        round_tripped, node,
        "KeyData::as_ffi/from_ffi's own documented guarantee: round-tripping \
         through the opaque accesskit::NodeId must return an equal key"
    );
}

#[test]
fn from_access_id_on_a_stale_or_foreign_id_fails_safely_not_silently() {
    let mut tree = Tree::new();
    let (k, s, p) = leaf(10.0, 10.0);
    let node = tree.insert(k, s, p);
    let access_id = to_access_id(node);

    tree.remove(node);

    // The real generational-safety claim (§5), applied to an
    // accessibility-client-supplied id: a stale accesskit::NodeId
    // for a since-removed node must not resolve to whatever now
    // occupies that slot.
    let stale = from_access_id(access_id);
    assert!(
        tree.get(stale).is_none(),
        "a stale accesskit::NodeId must round-trip to a NodeId that fails \
         this Tree's own generation check, not one that silently resolves"
    );
}

#[test]
fn activate_produces_activated_for_a_real_node_and_none_for_an_unknown_one() {
    let mut tree = Tree::new();
    let (k, s, p) = leaf(10.0, 10.0);
    let node = tree.insert(k, s, p);

    assert_eq!(tree.activate(node), DispatchOutcome::Activated(node));

    tree.remove(node);
    assert_eq!(
        tree.activate(node),
        DispatchOutcome::None,
        "activating a NodeId no longer in this Tree must be a safe no-op, \
         not a panic or a stale Activated outcome"
    );
}

#[test]
fn set_focus_to_jumps_directly_to_the_named_node() {
    let mut tree = Tree::new();
    let (k, s, p) = leaf(10.0, 10.0);
    let a = tree.insert(k, s, p);
    let (k, s, p) = leaf(10.0, 10.0);
    let b = tree.insert(k, s, p);

    // Unlike move_focus, set_focus_to doesn't need `a`/`b` to have
    // any access.actions at all -- it's a direct target, the same
    // way a mouse click names its target regardless of that node's
    // own access.actions.
    assert_eq!(tree.set_focus_to(a), Some((None, Some(a))));
    assert_eq!(tree.focused(), Some(a));
    assert_eq!(tree.set_focus_to(b), Some((Some(a), Some(b))));
    assert_eq!(
        tree.focused(),
        Some(b),
        "set_focus_to must jump straight to the named node, not compute a \
         tab-order neighbor"
    );
}

#[test]
fn set_focus_to_an_unknown_node_is_a_safe_no_op() {
    let mut tree = Tree::new();
    let (k, s, p) = leaf(10.0, 10.0);
    let real = tree.insert(k, s, p);
    tree.set_focus_to(real);
    let (k, s, p) = leaf(10.0, 10.0);
    let ghost = tree.insert(k, s, p);
    tree.remove(ghost);

    tree.set_focus_to(ghost);
    assert_eq!(
        tree.focused(),
        Some(real),
        "focusing an unknown NodeId must leave the real current focus untouched"
    );
}

#[test]
fn build_access_update_exposes_a_button_with_the_right_role_label_and_bounds() {
    use crate::access::AccessNodeData;
    use crate::access::{Action, Role};

    let mut tree = Tree::new();
    let (kind, style, paint) = leaf(120.0, 40.0);
    let button = tree.insert(kind, style, paint);
    tree.set_access(
        button,
        AccessNodeData::new(Role::Button)
            .with_label("Save")
            .with_action(Action::Click),
    );
    tree.compute_layout(
        button,
        Size {
            width: AvailableSpace::Definite(120.0),
            height: AvailableSpace::Definite(40.0),
        },
    );

    let update = tree.build_access_update(button);
    assert_eq!(
        update.nodes.len(),
        1,
        "expected exactly the one button node"
    );
    let (id, node) = &update.nodes[0];
    assert_eq!(*id, to_access_id(button));
    assert_eq!(node.role(), Role::Button);
    assert_eq!(node.label(), Some("Save"));
    assert!(
        node.supports_action(Action::Click),
        "expected the button's Click action to be reported"
    );
    let bounds = node.bounds().expect("a laid-out node must report bounds");
    assert_eq!(
        (bounds.x0, bounds.y0, bounds.x1, bounds.y1),
        (0.0, 0.0, 120.0, 40.0)
    );

    // `focus` must be a valid value even though nothing was
    // explicitly focused -- §10: "if no specific node has keyboard
    // focus, this must be set to the root."
    assert_eq!(update.focus, to_access_id(button));
    assert_eq!(update.tree_id, accesskit::TreeId::ROOT);
}

/// M15 Phase 1 (§5, §16.7): `TextFieldState::new`'s own real
/// contract -- `cursor` seeds at `content`'s own real end, a real
/// text field's own expected initial-cursor-at-end convention, not
/// `0`.
#[test]
fn text_field_state_new_seeds_the_cursor_at_the_end_of_the_initial_content() {
    let state = TextFieldState::new("hello", "Roboto", 400.0, 16.0);
    assert_eq!(state.content, "hello");
    assert_eq!(
        state.cursor, 5,
        "cursor must start at content's own real end (byte offset 5 for \"hello\")"
    );
    assert_eq!(state.selection_anchor, None);
}

/// A `TextField`'s own real, automatic accessibility derivation:
/// reads `content` directly from `NodeKind::TextField`, not a
/// second, separately-set copy.
#[test]
fn build_access_update_reports_the_real_value_role_and_focus_action_for_a_text_field() {
    use crate::access::{Action, Role};

    let mut tree = Tree::new();
    let (_, style, paint) = leaf(120.0, 32.0);
    let field = tree.insert(
        NodeKind::TextField(Box::new(TextFieldState::new(
            "hello", "Roboto", 400.0, 16.0,
        ))),
        style,
        paint,
    );
    tree.set_access(
        field,
        AccessNodeData::new(Role::TextInput).with_action(Action::Focus),
    );
    tree.compute_layout(
        field,
        Size {
            width: AvailableSpace::Definite(120.0),
            height: AvailableSpace::Definite(32.0),
        },
    );

    let update = tree.build_access_update(field);
    let (_, node) = &update.nodes[0];
    assert_eq!(node.role(), Role::TextInput);
    assert_eq!(
        node.value(),
        Some("hello"),
        "a TextField's own real content must reach the accessibility tree's value, \
         not a stale or missing one"
    );
    assert!(
        node.supports_action(Action::Focus),
        "a TextField must be a real Focus target for Tab/screen-reader reachability"
    );
}

/// M15 Phase 2 (§8, §10): a real `TextField`, already the `Tree`'s own real focused node
/// (every real editing test needs that, so seeding it here avoids
/// repeating a `set_focus_to` call in every single test below).
fn text_field_scene(content: &str) -> (Tree, NodeId, NodeId) {
    let mut tree = Tree::new();
    let (_, root_style, root_paint) = leaf(0.0, 0.0);
    let root = tree.insert(NodeKind::Container, root_style, root_paint);

    let field = tree.insert(
        NodeKind::TextField(Box::new(TextFieldState::new(
            content, "Roboto", 400.0, 16.0,
        ))),
        Style {
            size: Size {
                width: length(120.0),
                height: length(24.0),
            },
            ..Default::default()
        },
        PaintProperties::new(Color::from_rgba8(0xEE, 0xEE, 0xEE, 0xFF), 0.0, 1.0),
    );
    tree.add_child(root, field);
    tree.set_focus_to(field);
    (tree, root, field)
}

fn field_state(tree: &Tree, field: NodeId) -> &TextFieldState {
    let NodeKind::TextField(state) = &tree.get(field).unwrap().kind else {
        panic!("expected a TextField node");
    };
    state
}

fn dispatch_key(tree: &mut Tree, root: NodeId, key: Key) -> DispatchOutcome {
    tree.dispatch(
        root,
        InputEvent::KeyPressed { key, shift: false },
        Instant::now(),
    )
}

/// M15 Phase 3 (§16.7): `dispatch_key`'s own real `shift`-held
/// sibling, for selection-extension tests.
fn dispatch_shift_key(tree: &mut Tree, root: NodeId, key: Key) -> DispatchOutcome {
    tree.dispatch(
        root,
        InputEvent::KeyPressed { key, shift: true },
        Instant::now(),
    )
}

#[test]
fn text_input_with_no_focused_field_is_a_true_no_op() {
    let mut tree = Tree::new();
    let (_, root_style, root_paint) = leaf(0.0, 0.0);
    let root = tree.insert(NodeKind::Container, root_style, root_paint);
    let outcome = tree.dispatch(root, InputEvent::TextInput("a".to_string()), Instant::now());
    assert_eq!(outcome, DispatchOutcome::None);
}

#[test]
fn text_input_inserts_at_the_real_cursor_and_advances_it() {
    let (mut tree, root, field) = text_field_scene("hllo");
    // Real cursor starts at content's own end (`TextFieldState::
    // new`'s own contract) -- move it to byte offset 1 (after "h")
    // first via a real ArrowLeft x3 from the end, so the insert
    // below lands in the middle, not just appended.
    dispatch_key(&mut tree, root, Key::Home);
    dispatch_key(&mut tree, root, Key::ArrowRight);

    let outcome = tree.dispatch(root, InputEvent::TextInput("e".to_string()), Instant::now());
    assert_eq!(
        outcome,
        DispatchOutcome::Changed {
            node: field,
            old_value: ChangedValue::Text("hllo".to_string()),
        }
    );
    let state = field_state(&tree, field);
    assert_eq!(state.content, "hello");
    assert_eq!(
        state.cursor, 2,
        "cursor must advance past the real inserted text"
    );
}

#[test]
fn backspace_removes_the_real_char_before_the_cursor() {
    let (mut tree, root, field) = text_field_scene("hello");
    let outcome = dispatch_key(&mut tree, root, Key::Backspace);
    assert_eq!(
        outcome,
        DispatchOutcome::Changed {
            node: field,
            old_value: ChangedValue::Text("hello".to_string()),
        }
    );
    let state = field_state(&tree, field);
    assert_eq!(state.content, "hell");
    assert_eq!(state.cursor, 4);
}

#[test]
fn backspace_at_the_real_start_is_a_genuine_no_op() {
    let (mut tree, root, field) = text_field_scene("hello");
    dispatch_key(&mut tree, root, Key::Home);
    let outcome = dispatch_key(&mut tree, root, Key::Backspace);
    assert_eq!(
        outcome,
        DispatchOutcome::None,
        "no real edit happened, so this must not report Changed"
    );
    assert_eq!(field_state(&tree, field).content, "hello");
}

#[test]
fn delete_removes_the_real_char_after_the_cursor() {
    let (mut tree, root, field) = text_field_scene("hello");
    dispatch_key(&mut tree, root, Key::Home);
    let outcome = dispatch_key(&mut tree, root, Key::Delete);
    assert_eq!(
        outcome,
        DispatchOutcome::Changed {
            node: field,
            old_value: ChangedValue::Text("hello".to_string()),
        }
    );
    let state = field_state(&tree, field);
    assert_eq!(state.content, "ello");
    assert_eq!(state.cursor, 0);
}

#[test]
fn delete_at_the_real_end_is_a_genuine_no_op() {
    let (mut tree, root, field) = text_field_scene("hello");
    let outcome = dispatch_key(&mut tree, root, Key::Delete);
    assert_eq!(outcome, DispatchOutcome::None);
    assert_eq!(field_state(&tree, field).content, "hello");
}

#[test]
fn arrow_keys_move_the_real_cursor_without_reporting_changed() {
    let (mut tree, root, field) = text_field_scene("hi");
    assert_eq!(field_state(&tree, field).cursor, 2);

    let outcome = dispatch_key(&mut tree, root, Key::ArrowLeft);
    assert_eq!(
        outcome,
        DispatchOutcome::None,
        "pure cursor movement is not a bound-value change"
    );
    assert_eq!(field_state(&tree, field).cursor, 1);

    dispatch_key(&mut tree, root, Key::ArrowRight);
    assert_eq!(field_state(&tree, field).cursor, 2);
}

#[test]
fn home_and_end_jump_the_real_cursor_to_the_real_edges() {
    let (mut tree, root, field) = text_field_scene("hello");
    dispatch_key(&mut tree, root, Key::Home);
    assert_eq!(field_state(&tree, field).cursor, 0);
    dispatch_key(&mut tree, root, Key::End);
    assert_eq!(field_state(&tree, field).cursor, 5);
}

#[test]
fn a_focused_text_field_inserts_a_real_space_instead_of_activating() {
    let (mut tree, root, field) = text_field_scene("ab");
    dispatch_key(&mut tree, root, Key::Home);
    dispatch_key(&mut tree, root, Key::ArrowRight);
    let outcome = dispatch_key(&mut tree, root, Key::Space);
    assert_eq!(
        outcome,
        DispatchOutcome::Changed {
            node: field,
            old_value: ChangedValue::Text("ab".to_string()),
        },
        "Space on a focused TextField must be a real inserted character, not Activated"
    );
    assert_eq!(field_state(&tree, field).content, "a b");
}

#[test]
fn enter_on_a_focused_text_field_is_consumed_without_inserting_or_activating() {
    let (mut tree, root, field) = text_field_scene("hi");
    let outcome = dispatch_key(&mut tree, root, Key::Enter);
    assert_eq!(
        outcome,
        DispatchOutcome::None,
        "a single-line TextField must not activate on Enter, matching Space's own reasoning"
    );
    assert_eq!(
        field_state(&tree, field).content,
        "hi",
        "Enter must not insert a newline into a single-line field either"
    );
}

#[test]
fn tab_still_moves_focus_away_from_a_focused_text_field() {
    let (mut tree, root, field) = text_field_scene("hi");
    assert_eq!(tree.focused(), Some(field));
    dispatch_key(&mut tree, root, Key::Tab);
    assert_ne!(
        tree.focused(),
        Some(field),
        "Tab must still move focus away from a focused TextField"
    );
}

#[test]
fn backspace_removes_a_real_multi_byte_utf8_character_whole() {
    // "café" -- "é" is a real 2-byte UTF-8 scalar; a naive
    // byte-at-a-time backspace would corrupt it into invalid UTF-8.
    let (mut tree, root, field) = text_field_scene("café");
    let outcome = dispatch_key(&mut tree, root, Key::Backspace);
    assert_eq!(
        outcome,
        DispatchOutcome::Changed {
            node: field,
            old_value: ChangedValue::Text("café".to_string()),
        }
    );
    assert_eq!(field_state(&tree, field).content, "caf");
}

/// M30 Phase 9 Step 3 (§8, §10): `text_field_scene`'s own real
/// `multiline: true` sibling -- `Code Editor`'s own real scene,
/// every multiline test below builds on this.
fn multiline_field_scene(content: &str) -> (Tree, NodeId, NodeId) {
    let (mut tree, root, field) = text_field_scene(content);
    let NodeKind::TextField(state) = &mut tree.get_mut(field).unwrap().kind else {
        panic!("expected a TextField node");
    };
    state.multiline = true;
    (tree, root, field)
}

#[test]
fn enter_on_a_multiline_field_inserts_a_real_newline() {
    let (mut tree, root, field) = multiline_field_scene("ab");
    dispatch_key(&mut tree, root, Key::Home);
    dispatch_key(&mut tree, root, Key::ArrowRight);
    let outcome = dispatch_key(&mut tree, root, Key::Enter);
    assert_eq!(
        outcome,
        DispatchOutcome::Changed {
            node: field,
            old_value: ChangedValue::Text("ab".to_string()),
        },
        "Enter on a multiline TextField must be a real inserted newline, not consumed"
    );
    assert_eq!(field_state(&tree, field).content, "a\nb");
}

/// M31 Phase 2 (§8, §10): the real, new capability this phase adds
/// -- a focused *multiline* field claims `Tab` first and inserts a
/// real `\t`, rather than falling through to focus traversal the
/// way `tab_still_moves_focus_away_from_a_focused_text_field`'s own
/// single-line scene still does (unchanged, proven separately).
#[test]
fn tab_on_a_multiline_field_inserts_a_real_tab_character_instead_of_moving_focus() {
    let (mut tree, root, field) = multiline_field_scene("ab");
    dispatch_key(&mut tree, root, Key::Home);
    dispatch_key(&mut tree, root, Key::ArrowRight);
    let outcome = dispatch_key(&mut tree, root, Key::Tab);
    assert_eq!(
        outcome,
        DispatchOutcome::Changed {
            node: field,
            old_value: ChangedValue::Text("ab".to_string()),
        },
        "Tab on a multiline TextField must be a real inserted \\t, not consumed as a no-op"
    );
    assert_eq!(field_state(&tree, field).content, "a\tb");
    assert_eq!(
        tree.focused(),
        Some(field),
        "a multiline field claiming Tab for indentation must never lose focus over it"
    );
}

#[test]
fn tab_on_a_multiline_field_with_a_real_selection_replaces_it_instead_of_inserting_beside_it() {
    // The same real `delete_selection`-then-insert shape `Space`/
    // `Enter` already have for an active selection -- Tab is a
    // real inserted character too, not a special case.
    let (mut tree, root, field) = multiline_field_scene("abcd");
    dispatch_key(&mut tree, root, Key::Home);
    dispatch_shift_key(&mut tree, root, Key::ArrowRight);
    dispatch_shift_key(&mut tree, root, Key::ArrowRight); // selects "ab" (0..2)
    let outcome = dispatch_key(&mut tree, root, Key::Tab);
    assert_eq!(
        outcome,
        DispatchOutcome::Changed {
            node: field,
            old_value: ChangedValue::Text("abcd".to_string()),
        }
    );
    assert_eq!(
        field_state(&tree, field).content,
        "\tcd",
        "Tab must replace a real active selection, not insert beside it"
    );
}

#[test]
fn home_and_end_on_a_multiline_field_jump_to_the_current_line_not_the_whole_buffer() {
    let (mut tree, root, field) = multiline_field_scene("one\ntwo\nthree");
    // Real cursor starts at content's own end (inside "three") --
    // Home/End here must only ever reach "three"'s own real start/
    // end, never byte 0 or the whole buffer's own real end.
    dispatch_key(&mut tree, root, Key::Home);
    assert_eq!(
        field_state(&tree, field).cursor,
        8,
        "Home on a multiline field must land at the current line's own start (after the \
         second '\\n'), not the whole buffer's start"
    );
    dispatch_key(&mut tree, root, Key::End);
    assert_eq!(
        field_state(&tree, field).cursor,
        13,
        "End on a multiline field must land at the current line's own end (content.len(), \
         since 'three' is the last line), matching the whole-buffer case here by coincidence"
    );
}

#[test]
fn home_on_the_first_line_of_a_multiline_field_still_lands_at_byte_zero() {
    let (mut tree, root, field) = multiline_field_scene("one\ntwo");
    // Move the cursor into the first line explicitly, since a
    // fresh field starts at content's own end (inside "two").
    for _ in 0..4 {
        dispatch_key(&mut tree, root, Key::ArrowLeft);
    }
    assert_eq!(
        field_state(&tree, field).cursor,
        3,
        "sanity: cursor now right after \"one\""
    );
    dispatch_key(&mut tree, root, Key::Home);
    assert_eq!(field_state(&tree, field).cursor, 0);
}

#[test]
fn arrow_up_and_down_move_the_cursor_by_line_preserving_its_own_real_column() {
    let (mut tree, root, field) = multiline_field_scene("hello\nhi\nworld");
    // Cursor starts at content's own end -- inside "world", column 5.
    dispatch_key(&mut tree, root, Key::ArrowUp);
    assert_eq!(
        field_state(&tree, field).cursor,
        8,
        "ArrowUp must land on \"hi\"'s own end (column 5 clamped to its real length 2), \
         byte offset 6 (line start after first '\\n') + 2 = 8"
    );
    dispatch_key(&mut tree, root, Key::ArrowDown);
    assert_eq!(
        field_state(&tree, field).cursor,
        14,
        "ArrowDown must move back down to \"world\", recalling the real goal column 5 \
         remembered from before the ArrowUp (M38 Phase 2) -- landing exactly back at \
         the original starting position, not \"hi\"'s own clamped column 2"
    );
}

#[test]
fn arrow_up_and_down_remember_a_real_goal_column_through_a_shorter_line() {
    // M38 Phase 2 (§5, §8): real goal-column memory -- a
    // consecutive run of ArrowUp/ArrowDown must keep landing at
    // the *original* column even after an intermediate shorter
    // line clamps the real cursor to something smaller, not
    // silently adopt that clamped column as the new goal.
    let (mut tree, root, field) = multiline_field_scene("alphabet\nhi\nbanana");
    // Cursor starts at content's own end -- inside "banana", real
    // column 6 ("banana".chars().count()).
    let start = field_state(&tree, field).cursor;
    assert_eq!(start, 18, "sanity: cursor starts at content's own end");

    dispatch_key(&mut tree, root, Key::ArrowUp);
    assert_eq!(
        field_state(&tree, field).cursor,
        11,
        "ArrowUp must land on \"hi\"'s own end (column 6 clamped to its real length 2)"
    );
    dispatch_key(&mut tree, root, Key::ArrowUp);
    assert_eq!(
        field_state(&tree, field).cursor,
        6,
        "a second, consecutive ArrowUp must recall the real *original* goal column 6 \
         from \"banana\" -- landing on \"alphabet\"'s own real column 6 ('e') -- not \
         \"hi\"'s own clamped column 2 (which would land on 'p', byte 2, instead)"
    );
    dispatch_key(&mut tree, root, Key::ArrowDown);
    assert_eq!(
        field_state(&tree, field).cursor,
        11,
        "ArrowDown must move back down to \"hi\"'s own end (the remembered goal 6 still \
         clamps to \"hi\"'s own real length 2)"
    );
    dispatch_key(&mut tree, root, Key::ArrowDown);
    assert_eq!(
        field_state(&tree, field).cursor,
        start,
        "a second, consecutive ArrowDown must recall the same real goal column 6 and \
         land exactly back on the original starting position in \"banana\" -- a full \
         up-up-down-down round trip through a shorter line returns to where it began"
    );
}

#[test]
fn a_non_vertical_move_resets_the_remembered_goal_column() {
    // M38 Phase 2 (§5, §8): only a genuinely *consecutive* run of
    // ArrowUp/ArrowDown remembers a goal column -- any other
    // cursor-moving key in between must reset it, so the next
    // vertical move derives a fresh goal from wherever the cursor
    // now really sits, not a stale one from before the interrupt.
    let (mut tree, root, field) = multiline_field_scene("alphabet\nhi\nbanana");
    dispatch_key(&mut tree, root, Key::ArrowUp);
    assert_eq!(
        field_state(&tree, field).cursor,
        11,
        "sanity: first ArrowUp lands on \"hi\"'s own end, same as the memory test above"
    );
    // A single ArrowLeft is an ordinary horizontal move -- it must
    // reset the goal column even though it doesn't leave "hi".
    dispatch_key(&mut tree, root, Key::ArrowLeft);
    assert_eq!(
        field_state(&tree, field).cursor,
        10,
        "sanity: now between 'h' and 'i' in \"hi\", real column 1"
    );
    dispatch_key(&mut tree, root, Key::ArrowUp);
    assert_eq!(
        field_state(&tree, field).cursor,
        1,
        "ArrowUp after an intervening ArrowLeft must derive a *fresh* goal column (1, \
         \"hi\"'s own real column after the ArrowLeft) rather than recalling \"banana\"'s \
         own stale column 6 from before the interrupt -- landing on 'l' in \"alphabet\" \
         (byte 1), not 'e' (byte 6)"
    );
}

/// M38 Phase 3 (§5, §8) test scene helper: a real fold, set directly
/// on `field`'s own `TextFieldState.folded_ranges` (this file's own
/// private-field access, visible to `mod tests` as a child module of
/// the module that declares `Tree.nodes` -- the same real access
/// every other direct-state test setup in this file already uses).
fn set_folded_ranges(tree: &mut Tree, field: NodeId, range: std::ops::Range<usize>) {
    if let NodeKind::TextField(state) = &mut tree.nodes[field].kind {
        state.folded_ranges = vec![range];
    }
}

#[test]
fn arrow_down_snaps_the_cursor_out_of_a_folded_range_it_would_otherwise_land_inside() {
    // "one\ntwo\nthree\nfour", folded 4..15 ("two\nthree\nfo",
    // deliberately not real-line-aligned -- folds are arbitrary
    // app-supplied byte ranges, §31 Phase 5's own real "no code-
    // structure awareness" contract, so this is a real case this
    // codebase must handle correctly, not just the tidy aligned one).
    let (mut tree, root, field) = multiline_field_scene("one\ntwo\nthree\nfour");
    set_folded_ranges(&mut tree, field, 4..15);
    // Position the cursor inside "one" at real column 2 (byte 2,
    // between 'n' and 'e') so ArrowDown's own natural per-real-line
    // landing computes byte 6 -- strictly inside the fold.
    if let NodeKind::TextField(state) = &mut tree.nodes[field].kind {
        state.cursor = 2;
    }
    dispatch_key(&mut tree, root, Key::ArrowDown);
    assert_eq!(
        field_state(&tree, field).cursor,
        15,
        "ArrowDown's own natural landing (byte 6, real column 2 into \"two\") sits \
         strictly inside the fold 4..15 -- the cursor must snap forward to byte 15, \
         right after the fold's own real marker, not land somewhere genuinely invisible"
    );
}

#[test]
fn home_and_end_also_snap_the_cursor_out_of_a_folded_range() {
    let (mut tree, root, field) = multiline_field_scene("one\ntwo\nthree\nfour");
    set_folded_ranges(&mut tree, field, 4..15);
    // Cursor inside "three" (byte 10), itself already inside the
    // fold -- Home's own natural per-real-line landing (byte 8,
    // "three"'s own real start) is also strictly inside 4..15.
    if let NodeKind::TextField(state) = &mut tree.nodes[field].kind {
        state.cursor = 10;
    }
    dispatch_key(&mut tree, root, Key::Home);
    assert_eq!(
        field_state(&tree, field).cursor,
        15,
        "Home's own natural landing (byte 8, \"three\"'s own real line start) sits \
         strictly inside the fold -- must snap forward to byte 15"
    );
    if let NodeKind::TextField(state) = &mut tree.nodes[field].kind {
        state.cursor = 10;
    }
    dispatch_key(&mut tree, root, Key::End);
    assert_eq!(
        field_state(&tree, field).cursor,
        15,
        "End's own natural landing (byte 13, \"three\"'s own real line end) also sits \
         strictly inside the fold -- must snap forward to byte 15 too"
    );
}

#[test]
fn landing_exactly_at_a_folds_own_boundary_is_left_alone() {
    // A narrower fold, real-line-aligned this time (4..7, just
    // "two") -- ArrowDown's own natural landing at real column 0
    // lands exactly on the fold's own start (byte 4), a real,
    // visible boundary position (right where the "⋯" marker itself
    // sits), not hidden content. Must NOT be force-moved elsewhere.
    let (mut tree, root, field) = multiline_field_scene("one\ntwo\nthree\nfour");
    set_folded_ranges(&mut tree, field, 4..7);
    if let NodeKind::TextField(state) = &mut tree.nodes[field].kind {
        state.cursor = 0;
    }
    dispatch_key(&mut tree, root, Key::ArrowDown);
    assert_eq!(
        field_state(&tree, field).cursor,
        4,
        "landing exactly at a fold's own start boundary is a real, visible position -- \
         must be left alone, not snapped forward to the fold's own end"
    );
}

#[test]
fn arrow_down_at_the_last_line_is_a_true_no_op() {
    // A fresh field's own real cursor already starts at content's
    // own end (`TextFieldState::new`'s own contract) -- already on
    // the last line, no extra navigation needed to set this up.
    let (mut tree, root, field) = multiline_field_scene("one\ntwo");
    dispatch_key(&mut tree, root, Key::ArrowDown);
    assert_eq!(
        field_state(&tree, field).cursor,
        7,
        "ArrowDown at the buffer's own last line must be a true no-op"
    );
}

#[test]
fn arrow_up_at_the_first_line_is_a_true_no_op() {
    let (mut tree, root, field) = multiline_field_scene("one\ntwo");
    // Walk the real cursor all the way back to byte 0 via plain
    // ArrowLeft (already proven correct, clamps at 0) -- genuinely
    // on the first line, not `Home`, which (correctly, for
    // multiline) only ever jumps to the *current* line's own
    // start, still "two"'s own, not "one"'s.
    for _ in 0.."one\ntwo".len() {
        dispatch_key(&mut tree, root, Key::ArrowLeft);
    }
    assert_eq!(
        field_state(&tree, field).cursor,
        0,
        "sanity: cursor now at byte 0"
    );
    dispatch_key(&mut tree, root, Key::ArrowUp);
    assert_eq!(
        field_state(&tree, field).cursor,
        0,
        "ArrowUp at the buffer's own first line must be a true no-op"
    );
}

#[test]
fn arrow_up_down_home_end_and_enter_are_true_no_ops_for_a_non_multiline_field() {
    // The real backward-compatibility claim: every existing,
    // already-real `TextField` (multiline defaults to `false`)
    // must stay byte-for-byte unchanged by this step.
    let (mut tree, root, field) = text_field_scene("one\ntwo");
    let before = field_state(&tree, field).cursor;
    assert_eq!(
        dispatch_key(&mut tree, root, Key::ArrowUp),
        DispatchOutcome::None
    );
    assert_eq!(
        dispatch_key(&mut tree, root, Key::ArrowDown),
        DispatchOutcome::None
    );
    assert_eq!(
        field_state(&tree, field).cursor,
        before,
        "ArrowUp/ArrowDown must be true no-ops on a single-line field"
    );
    let outcome = dispatch_key(&mut tree, root, Key::Enter);
    assert_eq!(outcome, DispatchOutcome::None);
    assert_eq!(
        field_state(&tree, field).content,
        "one\ntwo",
        "Enter must still never insert a newline into a single-line field"
    );
}

#[test]
fn shift_arrow_extends_a_real_selection_from_the_cursor() {
    let (mut tree, root, field) = text_field_scene("hello");
    dispatch_key(&mut tree, root, Key::Home); // cursor -> 0

    dispatch_shift_key(&mut tree, root, Key::ArrowRight);
    dispatch_shift_key(&mut tree, root, Key::ArrowRight);

    let state = field_state(&tree, field);
    assert_eq!(
        state.selection_anchor,
        Some(0),
        "the anchor must stay at the real position the selection started from"
    );
    assert_eq!(
        state.cursor, 2,
        "the cursor is the selection's own real focus end"
    );
}

#[test]
fn a_bare_arrow_after_a_real_selection_collapses_to_its_near_edge() {
    let (mut tree, root, field) = text_field_scene("hello");
    dispatch_key(&mut tree, root, Key::Home);
    dispatch_shift_key(&mut tree, root, Key::ArrowRight);
    dispatch_shift_key(&mut tree, root, Key::ArrowRight); // selects "he" (0..2)

    // A bare (non-shift) Left must collapse to the selection's own
    // real left edge (0), not move one more char left from the
    // focus end -- real desktop-editor behavior.
    dispatch_key(&mut tree, root, Key::ArrowLeft);
    let state = field_state(&tree, field);
    assert_eq!(state.cursor, 0);
    assert_eq!(
        state.selection_anchor, None,
        "collapsing must clear the selection"
    );
}

#[test]
fn a_bare_arrow_after_a_real_selection_collapses_to_its_far_edge_when_moving_right() {
    let (mut tree, root, field) = text_field_scene("hello");
    dispatch_key(&mut tree, root, Key::Home);
    dispatch_shift_key(&mut tree, root, Key::ArrowRight);
    dispatch_shift_key(&mut tree, root, Key::ArrowRight); // selects "he" (0..2)

    dispatch_key(&mut tree, root, Key::ArrowRight);
    let state = field_state(&tree, field);
    assert_eq!(state.cursor, 2);
    assert_eq!(state.selection_anchor, None);
}

#[test]
fn shift_home_and_shift_end_extend_the_real_selection_to_the_real_edges() {
    let (mut tree, root, field) = text_field_scene("hello");
    // cursor starts at content's own end (5).
    dispatch_shift_key(&mut tree, root, Key::Home);
    let state = field_state(&tree, field);
    assert_eq!(state.selection_anchor, Some(5));
    assert_eq!(state.cursor, 0);

    dispatch_key(&mut tree, root, Key::End); // collapse, cursor -> end, clears selection
    dispatch_shift_key(&mut tree, root, Key::Home);
    assert_eq!(field_state(&tree, field).selection_anchor, Some(5));
}

#[test]
fn backspace_deletes_a_real_active_selection_instead_of_one_char() {
    let (mut tree, root, field) = text_field_scene("hello");
    dispatch_key(&mut tree, root, Key::Home);
    dispatch_shift_key(&mut tree, root, Key::ArrowRight);
    dispatch_shift_key(&mut tree, root, Key::ArrowRight); // selects "he"

    let outcome = dispatch_key(&mut tree, root, Key::Backspace);
    assert_eq!(
        outcome,
        DispatchOutcome::Changed {
            node: field,
            old_value: ChangedValue::Text("hello".to_string()),
        }
    );
    let state = field_state(&tree, field);
    assert_eq!(state.content, "llo");
    assert_eq!(state.cursor, 0);
    assert_eq!(state.selection_anchor, None);
}

#[test]
fn delete_deletes_a_real_active_selection_instead_of_one_char() {
    let (mut tree, root, field) = text_field_scene("hello");
    dispatch_key(&mut tree, root, Key::Home);
    dispatch_shift_key(&mut tree, root, Key::ArrowRight);
    dispatch_shift_key(&mut tree, root, Key::ArrowRight); // selects "he"

    let outcome = dispatch_key(&mut tree, root, Key::Delete);
    assert_eq!(
        outcome,
        DispatchOutcome::Changed {
            node: field,
            old_value: ChangedValue::Text("hello".to_string()),
        }
    );
    assert_eq!(field_state(&tree, field).content, "llo");
}

#[test]
fn typing_over_a_real_selection_replaces_it() {
    let (mut tree, root, field) = text_field_scene("hello");
    dispatch_key(&mut tree, root, Key::Home);
    dispatch_shift_key(&mut tree, root, Key::ArrowRight);
    dispatch_shift_key(&mut tree, root, Key::ArrowRight); // selects "he"

    let outcome = tree.dispatch(
        root,
        InputEvent::TextInput("HI".to_string()),
        Instant::now(),
    );
    assert_eq!(
        outcome,
        DispatchOutcome::Changed {
            node: field,
            old_value: ChangedValue::Text("hello".to_string()),
        }
    );
    let state = field_state(&tree, field);
    assert_eq!(state.content, "HIllo");
    assert_eq!(state.cursor, 2);
}

#[test]
fn space_over_a_real_selection_replaces_it() {
    let (mut tree, root, field) = text_field_scene("hello");
    dispatch_key(&mut tree, root, Key::Home);
    dispatch_shift_key(&mut tree, root, Key::ArrowRight);
    dispatch_shift_key(&mut tree, root, Key::ArrowRight); // selects "he"

    dispatch_key(&mut tree, root, Key::Space);
    assert_eq!(field_state(&tree, field).content, " llo");
}

#[test]
fn a_collapsed_zero_width_selection_is_not_treated_as_real() {
    // Shift+Right then Shift+Left back to the same spot leaves a
    // real selection_anchor set, but anchor == cursor -- a real
    // editor treats this as "no selection," not an empty delete.
    let (mut tree, root, field) = text_field_scene("hi");
    dispatch_key(&mut tree, root, Key::Home);
    dispatch_shift_key(&mut tree, root, Key::ArrowRight);
    dispatch_shift_key(&mut tree, root, Key::ArrowLeft);
    assert_eq!(field_state(&tree, field).cursor, 0);

    let outcome = dispatch_key(&mut tree, root, Key::Backspace);
    assert_eq!(
        outcome,
        DispatchOutcome::None,
        "a zero-width selection at the real start must still be a genuine no-op"
    );
    assert_eq!(field_state(&tree, field).content, "hi");
}

#[test]
fn text_field_selected_text_reads_the_real_selected_range() {
    let (mut tree, root, field) = text_field_scene("hello");
    dispatch_key(&mut tree, root, Key::Home);
    dispatch_shift_key(&mut tree, root, Key::ArrowRight);
    dispatch_shift_key(&mut tree, root, Key::ArrowRight);

    assert_eq!(tree.text_field_selected_text(field), Some("he".to_string()));
    assert_eq!(
        field_state(&tree, field).content,
        "hello",
        "a pure read must never mutate the field's own real content"
    );
}

#[test]
fn text_field_selected_text_is_none_with_no_real_selection() {
    let (tree, _root, field) = text_field_scene("hello");
    assert_eq!(
        tree.text_field_selected_text(field),
        None,
        "a freshly created field has no real selection at all"
    );
}

#[test]
fn text_field_selected_text_is_none_for_a_non_text_field_node() {
    let mut tree = Tree::new();
    let (kind, style, paint) = leaf(10.0, 10.0);
    let rect = tree.insert(kind, style, paint);
    assert_eq!(tree.text_field_selected_text(rect), None);
}

#[test]
fn cut_text_field_selection_reads_and_deletes_the_real_selection() {
    let (mut tree, root, field) = text_field_scene("hello");
    dispatch_key(&mut tree, root, Key::Home);
    dispatch_shift_key(&mut tree, root, Key::ArrowRight);
    dispatch_shift_key(&mut tree, root, Key::ArrowRight);

    let cut = tree.cut_text_field_selection(field);
    assert_eq!(cut, Some("he".to_string()));
    let state = field_state(&tree, field);
    assert_eq!(
        state.content, "llo",
        "the real selected range must actually be removed, not just read"
    );
    assert_eq!(state.cursor, 0);
    assert_eq!(state.selection_anchor, None);
}

#[test]
fn cut_text_field_selection_with_no_real_selection_is_a_true_no_op() {
    let (mut tree, _root, field) = text_field_scene("hello");
    assert_eq!(tree.cut_text_field_selection(field), None);
    assert_eq!(field_state(&tree, field).content, "hello");
}

fn dispatch_ime_preedit(tree: &mut Tree, root: NodeId, text: &str) -> DispatchOutcome {
    tree.dispatch(
        root,
        InputEvent::ImePreedit(text.to_string()),
        Instant::now(),
    )
}

#[test]
fn ime_preedit_sets_the_real_focused_fields_own_preview_without_touching_content() {
    let (mut tree, root, field) = text_field_scene("hi");
    let outcome = dispatch_ime_preedit(&mut tree, root, "n");
    assert_eq!(
        outcome,
        DispatchOutcome::None,
        "a composition preview is not a real content change"
    );
    let state = field_state(&tree, field);
    assert_eq!(state.preedit, Some("n".to_string()));
    assert_eq!(
        state.content, "hi",
        "a real preedit must never touch committed content"
    );
}

#[test]
fn ime_preedit_with_an_empty_string_clears_a_real_active_preview() {
    let (mut tree, root, field) = text_field_scene("hi");
    dispatch_ime_preedit(&mut tree, root, "n");
    assert_eq!(field_state(&tree, field).preedit, Some("n".to_string()));

    dispatch_ime_preedit(&mut tree, root, "");
    assert_eq!(
        field_state(&tree, field).preedit,
        None,
        "an empty preedit string is winit's own real 'cleared' convention"
    );
}

#[test]
fn ime_preedit_with_no_focused_field_is_a_true_no_op() {
    let mut tree = Tree::new();
    let (_, root_style, root_paint) = leaf(0.0, 0.0);
    let root = tree.insert(NodeKind::Container, root_style, root_paint);
    let outcome = dispatch_ime_preedit(&mut tree, root, "n");
    assert_eq!(outcome, DispatchOutcome::None);
}

#[test]
fn a_real_text_input_commit_clears_a_stale_preedit() {
    let (mut tree, root, field) = text_field_scene("hi");
    dispatch_ime_preedit(&mut tree, root, "n");
    assert_eq!(field_state(&tree, field).preedit, Some("n".to_string()));

    tree.dispatch(
        root,
        InputEvent::TextInput("\u{5462}".to_string()),
        Instant::now(),
    );
    let state = field_state(&tree, field);
    assert_eq!(state.content, "hi\u{5462}");
    assert_eq!(
        state.preedit, None,
        "a real Commit (reaching TextInput) must clear the stale preview"
    );
}

/// M18 Phase 1 (§8, §10): a real click-to-focus, scoped to
/// `TextField` -- `text_field_scene` is deliberately NOT reused here
/// (it pre-focuses via `set_focus_to`), since this test's own claim
/// is that a plain `PointerPressed` genuinely does the focusing.
#[test]
fn pointer_press_on_a_text_field_moves_focus_there() {
    let mut tree = Tree::new();
    let root_style = Style {
        size: Size {
            width: length(120.0),
            height: length(24.0),
        },
        ..Default::default()
    };
    let (_, _, root_paint) = leaf(0.0, 0.0);
    let root = tree.insert(NodeKind::Container, root_style, root_paint);
    let field = tree.insert(
        NodeKind::TextField(Box::new(TextFieldState::new("hi", "Roboto", 400.0, 16.0))),
        Style {
            size: Size {
                width: length(120.0),
                height: length(24.0),
            },
            ..Default::default()
        },
        PaintProperties::new(Color::from_rgba8(0xEE, 0xEE, 0xEE, 0xFF), 0.0, 1.0),
    );
    tree.add_child(root, field);
    tree.compute_layout(
        root,
        Size {
            width: AvailableSpace::Definite(120.0),
            height: AvailableSpace::Definite(24.0),
        },
    );
    assert_eq!(tree.focused(), None, "must start genuinely unfocused");

    tree.dispatch(
        root,
        InputEvent::PointerPressed {
            position: Point::new(10.0, 10.0),
            button: PointerButton::Primary,
        },
        Instant::now(),
    );
    assert_eq!(
        tree.focused(),
        Some(field),
        "a real click on a TextField must move real focus there, the same as every real \
         desktop text field"
    );
}

/// M53 Phase 1 (§8, §10, §11.3): the real gap found while scoping
/// context menus for `TextField`/`CodeEditor` -- a right-click must
/// focus the field too, or a Copy/Cut/Paste context-menu item would
/// act on whatever was last left-clicked, not the field the user
/// just right-clicked.
#[test]
fn pointer_right_click_on_a_text_field_also_moves_focus_there() {
    let mut tree = Tree::new();
    let root_style = Style {
        size: Size {
            width: length(120.0),
            height: length(24.0),
        },
        ..Default::default()
    };
    let (_, _, root_paint) = leaf(0.0, 0.0);
    let root = tree.insert(NodeKind::Container, root_style, root_paint);
    let field = tree.insert(
        NodeKind::TextField(Box::new(TextFieldState::new("hi", "Roboto", 400.0, 16.0))),
        Style {
            size: Size {
                width: length(120.0),
                height: length(24.0),
            },
            ..Default::default()
        },
        PaintProperties::new(Color::from_rgba8(0xEE, 0xEE, 0xEE, 0xFF), 0.0, 1.0),
    );
    tree.add_child(root, field);
    tree.compute_layout(
        root,
        Size {
            width: AvailableSpace::Definite(120.0),
            height: AvailableSpace::Definite(24.0),
        },
    );
    assert_eq!(tree.focused(), None, "must start genuinely unfocused");

    tree.dispatch(
        root,
        InputEvent::PointerPressed {
            position: Point::new(10.0, 10.0),
            button: PointerButton::Secondary,
        },
        Instant::now(),
    );
    assert_eq!(
        tree.focused(),
        Some(field),
        "a real right-click on a TextField must move real focus there too, so a context \
         menu opened by the same right-click acts on the correct field"
    );
}

#[test]
fn pointer_press_on_a_non_text_field_does_not_move_focus() {
    let mut tree = Tree::new();
    let (k, s, p) = leaf(100.0, 100.0);
    let root = tree.insert(k, s, p);
    tree.dispatch(
        root,
        InputEvent::PointerPressed {
            position: Point::new(10.0, 10.0),
            button: PointerButton::Primary,
        },
        Instant::now(),
    );
    assert_eq!(
        tree.focused(),
        None,
        "clicking an ordinary Rect must never move focus -- this is scoped to TextField, \
         not a generic click-to-focus for every node kind"
    );
}

/// M18 Phase 1 (§8, §11.9, §11.10): `hit_test_local`'s own real
/// claim -- it must report the SAME local point `draw_own`
/// painted into, under a real ancestor transform, not just the same
/// hit `NodeId` `hit_test` already proved (`hit_test_follows_an_
/// ancestor_translate_transform`, above).
#[test]
fn hit_test_local_reports_the_click_in_the_hit_nodes_own_local_space() {
    let mut tree = Tree::new();
    let root_style = Style {
        size: Size {
            width: length(200.0),
            height: length(200.0),
        },
        ..Default::default()
    };
    let (_, _, root_paint) = leaf(0.0, 0.0);
    let root = tree.insert(NodeKind::Container, root_style, root_paint);

    let (_, camera_style, camera_paint) = leaf(200.0, 200.0);
    let camera = tree.insert(NodeKind::Container, camera_style, camera_paint);
    tree.add_child(root, camera);

    let (k, s, p) = leaf(50.0, 50.0);
    let chip = tree.insert(k, s, p);
    tree.add_child(camera, chip);

    tree.compute_layout(
        root,
        Size {
            width: AvailableSpace::Definite(200.0),
            height: AvailableSpace::Definite(200.0),
        },
    );

    assert_eq!(
        tree.hit_test_local(root, Point::new(25.0, 25.0)),
        Some((chip, Point::new(25.0, 25.0))),
        "with no transform applied, the local point must equal the canvas point"
    );

    // Same real (100, 100) translate `hit_test_follows_an_ancestor_
    // translate_transform` already proves moves `chip` to canvas
    // (100,100)-(150,150) -- a click at canvas (125, 125) must
    // report chip's own LOCAL point as (25, 25), the inverse-
    // transformed coordinate `draw_own` itself painted at, not
    // the raw canvas point.
    tree.get_mut(camera).unwrap().paint.transform.current = Affine::translate((100.0, 100.0));
    assert_eq!(
        tree.hit_test_local(root, Point::new(125.0, 125.0)),
        Some((chip, Point::new(25.0, 25.0))),
    );
}

#[test]
fn set_text_field_cursor_moves_the_cursor_and_clears_a_selection() {
    let (mut tree, _root, field) = text_field_scene("hello");
    {
        let NodeKind::TextField(state) = &mut tree.get_mut(field).unwrap().kind else {
            panic!("expected a TextField node");
        };
        state.selection_anchor = Some(0);
        state.cursor = 5;
    }
    let moved = tree.set_text_field_cursor(field, 2);
    assert!(moved);
    let state = field_state(&tree, field);
    assert_eq!(state.cursor, 2);
    assert_eq!(
        state.selection_anchor, None,
        "a plain click-driven cursor move must collapse any active selection"
    );
}

#[test]
fn set_text_field_cursor_clamps_beyond_content_length() {
    let (mut tree, _root, field) = text_field_scene("hi");
    assert!(tree.set_text_field_cursor(field, 999));
    assert_eq!(field_state(&tree, field).cursor, 2);
}

/// M38 Phase 7 (§5, §8) test scene: a real, laid-out, focused
/// multiline field with 20 real lines ("line0".."line19"), each
/// exactly 6 real bytes ("lineN\n") so a target line's own real
/// byte offset is simply `line_index * 6` -- a real, bounded
/// 200x100 viewport, `font_size = 14.0` matching `Tree::CODE_
/// EDITOR_LINE_HEIGHT_RATIO`'s own real 1.35 multiplier for a
/// real, hand-verifiable `line_height = 18.9`.
///
/// M39 Phase 1 (§5, §8): the field is its own real `compute_
/// layout` root now -- real debugging while writing this phase's
/// own new horizontal scene found a real, latent bug in the
/// original wrapping-`Container`-root pattern this helper used to
/// have: `leaf(0.0, 0.0)`'s own explicit zero-width root style
/// puts the field inside a real, explicit *zero-width* flex-row
/// parent, and taffy's own default `flex_shrink: 1.0` genuinely
/// shrinks the field's own main-axis (width) size to fit that
/// zero available space -- real, silently wrong `layout(field).
/// size.width`, invisible only because no test here ever read it
/// (height survives, since it's the cross axis, where an explicit
/// size is honored directly rather than stretched). Mirrors
/// `scrollable_view`'s own already-correct pattern (`tree/tests.rs`, M38
/// Phase 6).
fn caret_follow_scene() -> (Tree, NodeId, NodeId) {
    let mut tree = Tree::new();
    let content = (0..20)
        .map(|i| format!("line{i}"))
        .collect::<Vec<_>>()
        .join("\n");
    let mut state = TextFieldState::new(content, "Monospace", 400.0, 14.0);
    state.multiline = true;
    let field = tree.insert(
        NodeKind::TextField(Box::new(state)),
        Style {
            size: Size {
                width: length(200.0),
                height: length(100.0),
            },
            ..Default::default()
        },
        PaintProperties::new(Color::from_rgba8(0xEE, 0xEE, 0xEE, 0xFF), 0.0, 1.0),
    );
    tree.compute_layout(
        field,
        Size {
            width: AvailableSpace::Definite(200.0),
            height: AvailableSpace::Definite(100.0),
        },
    );
    tree.set_focus_to(field);
    (tree, field, field)
}

#[test]
fn scroll_text_field_caret_into_view_scrolls_down_to_reveal_a_caret_below_the_viewport() {
    let (mut tree, _root, field) = caret_follow_scene();
    // Line 5's own real start: 5 lines * 6 real bytes ("lineN\n")
    // each = byte 30. caret_top = 5 * 18.9 = 94.5, caret_bottom =
    // 113.4 -- past the real 100px viewport (scroll starts at 0),
    // so this must scroll down to `caret_bottom - viewport_height`
    // = 113.4 - 100 = 13.4.
    tree.set_text_field_cursor(field, 30);
    let scroll = field_state(&tree, field).scroll_offset.current;
    assert!(
        (scroll - 13.4).abs() < 0.01,
        "must scroll down exactly enough to reveal line 5's own real bottom edge, got {scroll}"
    );
}

/// 0.5.1 (#53): the viewport the caret must stay in is the content box, not
/// the border box -- a field padded 20 top and bottom shows 60 of its 100.
#[test]
fn scroll_text_field_caret_into_view_uses_the_content_box_of_a_padded_field() {
    let mut tree = Tree::new();
    let content = (0..20)
        .map(|i| format!("line{i}"))
        .collect::<Vec<_>>()
        .join("\n");
    let mut state = TextFieldState::new(content, "Monospace", 400.0, 14.0);
    state.multiline = true;
    let field = tree.insert(
        NodeKind::TextField(Box::new(state)),
        Style {
            size: Size {
                width: length(200.0),
                height: length(100.0),
            },
            padding: taffy::geometry::Rect {
                left: length(0.0),
                top: length(20.0),
                right: length(0.0),
                bottom: length(20.0),
            },
            ..Default::default()
        },
        PaintProperties::new(Color::from_rgba8(0xEE, 0xEE, 0xEE, 0xFF), 0.0, 1.0),
    );
    tree.compute_layout(
        field,
        Size {
            width: AvailableSpace::Definite(200.0),
            height: AvailableSpace::Definite(100.0),
        },
    );
    tree.set_focus_to(field);
    // Line 5's bottom edge is at 113.4. A 100px viewport scrolls to 13.4
    // (the unpadded test above); the 60px content box scrolls to 53.4.
    tree.set_text_field_cursor(field, 30);
    let scroll = field_state(&tree, field).scroll_offset.current;
    assert!(
        (scroll - 53.4).abs() < 0.01,
        "must reveal the caret within the 60px content box, got {scroll}"
    );
}

#[test]
fn scroll_text_field_caret_into_view_scrolls_back_up_to_reveal_a_caret_above_the_viewport() {
    let (mut tree, _root, field) = caret_follow_scene();
    // First scroll down to a real, deep position (line 15).
    tree.set_text_field_cursor(field, 90);
    assert!(
        field_state(&tree, field).scroll_offset.current > 0.0,
        "sanity: scrolled down for line 15"
    );
    // Then jump the cursor back to the real content start (line 0)
    // -- caret_top = 0.0, below any positive scroll, so this must
    // scroll all the way back to exactly 0.0.
    tree.set_text_field_cursor(field, 0);
    let scroll = field_state(&tree, field).scroll_offset.current;
    assert_eq!(
        scroll, 0.0,
        "must scroll all the way back up to reveal line 0, got {scroll}"
    );
}

#[test]
fn scroll_text_field_caret_into_view_is_a_true_no_op_for_a_single_line_field() {
    // A single-line field never scrolls at all, regardless of how
    // long its own real content is or where the cursor lands --
    // `Tree::scroll_text_field_caret_into_view`'s own real
    // `!state.multiline` guard.
    let (mut tree, root, field) = text_field_scene("a very long single line of real text");
    dispatch_key(&mut tree, root, Key::Home);
    dispatch_key(&mut tree, root, Key::End);
    assert_eq!(
        field_state(&tree, field).scroll_offset.current,
        0.0,
        "a single-line field's own scroll_offset must never move"
    );
}

/// M39 Phase 1 (§5, §8) test scene: a real, laid-out, focused
/// multiline field with three real lines -- "short", 30 real
/// ASCII 'a' characters (genuinely wider than the real 100px
/// viewport at `font_size = 14.0`, `char_width = 14.0 * 0.6 =
/// 8.4`, so 30 chars = 252px), and "short2". Line 1 (the long
/// one) starts at real byte 6 ("short\n" is 6 bytes), so column
/// `N` on it sits at byte `6 + N` (every char is single-byte
/// ASCII).
fn horizontal_caret_follow_scene() -> (Tree, NodeId, NodeId) {
    // The field is its own real `compute_layout` root -- the
    // identical real pattern `scrollable_view` already proves
    // (`tree/tests.rs`'s own M38 Phase 6 scene helper): a wrapping
    // `Container` root with an explicit `leaf(0.0, 0.0)` style
    // (every *vertical* caret-follow scene's own pattern) puts the
    // field inside a real, explicit *zero-width* flex-row parent
    // -- real, found by direct debugging before writing this fix,
    // not assumed -- so taffy's own default `flex_shrink: 1.0`
    // genuinely shrinks the field's own main-axis (width) size
    // down to fit that zero available space, even though its own
    // `Style` asks for a real 100px. Height survives only because
    // it's the cross axis, where an explicit (non-`Auto`) size is
    // honored directly rather than stretched. Skipping the wrapper
    // root avoids the whole issue: an `Auto`-or-explicit-sized
    // top-level `compute_layout` root takes its own real size
    // straight from the available-space argument, no flex
    // algorithm involved.
    let mut tree = Tree::new();
    let content = format!("short\n{}\nshort2", "a".repeat(30));
    let mut state = TextFieldState::new(content, "Monospace", 400.0, 14.0);
    state.multiline = true;
    let field = tree.insert(
        NodeKind::TextField(Box::new(state)),
        Style {
            size: Size {
                width: length(100.0),
                height: length(100.0),
            },
            ..Default::default()
        },
        PaintProperties::new(Color::from_rgba8(0xEE, 0xEE, 0xEE, 0xFF), 0.0, 1.0),
    );
    tree.compute_layout(
        field,
        Size {
            width: AvailableSpace::Definite(100.0),
            height: AvailableSpace::Definite(100.0),
        },
    );
    tree.set_focus_to(field);
    (tree, field, field)
}

#[test]
fn scroll_text_field_caret_into_view_scrolls_right_to_reveal_a_caret_past_the_viewport() {
    let (mut tree, _root, field) = horizontal_caret_follow_scene();
    // Column 20 on the long line: caret_left = 20 * 8.4 = 168.0,
    // caret_right = 176.4 -- past the real 100px viewport (h-
    // scroll starts at 0), so this must scroll right exactly
    // enough to reveal it: 176.4 - 100 = 76.4.
    tree.set_text_field_cursor(field, 26);
    let h_scroll = field_state(&tree, field).horizontal_scroll_offset.current;
    assert!(
        (h_scroll - 76.4).abs() < 0.01,
        "must scroll right exactly enough to reveal column 20's own real right edge, got \
         {h_scroll}"
    );
}

#[test]
fn scroll_text_field_caret_into_view_scrolls_back_left_to_reveal_a_caret_before_the_viewport() {
    let (mut tree, _root, field) = horizontal_caret_follow_scene();
    // First scroll right to a real, deep column (column 25).
    tree.set_text_field_cursor(field, 31);
    assert!(
        field_state(&tree, field).horizontal_scroll_offset.current > 0.0,
        "sanity: scrolled right for column 25"
    );
    // Then jump the cursor back to the real line start (column 0,
    // byte 6) -- caret_left = 0.0, below any positive h-scroll, so
    // this must scroll all the way back to exactly 0.0.
    tree.set_text_field_cursor(field, 6);
    let h_scroll = field_state(&tree, field).horizontal_scroll_offset.current;
    assert_eq!(
        h_scroll, 0.0,
        "must scroll all the way back left to reveal column 0, got {h_scroll}"
    );
}

#[test]
fn set_text_field_cursor_snaps_to_a_real_char_boundary() {
    // "h" + a 3-byte character -- byte offset 2 lands inside it.
    let (mut tree, _root, field) = text_field_scene("h\u{5462}");
    assert!(tree.set_text_field_cursor(field, 2));
    let cursor = field_state(&tree, field).cursor;
    assert!(
        field_state(&tree, field).content.is_char_boundary(cursor),
        "a mid-character offset must snap back to a real char boundary, not corrupt state"
    );
    assert_eq!(
        cursor, 1,
        "must snap DOWN to the boundary before the requested offset"
    );
}

#[test]
fn set_text_field_cursor_on_a_non_text_field_is_a_true_no_op() {
    let mut tree = Tree::new();
    let (k, s, p) = leaf(100.0, 100.0);
    let root = tree.insert(k, s, p);
    assert!(!tree.set_text_field_cursor(root, 0));
}

#[test]
fn extend_text_field_selection_seeds_the_anchor_at_the_current_cursor_on_first_move() {
    let (mut tree, _root, field) = text_field_scene("hello");
    tree.set_text_field_cursor(field, 2);
    assert_eq!(field_state(&tree, field).selection_anchor, None);

    let extended = tree.extend_text_field_selection(field, 5);
    assert!(extended);
    let state = field_state(&tree, field);
    assert_eq!(
        state.selection_anchor,
        Some(2),
        "the anchor must seed at wherever the cursor already was, the real click position"
    );
    assert_eq!(state.cursor, 5);
}

#[test]
fn extend_text_field_selection_keeps_growing_without_moving_the_anchor() {
    let (mut tree, _root, field) = text_field_scene("hello world");
    tree.set_text_field_cursor(field, 0);
    tree.extend_text_field_selection(field, 3);
    tree.extend_text_field_selection(field, 7);
    tree.extend_text_field_selection(field, 11);
    let state = field_state(&tree, field);
    assert_eq!(
        state.selection_anchor,
        Some(0),
        "repeated drag-move calls must never re-seed or move the anchor"
    );
    assert_eq!(state.cursor, 11);
}

#[test]
fn extend_text_field_selection_clamps_beyond_content_length() {
    let (mut tree, _root, field) = text_field_scene("hi");
    tree.set_text_field_cursor(field, 0);
    assert!(tree.extend_text_field_selection(field, 999));
    assert_eq!(field_state(&tree, field).cursor, 2);
}

#[test]
fn extend_text_field_selection_on_a_non_text_field_is_a_true_no_op() {
    let mut tree = Tree::new();
    let (k, s, p) = leaf(100.0, 100.0);
    let root = tree.insert(k, s, p);
    assert!(!tree.extend_text_field_selection(root, 0));
}

/// M53 Phase 1 (§8, §10, §11.3): `select_all_text_field` -- real,
/// exact-value proof of the "Select All" convention (anchor at the
/// real start, cursor lands at the real end, matching every real
/// desktop text field's own Ctrl+A behavior).
#[test]
fn select_all_text_field_selects_from_start_to_end() {
    let (mut tree, _root, field) = text_field_scene("hello world");
    tree.set_text_field_cursor(field, 3);
    assert!(tree.select_all_text_field(field));
    let state = field_state(&tree, field);
    assert_eq!(state.selection_anchor, Some(0));
    assert_eq!(state.cursor, 11);
}

#[test]
fn select_all_text_field_on_empty_content_selects_an_empty_range() {
    let (mut tree, _root, field) = text_field_scene("");
    assert!(tree.select_all_text_field(field));
    let state = field_state(&tree, field);
    assert_eq!(state.selection_anchor, Some(0));
    assert_eq!(state.cursor, 0);
}

#[test]
fn select_all_text_field_lands_on_a_real_char_boundary_for_multi_byte_content() {
    // Three 3-byte real characters -- content.len() is a real char
    // boundary already (the end of the string always is), but this
    // proves the cursor lands at the genuine byte length, not a
    // truncated/miscounted one, for real multi-byte UTF-8 content.
    let (mut tree, _root, field) = text_field_scene("\u{5462}\u{5462}\u{5462}");
    assert!(tree.select_all_text_field(field));
    let state = field_state(&tree, field);
    assert_eq!(state.selection_anchor, Some(0));
    assert_eq!(
        state.cursor, 9,
        "three 3-byte chars must select through all 9 real bytes"
    );
    assert!(state.content.is_char_boundary(state.cursor));
}

#[test]
fn select_all_text_field_on_a_non_text_field_is_a_true_no_op() {
    let mut tree = Tree::new();
    let (k, s, p) = leaf(100.0, 100.0);
    let root = tree.insert(k, s, p);
    assert!(!tree.select_all_text_field(root));
}

/// M32 Phase 6 (§4, §5, §8): a real 3-row terminal seeded with
/// distinct real per-row content ("hello"/"world"/"tre!!"), padded
/// to `cols` with real blank cells -- mirrors `text_field_scene`'s
/// own role for the new `Terminal` selection tests below.
fn terminal_scene(rows: &[&str]) -> (Tree, NodeId) {
    let cols = rows.iter().map(|r| r.len()).max().unwrap_or(0) as u16;
    let mut tree = Tree::new();
    let mut state = TerminalState::new(cols, rows.len() as u16, "Roboto", 14.0);
    for (row_idx, row) in rows.iter().enumerate() {
        for (col_idx, ch) in row.chars().enumerate() {
            let idx = row_idx * usize::from(cols) + col_idx;
            state.cells[idx].ch = ch;
        }
    }
    let terminal = tree.insert(
        NodeKind::Terminal(Box::new(state)),
        Style::default(),
        PaintProperties::new(Color::from_rgba8(0, 0, 0, 0xFF), 0.0, 1.0),
    );
    (tree, terminal)
}

#[test]
fn set_terminal_selection_start_seeds_a_collapsed_selection_at_the_real_cell() {
    let (mut tree, term) = terminal_scene(&["hello"]);
    assert!(tree.set_terminal_selection_start(term, 0, 2));
    let NodeKind::Terminal(state) = &tree.get(term).unwrap().kind else {
        unreachable!()
    };
    assert_eq!(state.selection_start, Some((0, 2)));
    assert_eq!(
        state.selection_end,
        Some((0, 2)),
        "a plain press with no drag yet must leave a collapsed (start == end) selection"
    );
}

#[test]
fn set_terminal_selection_start_on_a_non_terminal_is_a_true_no_op() {
    let mut tree = Tree::new();
    let (k, s, p) = leaf(100.0, 100.0);
    let root = tree.insert(k, s, p);
    assert!(!tree.set_terminal_selection_start(root, 0, 0));
}

#[test]
fn extend_terminal_selection_grows_the_real_end_without_moving_the_start() {
    let (mut tree, term) = terminal_scene(&["hello"]);
    tree.set_terminal_selection_start(term, 0, 1);
    assert!(tree.extend_terminal_selection(term, 0, 3));
    let NodeKind::Terminal(state) = &tree.get(term).unwrap().kind else {
        unreachable!()
    };
    assert_eq!(
        state.selection_start,
        Some((0, 1)),
        "the real anchor must not move"
    );
    assert_eq!(state.selection_end, Some((0, 3)));
}

#[test]
fn extend_terminal_selection_seeds_the_start_if_none_was_set_yet() {
    // Real, defensive parity with `extend_text_field_selection`'s
    // own `get_or_insert`-at-first-move shape, even though every
    // real caller (`app.rs`'s own PointerMoved handling) only ever
    // extends after a real press already set the anchor.
    let (mut tree, term) = terminal_scene(&["hello"]);
    assert!(tree.extend_terminal_selection(term, 0, 3));
    let NodeKind::Terminal(state) = &tree.get(term).unwrap().kind else {
        unreachable!()
    };
    assert_eq!(state.selection_start, Some((0, 3)));
}

#[test]
fn terminal_selected_text_reads_a_real_single_row_range() {
    let (mut tree, term) = terminal_scene(&["hello"]);
    tree.set_terminal_selection_start(term, 0, 0);
    tree.extend_terminal_selection(term, 0, 3);
    assert_eq!(tree.terminal_selected_text(term), Some("hel".to_string()));
}

#[test]
fn terminal_selected_text_reads_real_linear_selection_across_multiple_rows() {
    let (mut tree, term) = terminal_scene(&["hello", "world", "tre!!"]);
    tree.set_terminal_selection_start(term, 0, 3);
    tree.extend_terminal_selection(term, 2, 2);
    assert_eq!(
        tree.terminal_selected_text(term),
        Some("lo\nworld\ntr".to_string()),
        "real linear (reading-order) selection: the rest of row 0 from col 3, all of row \
         1, row 2 up to (not including) col 2"
    );
}

#[test]
fn terminal_selected_text_normalizes_a_real_selection_dragged_backward() {
    // A real drag from bottom-right back up to top-left must read
    // identically to the same real range selected forward --
    // `text_field_selected_text`'s own `anchor`/`cursor` ordering
    // establishes the identical real precedent.
    let (mut tree, term) = terminal_scene(&["hello", "world"]);
    tree.set_terminal_selection_start(term, 1, 2);
    tree.extend_terminal_selection(term, 0, 1);
    assert_eq!(
        tree.terminal_selected_text(term),
        Some("ello\nwo".to_string())
    );
}

#[test]
fn terminal_selected_text_is_none_for_a_collapsed_or_absent_selection() {
    let (mut tree, term) = terminal_scene(&["hello"]);
    assert_eq!(
        tree.terminal_selected_text(term),
        None,
        "a freshly created terminal has no real selection at all"
    );
    tree.set_terminal_selection_start(term, 0, 2);
    assert_eq!(
        tree.terminal_selected_text(term),
        None,
        "a collapsed (start == end) selection reads as no real selection, the identical \
         real convention text_field_selected_text already established"
    );
}

/// M39 Phase 4 (§5, §7): a real blank cell's own four new
/// attribute fields must default `false` -- `TerminalCell::blank`
/// is the real value every never-written grid cell starts as
/// (`TerminalState::new`'s own `vec![TerminalCell::blank(); ...]`),
/// so a stray `true` default here would falsely style an entire
/// freshly-constructed terminal.
#[test]
fn terminal_cell_blank_defaults_every_new_attribute_to_false() {
    let cell = TerminalCell::blank();
    assert!(!cell.bold);
    assert!(!cell.dim);
    assert!(!cell.italic);
    assert!(!cell.underline);
    assert!(!cell.inverse);
}

#[test]
fn terminal_selected_text_is_none_for_a_non_terminal_node() {
    let mut tree = Tree::new();
    let (k, s, p) = leaf(10.0, 10.0);
    let rect = tree.insert(k, s, p);
    assert_eq!(tree.terminal_selected_text(rect), None);
}

#[test]
fn terminal_selected_text_trims_real_trailing_blank_cells_per_row() {
    // "hello" seeded into a 10-wide grid pads columns 5-9 with real
    // blank space cells -- selecting the whole row must not
    // include that real, never-actually-there padding.
    let mut tree = Tree::new();
    let mut state = TerminalState::new(10, 1, "Roboto", 14.0);
    for (col, ch) in "hello".chars().enumerate() {
        state.cells[col].ch = ch;
    }
    let term = tree.insert(
        NodeKind::Terminal(Box::new(state)),
        Style::default(),
        PaintProperties::new(Color::from_rgba8(0, 0, 0, 0xFF), 0.0, 1.0),
    );
    tree.set_terminal_selection_start(term, 0, 0);
    tree.extend_terminal_selection(term, 0, 10);
    assert_eq!(tree.terminal_selected_text(term), Some("hello".to_string()));
}

/// M22 Phase 1 (§5): `NodeKind::Image` round-trips through
/// `Tree::insert`/`Tree::get` exactly like every other kind -- no
/// special-cased storage, the same "common core + per-kind payload"
/// shape §1 Locked Decisions already establishes.
#[test]
fn nodekind_image_round_trips_through_insert_and_get() {
    let mut tree = Tree::new();
    let image_data = peniko::ImageData {
        data: peniko::Blob::from(vec![0xFFu8, 0x00, 0x00, 0xFF]),
        format: peniko::ImageFormat::Rgba8,
        alpha_type: peniko::ImageAlphaType::Alpha,
        width: 1,
        height: 1,
    };
    let (_, style, paint) = leaf(10.0, 10.0);
    let id = tree.insert(
        NodeKind::Image(ImageState::new(image_data.clone())),
        style,
        paint,
    );

    let NodeKind::Image(state) = &tree.get(id).unwrap().kind else {
        panic!("expected an Image node");
    };
    assert_eq!(state.image, image_data);
}

/// M30 Phase 5 Step 1 (§5, §7): real, direct coverage of the new
/// `Node.hit_testable` opt-out -- proves it actually changes what
/// `hit_test` returns, not just that it compiles. First asserts
/// the real bug this step found (a decorative inner `Rect`
/// permanently steals a hit from its own ancestor), then that
/// `set_hit_testable(id, false)` fixes exactly that.
#[test]
fn hit_testable_false_makes_a_node_defer_to_its_own_ancestor() {
    let mut tree = Tree::new();
    let root = tree.insert(
        NodeKind::Container,
        Style {
            size: Size {
                width: length(100.0),
                height: length(100.0),
            },
            ..Default::default()
        },
        PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 1.0),
    );

    let (kind, mut outer_style, paint) = leaf(100.0, 100.0);
    outer_style.position = Position::Absolute;
    outer_style.inset = TaffyRect {
        left: length(0.0),
        top: length(0.0),
        right: auto(),
        bottom: auto(),
    };
    let outer = tree.insert(kind, outer_style, paint);
    tree.add_child(root, outer);

    // A decorative inner Rect, absolutely positioned to overlap
    // `outer`'s own center point (50, 50).
    let (inner_kind, mut inner_style, inner_paint) = leaf(20.0, 20.0);
    inner_style.position = Position::Absolute;
    inner_style.inset = TaffyRect {
        left: length(40.0),
        top: length(40.0),
        right: auto(),
        bottom: auto(),
    };
    let inner = tree.insert(inner_kind, inner_style, inner_paint);
    tree.add_child(outer, inner);

    tree.compute_layout(
        root,
        Size {
            width: AvailableSpace::Definite(100.0),
            height: AvailableSpace::Definite(100.0),
        },
    );

    let point = Point::new(50.0, 50.0);
    assert_eq!(
        tree.hit_test(root, point),
        Some(inner),
        "before opting out, the decorative inner Rect must claim the hit -- the real bug \
         this step found"
    );

    tree.set_hit_testable(inner, false);
    assert_eq!(
        tree.hit_test(root, point),
        Some(outer),
        "after opting out, the same point must resolve to the ancestor instead"
    );
}

// --- M65 (§5, §6): the real per-kind existence counters
// `compute_layout`'s own sync_*_layouts functions consult instead
// of scanning. ------------------------------------------------

#[test]
fn existence_counters_track_insert_and_remove_for_every_real_kind() {
    let mut tree = Tree::new();
    assert_eq!(tree.scroll_view_count, 0);
    assert_eq!(tree.virtual_list_count, 0);

    let scroll_view = tree.insert(
        NodeKind::ScrollView(ScrollViewState::new(false)),
        Style::default(),
        PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 1.0),
    );
    assert_eq!(tree.scroll_view_count, 1);

    let virtual_list = tree.insert(
        NodeKind::VirtualList(VirtualListState::new(10, ItemExtent::Fixed(20.0))),
        Style::default(),
        PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 1.0),
    );
    assert_eq!(tree.virtual_list_count, 1);

    // Inserting an ordinary node of no tracked kind must not move
    // any counter -- the real "only what's actually relevant"
    // contract every counter above depends on.
    let plain = tree.insert(
        NodeKind::Container,
        Style::default(),
        PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 1.0),
    );
    assert_eq!(tree.scroll_view_count, 1);
    assert_eq!(tree.virtual_list_count, 1);

    tree.remove(scroll_view);
    assert_eq!(tree.scroll_view_count, 0);
    tree.remove(virtual_list);
    assert_eq!(tree.virtual_list_count, 0);
    tree.remove(plain);
}

/// An ancestor's removal recursively removes a `ScrollView` several
/// levels below it -- each recursive `Tree::remove` call must
/// decrement the counter for its own child, not just the top-level
/// call.
#[test]
fn existence_counters_decrement_correctly_through_recursive_removal() {
    let mut tree = Tree::new();
    let paint = || PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 1.0);
    let root = tree.insert(NodeKind::Container, Style::default(), paint());
    let middle = tree.insert(NodeKind::Container, Style::default(), paint());
    let scroll_view = tree.insert(
        NodeKind::ScrollView(ScrollViewState::new(false)),
        Style::default(),
        paint(),
    );
    tree.add_child(root, middle);
    tree.add_child(middle, scroll_view);
    assert_eq!(tree.scroll_view_count, 1);

    tree.remove(root);
    assert_eq!(
        tree.scroll_view_count, 0,
        "a ScrollView removed only as a side effect of an ancestor's own removal must still \
         decrement the real counter, not leave it stale"
    );
}

/// M36 Phase 1 (§5, §7, §11.7): a real `ScrollView` (100px viewport)
/// with a single 400px-tall child whose content height the caller
/// supplies explicitly.
fn scrollable_view(horizontal: bool) -> (Tree, NodeId, NodeId) {
    let mut tree = Tree::new();
    let (view_w, view_h) = if horizontal {
        (100.0, 50.0)
    } else {
        (100.0, 100.0)
    };
    let view = tree.insert(
        NodeKind::ScrollView(ScrollViewState::new(horizontal)),
        Style {
            size: Size {
                width: length(view_w),
                height: length(view_h),
            },
            ..Default::default()
        },
        PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 1.0),
    );
    let (content_w, content_h) = if horizontal {
        (400.0, 50.0)
    } else {
        (100.0, 400.0)
    };
    let (k, s, p) = leaf(content_w, content_h);
    let content = tree.insert(k, s, p);
    tree.add_child(view, content);
    tree.compute_layout(
        view,
        Size {
            width: AvailableSpace::Definite(view_w),
            height: AvailableSpace::Definite(view_h),
        },
    );
    (tree, view, content)
}

#[test]
fn scroll_scroll_view_by_moves_and_clamps_the_offset_at_both_ends() {
    // 400px of real content in a 100px viewport -- max_scroll =
    // 400 - 100 = 300.0, the identical real clamp math
    // `scroll_virtual_list_by`'s own sibling test already proves.
    let (mut tree, view, _content) = scrollable_view(false);

    tree.scroll_scroll_view_by(view, 50.0);
    let NodeKind::ScrollView(state) = &tree.get(view).unwrap().kind else {
        panic!("expected a ScrollView");
    };
    assert_eq!(state.scroll.current, 50.0);

    tree.scroll_scroll_view_by(view, 1000.0);
    let NodeKind::ScrollView(state) = &tree.get(view).unwrap().kind else {
        panic!("expected a ScrollView");
    };
    assert_eq!(
        state.scroll.current, 300.0,
        "must clamp to the real content-extent-minus-viewport max, not overshoot"
    );

    tree.scroll_scroll_view_by(view, -10_000.0);
    let NodeKind::ScrollView(state) = &tree.get(view).unwrap().kind else {
        panic!("expected a ScrollView");
    };
    assert_eq!(
        state.scroll.current, 0.0,
        "must clamp at the real lower bound too, not go negative"
    );
}

#[test]
fn thumb_geometry_computes_the_real_track_thumb_and_along_values() {
    // M38 Phase 6 (§5, §7, §11.7): 100px viewport, 400px content --
    // pyCopper's own real `thumb_geometry` math, ported directly.
    // track = 100 - 2*2 (margin) = 96. thumb = max(96*(100/400),
    // 32 (min length)) = max(24, 32) = 32.
    let (mut tree, view, _content) = scrollable_view(false);
    let NodeKind::ScrollView(state) = &tree.get(view).unwrap().kind else {
        panic!("expected a ScrollView");
    };
    assert_eq!(
        state.thumb_geometry(100.0, 400.0),
        (96.0, 32.0, 2.0),
        "at scroll 0, `along` must sit at the real starting margin"
    );

    tree.scroll_scroll_view_by(view, 150.0); // half of max_scroll (300)
    let NodeKind::ScrollView(state) = &tree.get(view).unwrap().kind else {
        panic!("expected a ScrollView");
    };
    let (track, thumb, along) = state.thumb_geometry(100.0, 400.0);
    assert_eq!(
        (track, thumb),
        (96.0, 32.0),
        "track/thumb don't depend on scroll position"
    );
    assert!(
        (along - 34.0).abs() < 0.001,
        "at half scroll, `along` must sit halfway across the real (track - thumb) = 64px \
         of travel (2 + 64*0.5 = 34), got {along}"
    );
}

#[test]
fn grabs_scroll_view_thumb_is_true_only_within_the_real_thumb_plus_slop() {
    // Vertical view: thumb sits on the right edge, x in
    // [100-4-2, 100-2] = [94, 98], y in [2, 34] at scroll 0
    // (`thumb_geometry_computes_the_real_track_thumb_and_along_
    // values`'s own proof above).
    let (tree, view, _content) = scrollable_view(false);
    assert!(
        tree.grabs_scroll_view_thumb(view, Point::new(96.0, 18.0)),
        "a point squarely inside the real thumb rect must grab it"
    );
    assert!(
        tree.grabs_scroll_view_thumb(view, Point::new(94.0 - 5.0, 18.0)),
        "a point just within the real SCROLLBAR_GRAB_SLOP tolerance must still grab it"
    );
    assert!(
        !tree.grabs_scroll_view_thumb(view, Point::new(10.0, 10.0)),
        "a point nowhere near the real thumb (e.g. over the scrolled content) must not grab it"
    );
}

#[test]
fn update_scroll_view_thumb_drag_moves_the_scroll_offset_proportionally_to_pointer_travel() {
    let (mut tree, view, _content) = scrollable_view(false);
    // Start a real drag exactly the way `PointerPressed`'s own
    // dispatch arm does: anchor the current pointer coordinate and
    // scroll offset, then mark the view as the live drag target.
    let NodeKind::ScrollView(state) = &mut tree.get_mut(view).unwrap().kind else {
        panic!("expected a ScrollView");
    };
    state.thumb_drag_anchor = Some((18.0, 0.0));
    tree.dragging = Some(view);

    // track - thumb = 96 - 32 = 64px of real thumb travel maps to
    // the real 300px of max_scroll -- moving the pointer down by
    // 32px (half the real travel) must move scroll by half of
    // max_scroll (150).
    tree.update_scroll_view_thumb_drag(view, Point::new(0.0, 50.0), Instant::now());
    let NodeKind::ScrollView(state) = &tree.get(view).unwrap().kind else {
        panic!("expected a ScrollView");
    };
    assert!(
        (state.scroll.current - 150.0).abs() < 0.001,
        "half the real thumb travel must move scroll by half of max_scroll, got {}",
        state.scroll.current
    );
}

#[test]
fn a_scroll_view_shorter_than_its_own_content_has_zero_max_scroll_content_extent_only_this_deep() {
    let mut tree = Tree::new();
    let view = tree.insert(
        NodeKind::ScrollView(ScrollViewState::new(false)),
        Style {
            size: Size {
                width: length(100.0),
                height: length(500.0),
            },
            ..Default::default()
        },
        PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 1.0),
    );
    let (k, s, p) = leaf(100.0, 200.0);
    let content = tree.insert(k, s, p);
    tree.add_child(view, content);
    tree.compute_layout(
        view,
        Size {
            width: AvailableSpace::Definite(100.0),
            height: AvailableSpace::Definite(500.0),
        },
    );

    tree.scroll_scroll_view_by(view, 50.0);
    let NodeKind::ScrollView(state) = &tree.get(view).unwrap().kind else {
        panic!("expected a ScrollView");
    };
    assert_eq!(
        state.scroll.current, 0.0,
        "content shorter than its own viewport can't scroll at all, correctly"
    );
}

#[test]
fn dispatch_scroll_over_a_scroll_views_child_updates_its_real_scroll_offset() {
    let (mut tree, view, content) = scrollable_view(false);
    let _ = content;
    // A real wheel notch over the scrolled content itself (not the
    // view's own root pixel) must still bubble up to the view's own
    // scroll offset, the identical real proof `VirtualList`'s own
    // sibling test already establishes.
    let outcome = tree.dispatch(
        view,
        InputEvent::Scroll {
            delta: ScrollDelta::Lines(0.0, -2.0),
            position: Point::new(50.0, 50.0),
        },
        Instant::now(),
    );
    assert_eq!(outcome, DispatchOutcome::None);
    let NodeKind::ScrollView(state) = &tree.get(view).unwrap().kind else {
        panic!("expected a ScrollView");
    };
    assert_eq!(
        state.scroll.current, 40.0,
        "2.0 lines * this crate's own 20px-per-line convention == 40.0px"
    );
}

/// M36 Phase 1 (§5, §7, §11.7): the real, decisive proof this whole
/// phase's own design choice exists for -- unlike `VirtualList`
/// (this phase's own investigation found a real, previously
/// undiscovered hit-test-after-scroll bug there), a real, specific
/// grandchild inside a `ScrollView`'s scrolled content must
/// resolve correctly to a point at its own current, post-scroll
/// screen position, and *not* to a point at its own stale,
/// pre-scroll position -- a genuinely decisive proof (unlike
/// testing the single content child alone, whose own full 400px
/// local extent would trivially contain almost any in-viewport
/// point regardless of whether scroll were ever subtracted at
/// all).
#[test]
fn hit_test_after_a_real_scroll_resolves_a_real_grandchild_at_its_own_post_scroll_position() {
    let (mut tree, view, content) = scrollable_view(false);
    // A real, narrow marker grandchild at content-local y 200..220
    // -- the identical real `Position::Absolute` + `inset`
    // technique every other precisely-positioned node in this
    // codebase already uses.
    let (k, s, p) = leaf(100.0, 20.0);
    let marker = tree.insert(k, s, p);
    let mut marker_style = tree.get(marker).unwrap().layout_style.clone();
    marker_style.position = Position::Absolute;
    marker_style.inset = TaffyRect {
        left: length(0.0),
        top: length(200.0),
        right: auto(),
        bottom: auto(),
    };
    tree.set_layout_style(marker, marker_style);
    tree.add_child(content, marker);

    tree.scroll_scroll_view_by(view, 150.0);
    tree.compute_layout(
        view,
        Size {
            width: AvailableSpace::Definite(100.0),
            height: AvailableSpace::Definite(100.0),
        },
    );

    // After scrolling by 150, the marker's own real content-local
    // slot (200..220) now sits at real screen y (200-150)..(220-150)
    // = 50..70 -- a point there must hit the marker specifically.
    let hit_post_scroll = tree.hit_test(view, Point::new(50.0, 60.0));
    assert_eq!(
        hit_post_scroll,
        Some(marker),
        "a real point at the marker's own genuine post-scroll screen position must hit it"
    );

    // The marker's own *stale*, pre-scroll screen position (as if
    // scroll had never been subtracted, real screen y 200..220) is
    // now entirely past the real 100px viewport -- `VirtualList`'s
    // own real gap would have kept resolving there; this design
    // must not.
    let hit_stale = tree.hit_test(view, Point::new(50.0, 210.0));
    assert_ne!(
        hit_stale,
        Some(marker),
        "a real point at the marker's own stale, pre-scroll position must not still \
         resolve to it -- that's the exact real bug this phase's own design avoids"
    );
    let _ = content;
}

// --- 0.4.3 M18: the scroll core added in 0.4.2 M12 and 0.4.3 M15 -----------

fn offset_of(tree: &Tree, view: NodeId) -> f64 {
    let NodeKind::ScrollView(state) = &tree.get(view).unwrap().kind else {
        panic!("expected a ScrollView");
    };
    state.scroll.current
}

/// A vertical 100x100 view over a 100x400 column holding a box of each
/// height in `heights`, top to bottom, laid out.
fn view_over_boxes(heights: &[f32]) -> (Tree, NodeId, Vec<NodeId>) {
    let mut tree = Tree::new();
    let view = tree.insert(
        NodeKind::ScrollView(ScrollViewState::new(false)),
        Style {
            size: Size {
                width: length(100.0),
                height: length(100.0),
            },
            ..Default::default()
        },
        PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 1.0),
    );
    let content = tree.insert(
        NodeKind::Rect,
        Style {
            size: Size {
                width: length(100.0),
                height: length(400.0),
            },
            flex_direction: FlexDirection::Column,
            ..Default::default()
        },
        PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 1.0),
    );
    tree.add_child(view, content);
    let boxes = heights
        .iter()
        .map(|&h| {
            let (k, s, p) = leaf(100.0, h);
            let id = tree.insert(k, s, p);
            tree.add_child(content, id);
            id
        })
        .collect();
    tree.compute_layout(
        view,
        Size {
            width: AvailableSpace::Definite(100.0),
            height: AvailableSpace::Definite(100.0),
        },
    );
    (tree, view, boxes)
}

#[test]
fn max_scroll_is_the_content_past_the_viewport_along_the_views_axis() {
    let (tree, view, _) = scrollable_view(false);
    assert_eq!(
        tree.max_scroll(view),
        Some(300.0),
        "400 of content, 100 of view"
    );
    let (tree, view, _) = scrollable_view(true);
    assert_eq!(
        tree.max_scroll(view),
        Some(300.0),
        "measured across, not down"
    );
}

#[test]
fn max_scroll_is_zero_when_content_fits_and_none_without_content() {
    let mut tree = Tree::new();
    let empty = tree.insert(
        NodeKind::ScrollView(ScrollViewState::new(false)),
        Style::default(),
        PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 1.0),
    );
    assert_eq!(tree.max_scroll(empty), None, "no content yet");
    let (k, s, p) = leaf(10.0, 10.0);
    let plain = tree.insert(k, s, p);
    assert_eq!(tree.max_scroll(plain), None, "not a scroll view");

    let mut tree = Tree::new();
    let view = tree.insert(
        NodeKind::ScrollView(ScrollViewState::new(false)),
        Style {
            size: Size {
                width: length(100.0),
                height: length(500.0),
            },
            ..Default::default()
        },
        PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 1.0),
    );
    let (k, s, p) = leaf(100.0, 200.0);
    let content = tree.insert(k, s, p);
    tree.add_child(view, content);
    tree.compute_layout(
        view,
        Size {
            width: AvailableSpace::Definite(100.0),
            height: AvailableSpace::Definite(500.0),
        },
    );
    assert_eq!(
        tree.max_scroll(view),
        Some(0.0),
        "content that fits has no travel"
    );
}

#[test]
fn scroll_view_for_key_finds_the_nearest_view_along_the_keys_axis() {
    // 0.4.4 M21: one that can move the key's way, too.
    let (mut tree, view, content) = scrollable_view(false);
    for key in [Key::ArrowDown, Key::PageDown, Key::End] {
        assert_eq!(
            tree.scroll_view_for_key(content, key),
            Some(view),
            "{key:?}"
        );
    }
    for key in [Key::ArrowUp, Key::PageUp, Key::Home] {
        assert_eq!(
            tree.scroll_view_for_key(content, key),
            None,
            "{key:?} at the top"
        );
    }
    for key in [Key::ArrowLeft, Key::ArrowRight] {
        assert_eq!(
            tree.scroll_view_for_key(content, key),
            None,
            "{key:?} is across"
        );
    }
    tree.scroll_scroll_view_by(view, 100.0);
    for key in [
        Key::ArrowUp,
        Key::ArrowDown,
        Key::PageUp,
        Key::PageDown,
        Key::Home,
        Key::End,
    ] {
        assert_eq!(
            tree.scroll_view_for_key(content, key),
            Some(view),
            "{key:?} mid-way"
        );
    }

    let (tree, view, content) = scrollable_view(true);
    for key in [Key::ArrowRight, Key::End] {
        assert_eq!(
            tree.scroll_view_for_key(content, key),
            Some(view),
            "{key:?}"
        );
    }
    for key in [Key::ArrowLeft, Key::Home, Key::PageDown] {
        assert_eq!(tree.scroll_view_for_key(content, key), None, "{key:?}");
    }
    assert_eq!(
        tree.scroll_view_for_key(view, Key::ArrowRight),
        Some(view),
        "itself"
    );
}

#[test]
fn scroll_view_for_key_passes_a_carousel_for_the_page_around_it() {
    // A 100x50 carousel over 400, at the top of a page with 300 of travel.
    let (mut tree, page, boxes) = view_over_boxes(&[50.0]);
    let carousel = tree.insert(
        NodeKind::ScrollView(ScrollViewState::new(true)),
        Style {
            size: Size {
                width: length(100.0),
                height: length(50.0),
            },
            ..Default::default()
        },
        PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 1.0),
    );
    let (k, s, p) = leaf(400.0, 50.0);
    let item = tree.insert(k, s, p);
    tree.add_child(carousel, item);
    tree.add_child(boxes[0], carousel);
    tree.compute_layout(
        page,
        Size {
            width: AvailableSpace::Definite(100.0),
            height: AvailableSpace::Definite(100.0),
        },
    );
    assert_eq!(
        tree.scroll_view_for_key(item, Key::ArrowRight),
        Some(carousel)
    );
    assert_eq!(
        tree.scroll_view_for_key(item, Key::ArrowDown),
        Some(page),
        "across it"
    );
    assert_eq!(
        tree.scroll_view_for_key(item, Key::End),
        Some(carousel),
        "nearest"
    );
    tree.scroll_scroll_view_by(carousel, 1000.0);
    assert_eq!(
        tree.scroll_view_for_key(item, Key::End),
        Some(page),
        "0.4.4 M21: at the carousel's end, End chains to the page"
    );
    assert_eq!(tree.scroll_view_for_key(item, Key::Home), Some(carousel));
    assert_eq!(
        tree.scroll_view_for_key(item, Key::ArrowRight),
        None,
        "nothing can"
    );
}

#[test]
fn scroll_by_key_moves_a_line_a_page_or_to_an_end_clamped() {
    let (mut tree, view, _) = scrollable_view(false);
    tree.take_dirty();
    assert!(tree.scroll_by_key(view, Key::ArrowDown));
    assert_eq!(offset_of(&tree, view), KEY_SCROLL_LINE);
    assert!(tree.take_dirty(), "a key scroll repaints");
    assert!(tree.scroll_by_key(view, Key::PageDown));
    assert_eq!(
        offset_of(&tree, view),
        KEY_SCROLL_LINE + 100.0,
        "one viewport"
    );
    assert!(tree.scroll_by_key(view, Key::End));
    assert_eq!(offset_of(&tree, view), 300.0);
    assert!(
        !tree.scroll_by_key(view, Key::PageDown),
        "clamped at the end"
    );
    assert_eq!(offset_of(&tree, view), 300.0);
    assert!(tree.scroll_by_key(view, Key::Home));
    assert_eq!(offset_of(&tree, view), 0.0);
    tree.take_dirty();
    assert!(
        !tree.scroll_by_key(view, Key::ArrowUp),
        "already at the top"
    );
    assert!(!tree.take_dirty(), "nothing moved, nothing to repaint");
}

#[test]
fn scroll_into_view_moves_the_least_that_shows_the_node() {
    // Boxes at 0..20, 20..250 (a spacer), and 250..270.
    let (mut tree, view, boxes) = view_over_boxes(&[20.0, 230.0, 20.0]);
    assert!(!tree.scroll_into_view(boxes[0]), "already visible");
    assert_eq!(offset_of(&tree, view), 0.0);
    assert!(tree.scroll_into_view(boxes[2]));
    assert_eq!(
        offset_of(&tree, view),
        170.0,
        "its bottom at the view's bottom"
    );
}

#[test]
fn scroll_into_view_aligns_a_node_longer_than_its_view_to_its_start() {
    // A 150-tall box at 250, in a 100-tall view.
    let (mut tree, view, boxes) = view_over_boxes(&[250.0, 150.0]);
    assert!(tree.scroll_into_view(boxes[1]));
    assert_eq!(offset_of(&tree, view), 250.0);
}

/// 0.5.4 (review): asking for scroll changes, and laying out, must not count a
/// scroll view that did not move as touched -- it sent the damage tracker down
/// its full walk on every frame of any tree that has one.
#[test]
fn an_idle_scroll_view_is_not_touched_by_scroll_changes_or_layout() {
    let (mut tree, view, _) = scrollable_view(false);
    tree.take_touched();
    assert!(tree.take_scroll_changes().is_empty());
    let touched = tree.take_touched();
    assert!(
        !touched.all && touched.ids.is_empty(),
        "a quiet check touches nothing"
    );
    tree.compute_layout(
        view,
        Size {
            width: AvailableSpace::Definite(100.0),
            height: AvailableSpace::Definite(100.0),
        },
    );
    tree.take_touched();
    tree.compute_layout(
        view,
        Size {
            width: AvailableSpace::Definite(100.0),
            height: AvailableSpace::Definite(100.0),
        },
    );
    let touched = tree.take_touched();
    assert!(
        !touched.all,
        "a repeated layout of an unmoved view is not a full re-walk"
    );
    // A move is still reported, and then the node is touched.
    tree.scroll_by_key(view, Key::ArrowDown);
    tree.take_touched();
    assert_eq!(tree.take_scroll_changes().len(), 1);
    assert!(tree.take_touched().ids.contains(&view));
}

#[test]
fn take_scroll_changes_reports_each_move_once() {
    let (mut tree, view, _) = scrollable_view(false);
    assert!(tree.take_scroll_changes().is_empty(), "nothing moved yet");
    tree.scroll_by_key(view, Key::ArrowDown);
    tree.scroll_by_key(view, Key::ArrowDown);
    assert_eq!(
        tree.take_scroll_changes(),
        vec![(view, 0.0, 2.0 * KEY_SCROLL_LINE)]
    );
    assert!(
        tree.take_scroll_changes().is_empty(),
        "reported, then quiet"
    );
    tree.scroll_by_key(view, Key::Home);
    assert_eq!(
        tree.take_scroll_changes(),
        vec![(view, 2.0 * KEY_SCROLL_LINE, 0.0)]
    );

    let mut tree = Tree::new();
    let (k, s, p) = leaf(10.0, 10.0);
    tree.insert(k, s, p);
    assert!(
        tree.take_scroll_changes().is_empty(),
        "no scroll views at all"
    );
}

#[test]
fn a_wheel_over_a_carousel_it_cannot_move_scrolls_the_page_around_it() {
    // 0.4.3 M17: a 100x50 horizontal carousel (over 400) at the top of the
    // page; a vertical wheel there has no part along the carousel's axis.
    let (mut tree, page, boxes) = view_over_boxes(&[50.0]);
    let carousel = tree.insert(
        NodeKind::ScrollView(ScrollViewState::new(true)),
        Style {
            size: Size {
                width: length(100.0),
                height: length(50.0),
            },
            ..Default::default()
        },
        PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 1.0),
    );
    let (k, s, p) = leaf(400.0, 50.0);
    let strip = tree.insert(k, s, p);
    tree.add_child(carousel, strip);
    tree.add_child(boxes[0], carousel);
    tree.compute_layout(
        page,
        Size {
            width: AvailableSpace::Definite(100.0),
            height: AvailableSpace::Definite(100.0),
        },
    );
    let wheel_at = |x: f64, y: f64, at_y: f64| InputEvent::Scroll {
        delta: ScrollDelta::Lines(x, y),
        position: Point::new(50.0, at_y),
    };
    tree.dispatch(page, wheel_at(0.0, -2.0, 25.0), Instant::now());
    assert_eq!(
        (offset_of(&tree, page), offset_of(&tree, carousel)),
        (40.0, 0.0)
    );
    // The page scrolled 40, so the carousel now spans y -40..10.
    tree.dispatch(page, wheel_at(-2.0, 0.0, 5.0), Instant::now());
    assert_eq!(
        (offset_of(&tree, page), offset_of(&tree, carousel)),
        (40.0, 40.0)
    );
}

/// A vertical 100x50 inner view over `content` of height, at the top of a
/// `view_over_boxes` page (100x100 over 400), laid out.
fn page_with_inner_view(content: f32) -> (Tree, NodeId, NodeId) {
    let (mut tree, page, boxes) = view_over_boxes(&[50.0]);
    let inner = tree.insert(
        NodeKind::ScrollView(ScrollViewState::new(false)),
        Style {
            size: Size {
                width: length(100.0),
                height: length(50.0),
            },
            ..Default::default()
        },
        PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 1.0),
    );
    let (k, s, p) = leaf(100.0, content);
    let inner_content = tree.insert(k, s, p);
    tree.add_child(inner, inner_content);
    tree.add_child(boxes[0], inner);
    tree.compute_layout(
        page,
        Size {
            width: AvailableSpace::Definite(100.0),
            height: AvailableSpace::Definite(100.0),
        },
    );
    (tree, page, inner)
}

#[test]
fn a_wheel_chains_past_a_view_that_cannot_move_its_way() {
    // 0.4.4 M21: over the inner view, whose 200 of content gives 150 of
    // travel. A wheel line down is 20px; `Lines(0, -1)` is one line down.
    let down = InputEvent::Scroll {
        delta: ScrollDelta::Lines(0.0, -1.0),
        position: Point::new(50.0, 25.0),
    };
    let up = InputEvent::Scroll {
        delta: ScrollDelta::Lines(0.0, 1.0),
        position: Point::new(50.0, 25.0),
    };
    let (mut tree, page, inner) = page_with_inner_view(200.0);
    assert!(!tree.can_scroll(inner, -1.0), "at the top: can't go up");
    assert!(tree.can_scroll(inner, 1.0));
    tree.dispatch(page, down.clone(), Instant::now());
    assert_eq!(
        (offset_of(&tree, page), offset_of(&tree, inner)),
        (0.0, 20.0),
        "mid-way, the inner view takes it"
    );
    tree.dispatch(page, up.clone(), Instant::now());
    tree.dispatch(page, up, Instant::now());
    assert_eq!(
        (offset_of(&tree, page), offset_of(&tree, inner)),
        (0.0, 0.0),
        "back at its top, and the page is too: nothing moves"
    );
    tree.scroll_scroll_view_by(inner, 1000.0);
    assert_eq!(offset_of(&tree, inner), 150.0);
    assert!(!tree.can_scroll(inner, 1.0), "at its end");
    tree.dispatch(page, down.clone(), Instant::now());
    assert_eq!(
        (offset_of(&tree, page), offset_of(&tree, inner)),
        (20.0, 150.0),
        "at its end, the wheel goes to the page"
    );

    let (mut tree, page, inner) = page_with_inner_view(30.0);
    assert!(!tree.can_scroll(inner, 1.0) && !tree.can_scroll(inner, -1.0));
    tree.dispatch(page, down, Instant::now());
    assert_eq!(
        offset_of(&tree, page),
        20.0,
        "content that fits has nothing to scroll: the page takes it"
    );
}

// --- 0.5.3 (#98): a text node's explicit width is rounded up, not down ----------

fn text_node(tree: &mut Tree, style: Style) -> NodeId {
    tree.insert(
        NodeKind::Text(crate::node::TextState {
            content: "Add a task".to_string(),
            font_family: "Roboto".to_string(),
            font_weight: 500.0,
            font_size: 14.0,
            align: crate::node::TextAlign::Start,
            line_height: None,
            options: Default::default(),
        }),
        style,
        PaintProperties::new(Color::from_rgba8(255, 255, 255, 255), 0.0, 1.0),
    )
}

/// Lays `child` out as the only child of a wide root and returns its width.
fn laid_out_width(kind_text: bool, style: Style) -> (f32, Style) {
    let mut tree = Tree::new();
    let (k, root_style, paint) = leaf(400.0, 100.0);
    let root = tree.insert(k, root_style, paint);
    let child = if kind_text {
        text_node(&mut tree, style)
    } else {
        let (k, _, paint) = leaf(0.0, 0.0);
        tree.insert(k, style, paint)
    };
    tree.add_child(root, child);
    tree.compute_layout(
        root,
        Size {
            width: AvailableSpace::Definite(400.0),
            height: AvailableSpace::Definite(100.0),
        },
    );
    (
        tree.layout(child).size.width,
        tree.get(child).unwrap().layout_style.clone(),
    )
}

fn width_style(width: f32) -> Style {
    Style {
        size: Size {
            width: length(width),
            height: length(20.0),
        },
        ..Default::default()
    }
}

#[test]
fn a_text_nodes_explicit_width_is_laid_out_rounded_up() {
    for (set, laid_out) in [
        (66.43, 67.0),
        (66.0, 66.0),
        (66.01, 67.0),
        (66.99, 67.0),
        (0.5, 1.0),
    ] {
        let (width, _) = laid_out_width(true, width_style(set));
        assert_eq!(width, laid_out, "width {set}");
    }
}

#[test]
fn a_boxes_width_is_still_rounded_as_before() {
    let (width, _) = laid_out_width(false, width_style(66.43));
    assert_eq!(width, 66.0, "only text gets the round-up");
}

#[test]
fn the_node_keeps_the_width_that_was_set() {
    let (_, style) = laid_out_width(true, width_style(66.43));
    assert_eq!(
        style.size.width,
        length(66.43),
        "get() reads back exactly what was set"
    );
}

#[test]
fn a_text_nodes_min_and_max_width_are_rounded_up_too() {
    let mut style = width_style(10.0);
    style.min_size.width = length(66.43);
    let (width, _) = laid_out_width(true, style);
    assert_eq!(
        width, 67.0,
        "a minimum the text needs is not rounded below it"
    );
    let mut style = Style {
        size: Size {
            width: taffy::style::Dimension::auto(),
            height: length(20.0),
        },
        ..Default::default()
    };
    style.max_size.width = length(66.43);
    style.flex_grow = 1.0;
    let (width, _) = laid_out_width(true, style);
    assert_eq!(
        width, 67.0,
        "a cap the text fits in is not rounded below it"
    );
}

#[test]
fn a_text_nodes_percent_and_auto_widths_are_untouched() {
    let style = Style {
        size: Size {
            width: taffy::style::Dimension::percent(0.5),
            height: length(20.0),
        },
        ..Default::default()
    };
    let (width, _) = laid_out_width(true, style);
    assert_eq!(width, 200.0);
}

#[test]
fn a_text_nodes_width_set_after_creation_is_rounded_up_too() {
    let mut tree = Tree::new();
    let (k, root_style, paint) = leaf(400.0, 100.0);
    let root = tree.insert(k, root_style, paint);
    let child = text_node(&mut tree, width_style(50.0));
    tree.add_child(root, child);
    tree.set_layout_style(child, width_style(66.43));
    tree.compute_layout(
        root,
        Size {
            width: AvailableSpace::Definite(400.0),
            height: AvailableSpace::Definite(100.0),
        },
    );
    assert_eq!(tree.layout(child).size.width, 67.0);
    assert_eq!(
        tree.get(child).unwrap().layout_style.size.width,
        length(66.43)
    );
}

/// 0.5.4 (#103): `tick_all` ticks only known-animating nodes between full
/// scans; an animation started anywhere, on any value, must still be seen.
mod active_ticking {
    use super::*;
    use crate::animation::MotionCurve;
    use std::time::Duration;

    const SECOND: Duration = Duration::from_secs(1);

    fn many(tree: &mut Tree, n: usize) -> Vec<NodeId> {
        (0..n)
            .map(|_| {
                let (kind, style, paint) = leaf(5.0, 5.0);
                tree.insert(kind, style, paint)
            })
            .collect()
    }

    #[test]
    fn an_animation_started_after_a_scan_is_still_ticked() {
        let mut tree = Tree::new();
        let ids = many(&mut tree, 50);
        let t0 = Instant::now();
        // A first tick with nothing running settles the scan.
        assert!(!tree.tick_all(t0).0);
        // Later starts on two different nodes, with ticks in between.
        tree.get_mut(ids[7]).unwrap().paint.opacity.animate_to(
            0.0,
            SECOND,
            MotionCurve::Linear,
            t0,
        );
        let (active, _) = tree.tick_all(t0 + SECOND / 2);
        assert!(active);
        tree.get_mut(ids[41]).unwrap().paint.opacity.animate_to(
            0.0,
            SECOND,
            MotionCurve::Linear,
            t0 + SECOND / 2,
        );
        assert!(tree.tick_all(t0 + SECOND * 3 / 4).0);
        let a = tree.get(ids[7]).unwrap().paint.opacity.current;
        let b = tree.get(ids[41]).unwrap().paint.opacity.current;
        assert!((a - 0.25).abs() < 0.01, "first keeps running: {a}");
        assert!((b - 0.75).abs() < 0.01, "second runs from its start: {b}");
        // Both finish, then the tree is idle.
        let (active, _) = tree.tick_all(t0 + SECOND * 5);
        assert!(!active);
        assert_eq!(tree.get(ids[7]).unwrap().paint.opacity.current, 0.0);
        assert_eq!(tree.get(ids[41]).unwrap().paint.opacity.current, 0.0);
        assert!(!tree.tick_all(t0 + SECOND * 6).0);
    }

    #[test]
    fn a_finished_animation_is_not_ticked_again() {
        let mut tree = Tree::new();
        let ids = many(&mut tree, 3);
        let t0 = Instant::now();
        tree.get_mut(ids[1]).unwrap().paint.opacity.animate_to(
            0.0,
            SECOND,
            MotionCurve::Linear,
            t0,
        );
        assert!(!tree.tick_all(t0 + SECOND * 2).0);
        // A value set directly after it finished stays put.
        tree.get_mut(ids[1]).unwrap().paint.opacity.current = 0.9;
        assert!(!tree.tick_all(t0 + SECOND * 3).0);
        assert_eq!(tree.get(ids[1]).unwrap().paint.opacity.current, 0.9);
    }

    #[test]
    fn a_removed_animating_node_is_skipped() {
        let mut tree = Tree::new();
        let ids = many(&mut tree, 3);
        let t0 = Instant::now();
        tree.get_mut(ids[0]).unwrap().paint.opacity.animate_to(
            0.0,
            SECOND,
            MotionCurve::Linear,
            t0,
        );
        assert!(tree.tick_all(t0 + SECOND / 2).0);
        tree.remove(ids[0]);
        assert!(!tree.tick_all(t0 + SECOND * 3 / 4).0);
    }

    #[test]
    fn a_text_fields_tint_and_a_scroll_views_offset_are_ticked() {
        let mut tree = Tree::new();
        let (_, style, paint) = leaf(5.0, 5.0);
        let field = tree.insert(
            NodeKind::TextField(Box::new(TextFieldState::new("hi", "Roboto", 400.0, 16.0))),
            style,
            paint,
        );
        let (_, style, paint) = leaf(5.0, 5.0);
        let view = tree.insert(
            NodeKind::ScrollView(ScrollViewState::new(false)),
            style,
            paint,
        );
        let t0 = Instant::now();
        assert!(!tree.tick_all(t0).0);
        if let NodeKind::TextField(s) = &mut tree.get_mut(field).unwrap().kind {
            s.text_tint.animate_to(
                Color::from_rgba8(0, 0, 255, 255),
                SECOND,
                MotionCurve::Linear,
                t0,
            );
        }
        if let NodeKind::ScrollView(s) = &mut tree.get_mut(view).unwrap().kind {
            s.scroll.animate_to(40.0, SECOND, MotionCurve::Linear, t0);
        }
        assert!(tree.tick_all(t0 + SECOND / 2).0);
        assert!(!tree.tick_all(t0 + SECOND * 2).0);
        let NodeKind::ScrollView(s) = &tree.get(view).unwrap().kind else {
            unreachable!()
        };
        assert_eq!(s.scroll.current, 40.0);
    }
}

/// 0.5.4 (#103): `cargo test -p engine-core --release tick_cost -- --ignored --nocapture`
#[test]
#[ignore = "timing, not correctness"]
fn tick_cost_with_one_animating_node_in_9216() {
    use crate::animation::MotionCurve;
    use std::time::Duration;
    let mut tree = Tree::new();
    let ids: Vec<NodeId> = (0..9216)
        .map(|_| {
            let (kind, style, paint) = leaf(5.0, 5.0);
            tree.insert(kind, style, paint)
        })
        .collect();
    let t0 = Instant::now();
    tree.get_mut(ids[100]).unwrap().paint.opacity.animate_to(
        0.0,
        Duration::from_secs(3600),
        MotionCurve::Linear,
        t0,
    );
    tree.tick_all(t0);
    let runs = 2000;
    let start = Instant::now();
    for i in 0..runs {
        tree.tick_all(t0 + Duration::from_millis(i));
    }
    println!("tick_all: {:?} per frame", start.elapsed() / runs as u32);
}

/// 0.5.4 (#105): a scroll offset moves what is painted and hit, and changes
/// no layout.
mod scroll_without_layout {
    use super::*;

    fn relayout(tree: &mut Tree, view: NodeId) {
        tree.compute_layout(
            view,
            Size {
                width: AvailableSpace::Definite(100.0),
                height: AvailableSpace::Definite(100.0),
            },
        );
    }

    #[test]
    fn scrolling_a_view_moves_its_content_and_not_its_layout() {
        let (mut tree, view, boxes) = view_over_boxes(&[100.0, 100.0, 100.0, 100.0]);
        let content = tree.get(view).unwrap().children[0];
        let before = tree.layout(content).location;
        assert_eq!(tree.absolute_position(boxes[2]), (0.0, 200.0));

        tree.scroll_scroll_view_by(view, 150.0);
        relayout(&mut tree, view);

        assert_eq!(
            tree.layout(content).location,
            before,
            "layout is unchanged by a scroll"
        );
        assert_eq!(tree.scroll_shift(content), (0.0, -150.0));
        assert_eq!(tree.scroll_shift(view), (0.0, 0.0));
        assert_eq!(tree.absolute_position(boxes[2]), (0.0, 50.0));
        // A point where box 2 now is hits box 2; its old place hits box 0/1.
        let hit = tree.hit_test_at(view, Point::new(10.0, 60.0), Affine::IDENTITY);
        assert_eq!(hit.map(|(id, _)| id), Some(boxes[2]));
    }

    #[test]
    fn a_view_scroll_leaves_taffy_with_nothing_to_redo() {
        let (mut tree, view, _boxes) = view_over_boxes(&[100.0, 100.0, 100.0, 100.0]);
        tree.scroll_scroll_view_by(view, 50.0);
        relayout(&mut tree, view);
        // The sync passes found nothing to change: no second layout pass.
        assert!(!tree.sync_scroll_view_layouts());
        assert!(!tree.sync_virtual_list_layouts());
    }

    #[test]
    fn a_horizontal_view_shifts_sideways() {
        let mut tree = Tree::new();
        let view = tree.insert(
            NodeKind::ScrollView(ScrollViewState::new(true)),
            Style {
                size: Size {
                    width: length(100.0),
                    height: length(50.0),
                },
                ..Default::default()
            },
            PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 1.0),
        );
        let (k, s, p) = leaf(400.0, 50.0);
        let strip = tree.insert(k, s, p);
        tree.add_child(view, strip);
        relayout(&mut tree, view);
        tree.scroll_scroll_view_by(view, 120.0);
        relayout(&mut tree, view);
        assert_eq!(tree.scroll_shift(strip), (-120.0, 0.0));
        assert_eq!(tree.absolute_position(strip), (-120.0, 0.0));
    }

    #[test]
    fn a_node_outside_a_scroller_is_not_shifted() {
        let (tree, view, boxes) = view_over_boxes(&[100.0]);
        assert_eq!(tree.scroll_shift(boxes[0]), (0.0, 0.0));
        assert_eq!(tree.scroll_shift(view), (0.0, 0.0));
    }
}

/// 0.5.4 (#112): selection in static text.
mod static_selection {
    use super::*;

    fn text(tree: &mut Tree, content: &str) -> NodeId {
        let (_, style, paint) = leaf(100.0, 20.0);
        tree.insert(
            NodeKind::Text(crate::TextState {
                content: content.to_string(),
                font_family: "Roboto".to_string(),
                font_weight: 400.0,
                font_size: 16.0,
                align: crate::TextAlign::Start,
                line_height: None,
                options: Default::default(),
            }),
            style,
            paint,
        )
    }

    #[test]
    fn a_selection_selects_clamps_and_copies() {
        let mut tree = Tree::new();
        let id = text(&mut tree, "héllo world");
        assert!(tree.set_text_selection(id, 0, 6));
        assert_eq!(tree.static_selected_text().as_deref(), Some("héllo"));
        // Either order, and inside the two-byte 'é' (bytes 1..3) snaps back.
        assert!(tree.set_text_selection(id, 6, 2));
        assert_eq!(tree.static_selected_text().as_deref(), Some("éllo"));
        // Past the end is clamped; an empty selection copies nothing.
        assert!(tree.set_text_selection(id, 8, 400));
        assert_eq!(tree.static_selected_text().as_deref(), Some("orld"));
        tree.set_text_selection(id, 3, 3);
        assert_eq!(tree.static_selected_text(), None);
    }

    #[test]
    fn extending_keeps_the_anchor() {
        let mut tree = Tree::new();
        let id = text(&mut tree, "abcdef");
        tree.set_text_selection(id, 2, 2);
        tree.extend_text_selection(id, 5);
        tree.extend_text_selection(id, 4);
        assert_eq!(tree.static_selected_text().as_deref(), Some("cd"));
    }

    #[test]
    fn one_node_owns_the_selection_at_a_time() {
        let mut tree = Tree::new();
        let (a, b) = (text(&mut tree, "first"), text(&mut tree, "second"));
        tree.set_text_selection(a, 0, 5);
        tree.set_text_selection(b, 0, 3);
        assert_eq!(tree.static_selected_text().as_deref(), Some("sec"));
        assert_eq!(tree.text_selected_text(a), None, "the first was cleared");
        tree.clear_text_selection();
        assert_eq!(tree.static_selected_text(), None);
    }

    #[test]
    fn select_all_takes_the_whole_text_of_the_selected_node() {
        let mut tree = Tree::new();
        let id = text(&mut tree, "héllo world");
        assert!(!tree.select_all_static_text(), "nothing is selected yet");
        tree.set_text_selection(id, 2, 2);
        assert!(tree.select_all_static_text());
        assert_eq!(tree.static_selected_text().as_deref(), Some("héllo world"));
    }

    #[test]
    fn shift_arrows_home_and_end_move_the_focus_end_by_character() {
        use crate::Key;
        let mut tree = Tree::new();
        let id = text(&mut tree, "aéb");
        tree.set_text_selection(id, 1, 1);
        assert!(tree.extend_static_selection(Key::ArrowRight));
        assert_eq!(
            tree.static_selected_text().as_deref(),
            Some("é"),
            "a two-byte char at once"
        );
        assert!(tree.extend_static_selection(Key::ArrowRight));
        assert_eq!(tree.static_selected_text().as_deref(), Some("éb"));
        assert!(
            tree.extend_static_selection(Key::ArrowRight),
            "at the end it stays"
        );
        assert_eq!(tree.static_selected_text().as_deref(), Some("éb"));
        assert!(tree.extend_static_selection(Key::ArrowLeft));
        assert!(tree.extend_static_selection(Key::ArrowLeft));
        assert_eq!(tree.static_selected_text(), None, "back to the anchor");
        tree.extend_static_selection(Key::ArrowLeft);
        assert_eq!(
            tree.static_selected_text().as_deref(),
            Some("a"),
            "past it, the other way"
        );
        tree.extend_static_selection(Key::End);
        assert_eq!(tree.static_selected_text().as_deref(), Some("éb"));
        tree.extend_static_selection(Key::Home);
        assert_eq!(tree.static_selected_text().as_deref(), Some("a"));
        assert!(
            !tree.extend_static_selection(Key::ArrowUp),
            "other keys are not ours"
        );
    }

    #[test]
    fn a_shifted_arrow_key_event_extends_the_selection_unless_an_input_is_focused() {
        use crate::{InputEvent, Key};
        let mut tree = Tree::new();
        let id = text(&mut tree, "abcdef");
        tree.set_text_selection(id, 2, 2);
        let key = |tree: &mut Tree, shift| {
            tree.dispatch(
                id,
                InputEvent::KeyPressed {
                    key: Key::ArrowRight,
                    shift,
                },
                Instant::now(),
            )
        };
        key(&mut tree, false);
        assert_eq!(tree.static_selected_text(), None, "no shift, no extension");
        key(&mut tree, true);
        key(&mut tree, true);
        assert_eq!(tree.static_selected_text().as_deref(), Some("cd"));
    }

    #[test]
    fn a_link_is_found_by_offset_and_the_last_overlapping_one_wins() {
        let mut tree = Tree::new();
        let id = text(&mut tree, "Visit the docs now");
        let link = |start, end, href: &str| crate::TextSpan {
            start,
            end,
            link: Some(href.to_string()),
            ..Default::default()
        };
        if let NodeKind::Text(state) = &mut tree.get_mut(id).unwrap().kind {
            state.options.spans = vec![
                link(0, 14, "outer"),
                link(10, 14, "inner"),
                crate::TextSpan {
                    start: 5,
                    end: 8,
                    ..Default::default()
                },
            ];
        }
        assert_eq!(tree.text_link_at(id, 2), Some("outer"));
        assert_eq!(tree.text_link_at(id, 10), Some("inner"));
        assert_eq!(tree.text_link_at(id, 13), Some("inner"));
        assert_eq!(tree.text_link_at(id, 14), None, "the end is exclusive");
        assert_eq!(
            tree.text_link_at(id, 6),
            Some("outer"),
            "a styling span adds no link of its own"
        );
    }

    #[test]
    fn text_with_a_link_takes_hits_and_plain_text_does_not() {
        let mut tree = Tree::new();
        let root = tree.insert(
            NodeKind::Container,
            Style {
                size: Size {
                    width: length(100.0),
                    height: length(60.0),
                },
                flex_direction: taffy::FlexDirection::Column,
                ..Default::default()
            },
            PaintProperties::new(Color::from_rgba8(0, 0, 0, 255), 0.0, 1.0),
        );
        let (linked, plain) = (text(&mut tree, "link"), text(&mut tree, "plain"));
        tree.add_child(root, linked);
        tree.add_child(root, plain);
        if let NodeKind::Text(state) = &mut tree.get_mut(linked).unwrap().kind {
            state.options.spans = vec![crate::TextSpan {
                start: 0,
                end: 4,
                link: Some("x".into()),
                ..Default::default()
            }];
        }
        tree.compute_layout(
            root,
            Size {
                width: AvailableSpace::Definite(100.0),
                height: AvailableSpace::Definite(60.0),
            },
        );
        assert_eq!(tree.hit_test(root, Point::new(5.0, 5.0)), Some(linked));
        assert_eq!(
            tree.hit_test(root, Point::new(5.0, 25.0)),
            Some(root),
            "plain text is decoration"
        );
    }

    #[test]
    fn ctrl_arrows_move_the_selection_by_word() {
        use crate::Key;
        let mut tree = Tree::new();
        let id = text(&mut tree, "one  two, three");
        tree.set_text_selection(id, 0, 0);
        let word = |tree: &mut Tree, key| {
            tree.extend_static_selection_by(key, true);
            tree.static_selection_ends().unwrap().1.1
        };
        assert_eq!(
            word(&mut tree, Key::ArrowRight),
            5,
            "past 'one' and its spaces, to 'two'"
        );
        assert_eq!(
            word(&mut tree, Key::ArrowRight),
            8,
            "the comma is its own word"
        );
        assert_eq!(word(&mut tree, Key::ArrowRight), 10, "then 'three'");
        assert_eq!(word(&mut tree, Key::ArrowRight), 15, "to the end");
        assert_eq!(word(&mut tree, Key::ArrowLeft), 10);
        assert_eq!(word(&mut tree, Key::ArrowLeft), 8);
        assert_eq!(word(&mut tree, Key::ArrowLeft), 5);
        assert_eq!(word(&mut tree, Key::ArrowLeft), 0);
        assert_eq!(word(&mut tree, Key::ArrowLeft), 0, "stays at the start");
    }

    #[test]
    fn a_horizontal_move_at_the_end_of_a_text_goes_on_into_the_next() {
        use crate::Key;
        let mut tree = Tree::new();
        let (_, [a, b, c]) = paragraphs(&mut tree);
        tree.set_text_selection(a, 3, 3);
        tree.extend_static_selection(Key::End);
        tree.extend_static_selection(Key::ArrowRight);
        assert_eq!(tree.static_selection_ends().unwrap().1, (b, 0));
        tree.extend_static_selection(Key::ArrowRight);
        assert_eq!(tree.static_selected_text().as_deref(), Some("ha\nb"));
        tree.extend_static_selection_by(Key::ArrowRight, true);
        assert_eq!(range_of(&tree, b), Some((0, 5)));
        tree.extend_static_selection(Key::End);
        tree.extend_static_selection(Key::ArrowRight);
        assert_eq!(tree.static_selection_ends().unwrap().1, (c, 0));
        // And back, over the boundary.
        tree.extend_static_selection(Key::ArrowLeft);
        assert_eq!(tree.static_selection_ends().unwrap().1, (b, 5));
        // The first text's start has nowhere to go: it stays.
        tree.set_text_selection(a, 0, 0);
        assert!(tree.extend_static_selection(Key::ArrowLeft));
        assert_eq!(tree.static_selection_ends().unwrap().1, (a, 0));
    }

    #[test]
    fn with_no_line_to_move_to_the_vertical_edge_is_the_text_edge_then_the_next_text() {
        let mut tree = Tree::new();
        let (_, [a, b, _]) = paragraphs(&mut tree);
        tree.set_text_selection(a, 2, 2);
        assert_eq!(tree.static_selection_vertical_edge(false), Some((a, 0)));
        assert_eq!(tree.static_selection_vertical_edge(true), Some((a, 5)));
        tree.set_text_selection(a, 0, 5);
        assert_eq!(
            tree.static_selection_vertical_edge(true),
            Some((b, 5)),
            "already at the end"
        );
        tree.set_text_selection(b, 0, 0);
        assert_eq!(tree.static_selection_vertical_edge(false), Some((a, 0)));
        tree.clear_text_selection();
        assert_eq!(tree.static_selection_vertical_edge(true), None);
    }

    /// A selectable "Visit the docs now" with "docs" (10..14) a link.
    fn linked(tree: &mut Tree) -> NodeId {
        let id = text(tree, "Visit the docs now");
        if let NodeKind::Text(state) = &mut tree.get_mut(id).unwrap().kind {
            state.options.selectable = true;
            state.options.spans = vec![crate::TextSpan {
                start: 10,
                end: 14,
                link: Some("https://example.com/docs".into()),
                ..Default::default()
            }];
        }
        tree.compute_layout(
            id,
            Size {
                width: AvailableSpace::Definite(100.0),
                height: AvailableSpace::Definite(20.0),
            },
        );
        id
    }

    fn node_of(update: &accesskit::TreeUpdate, id: accesskit::NodeId) -> &accesskit::Node {
        &update
            .nodes
            .iter()
            .find(|(i, _)| *i == id)
            .expect("in the update")
            .1
    }

    #[test]
    fn plain_text_makes_no_text_runs_and_keeps_its_role() {
        let mut tree = Tree::new();
        let id = text(&mut tree, "plain");
        tree.compute_layout(
            id,
            Size {
                width: AvailableSpace::Definite(100.0),
                height: AvailableSpace::Definite(20.0),
            },
        );
        let update = tree.build_access_update(id);
        assert_eq!(update.nodes.len(), 1);
        assert_eq!(
            node_of(&update, to_access_id(id)).role(),
            accesskit::Role::Unknown
        );
    }

    #[test]
    fn linked_text_is_a_label_of_runs_and_a_link_with_its_url() {
        let mut tree = Tree::new();
        let id = linked(&mut tree);
        let update = tree.build_access_update(id);
        let container = node_of(&update, to_access_id(id));
        assert_eq!(container.role(), accesskit::Role::Label);
        let kids = container.children().to_vec();
        assert_eq!(kids.len(), 3, "before, the link, after");
        let roles: Vec<_> = kids.iter().map(|k| node_of(&update, *k).role()).collect();
        assert_eq!(
            roles,
            [
                accesskit::Role::TextRun,
                accesskit::Role::Link,
                accesskit::Role::TextRun
            ]
        );
        let link = node_of(&update, kids[1]);
        assert_eq!(link.url(), Some("https://example.com/docs"));
        assert!(link.supports_action(accesskit::Action::Click));
        assert_eq!(link.label(), Some("docs"));
        let inner = node_of(&update, link.children()[0]);
        assert_eq!(inner.value(), Some("docs"));
        assert_eq!(inner.character_lengths(), &[1u8, 1, 1, 1]);
        // The runs cover the content.
        let all: String = kids
            .iter()
            .map(|k| {
                let n = node_of(&update, *k);
                let run = if n.role() == accesskit::Role::Link {
                    node_of(&update, n.children()[0])
                } else {
                    n
                };
                run.value().unwrap().to_string()
            })
            .collect();
        assert_eq!(all, "Visit the docs now");
    }

    #[test]
    fn a_selection_is_reported_as_run_positions_and_multibyte_text_counts_characters() {
        let mut tree = Tree::new();
        let id = text(&mut tree, "héllo wörld");
        tree.set_text_selection(id, 0, 0);
        if let NodeKind::Text(state) = &mut tree.get_mut(id).unwrap().kind {
            state.options.selectable = true;
        }
        // "héllo" is 6 bytes, 5 characters; select through the space.
        tree.set_text_selection(id, 1, 7);
        tree.compute_layout(
            id,
            Size {
                width: AvailableSpace::Definite(100.0),
                height: AvailableSpace::Definite(20.0),
            },
        );
        let update = tree.build_access_update(id);
        let selection = *node_of(&update, to_access_id(id))
            .text_selection()
            .expect("a selection");
        assert_eq!(selection.anchor.character_index, 1);
        assert_eq!(
            selection.focus.character_index, 6,
            "byte 7 is the 7th character boundary after 'é'"
        );
        assert_eq!(selection.anchor.node, selection.focus.node);
        let run = node_of(&update, selection.anchor.node);
        assert_eq!(run.value(), Some("héllo wörld"));
        assert_eq!(run.character_lengths()[1], 2, "é is two bytes");
        assert!(
            node_of(&update, to_access_id(id)).supports_action(accesskit::Action::SetTextSelection)
        );
    }

    /// The tree as AccessKit's own consumer (what the platform adapters use)
    /// reads it: positions must resolve or it panics.
    #[test]
    fn the_consumer_reads_the_selected_text_back() {
        let mut tree = Tree::new();
        let id = linked(&mut tree);
        tree.set_text_selection(id, 6, 16);
        tree.compute_layout(
            id,
            Size {
                width: AvailableSpace::Definite(100.0),
                height: AvailableSpace::Definite(20.0),
            },
        );
        let consumer = accesskit_consumer::Tree::new(tree.build_access_update(id), true);
        let root = consumer.state().root();
        let range = root.text_selection().expect("a selection");
        // Across a link, from the plain run, through the link's, into the next.
        assert_eq!(range.text(), "the docs n");
        assert_eq!(root.document_range().text(), "Visit the docs now");
    }

    #[test]
    fn the_ids_of_runs_and_links_resolve_and_real_ids_do_not() {
        let mut tree = Tree::new();
        let id = linked(&mut tree);
        let update = tree.build_access_update(id);
        let kids = node_of(&update, to_access_id(id)).children().to_vec();
        assert_eq!(
            tree.resolve_text_part(kids[1]),
            Some(crate::TextPart::Link {
                owner: id,
                href: "https://example.com/docs".into(),
                start: 10,
                end: 14,
            })
        );
        assert_eq!(
            tree.resolve_text_part(kids[2]),
            Some(crate::TextPart::Run {
                owner: id,
                start: 14,
                end: 18
            })
        );
        assert_eq!(
            tree.resolve_text_part(to_access_id(id)),
            None,
            "a real node is not a part"
        );
        // Ids are unique across the update.
        let mut ids: Vec<_> = update.nodes.iter().map(|(i, _)| *i).collect();
        ids.sort();
        let n = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), n);
    }

    #[test]
    fn a_screen_readers_text_selection_selects_the_text() {
        let mut tree = Tree::new();
        let id = linked(&mut tree);
        let update = tree.build_access_update(id);
        let kids = node_of(&update, to_access_id(id)).children().to_vec();
        let link_run = node_of(&update, kids[1]).children()[0];
        let position = |node, character_index| accesskit::TextPosition {
            node,
            character_index,
        };
        // From the third character of "docs" to the first of " now".
        let selection = accesskit::TextSelection {
            anchor: position(link_run, 2),
            focus: position(kids[2], 1),
        };
        assert!(tree.set_text_selection_from_access(&selection));
        assert_eq!(tree.static_selected_text().as_deref(), Some("cs "));
        // A position in no text node refuses and changes nothing.
        let bogus = accesskit::TextSelection {
            anchor: position(to_access_id(id), 0),
            focus: position(kids[2], 1),
        };
        assert!(!tree.set_text_selection_from_access(&bogus));
        assert_eq!(tree.static_selected_text().as_deref(), Some("cs "));
    }

    /// A column holding paragraphs "alpha", "bravo", "charlie", all selectable.
    fn paragraphs(tree: &mut Tree) -> (NodeId, [NodeId; 3]) {
        let root = tree.insert(
            NodeKind::Container,
            Style {
                flex_direction: taffy::FlexDirection::Column,
                ..Default::default()
            },
            PaintProperties::new(Color::from_rgba8(0, 0, 0, 255), 0.0, 1.0),
        );
        let ids = ["alpha", "bravo", "charlie"].map(|content| {
            let id = text(tree, content);
            if let NodeKind::Text(state) = &mut tree.get_mut(id).unwrap().kind {
                state.options.selectable = true;
            }
            tree.add_child(root, id);
            id
        });
        (root, ids)
    }

    fn range_of(tree: &Tree, id: NodeId) -> Option<(usize, usize)> {
        match &tree.get(id).unwrap().kind {
            NodeKind::Text(state) => state.options.selection,
            _ => None,
        }
    }

    #[test]
    fn a_selection_runs_from_one_text_into_another_taking_what_lies_between() {
        let mut tree = Tree::new();
        let (_, [a, b, c]) = paragraphs(&mut tree);
        assert!(tree.select_across((a, 2), (c, 3)));
        assert_eq!(
            range_of(&tree, a),
            Some((2, 5)),
            "from the anchor to the end"
        );
        assert_eq!(range_of(&tree, b), Some((0, 5)), "the middle whole");
        assert_eq!(range_of(&tree, c), Some((0, 3)), "the start to the focus");
        assert_eq!(
            tree.static_selected_text().as_deref(),
            Some("pha\nbravo\ncha")
        );
    }

    #[test]
    fn a_backwards_selection_covers_the_same_text() {
        let mut tree = Tree::new();
        let (_, [a, _, c]) = paragraphs(&mut tree);
        assert!(tree.select_across((c, 3), (a, 2)));
        assert_eq!(
            tree.static_selected_text().as_deref(),
            Some("pha\nbravo\ncha")
        );
    }

    #[test]
    fn extending_a_drag_into_other_texts_and_back_selects_and_releases_them() {
        let mut tree = Tree::new();
        let (_, [a, b, c]) = paragraphs(&mut tree);
        tree.set_text_selection(a, 1, 1);
        tree.extend_text_selection(a, 3);
        assert_eq!(range_of(&tree, b), None);
        tree.extend_text_selection(c, 2);
        assert_eq!(range_of(&tree, b), Some((0, 5)));
        assert_eq!(range_of(&tree, c), Some((0, 2)));
        tree.extend_text_selection(b, 2);
        assert_eq!(range_of(&tree, c), None, "the third is let go");
        assert_eq!(range_of(&tree, b), Some((0, 2)));
        assert_eq!(tree.static_selected_text().as_deref(), Some("lpha\nbr"));
        tree.extend_text_selection(a, 3);
        assert_eq!(range_of(&tree, b), None);
        assert_eq!(tree.static_selected_text().as_deref(), Some("lp"));
    }

    #[test]
    fn texts_that_are_not_selectable_or_not_visible_are_skipped_between() {
        let mut tree = Tree::new();
        let (root, [a, b, c]) = paragraphs(&mut tree);
        if let NodeKind::Text(state) = &mut tree.get_mut(b).unwrap().kind {
            state.options.selectable = false;
        }
        let d = text(&mut tree, "delta");
        if let NodeKind::Text(state) = &mut tree.get_mut(d).unwrap().kind {
            state.options.selectable = true;
        }
        tree.add_child(root, d);
        tree.get_mut(c).unwrap().visible = false;
        assert!(tree.select_across((a, 0), (d, 5)));
        assert_eq!(range_of(&tree, b), None);
        assert_eq!(range_of(&tree, c), None);
        assert_eq!(tree.static_selected_text().as_deref(), Some("alpha\ndelta"));
    }

    #[test]
    fn a_new_selection_or_a_clear_releases_every_text_and_other_trees_are_refused() {
        let mut tree = Tree::new();
        let (_, [a, b, c]) = paragraphs(&mut tree);
        tree.select_across((a, 0), (c, 7));
        tree.set_text_selection(b, 1, 3);
        assert_eq!((range_of(&tree, a), range_of(&tree, c)), (None, None));
        assert_eq!(tree.static_selected_text().as_deref(), Some("ra"));
        tree.select_across((a, 0), (c, 7));
        tree.clear_text_selection();
        assert!([a, b, c].iter().all(|id| range_of(&tree, *id).is_none()));
        assert_eq!(tree.static_selected_text(), None);
        let (_, [other, ..]) = paragraphs(&mut tree);
        assert!(
            !tree.select_across((a, 0), (other, 1)),
            "two separate trees"
        );
    }

    #[test]
    fn a_screen_reader_can_select_from_one_text_to_another() {
        let mut tree = Tree::new();
        let (root, [a, _, c]) = paragraphs(&mut tree);
        tree.compute_layout(
            root,
            Size {
                width: AvailableSpace::Definite(100.0),
                height: AvailableSpace::Definite(60.0),
            },
        );
        let update = tree.build_access_update(root);
        let run_of = |id: NodeId| node_of(&update, to_access_id(id)).children()[0];
        let at = |node, character_index| accesskit::TextPosition {
            node,
            character_index,
        };
        let selection = accesskit::TextSelection {
            anchor: at(run_of(a), 3),
            focus: at(run_of(c), 2),
        };
        assert!(tree.set_text_selection_from_access(&selection));
        assert_eq!(
            tree.static_selected_text().as_deref(),
            Some("ha\nbravo\nch")
        );
    }

    #[test]
    fn removing_a_selected_text_releases_the_selection_and_its_lines() {
        let mut tree = Tree::new();
        let (_, [a, b, c]) = paragraphs(&mut tree);
        tree.select_across((a, 1), (c, 2));
        tree.remove(a);
        assert_eq!(tree.static_selected_text(), None, "the start node is gone");
        assert!(tree.static_selection_ends().is_none());
        assert_eq!((range_of(&tree, b), range_of(&tree, c)), (None, None));
        // Removing an unrelated node leaves a selection alone.
        tree.select_across((b, 0), (c, 2));
        let (_, [other, ..]) = paragraphs(&mut tree);
        tree.remove(other);
        assert_eq!(tree.static_selected_text().as_deref(), Some("bravo\nch"));
    }

    #[test]
    fn word_starts_past_255_characters_are_left_out_not_wrapped() {
        let mut tree = Tree::new();
        let words = "ab ".repeat(150);
        let id = text(&mut tree, &words);
        if let NodeKind::Text(state) = &mut tree.get_mut(id).unwrap().kind {
            state.options.selectable = true;
        }
        tree.compute_layout(
            id,
            Size {
                width: AvailableSpace::Definite(100.0),
                height: AvailableSpace::Definite(20.0),
            },
        );
        let update = tree.build_access_update(id);
        let run = update
            .nodes
            .iter()
            .find(|(_, n)| n.role() == accesskit::Role::TextRun)
            .unwrap();
        let starts = run.1.word_starts();
        assert!(!starts.is_empty());
        assert!(
            starts.windows(2).all(|w| w[0] < w[1]),
            "still sorted, nothing wrapped: {starts:?}"
        );
        assert_eq!(
            *starts.last().unwrap(),
            255,
            "the last word that fits in a u8"
        );
    }

    #[test]
    fn a_drag_within_one_text_touches_only_that_text_and_matches_a_fresh_selection() {
        let mut tree = Tree::new();
        let (_, [a, b, c]) = paragraphs(&mut tree);
        tree.set_text_selection(a, 1, 1);
        tree.extend_text_selection(c, 2);
        tree.take_touched();
        // Moving the end within the last text: only it changes.
        tree.extend_text_selection(c, 4);
        let touched = tree.take_touched();
        assert!(!touched.all, "no whole-tree touch");
        assert_eq!(touched.ids, vec![c], "only the text the end moved in");
        // The same position again changes nothing at all.
        tree.extend_text_selection(c, 4);
        let touched = tree.take_touched();
        assert!(!touched.all && touched.ids.is_empty(), "{:?}", touched.ids);
        assert_eq!(range_of(&tree, b), Some((0, 5)));

        // A long wandering drag across the three texts, forwards and back,
        // agrees at every step with selecting the same ends from scratch.
        let mut seed = 7u32;
        let mut next = move |n: usize| {
            seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
            ((seed >> 16) as usize) % n
        };
        let nodes = [a, b, c];
        let lens = [5usize, 5, 7];
        tree.set_text_selection(b, 2, 2);
        for _ in 0..200 {
            let k = next(3);
            let offset = next(lens[k] + 1);
            tree.extend_text_selection(nodes[k], offset);
            let mut fresh = Tree::new();
            let (_, f) = paragraphs(&mut fresh);
            fresh.select_across((f[1], 2), (f[k], offset));
            for i in 0..3 {
                assert_eq!(
                    range_of(&tree, nodes[i]),
                    range_of(&fresh, f[i]),
                    "step to ({k}, {offset}), text {i}"
                );
            }
            assert_eq!(tree.static_selected_text(), fresh.static_selected_text());
        }
    }

    #[test]
    fn a_non_text_node_is_refused_and_a_selection_set_directly_can_be_adopted() {
        let mut tree = Tree::new();
        let (kind, style, paint) = leaf(10.0, 10.0);
        let rect = tree.insert(kind, style, paint);
        assert!(!tree.set_text_selection(rect, 0, 1));
        let id = text(&mut tree, "abc");
        if let NodeKind::Text(state) = &mut tree.get_mut(id).unwrap().kind {
            state.options.selection = Some((0, 2));
        }
        assert_eq!(tree.static_selected_text(), None, "not the owner yet");
        tree.adopt_text_selection(id);
        assert_eq!(tree.static_selected_text().as_deref(), Some("ab"));
    }
}

/// 0.5.4 (#136): momentum scrolling.
mod momentum {
    use super::*;
    use crate::tree::scroll::fling_plan;
    use std::time::Duration;

    fn relayout(tree: &mut Tree, view: NodeId) {
        tree.compute_layout(
            view,
            Size {
                width: AvailableSpace::Definite(100.0),
                height: AvailableSpace::Definite(100.0),
            },
        );
    }

    #[test]
    fn a_plan_starts_at_the_flick_speed_and_ends_at_rest() {
        let (distance, duration, k) = fling_plan(1000.0, 0.0, 1e9).expect("a fast flick coasts");
        assert!(distance > 0.0);
        // Initial speed of distance * f(x) with f(x) = (1 - e^-kx)/(1 - e^-k):
        // f'(0) = k / (1 - e^-k), per second: / duration.
        let initial = distance * (k / (1.0 - (-k).exp())) / duration.as_secs_f64();
        assert!(
            (initial - 1000.0).abs() < 1.0,
            "starts at the flick speed: {initial}"
        );
        // Ends at the stop speed.
        let end = distance * (k * (-k).exp() / (1.0 - (-k).exp())) / duration.as_secs_f64();
        assert!((end - 30.0).abs() < 1.0, "ends at the stop speed: {end}");
        assert!(duration > Duration::from_millis(500) && duration < Duration::from_secs(3));
        // A faster flick goes further and lasts longer; direction is the sign.
        let (faster, longer, _) = fling_plan(3000.0, 0.0, 1e9).unwrap();
        assert!(faster > distance && longer > duration);
        let (back, ..) = fling_plan(-1000.0, 5000.0, 1e9).unwrap();
        assert_eq!(back, -distance);
    }

    #[test]
    fn a_slow_flick_or_one_with_no_room_does_not_coast() {
        assert!(
            fling_plan(100.0, 0.0, 1000.0).is_none(),
            "below the minimum"
        );
        assert!(fling_plan(f64::NAN, 0.0, 1000.0).is_none());
        assert!(
            fling_plan(1000.0, 500.0, 500.0).is_none(),
            "already at the end"
        );
        assert!(
            fling_plan(-1000.0, 0.0, 500.0).is_none(),
            "already at the start"
        );
    }

    #[test]
    fn a_coast_that_would_pass_the_end_stops_there_sooner() {
        let free = fling_plan(1000.0, 0.0, 1e9).unwrap();
        let (distance, duration, _) = fling_plan(1000.0, 0.0, 100.0).unwrap();
        assert!((distance - 100.0).abs() < 1e-9, "only the room there is");
        assert!(duration < free.1, "and in less time");
    }

    #[test]
    fn a_fling_animates_a_scroll_view_and_it_comes_to_rest_at_the_target() {
        let (mut tree, view, _) = view_over_boxes(&[100.0, 100.0, 100.0, 100.0]);
        let content = tree.get(view).unwrap().children[0];
        let t0 = Instant::now();
        // The finger moves up at 800 px/s: the content follows, toward the end.
        assert!(tree.fling_scroll(content, peniko::kurbo::Vec2::new(0.0, -800.0), t0));
        let offset = |tree: &Tree| match &tree.get(view).unwrap().kind {
            NodeKind::ScrollView(state) => state.scroll.current,
            _ => unreachable!(),
        };
        assert_eq!(offset(&tree), 0.0, "it has not moved yet");
        tree.tick_all(t0 + Duration::from_millis(100));
        let early = offset(&tree);
        assert!(early > 20.0, "fast at first: {early}");
        tree.tick_all(t0 + Duration::from_millis(300));
        let later = offset(&tree);
        assert!(later > early);
        let (still, _) = tree.tick_all(t0 + Duration::from_secs(10));
        assert!(!still, "it stops");
        let rest = offset(&tree);
        assert!(
            rest > later && rest <= 300.0,
            "at rest within the content: {rest}"
        );
    }

    #[test]
    fn a_fling_slows_down() {
        let (mut tree, view, _) = view_over_boxes(&[100.0, 100.0, 100.0, 100.0]);
        let content = tree.get(view).unwrap().children[0];
        let t0 = Instant::now();
        tree.fling_scroll(content, peniko::kurbo::Vec2::new(0.0, -1500.0), t0);
        let at = |tree: &mut Tree, ms| {
            tree.tick_all(t0 + Duration::from_millis(ms));
            match &tree.get(view).unwrap().kind {
                NodeKind::ScrollView(state) => state.scroll.current,
                _ => unreachable!(),
            }
        };
        let (a, b, c) = (at(&mut tree, 50), at(&mut tree, 100), at(&mut tree, 150));
        assert!(b - a > c - b, "each step covers less than the one before");
    }

    #[test]
    fn a_hand_on_the_content_stops_a_fling_where_it_is() {
        let (mut tree, view, _) = view_over_boxes(&[100.0, 100.0, 100.0, 100.0]);
        let content = tree.get(view).unwrap().children[0];
        let t0 = Instant::now();
        tree.fling_scroll(content, peniko::kurbo::Vec2::new(0.0, -1500.0), t0);
        tree.tick_all(t0 + Duration::from_millis(100));
        let NodeKind::ScrollView(state) = &tree.get(view).unwrap().kind else {
            unreachable!()
        };
        let caught = state.scroll.current;
        tree.stop_scroll_animation(content);
        let (active, _) = tree.tick_all(t0 + Duration::from_millis(400));
        assert!(!active);
        let NodeKind::ScrollView(state) = &tree.get(view).unwrap().kind else {
            unreachable!()
        };
        assert_eq!(
            state.scroll.current, caught,
            "it stayed where it was caught"
        );

        // A manual scroll also stops one in flight.
        tree.fling_scroll(
            content,
            peniko::kurbo::Vec2::new(0.0, -1500.0),
            t0 + Duration::from_secs(1),
        );
        tree.scroll_scroll_view_by(view, 5.0);
        assert!(!tree.tick_all(t0 + Duration::from_secs(5)).0);
    }

    #[test]
    fn nothing_scrolls_when_nothing_can_and_a_flick_toward_a_blocked_end_is_refused() {
        let (mut tree, view, boxes) = view_over_boxes(&[100.0]);
        let t0 = Instant::now();
        // At the start, a finger moving down (content toward the start) can't scroll.
        assert!(!tree.fling_scroll(boxes[0], peniko::kurbo::Vec2::new(0.0, 900.0), t0));
        // A node that is in no scroller.
        let (kind, style, paint) = leaf(10.0, 10.0);
        let lone = tree.insert(kind, style, paint);
        assert!(!tree.fling_scroll(lone, peniko::kurbo::Vec2::new(0.0, -900.0), t0));
        relayout(&mut tree, view);
    }
}

/// 0.5.4 (#139): sticky positioning.
mod sticky {
    use super::*;

    /// A 100 px tall vertical scroll view over four 100 px sections, each with
    /// a 20 px sticky header at its top and a body under it.
    fn sections() -> (Tree, NodeId, Vec<NodeId>) {
        let mut tree = Tree::new();
        let view = tree.insert(
            NodeKind::ScrollView(ScrollViewState::new(false)),
            Style {
                size: Size {
                    width: length(100.0),
                    height: length(100.0),
                },
                ..Default::default()
            },
            PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 1.0),
        );
        let content = tree.insert(
            NodeKind::Container,
            Style {
                flex_direction: FlexDirection::Column,
                size: Size {
                    width: length(100.0),
                    height: length(400.0),
                },
                ..Default::default()
            },
            PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 1.0),
        );
        tree.add_child(view, content);
        let mut headers = Vec::new();
        for _ in 0..4 {
            let section = tree.insert(
                NodeKind::Container,
                Style {
                    flex_direction: FlexDirection::Column,
                    flex_shrink: 0.0,
                    size: Size {
                        width: length(100.0),
                        height: length(100.0),
                    },
                    ..Default::default()
                },
                PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 1.0),
            );
            tree.add_child(content, section);
            let (k, mut s, p) = leaf(100.0, 20.0);
            s.flex_shrink = 0.0;
            let header = tree.insert(k, s, p);
            tree.add_child(section, header);
            tree.set_sticky(header, Some(0.0));
            headers.push(header);
            let (k, mut s, p) = leaf(100.0, 80.0);
            s.flex_shrink = 0.0;
            let body = tree.insert(k, s, p);
            tree.add_child(section, body);
        }
        tree.compute_layout(
            view,
            Size {
                width: AvailableSpace::Definite(100.0),
                height: AvailableSpace::Definite(100.0),
            },
        );
        (tree, view, headers)
    }

    fn scroll_to(tree: &mut Tree, view: NodeId, offset: f64) {
        tree.scroll_scroll_view_by(view, -1000.0);
        tree.scroll_scroll_view_by(view, offset);
    }

    /// Where `id` is on screen, vertically.
    fn y(tree: &Tree, id: NodeId) -> f64 {
        tree.absolute_position(id).1
    }

    #[test]
    fn a_header_holds_the_edge_while_the_next_has_not_arrived() {
        let (mut tree, view, headers) = sections();
        scroll_to(&mut tree, view, 0.0);
        assert_eq!((y(&tree, headers[0]), y(&tree, headers[1])), (0.0, 100.0));
        // Scrolled 60: the first header would be at -60; it sticks at 0. The
        // second is at 100 - 60 = 40, not there yet.
        scroll_to(&mut tree, view, 60.0);
        assert_eq!(y(&tree, headers[0]), 0.0);
        assert_eq!(y(&tree, headers[1]), 40.0);
    }

    #[test]
    fn a_stuck_header_leaves_with_its_section() {
        let (mut tree, view, headers) = sections();
        // At 90 the section ends at 10 on screen: its header (20 tall) can be
        // no lower than touching that, so it is at -10, pushed up; the next
        // header is at 10.
        scroll_to(&mut tree, view, 90.0);
        assert_eq!(y(&tree, headers[0]), -10.0);
        assert_eq!(y(&tree, headers[1]), 10.0);
        // At 150 the first is long gone and the second holds the edge.
        scroll_to(&mut tree, view, 150.0);
        assert_eq!(y(&tree, headers[0]), -70.0);
        assert_eq!(y(&tree, headers[1]), 0.0);
        // At the end, the last header is at the edge.
        scroll_to(&mut tree, view, 300.0);
        assert_eq!(y(&tree, headers[3]), 0.0);
    }

    #[test]
    fn an_inset_holds_it_that_far_from_the_edge() {
        let (mut tree, view, headers) = sections();
        tree.set_sticky(headers[0], Some(12.0));
        scroll_to(&mut tree, view, 40.0);
        assert_eq!(y(&tree, headers[0]), 12.0);
        // Even barely scrolled, the first header (natural 0) is already inside
        // the inset, so it is pushed down to hold it.
        scroll_to(&mut tree, view, 5.0);
        assert_eq!(y(&tree, headers[0]), 12.0);
        // The second (natural 100) only reaches the inset at a scroll of 88.
        scroll_to(&mut tree, view, 50.0);
        assert_eq!(y(&tree, headers[1]), 50.0);
        tree.set_sticky(headers[1], Some(12.0));
        scroll_to(&mut tree, view, 88.0);
        assert_eq!(y(&tree, headers[1]), 12.0);
    }

    #[test]
    fn a_pointer_over_the_stuck_header_hits_it() {
        let (mut tree, view, headers) = sections();
        scroll_to(&mut tree, view, 60.0);
        // As in CSS, content that paints later covers a stuck header unless the
        // header is above it: give it a z_index.
        let covered = tree.hit_test_local(view, Point::new(10.0, 5.0));
        assert_ne!(
            covered.map(|(id, _)| id),
            Some(headers[0]),
            "the body paints over it"
        );
        tree.get_mut(headers[0]).unwrap().z_index = 1;
        let hit = tree.hit_test_local(view, Point::new(10.0, 5.0));
        assert_eq!(hit.map(|(id, _)| id), Some(headers[0]));
        // Where the header was before it stuck is the body under the edge now.
        let below = tree.hit_test_local(view, Point::new(10.0, 30.0));
        assert_ne!(below.map(|(id, _)| id), Some(headers[0]));
    }

    #[test]
    fn making_it_ordinary_again_scrolls_it_with_the_content() {
        let (mut tree, view, headers) = sections();
        tree.set_sticky(headers[0], None);
        scroll_to(&mut tree, view, 60.0);
        assert_eq!(y(&tree, headers[0]), -60.0);
    }

    #[test]
    fn a_direct_child_of_the_scroller_or_a_node_outside_one_does_not_stick() {
        let (mut tree, view, _) = sections();
        let content = tree.get(view).unwrap().children[0];
        tree.set_sticky(content, Some(0.0));
        scroll_to(&mut tree, view, 60.0);
        assert_eq!(y(&tree, content), -60.0, "it IS the content");

        let (k, s, p) = leaf(10.0, 10.0);
        let lone = tree.insert(k, s, p);
        tree.set_sticky(lone, Some(0.0));
        assert_eq!(tree.scroll_shift(lone), (0.0, 0.0));
    }

    #[test]
    fn a_horizontal_scroller_sticks_along_x() {
        let mut tree = Tree::new();
        let view = tree.insert(
            NodeKind::ScrollView(ScrollViewState::new(true)),
            Style {
                size: Size {
                    width: length(100.0),
                    height: length(50.0),
                },
                ..Default::default()
            },
            PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 1.0),
        );
        let (k, mut s, p) = leaf(400.0, 50.0);
        s.flex_direction = FlexDirection::Row;
        let strip = tree.insert(k, s, p);
        tree.add_child(view, strip);
        let (k, s, p) = leaf(30.0, 50.0);
        let label = tree.insert(k, s, p);
        tree.add_child(strip, label);
        tree.set_sticky(label, Some(0.0));
        tree.compute_layout(
            view,
            Size {
                width: AvailableSpace::Definite(100.0),
                height: AvailableSpace::Definite(50.0),
            },
        );
        tree.scroll_scroll_view_by(view, 120.0);
        // Natural x 0; scrolled 120 it would be at -120; it sticks at 0 -- but is
        // bounded by its parent (the 400 px strip): 400 - 30 = 370 room.
        assert_eq!(tree.absolute_position(label).0, 0.0);
    }
}

/// 0.5.4 (#150): `has_images`/`has_svgs` let a per-frame consumer skip a scan,
/// so they must follow insertions and removals, including a removed subtree's.
#[test]
fn image_and_svg_counters_follow_insert_and_remove() {
    let paint = || PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 1.0);
    let mut tree = Tree::new();
    assert!(!tree.has_images() && !tree.has_svgs());
    let parent = tree.insert(NodeKind::Container, Style::default(), paint());
    let image = tree.insert(
        NodeKind::Image(ImageState::blank()),
        Style::default(),
        paint(),
    );
    let svg = tree.insert(
        NodeKind::Svg(crate::SvgState::empty()),
        Style::default(),
        paint(),
    );
    tree.add_child(parent, image);
    tree.add_child(parent, svg);
    assert!(tree.has_images() && tree.has_svgs());
    // Removing the parent removes both, counted once each.
    tree.remove(parent);
    assert!(!tree.has_images() && !tree.has_svgs());
    assert_eq!((tree.image_count, tree.svg_count), (0, 0));
}
