//! M4 Phase 9 (§11.4): the real Python-facing docking API, and the
//! drag-to-rearrange orchestration §11.4's own text describes ("reuses
//! ... the same pointer-event dispatch already wired for everything
//! else -- not a new input-handling path").
//!
//! `DockLayout`/`DockZone`/`Tree::apply_active_tab` are real since M3
//! step 15 Stage B, but had zero Python-facing wrapper anywhere
//! (confirmed via grep before this phase) -- a "zone" is just an
//! ordinary node the app builds itself, and `DockLayout` is pure
//! external bookkeeping `Tree` never stores, so which nodes are
//! "handles"/"panels"/"zones" are exactly the meaning-dependent facts
//! §2 Design Principle 6 says `engine-core` has no business knowing.
//! This whole module lives in `engine-py` on purpose; it needed zero
//! new `engine-core` code -- `Tree::hit_test`, `Tree::apply_active_tab`,
//! and the already-`pub` `Node::parent` field are exactly what's
//! needed.
//!
//! **Real bug found and fixed before writing the happy path:**
//! `Tree::apply_active_tab` only checks whether its *target* container
//! already lists the active panel as a child -- it never checks
//! whether the panel is still attached to a *different* parent first.
//! Calling it naively while moving a panel between zones would
//! `add_child` a still-attached node, corrupting the tree (the same
//! "`add_child` has no dedup" class of bug M4 Phase 7 already found
//! once for `open_overlay`). `end_drag_at` below explicitly `Tree::
//! detach`es the panel from its *old* zone's container first, guarded
//! by the same containment check `apply_active_tab` itself already
//! uses.
//!
//! **Deliberately no drop-zone highlight overlay.** `open_overlay`'s
//! own `inset` computation is hardcoded to place content *below* its
//! anchor (a dropdown-menu placement, confirmed by reading the actual
//! computation) -- it can't cover a target zone's own bounds, which is
//! what a highlight needs. Real, additive `engine-core` work to add
//! that placement mode is out of proportion to this phase's actual
//! core claim ("a real drag provably moves a panel between zones");
//! named here as a real, separate, stated gap, not manufactured or
//! silently dropped. One consequence: nothing needs to track *which*
//! zone is under the pointer mid-drag -- only the drop point, once, at
//! release -- so there's no `PointerMoved`-time callback for docking
//! at all, just press (start) and release (end).

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use engine_core::{DockLayout, DockSide, DockZone, NodeId, Tree};
use peniko::kurbo::Point;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

/// `containers`/`handles` are plain data (no `Py<PyAny>`), the same
/// real reason `Node`'s own `context_menus` field (M4 Phase 7) needs no
/// GC-traversal obligation. A `Vec` for `containers`, not a `HashMap`
/// keyed by `DockSide` -- `DockSide` doesn't derive `Hash` (checked
/// directly), and linear-scanning at most 5 real entries is cheaper
/// than widening `engine-core`'s own struct for a lookup this small.
pub(crate) struct DockState {
    pub(crate) layout: DockLayout,
    containers: Vec<(DockSide, NodeId)>,
    handles: HashMap<NodeId, NodeId>,
    dragging: Option<NodeId>,
}

impl DockState {
    pub(crate) fn new() -> Self {
        Self {
            layout: DockLayout::new(),
            containers: Vec::new(),
            handles: HashMap::new(),
            dragging: None,
        }
    }
}

pub(crate) type SharedDockState = Rc<RefCell<DockState>>;

/// `press_key`'s own string-vocabulary pattern (M4 Phase 2), applied to
/// `DockSide`'s five real variants.
pub(crate) fn parse_dock_side(side: &str) -> PyResult<DockSide> {
    match side {
        "left" => Ok(DockSide::Left),
        "right" => Ok(DockSide::Right),
        "top" => Ok(DockSide::Top),
        "bottom" => Ok(DockSide::Bottom),
        "center" => Ok(DockSide::Center),
        other => Err(PyValueError::new_err(format!(
            "unknown dock side {other:?} -- expected one of \"left\", \"right\", \"top\", \
             \"bottom\", \"center\""
        ))),
    }
}

