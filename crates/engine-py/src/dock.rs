//! M4 Phase 9 (§11.4), reduced to D10's bare bones in M99: docking's
//! mechanism, with its presentation left to the framework. A zone is an
//! ordinary node the framework builds and registers (`add_dock_zone`);
//! `DockLayout` is pure bookkeeping `Tree` never stores; `tre` docks
//! panels into zones, shows each zone's active panel, and moves a dragged
//! panel to the zone it's dropped on. What a drag looks like -- the
//! handle, the highlight over a target zone -- is the framework's: it
//! starts a drag with `start_panel_drag` from its own handle's
//! `pointer_down`, and draws its highlight from the `dock_target` window
//! event; `dock_drop` reports where the drag ended.
//!
//! Drags run inside `dispatch::process_input`, so a real pointer and
//! `window.simulate` drive them identically.
//!
//! `dock_panel` and a drag's drop move a panel through one path,
//! `move_panel`, which keeps every zone's panel list in step with the
//! tree: a panel is listed in exactly one zone (issue #14).

use std::cell::RefCell;
use std::rc::Rc;

use engine_core::{DockLayout, DockSide, DockZone, NodeId, Tree};
use peniko::kurbo::Point;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

/// `containers` is a `Vec`, not a map keyed by `DockSide` -- `DockSide`
/// doesn't derive `Hash`, and at most five entries scan faster anyway.
pub(crate) struct DockState {
    pub(crate) layout: DockLayout,
    containers: Vec<(DockSide, NodeId)>,
    /// The panel being dragged, and the zone last reported under the
    /// pointer (`dock_target`).
    dragging: Option<(NodeId, Option<DockSide>)>,
}

impl DockState {
    pub(crate) fn new() -> Self {
        Self {
            layout: DockLayout::new(),
            containers: Vec::new(),
            dragging: None,
        }
    }
}

pub(crate) type SharedDockState = Rc<RefCell<DockState>>;

const SIDES: [(&str, DockSide); 5] = [
    ("left", DockSide::Left),
    ("right", DockSide::Right),
    ("top", DockSide::Top),
    ("bottom", DockSide::Bottom),
    ("center", DockSide::Center),
];

pub(crate) fn parse_dock_side(side: &str) -> PyResult<DockSide> {
    SIDES
        .iter()
        .find(|(name, _)| *name == side)
        .map(|&(_, side)| side)
        .ok_or_else(|| {
            PyValueError::new_err(format!(
                "unknown dock side {side:?} -- expected one of \"left\", \"right\", \"top\", \
                 \"bottom\", \"center\""
            ))
        })
}

/// The name `parse_dock_side` takes, for the docking events.
pub(crate) fn side_name(side: DockSide) -> &'static str {
    SIDES
        .iter()
        .find(|&&(_, s)| s == side)
        .map(|&(name, _)| name)
        .expect("SIDES names every DockSide")
}

pub(crate) fn add_dock_zone(dock: &SharedDockState, side: DockSide, container: NodeId, size: f64) {
    let mut dock = dock.borrow_mut();
    dock.layout.set_zone(side, DockZone::new(size));
    dock.containers.push((side, container));
}

/// Docks `panel` into `side`'s zone and shows it. A panel already docked
/// in another zone is moved -- taken out of that zone's panels, whose
/// shown panel is re-picked -- exactly as a drag's drop moves it (issue
/// #14: it used to stay listed in its old zone).
pub(crate) fn dock_panel(
    dock: &SharedDockState,
    tree: &Rc<RefCell<Tree>>,
    side: DockSide,
    panel: NodeId,
) -> PyResult<()> {
    let container = container_for(dock, side)?;
    if dock.borrow().layout.zone(side).is_none() {
        return Err(PyValueError::new_err(format!(
            "no dock zone registered for {side:?}"
        )));
    }
    let source = zone_holding(&dock.borrow(), panel);
    if source == Some(side) {
        let mut dock = dock.borrow_mut();
        let zone = dock.layout.zone_mut(side).expect("checked above");
        zone.active_tab = zone.panels.iter().position(|&p| p == panel).unwrap_or(0);
        tree.borrow_mut().apply_active_tab(container, zone);
        return Ok(());
    }
    if source.is_none() {
        // Not docked: take it from wherever the framework attached it.
        let mut tree = tree.borrow_mut();
        if let Some(old_parent) = tree.get(panel).and_then(|n| n.parent) {
            tree.detach(old_parent, panel);
        }
    }
    move_panel(dock, tree, panel, side);
    Ok(())
}

