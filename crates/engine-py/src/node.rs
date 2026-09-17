//! `Node` (§8's `PyNode`, renamed to match what Python actually sees --
//! `tre.Node`, not `tre.PyNode`) -- narrower than §8's own full sketch:
//! `animate()`/`get()` plus, as of M4 Phase 1 step 3, `set_on_click`, and
//! as of M4 Phase 6, `set_on_hover_enter`/`set_on_hover_exit`.
//! Still no `add_child` (no cycle to reject, so `EngineError::
//! CycleRejected` isn't implemented yet either) -- additive when its own
//! later build-order step needs it.
//!
//! Handler callback storage (`handlers`) is an `Rc<RefCell<HashMap<
//! (NodeId, EventKind), Py<PyAny>>>>` *shared* with the owning `PyWindow`
//! (or `View`) -- created once there, cloned into every `Node` handed
//! out, the exact same sharing shape `tree: Rc<RefCell<Tree>>` already
//! uses. This is what actually resolves §8's own review note (a Python
//! callback stored in a Rust struct is a real GC-cycle risk unless the
//! owning `#[pyclass]` implements `__traverse__`/`__clear__`) without
//! needing `Node` to hold a back-reference to its own owner: `PyWindow`/
//! `View`'s own `__traverse__` already visits everything in this same
//! shared map. Keyed by `(NodeId, EventKind)` rather than one map per
//! event kind since M4 Phase 6 (§16.2) -- the Rule of Three, once
//! `Click`/`HoverEnter`/`HoverExit` all needed the same "look up a
//! registered handler for this node, call it" shape.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::{Duration, Instant};

use engine_core::{Action, EventKind, MotionCurve, NodeId, NodeKind, ShapeKey, Tree};
use peniko::Color;
use peniko::kurbo::{Affine, BezPath};
use pyo3::prelude::*;

use crate::dispatch::{HandlerMap, SharedCompletions};
use crate::error::EngineError;
use crate::window::SharedTheme;

#[pyclass(unsendable)]
pub struct Node {
    pub(crate) id: NodeId,
    pub(crate) tree: Rc<RefCell<Tree>>,
    pub(crate) handlers: HandlerMap,
    /// M4 Phase 7 (§11.3): `anchor NodeId -> content NodeId`, shared
    /// with the owning `PyWindow`/`View` the same way `handlers` is --
    /// no `Py<PyAny>` involved at all (unlike `handlers`), so no
    /// GC-traversal obligation the way a stored Python callback needs.
    pub(crate) context_menus: Rc<RefCell<HashMap<NodeId, NodeId>>>,
    /// M7 Phase 3 (§7.1): shared with the owning `PyWindow` the same
    /// way `context_menus` is -- no `Py<PyAny>` involved, no GC
    /// obligation. A `View`-created `Node` gets a fresh, private,
    /// never-`Window`-linked instance instead (`view.rs`), matching
    /// this phase's own stated scope.
    pub(crate) theme: SharedTheme,
    /// M9 Phase 2 (§5): `animate(..., on_complete=...)`'s own registry,
    /// shared with the owning `PyWindow` the same way `handlers` is --
    /// real `Py<PyAny>` callbacks live here, but `PyWindow`'s own
    /// `__traverse__`/`__clear__` already cover this same shared
    /// `Rc<RefCell<...>>` (it's the same underlying map, not a
    /// separate copy), so `Node` itself needs no new GC obligation of
    /// its own -- matching `handlers`' own existing precedent (`Node`
    /// has never implemented `__traverse__`/`__clear__` itself). A
    /// `View`-created `Node` gets a fresh, private instance instead,
    /// matching `theme`'s own precedent -- `View` has no real
    /// per-frame render loop to ever drain a completion through.
    pub(crate) completions: SharedCompletions,
}

