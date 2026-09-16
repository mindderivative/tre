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

use engine_core::{Action, EventKind, MotionCurve, NodeId, NodeKind, Tree};
use peniko::Color;
use peniko::kurbo::Affine;
use pyo3::prelude::*;

use crate::dispatch::HandlerMap;
use crate::error::EngineError;

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
    #[pyo3(signature = (property, to, duration_ms=0))]
    pub(crate) fn animate(
        &self,
        property: &str,
        to: Bound<'_, PyAny>,
        duration_ms: u64,
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
                node.paint
                    .opacity
                    .animate_to(value, duration, MotionCurve::Linear, now);
            }
            "corner_radius" => {
                let value = extract_f64(&to, property)?;
                node.paint
                    .corner_radius
                    .animate_to(value, duration, MotionCurve::Linear, now);
            }
            "elevation" => {
                let value = extract_f64(&to, property)?;
                node.paint
                    .elevation
                    .animate_to(value, duration, MotionCurve::Linear, now);
            }
            "background" => {
                let value = extract_color(&to, property)?;
                node.paint
                    .background
                    .animate_to(value, duration, MotionCurve::Linear, now);
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
                node.paint
                    .transform
                    .animate_to(value, duration, MotionCurve::Linear, now);
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
    fn set_context_menu(&self, content: PyRef<'_, Node>) {
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
    /// Deliberately a separate method, not folded into `set_on_click`:
    /// a purely-hoverable, non-clickable node is a real, independent
    /// case §7.3 itself describes (hover is specified separately from
    /// click), and implicitly paying the extra per-frame animation cost
    /// just because a node got a click handler would be a surprising
    /// side effect for a caller who only wanted the click -- Design
    /// Principle 6's "only a node that opts in pays the cost" applies
    /// to each independently.
    fn enable_interaction(&self) {
        self.tree.borrow_mut().interaction_mut(self.id);
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