pub(crate) fn add_dock_zone(dock: &SharedDockState, side: DockSide, container: NodeId, size: f64) {
    let mut dock = dock.borrow_mut();
    dock.layout.set_zone(side, DockZone::new(size));
    dock.containers.push((side, container));
}

/// Real initial "put a panel in a zone" setup -- also the exact step
/// `end_drag_at` reuses for the "attach into the new zone" half of a
/// real drag, so there is one mechanism for "a panel becomes this
/// zone's active tab," not two.
pub(crate) fn dock_panel(
    dock: &SharedDockState,
    tree: &Rc<RefCell<Tree>>,
    side: DockSide,
    panel: NodeId,
) -> PyResult<()> {
    let container = container_for(dock, side)?;
    // The real fix this module's own doc comment names for `end_drag_
    // at`, needed here too: every node-creation method (`add_rect`,
    // etc.) attaches its new node to the window's own root immediately,
    // so a freshly built panel already has a parent -- detach it first,
    // guarded by a real containment check, or `apply_active_tab`'s own
    // `add_child` below would attach an already-attached node, leaving
    // a stale reference in its old parent's `children` (confirmed for
    // real: this was caught by `examples/docking.py`'s own live
    // `accesskit` validation panicking on a duplicate child, not by
    // any pytest test, since none of them render a real frame).
    {
        let mut tree_mut = tree.borrow_mut();
        if let Some(old_parent) = tree_mut.get(panel).and_then(|n| n.parent) {
            tree_mut.detach(old_parent, panel);
        }
    }
    let mut dock = dock.borrow_mut();
    let zone = dock
        .layout
        .zone_mut(side)
        .ok_or_else(|| PyValueError::new_err(format!("no dock zone registered for {side:?}")))?;
    if !zone.panels.contains(&panel) {
        zone.panels.push(panel);
    }
    zone.active_tab = zone.panels.iter().position(|&p| p == panel).unwrap_or(0);
    let zone_ref = &*zone;
    tree.borrow_mut().apply_active_tab(container, zone_ref);
    Ok(())
}

pub(crate) fn set_active_tab(
    dock: &SharedDockState,
    tree: &Rc<RefCell<Tree>>,
    side: DockSide,
    index: usize,
) -> PyResult<()> {
    let container = container_for(dock, side)?;
    let mut dock = dock.borrow_mut();
    let zone = dock
        .layout
        .zone_mut(side)
        .ok_or_else(|| PyValueError::new_err(format!("no dock zone registered for {side:?}")))?;
    if index >= zone.panels.len() {
        return Err(PyValueError::new_err(format!(
            "set_active_tab: index {index} out of range for {side:?}'s {} panel(s)",
            zone.panels.len()
        )));
    }
    zone.active_tab = index;
    let zone_ref = &*zone;
    tree.borrow_mut().apply_active_tab(container, zone_ref);
    Ok(())
}

pub(crate) fn set_dock_handle(dock: &SharedDockState, handle: NodeId, panel: NodeId) {
    dock.borrow_mut().handles.insert(handle, panel);
}

/// Starts tracking a real drag if `node` is a registered handle.
/// Returns whether a drag actually started -- a press on any other
/// node is a real, safe no-op, matching every other "press this,
/// nothing happens if it isn't registered" precedent in this codebase
/// (`set_on_click`'s own no-handler case, etc.).
pub(crate) fn start_drag(dock: &SharedDockState, node: NodeId) -> bool {
    let mut dock = dock.borrow_mut();
    if let Some(&panel) = dock.handles.get(&node) {
        dock.dragging = Some(panel);
        true
    } else {
        false
    }
}

/// Finds which registered zone currently holds `panel`, if any.
fn zone_holding(dock: &DockState, panel: NodeId) -> Option<DockSide> {
    dock.containers.iter().find_map(|&(side, _)| {
        dock.layout
            .zone(side)
            .filter(|zone| zone.panels.contains(&panel))
            .map(|_| side)
    })
}