#[pymethods]
impl Node {
    /// Starts (or retargets, §5's own `animate_to` semantics) an
    /// animation on one property. Registers the work and returns
    /// immediately -- never blocks waiting for the animation to finish
    /// (§8's own design rule). Dispatch is two-level per §8's review
    /// note: `PaintProperties`' own fields first (universal, every kind
    /// has them), then the node's `NodeKind` payload's fields if it has
    /// one -- `Text`'s `TextState` has none yet (§14 step 4 added no
    /// `Animated` fields to it), so that second level currently always
    /// falls through to `UnknownProperty`, which is the honest, correct
    /// behavior today, not a gap.
    /// M9 Phase 2 (§5): `on_complete`, when given, is called with no
    /// arguments exactly once, the real tick this specific animation
    /// genuinely finishes (`App::run`'s own per-frame loop is what
    /// actually drains and invokes it -- a `Window`-created node's
    /// callback fires for real; a `View`-created node's callback is
    /// registered the same way but never fires, since `View` has no
    /// real per-frame render loop to drain it through, the same stated
    /// scope limit `Window.set_theme` vs. `View`'s own theme already
    /// established, M7 Phase 3).
    #[pyo3(signature = (property, to, duration_ms=0, on_complete=None))]
    pub(crate) fn animate(
        &self,
        property: &str,
        to: Bound<'_, PyAny>,
        duration_ms: u64,
        on_complete: Option<Py<PyAny>>,
    ) -> PyResult<()> {
        let duration = Duration::from_millis(duration_ms);
        let now = Instant::now();
        let mut tree = self.tree.borrow_mut();
        let node = tree.get_mut(self.id).expect(
            "Node holds a NodeId missing from its own Tree -- an engine-py bug, not a user error",
        );
        let kind = kind_name(&node.kind);

        match property {
            "opacity" => {
                let value = extract_f64(&to, property)?;
                let handle = on_complete.map(|cb| self.completions.borrow_mut().register(cb));
                animate_field(&mut node.paint.opacity, value, duration, now, handle);
            }
            "corner_radius" => {
                let value = extract_f64(&to, property)?;
                let handle = on_complete.map(|cb| self.completions.borrow_mut().register(cb));
                animate_field(&mut node.paint.corner_radius, value, duration, now, handle);
            }
            "elevation" => {
                let value = extract_f64(&to, property)?;
                let handle = on_complete.map(|cb| self.completions.borrow_mut().register(cb));
                animate_field(&mut node.paint.elevation, value, duration, now, handle);
            }
            "background" => {
                let value = extract_color(&to, property)?;
                let handle = on_complete.map(|cb| self.completions.borrow_mut().register(cb));
                animate_field(&mut node.paint.background, value, duration, now, handle);
            }
            // M6 Phase 2 (§8): "pan offset × zoom scale" (§11.9's own
            // text), not a raw 6-coefficient `Affine` -- matches
            // `Interpolate for Affine`'s own real limitation (M5 Phase
            // 1): a plain componentwise coefficient lerp, exact only
            // for the no-rotation/shear subspace this 3-tuple can only
            // ever construct. A rotation-capable API is additive
            // whenever `Interpolate` itself gets a real decomposition.
            "transform" => {
                let (tx, ty, scale) = extract_translate_scale(&to, property)?;
                let value = Affine::translate((tx, ty)) * Affine::scale(scale);
                let handle = on_complete.map(|cb| self.completions.borrow_mut().register(cb));
                animate_field(&mut node.paint.transform, value, duration, now, handle);
            }
            // M7 Phase 4 (§7.4): a list of `(x, y)` vertices, since
            // Python has no `BezPath` type to hand over directly --
            // `extract_shape_points` builds one (closed, straight-line
            // segments between each point, matching `ShapeKey`'s own
            // "vertices only" scope), then `ShapeKey::from_path` does
            // the real extraction. The interpolation itself (real
            // correspondence-search-then-lerp, not naive per-index
            // pairing) is `Interpolate for ShapeKey`'s own job, already
            // real and unit-tested since M3 step 10 -- this arm only
            // ever supplies the *target* shape.
            "shape" => {
                let points = extract_shape_points(&to, property)?;
                let mut path = BezPath::new();
                let mut points = points.into_iter();
                if let Some(first) = points.next() {
                    path.move_to(first);
                    for point in points {
                        path.line_to(point);
                    }
                    path.close_path();
                }
                let value = ShapeKey::from_path(&path);
                let handle = on_complete.map(|cb| self.completions.borrow_mut().register(cb));
                animate_field(&mut node.paint.shape, value, duration, now, handle);
            }
            _ => {
                return Err(EngineError::UnknownProperty {
                    kind,
                    property: property.to_string(),
                }
                .into());
            }
        }
        Ok(())
    }

