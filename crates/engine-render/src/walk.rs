//! 0.4.0: the one traversal of a laid-out tree, as painting sees it.
//!
//! Painting (`build_scene`) and change tracking (`DamageTracker`) must
//! agree on which nodes are drawn and where -- a node the tracker skips
//! but paint draws is a node whose changes never reach the screen. So
//! the rules live here once, and each walk is a `Visitor`:
//!
//! - a hidden node (M96) is skipped, subtree included;
//! - a node's transform is its parent's, then its layout position, then
//!   its own transform parts (`composed_transform`);
//! - a node whose box, under that transform, misses the visible rect is
//!   skipped with its subtree (M8 Phase 1, §11.8) -- a whole-subtree
//!   skip, not a per-pixel clip, and nothing is encoded for it;
//! - a fully transparent node (M95 group opacity) is skipped with its
//!   subtree;
//! - a node that clips its children (`clips_children`: scroll views,
//!   virtual lists, `clip_children`) narrows the visible rect to its box
//!   for them;
//! - children are visited in paint order.

use engine_core::{Node, NodeId, Tree};
use peniko::kurbo::{Affine, Rect};

use crate::{clips_children, composed_transform, transformed_bounds};

/// One node as the walk reaches it.
pub(crate) struct Visit<'t> {
    pub id: NodeId,
    pub node: &'t Node,
    /// The node's full transform, window space.
    pub composed: Affine,
    /// The node's layout size.
    pub w: f64,
    pub h: f64,
    /// The node's box under `composed`, window space.
    pub bounds: Rect,
    /// What its ancestors leave visible, window space.
    pub visible: Rect,
    /// Its opacity times its ancestors' -- what it composites at.
    pub opacity: f64,
    pub parent: Option<NodeId>,
    /// Its index among its parent's children, in paint order.
    pub order: usize,
    /// The window's own scale, the grid layout offsets snap to (0.5.4, review).
    pub grid: f64,
}

/// One walk's own work at each node.
pub(crate) trait Visitor<'t> {
    /// A node the rules reach. Returning `false` skips its subtree, and
    /// its `leave`.
    fn enter(&mut self, visit: &Visit<'t>) -> bool;

    /// After the children of a node `enter` accepted.
    fn leave(&mut self, _visit: &Visit<'t>) {}

    /// Whether to pass over `child` and its subtree without reaching it at
    /// all (0.5.4, #149): asked before the walk works out where it is, so a
    /// walk that only wants the nodes near some damage does not pay for the
    /// ones far from it. Whatever is skipped is as if `enter` had said no.
    fn skip(&mut self, _child: NodeId) -> bool {
        false
    }
}

/// Walks `root`'s tree within `visible`, calling `visitor` at each node.
/// `base` is the transform every node starts under: the window's display
/// scale (0.5.4, #102), so layout stays in logical pixels while `composed`,
/// `bounds` and `visible` are in physical ones.
pub(crate) fn walk<'t>(
    tree: &'t Tree,
    root: NodeId,
    base: Affine,
    visible: Rect,
    visitor: &mut impl Visitor<'t>,
) {
    visit(
        tree,
        root,
        base,
        visible,
        1.0,
        None,
        0,
        base.as_coeffs()[0],
        visitor,
    );
}

/// 0.5.1 (#69): walks the subtree under `id` as if it were the whole tree,
/// for an effect's offscreen render: `id` is placed at the origin with no
/// transform of its own and no opacity of its own (the main scene applies
/// both to the effect's result), and its box is the visible area.
pub(crate) fn walk_root<'t>(
    tree: &'t Tree,
    id: NodeId,
    base: Affine,
    visitor: &mut impl Visitor<'t>,
) {
    let Some(node) = tree.get(id) else { return };
    let layout = tree.layout(id);
    let (w, h) = (f64::from(layout.size.width), f64::from(layout.size.height));
    let bounds = transformed_bounds(base, Rect::new(0.0, 0.0, w, h));
    let here = Visit {
        id,
        node,
        composed: base,
        w,
        h,
        bounds,
        visible: bounds,
        opacity: 1.0,
        parent: None,
        order: 0,
        grid: base.as_coeffs()[0],
    };
    descend(tree, &here, visitor);
}

/// A node the rules accepted: the visitor's `enter`, then the children in
/// paint order, then `leave`.
fn descend<'t>(tree: &'t Tree, here: &Visit<'t>, visitor: &mut impl Visitor<'t>) {
    if !visitor.enter(here) {
        return;
    }
    let child_visible = child_visible(here);
    for (index, &child) in tree.children_in_paint_order(here.id).iter().enumerate() {
        if visitor.skip(child) {
            continue;
        }
        visit(
            tree,
            child,
            here.composed,
            child_visible,
            here.opacity,
            Some(here.id),
            index,
            here.grid,
            visitor,
        );
    }
    visitor.leave(here);
}

#[allow(clippy::too_many_arguments)]
fn visit<'t>(
    tree: &'t Tree,
    id: NodeId,
    parent_transform: Affine,
    visible: Rect,
    parent_opacity: f64,
    parent: Option<NodeId>,
    order: usize,
    grid: f64,
    visitor: &mut impl Visitor<'t>,
) {
    let Some(here) = resolve(
        tree,
        id,
        parent_transform,
        visible,
        parent_opacity,
        parent,
        order,
        grid,
    ) else {
        return;
    };
    descend(tree, &here, visitor);
}

/// The rules of the walk applied to one node: where it is under its
/// parent's transform, and `None` when they skip it (hidden, off the visible
/// rect, fully transparent). 0.5.4 (#125): split out so the damage tracker
/// can reach a node without walking down to it from the root.
#[allow(clippy::too_many_arguments)]
pub(crate) fn resolve<'t>(
    tree: &'t Tree,
    id: NodeId,
    parent_transform: Affine,
    visible: Rect,
    parent_opacity: f64,
    parent: Option<NodeId>,
    order: usize,
    grid: f64,
) -> Option<Visit<'t>> {
    let node = tree.get(id)?;
    if !node.visible {
        return None;
    }
    let layout = tree.layout(id);
    let (w, h) = (f64::from(layout.size.width), f64::from(layout.size.height));
    let (sx, sy) = tree.scroll_shift(id);
    let position = (
        f64::from(layout.location.x) + sx,
        f64::from(layout.location.y) + sy,
    );
    let composed = composed_transform(parent_transform, position, node, w, h, grid);
    let bounds = transformed_bounds(composed, Rect::new(0.0, 0.0, w, h));
    if !bounds.overlaps(visible) {
        return None;
    }
    let opacity = node.paint.opacity.current;
    if opacity <= 0.0 {
        return None;
    }
    Some(Visit {
        id,
        node,
        composed,
        w,
        h,
        bounds,
        visible,
        opacity: parent_opacity * opacity,
        parent,
        order,
        grid,
    })
}

/// What a node leaves visible to its children: its own visible rect, narrowed
/// to its box when it clips them.
pub(crate) fn child_visible(here: &Visit<'_>) -> Rect {
    if clips_children(here.node) {
        here.visible.intersect(here.bounds)
    } else {
        here.visible
    }
}