fn container_for(dock: &SharedDockState, side: DockSide) -> PyResult<NodeId> {
    dock.borrow()
        .containers
        .iter()
        .find(|&&(s, _)| s == side)
        .map(|&(_, container)| container)
        .ok_or_else(|| PyValueError::new_err(format!("no dock zone registered for {side:?}")))
}

/// Walks real `Node::parent` links from `node` upward, returning the
/// first registered zone whose container is `node` itself or one of
/// its ancestors -- necessary because a zone's own active panel always
/// covers its container's full bounds, so `Tree::hit_test`'s reverse-
/// paint-order walk always finds the panel first, never the container
/// itself (confirmed by reading `hit_test`'s own recursive shape).
fn enclosing_zone(tree: &Tree, dock: &DockState, mut node: NodeId) -> Option<DockSide> {
    loop {
        if let Some(&(side, _)) = dock.containers.iter().find(|&&(_, c)| c == node) {
            return Some(side);
        }
        node = tree.get(node)?.parent?;
    }
}

/// The real "release" half of a drag: hit-tests `position`, finds the
/// enclosing registered zone (if any), and -- only if it's a real
/// target different from the panel's current zone -- reparents it for
/// real. Always clears `dragging`, whether or not anything actually
/// moved (a drop outside any zone, or back into the same zone, is a
/// real, safe cancel, not an error).
pub(crate) fn end_drag_at(
    dock: &SharedDockState,
    tree: &Rc<RefCell<Tree>>,
    root: NodeId,
    position: Point,
) {
    let panel = {
        let mut dock_mut = dock.borrow_mut();
        match dock_mut.dragging.take() {
            Some(panel) => panel,
            None => return,
        }
    };

    let hit = tree.borrow().hit_test(root, position);
    let target_side = match hit {
        Some(node) => {
            let dock_ref = dock.borrow();
            enclosing_zone(&tree.borrow(), &dock_ref, node)
        }
        None => None,
    };

    let Some(target_side) = target_side else {
        return;
    };

    let mut dock_mut = dock.borrow_mut();
    let source_side = zone_holding(&dock_mut, panel);
    if source_side == Some(target_side) {
        return;
    }

    let mut tree_mut = tree.borrow_mut();

    if let Some(source_side) = source_side {
        let source_container = dock_mut
            .containers
            .iter()
            .find(|&&(s, _)| s == source_side)
            .map(|&(_, c)| c);
        if let Some(zone) = dock_mut.layout.zone_mut(source_side) {
            zone.panels.retain(|p| *p != panel);
            if zone.active_tab >= zone.panels.len() {
                zone.active_tab = zone.panels.len().saturating_sub(1);
            }
        }
        if let Some(source_container) = source_container {
            // The real fix this module's own doc comment names: detach
            // first, guarded by the same containment check `apply_
            // active_tab` itself uses, before the target zone ever
            // touches this panel -- otherwise `add_child` below would
            // attach an already-attached node.
            if tree_mut
                .get(source_container)
                .is_some_and(|c| c.children.contains(&panel))
            {
                tree_mut.detach(source_container, panel);
            }
            if let Some(zone) = dock_mut.layout.zone(source_side)
                && !zone.panels.is_empty()
            {
                tree_mut.apply_active_tab(source_container, zone);
            }
        }
    }

    let target_container = dock_mut
        .containers
        .iter()
        .find(|&&(s, _)| s == target_side)
        .map(|&(_, c)| c);
    if let (Some(target_container), Some(zone)) =
        (target_container, dock_mut.layout.zone_mut(target_side))
    {
        if !zone.panels.contains(&panel) {
            zone.panels.push(panel);
        }
        zone.active_tab = zone.panels.iter().position(|&p| p == panel).unwrap_or(0);
        let zone_ref = &*zone;
        tree_mut.apply_active_tab(target_container, zone_ref);
    }
}