    /// Reads a numeric property's current (possibly still-animating)
    /// value -- `animate()`'s missing counterpart, added at §14 step 12
    /// once something (a binding's own applied value, §16.2) actually
    /// needed to be observed from Python rather than only ever written.
    /// `background` isn't included: it isn't a single `f64`, and
    /// nothing yet needs to read it back.
    fn get(&self, property: &str) -> PyResult<f64> {
        let tree = self.tree.borrow();
        let node = tree.get(self.id).expect(
            "Node holds a NodeId missing from its own Tree -- an engine-py bug, not a user error",
        );
        let kind = kind_name(&node.kind);
        match property {
            "opacity" => Ok(node.paint.opacity.current),
            "corner_radius" => Ok(node.paint.corner_radius.current),
            "elevation" => Ok(node.paint.elevation.current),
            _ => Err(EngineError::UnknownProperty {
                kind,
                property: property.to_string(),
            }
            .into()),
        }
    }

    /// M4 Phase 1 step 3 (§4, §11.10): registers `callback` to run when
    /// this node is *activated* -- a real primary-button click released
    /// over it, or `Enter`/`Space` while it's the keyboard-focused node
    /// (`Tree::dispatch`'s own `DispatchOutcome::Activated`, §2 Design
    /// Principle 6: `Tree` only knows *that* activation happened, this
    /// is where it's given meaning). Also adds `Action::Click` to this
    /// node's own `access.actions` if it isn't already there -- the same
    /// real signal `Tree::move_focus` already keys "interactive" off
    /// (§10), so a node this is called on becomes Tab-reachable for
    /// free, not just mouse-clickable; §10's own "keyboard operability
    /// ships from day one" stance applied to the one call site that
    /// actually makes a node interactive for the first time.
    pub(crate) fn set_on_click(&self, callback: Py<PyAny>) {
        self.handlers
            .borrow_mut()
            .insert((self.id, EventKind::Click), callback);
        if let Some(node) = self.tree.borrow_mut().get_mut(self.id)
            && !node.access.actions.contains(&Action::Click)
        {
            node.access.actions.push(Action::Click);
        }
    }

    /// M4 Phase 6 (§7.3, §16.2): registers `callback` to run when this
    /// node becomes the hovered node -- `Tree::dispatch`'s own
    /// `DispatchOutcome::HoverChanged`, fired independent of whether
    /// this node ever opted into `InteractionState`'s own visual
    /// animation (`enable_interaction`, M4 Phase 5) -- §7.3's own text:
    /// "the default MD3 visual never depends on anything handling it."
    pub(crate) fn set_on_hover_enter(&self, callback: Py<PyAny>) {
        self.handlers
            .borrow_mut()
            .insert((self.id, EventKind::HoverEnter), callback);
    }

    /// The `HoverExit` counterpart to `set_on_hover_enter` -- fired when
    /// this node stops being the hovered node.
    pub(crate) fn set_on_hover_exit(&self, callback: Py<PyAny>) {
        self.handlers
            .borrow_mut()
            .insert((self.id, EventKind::HoverExit), callback);
    }