pub(crate) fn set_active_panel(
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
            "set_active_panel: index {index} out of range for {:?}'s {} panel(s)",
            side_name(side),
            zone.panels.len()
        )));
    }
    zone.active_tab = index;
    tree.borrow_mut().apply_active_tab(container, zone);
    Ok(())
}

/// Starts dragging `panel`, which must be docked in a zone.
pub(crate) fn start_drag(dock: &SharedDockState, panel: NodeId) -> PyResult<()> {
    let mut dock = dock.borrow_mut();
    if zone_holding(&dock, panel).is_none() {
        return Err(PyValueError::new_err(
            "start_panel_drag: that node isn't a docked panel -- dock it with dock_panel first",
        ));
    }
    dock.dragging = Some((panel, None));
    Ok(())
}

/// The pointer moved during a drag: the zone now under it, when that
/// differs from the one last reported -- what `dock_target` delivers.
/// `None` when no drag is in progress or the zone didn't change.
pub(crate) fn drag_to(
    dock: &SharedDockState,
    tree: &Rc<RefCell<Tree>>,
    root: NodeId,
    position: Point,
) -> Option<Option<DockSide>> {
    let (_, last) = dock.borrow().dragging?;
    let side = zone_at(dock, tree, root, position);
    if side == last {
        return None;
    }
    if let Some((_, target)) = dock.borrow_mut().dragging.as_mut() {
        *target = side;
    }
    Some(side)
}

/// The drag ended at `position`: moves the panel into the zone there, if
/// any, and returns the panel and that zone -- what `dock_drop` delivers.
/// `None` when no drag was in progress.
pub(crate) fn drop(
    dock: &SharedDockState,
    tree: &Rc<RefCell<Tree>>,
    root: NodeId,
    position: Point,
) -> Option<(NodeId, Option<DockSide>)> {
    let (panel, _) = dock.borrow_mut().dragging.take()?;
    let target = zone_at(dock, tree, root, position);
    if let Some(target) = target {
        move_panel(dock, tree, panel, target);
    }
    Some((panel, target))
}

fn zone_at(
    dock: &SharedDockState,
    tree: &Rc<RefCell<Tree>>,
    root: NodeId,
    position: Point,
) -> Option<DockSide> {
    let tree = tree.borrow();
    let hit = tree.hit_test(root, position)?;
    enclosing_zone(&tree, &dock.borrow(), hit)
}

fn move_panel(dock: &SharedDockState, tree: &Rc<RefCell<Tree>>, panel: NodeId, target: DockSide) {
    let mut dock = dock.borrow_mut();
    let source = zone_holding(&dock, panel);
    if source == Some(target) {
        return;
    }
    let mut tree = tree.borrow_mut();
    if let Some(source) = source {
        let source_container = container_of(&dock, source);
        if let Some(zone) = dock.layout.zone_mut(source) {
            zone.panels.retain(|p| *p != panel);
            if zone.active_tab >= zone.panels.len() {
                zone.active_tab = zone.panels.len().saturating_sub(1);
            }
        }
        if let Some(container) = source_container {
            if tree
                .get(container)
                .is_some_and(|c| c.children.contains(&panel))
            {
                tree.detach(container, panel);
            }
            if let Some(zone) = dock.layout.zone(source)
                && !zone.panels.is_empty()
            {
                tree.apply_active_tab(container, zone);
            }
        }
    }
    let target_container = container_of(&dock, target);
    if let (Some(container), Some(zone)) = (target_container, dock.layout.zone_mut(target)) {
        if !zone.panels.contains(&panel) {
            zone.panels.push(panel);
        }
        zone.active_tab = zone.panels.iter().position(|&p| p == panel).unwrap_or(0);
        tree.apply_active_tab(container, zone);
    }
}

fn zone_holding(dock: &DockState, panel: NodeId) -> Option<DockSide> {
    dock.containers.iter().find_map(|&(side, _)| {
        dock.layout
            .zone(side)
            .filter(|zone| zone.panels.contains(&panel))
            .map(|_| side)
    })
}

fn container_of(dock: &DockState, side: DockSide) -> Option<NodeId> {
    dock.containers
        .iter()
        .find(|&&(s, _)| s == side)
        .map(|&(_, c)| c)
}

fn container_for(dock: &SharedDockState, side: DockSide) -> PyResult<NodeId> {
    container_of(&dock.borrow(), side)
        .ok_or_else(|| PyValueError::new_err(format!("no dock zone registered for {side:?}")))
}

fn enclosing_zone(tree: &Tree, dock: &DockState, mut node: NodeId) -> Option<DockSide> {
    loop {
        if let Some(&(side, _)) = dock.containers.iter().find(|&&(_, c)| c == node) {
            return Some(side);
        }
        node = tree.get(node)?.parent?;
    }
}
