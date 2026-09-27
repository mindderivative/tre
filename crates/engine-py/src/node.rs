//! `Node` (§8's `PyNode`, renamed to match what Python actually sees --
//! `tre.Node`, not `tre.PyNode`) -- narrower than §8's own full sketch:
//! `animate()`/`get()` plus, as of M4 Phase 1 step 3, `set_on_click`, and
//! as of M4 Phase 6, `set_on_hover_enter`/`set_on_hover_exit`.
//! Still no `add_child` (no cycle to reject, so `EngineError::
//! CycleRejected` isn't implemented yet either) -- additive when its own
//! later build-order step needs it.
//!
//! Handler callback storage (`handlers`) is an `Rc<RefCell<HashMap<
//! (NodeId, EventKind), (Py<PyAny>, bool)>>>` (the `bool`, M54 Phase 2,
//! is whether this handler arity-sniffed as wanting a real `Event`
//! argument) *shared* with the owning `PyWindow` (or `View`) -- created
//! once there, cloned into every `Node` handed
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
use std::rc::Rc;
use std::time::{Duration, Instant};

use engine_core::{MotionCurve, NodeId, NodeKind, Tree};
use peniko::Color;
use peniko::kurbo::Affine;
use pyo3::prelude::*;

use crate::dispatch::{HandlerMap, SharedCompletions};
use crate::error::EngineError;
use crate::node_handles;
use crate::thread_bound::ThreadBound;

/// A Python handle to one node. M96: a thread-checked shell around
/// `NodeState` (see `thread_bound`), so the cyclic collector can free it on
/// any thread; every method reaches the state through `Deref`. Built only
/// through `From<NodeState>`, which counts the handle (`node_handles`).
#[pyclass]
pub struct Node(ThreadBound<NodeState>);

impl std::ops::Deref for Node {
    type Target = NodeState;

    fn deref(&self) -> &NodeState {
        &self.0
    }
}

impl NodeState {
    /// A new handle to `id`, another node in this node's tree.
    pub(crate) fn handle_to(&self, id: NodeId) -> Node {
        Node::from(NodeState {
            id,
            tree: self.tree.clone(),
            handlers: self.handlers.clone(),
            completions: self.completions.clone(),
        })
    }

    /// When focus is on this node or inside it, clears it and fires `unfocus`
    /// (and the legacy focus-exit handler) -- before a detach or free, so
    /// `unfocus` bubbles through the tree as it still is.
    fn release_focus_within(&self, py: Python<'_>) {
        let inside = {
            let tree = self.tree.borrow();
            tree.focused()
                .is_some_and(|focused| tree.ancestors(focused).any(|id| id == self.id))
        };
        if !inside {
            return;
        }
        let transition = self.tree.borrow_mut().clear_focus();
        if let Some((old, new)) = transition {
            crate::dispatch::fire_focus_transition(
                &self.handlers,
                &self.tree,
                &self.completions,
                old,
                new,
                py,
            );
        }
    }

    /// `Err(Destroyed)` once this handle's node has been freed.
    pub(crate) fn check_alive(&self) -> PyResult<()> {
        if self.tree.borrow().get(self.id).is_some() {
            Ok(())
        } else {
            Err(EngineError::Destroyed.into())
        }
    }

    /// Both nodes alive and in the same tree -- what attaching one under
    /// the other needs.
    fn check_pair(&self, child: &NodeState) -> PyResult<()> {
        if !Rc::ptr_eq(&self.tree, &child.tree) {
            return Err(EngineError::ForeignNode.into());
        }
        self.check_alive()?;
        child.check_alive()
    }
}

impl From<NodeState> for Node {
    fn from(state: NodeState) -> Self {
        node_handles::retain(&state);
        Node(ThreadBound::new(state))
    }
}

impl Drop for NodeState {
    fn drop(&mut self) {
        node_handles::release(self);
    }
}