    /// M4 Phase 7 (§11.3): registers `content` as this node's real
    /// right-click context menu -- opened for real by
    /// `dispatch::open_context_menu` on a `DispatchOutcome::
    /// SecondaryActivated`. `open_overlay`'s own contract requires
    /// `content` to be unattached (its `add_child` has no dedup, so
    /// attaching an already-attached node would corrupt the tree,
    /// confirmed by reading `add_child`'s real implementation) --
    /// `content` typically already has a parent, since every existing
    /// node-creation method (`add_rect`, etc.) attaches immediately, so
    /// this detaches it first via `Tree::detach`, the same real,
    /// existing mechanism docking's own tab-switching already uses to
    /// keep a node "alive, parentless, ready for `add_child` elsewhere
    /// later."
    ///
    /// M10 Phase 2 (§8): `content` must belong to this same `Node`'s
    /// own `Window` -- `NodeId` is only unique within the `Tree` that
    /// minted it, so a foreign `Node` would store a foreign id that
    /// could alias an unrelated real node the next time it's read back
    /// (the exact real gap `Node.add_child`'s own `Rc::ptr_eq` check
    /// already closed, M6 Phase 1 -- mirrored here verbatim).
    fn set_context_menu(&self, content: PyRef<'_, Node>) -> PyResult<()> {
        if !Rc::ptr_eq(&self.tree, &content.tree) {
            return Err(EngineError::ForeignNode.into());
        }
        let mut tree = self.tree.borrow_mut();
        if let Some(parent) = tree
            .get(content.id)
            .expect("set_context_menu: content NodeId not found in this Tree")
            .parent
        {
            tree.detach(parent, content.id);
        }
        drop(tree);
        self.context_menus.borrow_mut().insert(self.id, content.id);
        Ok(())
    }

    /// M4 Phase 5 (§7.3): opts this node into ripple/hover state-layer
    /// animation -- a thin call into `Tree::interaction_mut`, the same
    /// real, already-correctly-wired mechanism `Tree::dispatch`'s
    /// `PointerPressed` (ripple spawn) and `update_hover` (hover
    /// animation) have used since M3 Phase 5 step 9 and M4 Phase 1
    /// respectively. Nothing was missing at the dispatch level -- only
    /// that no `engine-py` call site ever opted a real Python-created
    /// node in at all, confirmed via grep before this method existed.
    ///
    /// M7 Phase 3 (§7.1) additionally applies this `Window`'s current
    /// theme's real "on-surface" color immediately, so a node enabled
    /// *after* `Window.set_theme` doesn't paint the plain-black default
    /// until the next live theme change -- `Window.set_theme` itself
    /// (§7.1 Step 1) is the counterpart for a node already enabled
    /// *before* the theme was set.
    ///
    /// Deliberately a separate method, not folded into `set_on_click`:
    /// a purely-hoverable, non-clickable node is a real, independent
    /// case §7.3 itself describes (hover is specified separately from
    /// click), and implicitly paying the extra per-frame animation cost
    /// just because a node got a click handler would be a surprising
    /// side effect for a caller who only wanted the click -- Design
    /// Principle 6's "only a node that opts in pays the cost" applies
    /// to each independently.
    fn enable_interaction(&self) {
        let tint = self.theme.borrow().on_surface();
        if let Some(state) = self.tree.borrow_mut().interaction_mut(self.id) {
            state.tint = tint;
        }
    }

    /// M6 Phase 1 (§8): attaches `child` under this node, rejecting a
    /// cycle with a real `PyValueError` instead of corrupting the tree
    /// -- §8's own original sketch, the first thing to actually need
    /// `EngineError::CycleRejected`. Checked *before* either `Tree` is
    /// touched: `child` must belong to this same `Node`'s own `Window`
    /// (`Rc::ptr_eq` on the shared `Tree` handle) -- a `NodeId` is only
    /// unique within the `Tree` that minted it, and handing a foreign
    /// one to this `Tree`'s own `taffy` tree risks real corruption, not
    /// just a wrong result. If `child` already has a different parent
    /// (every existing node-creation method attaches immediately, so it
    /// usually does), `Tree::try_add_child` detaches it first via the
    /// same `Tree::detach` mechanism `set_context_menu` already uses,
    /// rather than corrupting the tree the "`add_child` has no dedup"
    /// way M4 Phase 7/9 each already found once.
    fn add_child(&self, child: PyRef<'_, Node>) -> PyResult<()> {
        if !Rc::ptr_eq(&self.tree, &child.tree) {
            return Err(EngineError::ForeignNode.into());
        }
        if self.tree.borrow_mut().try_add_child(self.id, child.id) {
            Ok(())
        } else {
            Err(EngineError::CycleRejected.into())
        }
    }
}

/// M9 Phase 2 (§5): `animate()`'s own shared "start this field
/// animating, optionally with a real completion handle" dispatch --
/// generic over every `T: Interpolate + Clone` an `Animated<T>` can
/// wrap (`f64`, `Color`, `Affine`, `ShapeKey`), so each of `animate()`'s
/// six match arms needs one call, not its own copy of this branch.
/// `MotionCurve::Linear` matches every one of those arms' own existing,
/// unchanged choice.
fn animate_field<T: engine_core::Interpolate + Clone>(
    field: &mut engine_core::Animated<T>,
    value: T,
    duration: Duration,
    now: Instant,
    handle: Option<engine_core::CompletionHandle>,
) {
    match handle {
        Some(handle) => {
            field.animate_to_with_completion(value, duration, MotionCurve::Linear, now, handle);
        }
        None => field.animate_to(value, duration, MotionCurve::Linear, now),
    }
}

fn kind_name(kind: &NodeKind) -> &'static str {
    match kind {
        NodeKind::Rect => "Rect",
        NodeKind::Container => "Container",
        NodeKind::Text(_) => "Text",
        NodeKind::Splitter(_) => "Splitter",
        NodeKind::VirtualList(_) => "VirtualList",
        NodeKind::Canvas(_) => "Canvas",
    }
}

fn type_name_of(value: &Bound<'_, PyAny>) -> String {
    value
        .get_type()
        .name()
        .map(|name| name.to_string())
        .unwrap_or_else(|_| "<unknown type>".to_string())
}

fn extract_f64(to: &Bound<'_, PyAny>, property: &str) -> Result<f64, EngineError> {
    to.extract::<f64>().map_err(|_| EngineError::TypeMismatch {
        property: property.to_string(),
        expected: "a float",
        actual: type_name_of(to),
    })
}

fn extract_color(to: &Bound<'_, PyAny>, property: &str) -> Result<Color, EngineError> {
    to.extract::<(u8, u8, u8, u8)>()
        .map(|(r, g, b, a)| Color::from_rgba8(r, g, b, a))
        .map_err(|_| EngineError::TypeMismatch {
            property: property.to_string(),
            expected: "an (r, g, b, a) tuple of 0-255 ints",
            actual: type_name_of(to),
        })
}

/// M6 Phase 2: `"transform"`'s own real, narrower shape -- see
/// `animate()`'s own doc comment for why this is `(translate_x,
/// translate_y, scale)`, not a raw `Affine` coefficient tuple.
fn extract_translate_scale(
    to: &Bound<'_, PyAny>,
    property: &str,
) -> Result<(f64, f64, f64), EngineError> {
    to.extract::<(f64, f64, f64)>()
        .map_err(|_| EngineError::TypeMismatch {
            property: property.to_string(),
            expected: "a (translate_x, translate_y, scale) tuple of floats",
            actual: type_name_of(to),
        })
}

/// M7 Phase 4 (§7.4): `"shape"`'s own real, narrower shape -- a plain
/// list of `(x, y)` vertices, since Python has no `BezPath` type to
/// hand over directly. Mirrors `extract_translate_scale`'s own role:
/// convert Python's plain tuples into the real value `ShapeKey::
/// from_path` needs, nothing more.
fn extract_shape_points(
    to: &Bound<'_, PyAny>,
    property: &str,
) -> Result<Vec<(f64, f64)>, EngineError> {
    to.extract::<Vec<(f64, f64)>>()
        .map_err(|_| EngineError::TypeMismatch {
            property: property.to_string(),
            expected: "a list of (x, y) float tuples",
            actual: type_name_of(to),
        })
}