pub struct NodeState {
    pub(crate) id: NodeId,
    pub(crate) tree: Rc<RefCell<Tree>>,
    pub(crate) handlers: HandlerMap,
    /// M4 Phase 7 (§11.3): `anchor NodeId -> content NodeId`, shared
    /// with the owning `PyWindow`/`View` the same way `handlers` is --
    /// no `Py<PyAny>` involved at all (unlike `handlers`), so no
    /// GC-traversal obligation the way a stored Python callback needs.
    /// M7 Phase 3 (§7.1): shared with the owning `PyWindow` the same
    /// way `context_menus` is -- no `Py<PyAny>` involved, no GC
    /// obligation. A `View`-created `Node` gets a fresh, private,
    /// never-`Window`-linked instance instead (`view.rs`), matching
    /// this phase's own stated scope.
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
    #[pyo3(signature = (property, to, duration_ms=0, easing=None, on_complete=None))]
    pub(crate) fn animate(
        &self,
        property: &str,
        to: Bound<'_, PyAny>,
        duration_ms: u64,
        easing: Option<Bound<'_, PyAny>>,
        on_complete: Option<Py<PyAny>>,
    ) -> PyResult<()> {
        let duration = Duration::from_millis(duration_ms);
        let now = crate::clock::now(&self.tree);
        let curve = parse_easing(easing.as_ref())?;
        renamed_property(property)?;
        let mut tree = self.tree.borrow_mut();
        let node = tree.get_mut(self.id).expect(
            "Node holds a NodeId missing from its own Tree -- an engine-py bug, not a user error",
        );
        let kind = kind_name(&node.kind);

        match property {
            "opacity" => {
                let value = extract_f64(&to, property)?;
                let handle = on_complete.map(|cb| self.completions.borrow_mut().register(cb));
                animate_field(&mut node.paint.opacity, value, duration, curve, now, handle);
            }
            // M95: a number animates the uniform radius, or all four
            // corners when they're set separately; a 4-tuple animates the
            // four corners, starting from the uniform radius if they
            // weren't separate yet.
            "corner_radius" => {
                let radius = crate::node_props::parse_radius(&to, property)?;
                let handle = on_complete.map(|cb| self.completions.borrow_mut().register(cb));
                match (radius, &mut node.paint.corner_radii_override) {
                    (crate::node_props::Radius::Uniform(value), None) => {
                        animate_field(
                            &mut node.paint.corner_radius,
                            value,
                            duration,
                            curve,
                            now,
                            handle,
                        );
                    }
                    (crate::node_props::Radius::Uniform(value), Some(radii)) => {
                        animate_field(
                            radii,
                            engine_core::CornerRadii([value; 4]),
                            duration,
                            curve,
                            now,
                            handle,
                        );
                    }
                    (crate::node_props::Radius::Corners(corners), radii) => {
                        let uniform = node.paint.corner_radius.current;
                        let radii = radii.get_or_insert_with(|| {
                            engine_core::Animated::new(engine_core::CornerRadii([uniform; 4]))
                        });
                        animate_field(
                            radii,
                            engine_core::CornerRadii(corners),
                            duration,
                            curve,
                            now,
                            handle,
                        );
                    }
                }
            }
            // M95: the target API's paint names.
            "fill" => {
                let value = crate::node_props::parse_color(&to, property)?;
                let handle = on_complete.map(|cb| self.completions.borrow_mut().register(cb));
                match &mut node.kind {
                    NodeKind::TextField(state) => {
                        animate_field(&mut state.text_tint, value, duration, curve, now, handle);
                    }
                    _ => animate_field(
                        &mut node.paint.background,
                        value,
                        duration,
                        curve,
                        now,
                        handle,
                    ),
                }
            }
            "stroke_color" => {
                let value = crate::node_props::parse_color(&to, property)?;
                let handle = on_complete.map(|cb| self.completions.borrow_mut().register(cb));
                animate_field(
                    &mut node.paint.border_color,
                    value,
                    duration,
                    curve,
                    now,
                    handle,
                );
            }
            // M96: a scroll view's offset, eased -- how a carousel snaps.
            "scroll_offset" => {
                let value = crate::node_props::parse_non_negative(&to, property)?;
                let NodeKind::ScrollView(state) = &mut node.kind else {
                    return Err(pyo3::exceptions::PyValueError::new_err(
                        "node property `scroll_offset` applies only to a scroll_view node",
                    ));
                };
                let handle = on_complete.map(|cb| self.completions.borrow_mut().register(cb));
                animate_field(&mut state.scroll, value, duration, curve, now, handle);
            }
            // M96: the target API's transform parts, each with its own
            // animation (`NodeTransform`).
            "translate_x" | "translate_y" | "scale" | "rotation_deg" => {
                let value = if property == "scale" {
                    crate::node_props::parse_non_negative(&to, property)?
                } else {
                    extract_f64(&to, property)?
                };
                let handle = on_complete.map(|cb| self.completions.borrow_mut().register(cb));
                let parts = &mut node.paint.node_transform;
                let field = match property {
                    "translate_x" => &mut parts.translate_x,
                    "translate_y" => &mut parts.translate_y,
                    "scale" => &mut parts.scale,
                    _ => &mut parts.rotation_deg,
                };
                animate_field(field, value, duration, curve, now, handle);
            }
            "stroke_width" => {
                let value = crate::node_props::parse_non_negative(&to, property)?;
                let handle = on_complete.map(|cb| self.completions.borrow_mut().register(cb));
                animate_field(
                    &mut node.paint.border_width,
                    value,
                    duration,
                    curve,
                    now,
                    handle,
                );
            }
            "shadows" => {
                let value = engine_core::Shadows(crate::node_props::parse_shadows(&to, property)?);
                let handle = on_complete.map(|cb| self.completions.borrow_mut().register(cb));
                animate_field(&mut node.paint.shadows, value, duration, curve, now, handle);
            }
            "background" => {
                if is_glyph_kind(&node.kind) {
                    return Err(pyo3::exceptions::PyValueError::new_err(format!(
                        "{kind} has no fill, so 'background' doesn't apply -- its glyph/text color \
                         is 'foreground'"
                    )));
                }
                let value = extract_color(&to, property)?;
                let handle = on_complete.map(|cb| self.completions.borrow_mut().register(cb));
                animate_field(
                    &mut node.paint.background,
                    value,
                    duration,
                    curve,
                    now,
                    handle,
                );
            }
            // M90: a text node's glyph color, stored in `paint.background`.
            "foreground" => {
                let value = extract_color(&to, property)?;
                match &mut node.kind {
                    NodeKind::Text(_) => {
                        let handle =
                            on_complete.map(|cb| self.completions.borrow_mut().register(cb));
                        animate_field(
                            &mut node.paint.background,
                            value,
                            duration,
                            curve,
                            now,
                            handle,
                        );
                    }
                    _ => {
                        return Err(EngineError::UnknownProperty {
                            kind,
                            property: property.to_string(),
                        }
                        .into());
                    }
                }
            }
            // M48 (§5, §7): `border_color`/`border_width` are real
            // `PaintProperties` fields since M30 Phase 1 but were never
            // reachable from `animate()` -- confirmed via direct read of
            // this match's own exhaustive arm list before this change.
            // Mirrors `"background"`/`"corner_radius"` exactly; no new
            // engine-core work needed, the fields are already `Animated`.
            "border_color" => {
                let value = extract_color(&to, property)?;
                let handle = on_complete.map(|cb| self.completions.borrow_mut().register(cb));
                animate_field(
                    &mut node.paint.border_color,
                    value,
                    duration,
                    curve,
                    now,
                    handle,
                );
            }
            "border_width" => {
                let value = extract_f64(&to, property)?;
                let handle = on_complete.map(|cb| self.completions.borrow_mut().register(cb));
                animate_field(
                    &mut node.paint.border_width,
                    value,
                    duration,
                    curve,
                    now,
                    handle,
                );
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
                animate_field(
                    &mut node.paint.transform,
                    value,
                    duration,
                    curve,
                    now,
                    handle,
                );
            }
            // M30 Phase 3 Step 2 (§8): the progress indicators' own
            // arm. M90: `Slider` joins it -- a slider's position was
            // `thumb_position` here but `value` everywhere else.
            // M95: a path's data morphs; its stroke trim animates.
            "data" | "trim_start" | "trim_end" => {
                let NodeKind::Path(state) = &mut node.kind else {
                    return Err(pyo3::exceptions::PyValueError::new_err(format!(
                        "node property `{property}` applies only to a path node"
                    )));
                };
                let handle = |completions: &SharedCompletions| {
                    on_complete.map(|cb| completions.borrow_mut().register(cb))
                };
                if property == "data" {
                    let data: String = to.extract().map_err(|_| {
                        pyo3::exceptions::PyValueError::new_err(
                            "node property `data` must be SVG path data (a str)",
                        )
                    })?;
                    let value = engine_core::PathData::from_svg(&data).map_err(|err| {
                        pyo3::exceptions::PyValueError::new_err(format!(
                            "node property `data` isn't valid SVG path data: {err}"
                        ))
                    })?;
                    animate_field(
                        &mut state.data,
                        value,
                        duration,
                        curve,
                        now,
                        handle(&self.completions),
                    );
                } else {
                    let value = extract_f64(&to, property)?;
                    if !(0.0..=1.0).contains(&value) {
                        return Err(pyo3::exceptions::PyValueError::new_err(format!(
                            "node property `{property}` must be a number from 0.0 to 1.0"
                        )));
                    }
                    let field = if property == "trim_start" {
                        &mut state.trim_start
                    } else {
                        &mut state.trim_end
                    };
                    animate_field(
                        field,
                        value,
                        duration,
                        curve,
                        now,
                        handle(&self.completions),
                    );
                }
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
        self.check_pair(&child)?;
        if self.tree.borrow_mut().try_add_child(self.id, child.id) {
            Ok(())
        } else {
            Err(EngineError::CycleRejected.into())
        }
    }

    /// M96: attaches `child` so it ends up at `index` among this node's
    /// children, moving it if it's already attached anywhere -- the
    /// keyed-reorder primitive. A moved node keeps its identity, listeners,
    /// focus, and running animations. `index` counts the children as they
    /// are once `child` has left its old place, so afterwards
    /// `children()[index] == child`.
    fn insert_child(&self, index: usize, child: PyRef<'_, Node>) -> PyResult<()> {
        self.check_pair(&child)?;
        let mut tree = self.tree.borrow_mut();
        let siblings = tree.content_children(self.id).len();
        let already_here = tree.content_children(self.id).contains(&child.id);
        let limit = siblings - usize::from(already_here);
        if index > limit {
            return Err(pyo3::exceptions::PyIndexError::new_err(format!(
                "index {index} is out of range: this node would have {} children, so the \
                 index must be 0 to {limit}",
                limit + 1
            )));
        }
        if tree.insert_child(self.id, index, child.id) {
            Ok(())
        } else {
            Err(EngineError::CycleRejected.into())
        }
    }

    /// M96: this node's children, in order -- open layers aren't among
    /// the root's.
    fn children(&self) -> PyResult<Vec<Node>> {
        self.check_alive()?;
        let ids = self.tree.borrow().content_children(self.id).to_vec();
        Ok(ids.into_iter().map(|id| self.handle_to(id)).collect())
    }

    /// M96: this node's parent, or `None` for a detached node, the root, or
    /// a shown layer.
    fn parent(&self) -> PyResult<Option<Node>> {
        let parent = {
            let tree = self.tree.borrow();
            let parent = tree.get(self.id).ok_or(EngineError::Destroyed)?.parent;
            parent.filter(|_| !tree.is_layer(self.id))
        };
        Ok(parent.map(|id| self.handle_to(id)))
    }

    /// M96 (R5): detaches this node from its parent. It stays alive and can
    /// be attached again while any handle to it, or to anything under it,
    /// exists; after that it's freed automatically.
    ///
    /// Focus inside it leaves with it: the focused node gets `unfocus`, and
    /// nothing is focused until something else is. Everything else it
    /// holds -- scroll offsets, a text input's text and selection, running
    /// animations, which keep advancing -- is kept for when it's attached
    /// again.
    fn remove(&self, py: Python<'_>) -> PyResult<()> {
        self.check_alive()?;
        self.release_focus_within(py);
        self.tree.borrow_mut().detach_collectible(self.id);
        Ok(())
    }

    /// M96: frees this node and its whole subtree now, with their
    /// listeners. Any handle to a freed node raises `ValueError` on use.
    fn destroy(&self, py: Python<'_>) -> PyResult<()> {
        self.check_alive()?;
        self.release_focus_within(py);
        let freed = self.tree.borrow_mut().destroy(self.id);
        node_handles::prune(&self.handlers, &freed);
        Ok(())
    }
}

/// M9 Phase 2 (§5): `animate()`'s own shared "start this field
/// animating, optionally with a real completion handle" dispatch --
/// generic over every `T: Interpolate + Clone` an `Animated<T>` can
/// wrap (`f64`, `Color`, `Affine`), so each of `animate()`'s
/// six match arms needs one call, not its own copy of this branch.
/// `MotionCurve::Linear` matches every one of those arms' own existing,
/// unchanged choice.
/// M95: `animate`'s `easing` -- `None` or `"linear"`, or a cubic bezier
/// `(x1, y1, x2, y2)` with `x1` and `x2` in `0.0..=1.0`, as CSS
/// `cubic-bezier()` takes them.
fn parse_easing(easing: Option<&Bound<'_, PyAny>>) -> PyResult<MotionCurve> {
    let expected = "easing must be \"linear\" or a cubic bezier (x1, y1, x2, y2) with x1 and x2 \
                    from 0.0 to 1.0";
    let Some(easing) = easing else {
        return Ok(MotionCurve::Linear);
    };
    if easing.is_none() {
        return Ok(MotionCurve::Linear);
    }
    if let Ok(name) = easing.extract::<String>() {
        return if name == "linear" {
            Ok(MotionCurve::Linear)
        } else {
            Err(pyo3::exceptions::PyValueError::new_err(expected))
        };
    }
    let (x1, y1, x2, y2): (f64, f64, f64, f64) = easing
        .extract()
        .map_err(|_| pyo3::exceptions::PyValueError::new_err(expected))?;
    if !(0.0..=1.0).contains(&x1) || !(0.0..=1.0).contains(&x2) {
        return Err(pyo3::exceptions::PyValueError::new_err(expected));
    }
    Ok(MotionCurve::Bezier(x1, y1, x2, y2))
}

fn animate_field<T: engine_core::Interpolate + Clone>(
    field: &mut engine_core::Animated<T>,
    value: T,
    duration: Duration,
    curve: MotionCurve,
    now: Instant,
    handle: Option<engine_core::CompletionHandle>,
) {
    match handle {
        Some(handle) => {
            field.animate_to_with_completion(value, duration, curve, now, handle);
        }
        None => field.animate_to(value, duration, curve, now),
    }
}

/// M59 (§5, §16.3): `Node.set_layout`'s own `align_items=`/`justify_
/// content=` string parsing -- the same "small vocabulary, plain
/// string, `ValueError` on unrecognized" convention `press_key`'s own
/// "unknown key" error already established, not a dedicated Python-
/// facing enum type (every other style kwarg on this method is already
/// a plain scalar). Deliberately the same bounded subset `engine-spec`
/// ::`AlignItemsSpec`'s own real vocabulary uses, so the imperative and
/// declarative paths agree on what's real here.
/// M71 (§5, §8, §16.1): `set_layout`'s own real `flex_direction=`
/// string parsing -- the identical "small vocabulary, plain string,
/// `ValueError` on unrecognized" convention `parse_align_items`/
/// `parse_justify_content` (below) already establish. `"horizontal"`/
/// `"vertical"`, not taffy's own `"row"`/`"column"` -- see `set_layout`'s
/// own doc comment for the real reasoning (the identical vocabulary
/// fix M70 already made to the declarative `FlexDirectionSpec` layer).
/// M90: a property name renamed in 0.3.3 fails naming its replacement,
/// rather than as a generic unknown property.
fn renamed_property(property: &str) -> PyResult<()> {
    let replacement = match property {
        "thumb_position" => "value",
        _ => return Ok(()),
    };
    Err(pyo3::exceptions::PyValueError::new_err(format!(
        "{property:?} was renamed to {replacement:?} in tre 0.3.3"
    )))
}

/// M90: kinds whose paint color is their glyph or text -- they take
/// `foreground`, and have no fill for `background` to describe.
fn is_glyph_kind(kind: &NodeKind) -> bool {
    matches!(kind, NodeKind::Text(_))
}

/// M82: shared by `push_frame` and `Window.add_image_from_bytes` --
/// both accept a caller-decoded, straight-alpha RGBA8 buffer with no
/// `tre`-side decoding at all, and both need the identical real length
/// check (and identical error wording) against that contract.
pub(crate) fn validate_rgba_frame_len(
    context: &str,
    rgba_len: usize,
    width: u32,
    height: u32,
) -> PyResult<()> {
    let expected_len = (width as usize)
        .checked_mul(height as usize)
        .and_then(|pixels| pixels.checked_mul(4));
    if expected_len != Some(rgba_len) {
        return Err(pyo3::exceptions::PyValueError::new_err(format!(
            "{context}: rgba has {rgba_len} bytes, but a {width}x{height} RGBA8 frame needs {}",
            expected_len.map_or("too many to represent".to_string(), |n| n.to_string()),
        )));
    }
    Ok(())
}

fn kind_name(kind: &NodeKind) -> &'static str {
    match kind {
        NodeKind::Rect => "Rect",
        NodeKind::Container => "Container",
        NodeKind::Text(_) => "Text",
        NodeKind::VirtualList(_) => "VirtualList",
        NodeKind::Canvas(_) => "Canvas",
        NodeKind::TextField(_) => "TextField",
        NodeKind::Image(_) => "Image",
        NodeKind::Path(_) => "Path",
        NodeKind::Terminal(_) => "Terminal",
        NodeKind::ScrollView(_) => "ScrollView",
    }
}

/// M97: `get("kind")` -- a node's kind by the name `window.create` takes
/// it under.
pub(crate) fn kind_id(kind: &NodeKind) -> &'static str {
    match kind {
        NodeKind::Rect => "box",
        NodeKind::Container => "container",
        NodeKind::Text(_) => "text",
        NodeKind::VirtualList(_) => "virtual_list",
        NodeKind::Canvas(_) => "canvas",
        NodeKind::TextField(_) => "text_input",
        NodeKind::Image(_) => "image",
        NodeKind::Path(_) => "path",
        NodeKind::Terminal(_) => "terminal",
        NodeKind::ScrollView(_) => "scroll_view",
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

impl Node {
    /// Reads a numeric property's current (possibly still-animating)
    /// M94: no longer a Python method itself -- `Node.get`
    /// (`node_events.rs`) serves the M94 properties and falls back to
    /// this for the animatable numeric ones.
    /// value -- `animate()`'s missing counterpart, added at §14 step 12
    /// once something (a binding's own applied value, §16.2) actually
    /// needed to be observed from Python rather than only ever written.
    /// `background` isn't included: it isn't a single `f64`, and
    /// nothing yet needs to read it back.
    pub(crate) fn get_number(&self, property: &str) -> PyResult<f64> {
        renamed_property(property)?;
        let tree = self.tree.borrow();
        let node = tree.get(self.id).expect(
            "Node holds a NodeId missing from its own Tree -- an engine-py bug, not a user error",
        );
        let kind = kind_name(&node.kind);
        match property {
            "opacity" => Ok(node.paint.opacity.current),
            "corner_radius" => Ok(node.paint.corner_radius.current),
            // M48: `border_width` is a plain `Animated<f64>`, the same
            // shape as `corner_radius`/`elevation` above -- `border_
            // color` stays excluded, the same real reason `background`
            // already is (not a single `f64`).
            "border_width" => Ok(node.paint.border_width.current),
            _ => Err(EngineError::UnknownProperty {
                kind,
                property: property.to_string(),
            }
            .into()),
        }
    }
}
