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
use std::collections::HashMap;
use std::rc::Rc;
use std::time::{Duration, Instant};

use engine_core::{
    Action, EventKind, MotionCurve, NodeId, NodeKind, ShapeKey, TimePickerDialMode, Tree,
};
use peniko::Color;
use peniko::kurbo::{Affine, BezPath};
use pyo3::prelude::*;
use taffy::prelude::{AlignItems, FlexDirection, JustifyContent, Rect as TaffyRect, Size, length};

use crate::dispatch::{HandlerMap, SharedCompletions, call_handler};
use crate::error::EngineError;
use crate::event::{Event, NodeContext};
use crate::window::SharedTheme;

/// `set_syntax_spans`'s own real `(start, end, (r, g, b, a))` element
/// type, named purely to keep that signature under clippy's type-
/// complexity lint -- not a real domain concept reused anywhere else.
type SyntaxSpanInput = (usize, usize, (u8, u8, u8, u8));

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
                if is_glyph_kind(&node.kind) {
                    return Err(pyo3::exceptions::PyValueError::new_err(format!(
                        "{kind} has no fill, so 'background' doesn't apply -- its glyph/text color \
                         is 'foreground'"
                    )));
                }
                let value = extract_color(&to, property)?;
                let handle = on_complete.map(|cb| self.completions.borrow_mut().register(cb));
                animate_field(&mut node.paint.background, value, duration, now, handle);
            }
            // M90: the glyph/text color of `Text`/`Link`/`Icon`/
            // `LoadingIndicator`. `Text`/`Link`/`LoadingIndicator` store
            // it in `paint.background`; an `Icon` in `IconState.tint`
            // (`Animated` since M92). Both ease the same way.
            "foreground" => {
                let value = extract_color(&to, property)?;
                match &mut node.kind {
                    NodeKind::Icon(state) => {
                        let handle =
                            on_complete.map(|cb| self.completions.borrow_mut().register(cb));
                        animate_field(&mut state.tint, value, duration, now, handle);
                    }
                    NodeKind::Text(_) | NodeKind::Link(_) | NodeKind::LoadingIndicator(_) => {
                        let handle =
                            on_complete.map(|cb| self.completions.borrow_mut().register(cb));
                        animate_field(&mut node.paint.background, value, duration, now, handle);
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
                animate_field(&mut node.paint.border_color, value, duration, now, handle);
            }
            "border_width" => {
                let value = extract_f64(&to, property)?;
                let handle = on_complete.map(|cb| self.completions.borrow_mut().register(cb));
                animate_field(&mut node.paint.border_width, value, duration, now, handle);
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
            // M14 Phase 1 (§8): the first real arm of the "two-level
            // dispatch" ARCHITECTURE.md §8 describes -- `property`
            // against `PaintProperties`' own universal fields first
            // (above), then against the node's own `NodeKind` payload
            // if it has one. Only resolves on a real `Checkbox`.
            "check_progress" => match &mut node.kind {
                NodeKind::Checkbox(state) => {
                    let value = extract_f64(&to, property)?;
                    let handle = on_complete.map(|cb| self.completions.borrow_mut().register(cb));
                    animate_field(&mut state.check_progress, value, duration, now, handle);
                }
                _ => {
                    return Err(EngineError::UnknownProperty {
                        kind,
                        property: property.to_string(),
                    }
                    .into());
                }
            },
            // M14 Phase 2 (§8): the second real kind-payload arm -- a
            // real, app-triggered eased move (a keyboard nudge, say),
            // distinct from the real drag path (`Tree::set_slider_
            // position`, driven entirely inside `engine-core`'s own
            // dispatch, never through here).
            // M30 Phase 2 Step 1 (§8): `check_progress`'s own real
            // arm, mirrored for `RadioButton`.
            "select_progress" => match &mut node.kind {
                NodeKind::RadioButton(state) => {
                    let value = extract_f64(&to, property)?;
                    let handle = on_complete.map(|cb| self.completions.borrow_mut().register(cb));
                    animate_field(&mut state.select_progress, value, duration, now, handle);
                }
                _ => {
                    return Err(EngineError::UnknownProperty {
                        kind,
                        property: property.to_string(),
                    }
                    .into());
                }
            },
            // M35 Phase 2 (§8): `Split Button`'s own real trailing-icon
            // rotation -- degrees of clockwise rotation, the identical
            // "a scalar progress value" shape `check_progress`/`select_
            // progress`/`toggle_progress` already establish. Only
            // resolves on a real `Icon` (`IconState`'s own doc comment
            // has the full real reason this isn't routed through the
            // universal `"transform"` arm above).
            "rotation" => match &mut node.kind {
                NodeKind::Icon(state) => {
                    let value = extract_f64(&to, property)?;
                    let handle = on_complete.map(|cb| self.completions.borrow_mut().register(cb));
                    animate_field(&mut state.rotation, value, duration, now, handle);
                }
                _ => {
                    return Err(EngineError::UnknownProperty {
                        kind,
                        property: property.to_string(),
                    }
                    .into());
                }
            },
            // M30 Phase 2 Step 2 (§8): the same real arm, mirrored a
            // third time for `Switch`.
            "toggle_progress" => match &mut node.kind {
                NodeKind::Switch(state) => {
                    let value = extract_f64(&to, property)?;
                    let handle = on_complete.map(|cb| self.completions.borrow_mut().register(cb));
                    animate_field(&mut state.toggle_progress, value, duration, now, handle);
                }
                _ => {
                    return Err(EngineError::UnknownProperty {
                        kind,
                        property: property.to_string(),
                    }
                    .into());
                }
            },
            // M30 Phase 3 Step 2 (§8): the progress indicators' own
            // arm. M90: `Slider` joins it -- a slider's position was
            // `thumb_position` here but `value` everywhere else.
            "value" => match &mut node.kind {
                NodeKind::Slider(state) => {
                    let value = extract_f64(&to, property)?;
                    let handle = on_complete.map(|cb| self.completions.borrow_mut().register(cb));
                    animate_field(&mut state.thumb_position, value, duration, now, handle);
                }
                NodeKind::LinearProgress(state) => {
                    let value = extract_f64(&to, property)?;
                    let handle = on_complete.map(|cb| self.completions.borrow_mut().register(cb));
                    animate_field(&mut state.value, value, duration, now, handle);
                }
                NodeKind::CircularProgress(state) => {
                    let value = extract_f64(&to, property)?;
                    let handle = on_complete.map(|cb| self.completions.borrow_mut().register(cb));
                    animate_field(&mut state.value, value, duration, now, handle);
                }
                _ => {
                    return Err(EngineError::UnknownProperty {
                        kind,
                        property: property.to_string(),
                    }
                    .into());
                }
            },
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
    pub(crate) fn get(&self, property: &str) -> PyResult<f64> {
        renamed_property(property)?;
        let tree = self.tree.borrow();
        let node = tree.get(self.id).expect(
            "Node holds a NodeId missing from its own Tree -- an engine-py bug, not a user error",
        );
        let kind = kind_name(&node.kind);
        match property {
            "opacity" => Ok(node.paint.opacity.current),
            "corner_radius" => Ok(node.paint.corner_radius.current),
            "elevation" => Ok(node.paint.elevation.current),
            // M48: `border_width` is a plain `Animated<f64>`, the same
            // shape as `corner_radius`/`elevation` above -- `border_
            // color` stays excluded, the same real reason `background`
            // already is (not a single `f64`).
            "border_width" => Ok(node.paint.border_width.current),
            "check_progress" => match &node.kind {
                NodeKind::Checkbox(state) => Ok(state.check_progress.current),
                _ => Err(EngineError::UnknownProperty {
                    kind,
                    property: property.to_string(),
                }
                .into()),
            },
            "select_progress" => match &node.kind {
                NodeKind::RadioButton(state) => Ok(state.select_progress.current),
                _ => Err(EngineError::UnknownProperty {
                    kind,
                    property: property.to_string(),
                }
                .into()),
            },
            "toggle_progress" => match &node.kind {
                NodeKind::Switch(state) => Ok(state.toggle_progress.current),
                _ => Err(EngineError::UnknownProperty {
                    kind,
                    property: property.to_string(),
                }
                .into()),
            },
            "value" => match &node.kind {
                NodeKind::Slider(state) => Ok(state.thumb_position.current),
                NodeKind::LinearProgress(state) => Ok(state.value.current),
                NodeKind::CircularProgress(state) => Ok(state.value.current),
                _ => Err(EngineError::UnknownProperty {
                    kind,
                    property: property.to_string(),
                }
                .into()),
            },
            _ => Err(EngineError::UnknownProperty {
                kind,
                property: property.to_string(),
            }
            .into()),
        }
    }

    /// M48 (§5, §7, §11): the general live layout-mutation API this
    /// engine never had -- `width`/`height`/`padding`/`gap` were fixed
    /// at construction with no way to change them afterward from Python
    /// or YAML, confirmed via grep before this method existed (the only
    /// real precedent was `Window.resize_terminal`'s narrow, size-only
    /// version, `window_factory.rs`). Generalizes that exact real
    /// pattern instead of inventing a new one: clone this node's current
    /// `layout_style`, patch only the fields the caller actually passed
    /// (`None` by default, so a partial call like `set_layout(width=200)`
    /// leaves every other field untouched), then push the result back
    /// through `Tree::set_layout_style` -- the one sanctioned way to
    /// mutate `layout_style` post-insert (its own doc comment: direct
    /// field mutation desyncs `taffy`'s internal copy, the real bug
    /// M32 Phase 2 hit). Immediate, not eased -- these fields aren't
    /// `Animated<T>` (a layout box doesn't tween the way a paint
    /// property does in this engine), so this is a parallel dispatch
    /// next to `animate()`, not an extra match arm inside it.
    ///
    /// M59 (§5, §16.3): widened with per-side padding/margin (each
    /// independently optional, layered *on top of* the uniform
    /// `padding=`/`margin=` when both are given -- the per-side kwarg
    /// always wins for that one side, matching `StyleSpec`'s own real
    /// `SpacingSpec` "uniform or per-side" duality, M59's own new
    /// `engine-spec` counterpart), `flex_grow`/`flex_shrink`/
    /// `flex_basis`, and `align_items`/`justify_content` (plain
    /// lowercase-snake-case strings, matching `press_key`'s own
    /// established "small vocabulary, `ValueError` on unrecognized"
    /// convention -- no dedicated Python-facing enum type, consistent
    /// with every other style kwarg on this method already being a
    /// plain scalar). `Window.add_rect`/other factories deliberately
    /// gain none of these as constructor kwargs -- this method already
    /// gives every already-built node (regardless of which factory
    /// created it) the identical real imperative path, named out of
    /// scope, not silently skipped.
    // M71 (§5, §8, §16.1): `flex_direction` added -- a real, confirmed
    // gap found while building Tesserae's own Python widget catalog on
    // top of this method: no imperative way existed anywhere to set a
    // node's own main axis at all (`Window.add_rect`/every other
    // factory returns a node whose real taffy default is already
    // `Row` -- `taffy::style::Display::DEFAULT`/its own sibling
    // `FlexDirection` default -- with no constructor kwarg or `Node`
    // method to ever change it), so a Python-composed widget needing a
    // real vertical stack (a dialog's headline/body/actions, a
    // snackbar's text/action, a list) had no way to ask for one.
    // `"horizontal"`/`"vertical"`, not taffy's own `"row"`/`"column"`
    // -- the identical real vocabulary fix M70 already made to the
    // declarative `FlexDirectionSpec` layer, extended here for the
    // identical real reason (`spec.rs`'s own doc comment: "row"/
    // "column" collide with spreadsheet/datasheet vocabulary), so an
    // app author sees one consistent axis vocabulary across both the
    // declarative and imperative surfaces, not two competing ones.
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (
        width=None, height=None,
        padding=None, padding_top=None, padding_right=None, padding_bottom=None, padding_left=None,
        margin=None, margin_top=None, margin_right=None, margin_bottom=None, margin_left=None,
        gap=None, flex_grow=None, flex_shrink=None, flex_basis=None,
        align_items=None, justify_content=None, flex_direction=None,
    ))]
    pub(crate) fn set_layout(
        &self,
        width: Option<f32>,
        height: Option<f32>,
        padding: Option<f32>,
        padding_top: Option<f32>,
        padding_right: Option<f32>,
        padding_bottom: Option<f32>,
        padding_left: Option<f32>,
        margin: Option<f32>,
        margin_top: Option<f32>,
        margin_right: Option<f32>,
        margin_bottom: Option<f32>,
        margin_left: Option<f32>,
        gap: Option<f32>,
        flex_grow: Option<f32>,
        flex_shrink: Option<f32>,
        flex_basis: Option<f32>,
        align_items: Option<&str>,
        justify_content: Option<&str>,
        flex_direction: Option<&str>,
    ) -> PyResult<()> {
        let align_items = align_items.map(parse_align_items).transpose()?;
        let justify_content = justify_content.map(parse_justify_content).transpose()?;
        let flex_direction = flex_direction.map(parse_flex_direction).transpose()?;

        let mut tree = self.tree.borrow_mut();
        let mut style = tree
            .get(self.id)
            .expect("Node holds a NodeId missing from its own Tree -- an engine-py bug")
            .layout_style
            .clone();
        if let Some(width) = width {
            style.size.width = length(width);
        }
        if let Some(height) = height {
            style.size.height = length(height);
        }
        if let Some(padding) = padding {
            style.padding = TaffyRect {
                left: length(padding),
                right: length(padding),
                top: length(padding),
                bottom: length(padding),
            };
        }
        if let Some(v) = padding_top {
            style.padding.top = length(v);
        }
        if let Some(v) = padding_right {
            style.padding.right = length(v);
        }
        if let Some(v) = padding_bottom {
            style.padding.bottom = length(v);
        }
        if let Some(v) = padding_left {
            style.padding.left = length(v);
        }
        if let Some(margin) = margin {
            style.margin = TaffyRect {
                left: length(margin),
                right: length(margin),
                top: length(margin),
                bottom: length(margin),
            };
        }
        if let Some(v) = margin_top {
            style.margin.top = length(v);
        }
        if let Some(v) = margin_right {
            style.margin.right = length(v);
        }
        if let Some(v) = margin_bottom {
            style.margin.bottom = length(v);
        }
        if let Some(v) = margin_left {
            style.margin.left = length(v);
        }
        if let Some(gap) = gap {
            style.gap = Size {
                width: length(gap),
                height: length(gap),
            };
        }
        if let Some(flex_grow) = flex_grow {
            style.flex_grow = flex_grow;
        }
        if let Some(flex_shrink) = flex_shrink {
            style.flex_shrink = flex_shrink;
        }
        if let Some(flex_basis) = flex_basis {
            style.flex_basis = length(flex_basis);
        }
        if let Some(align_items) = align_items {
            style.align_items = Some(align_items);
        }
        if let Some(justify_content) = justify_content {
            style.justify_content = Some(justify_content);
        }
        if let Some(flex_direction) = flex_direction {
            style.flex_direction = flex_direction;
        }
        tree.set_layout_style(self.id, style);
        Ok(())
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
    pub(crate) fn set_on_click(&self, callback: Py<PyAny>, py: Python<'_>) -> PyResult<()> {
        crate::dispatch::register_handler(
            &self.handlers,
            (self.id, EventKind::Click),
            callback,
            py,
        )?;
        if let Some(node) = self.tree.borrow_mut().get_mut(self.id)
            && !node.access.actions.contains(&Action::Click)
        {
            node.access.actions.push(Action::Click);
        }
        Ok(())
    }

    /// M4 Phase 6 (§7.3, §16.2): registers `callback` to run when this
    /// node becomes the hovered node -- `Tree::dispatch`'s own
    /// `DispatchOutcome::HoverChanged`, fired independent of whether
    /// this node ever opted into `InteractionState`'s own visual
    /// animation (`enable_interaction`, M4 Phase 5) -- §7.3's own text:
    /// "the default MD3 visual never depends on anything handling it."
    pub(crate) fn set_on_hover_enter(&self, callback: Py<PyAny>, py: Python<'_>) -> PyResult<()> {
        crate::dispatch::register_handler(
            &self.handlers,
            (self.id, EventKind::HoverEnter),
            callback,
            py,
        )
    }

    /// The `HoverExit` counterpart to `set_on_hover_enter` -- fired when
    /// this node stops being the hovered node.
    pub(crate) fn set_on_hover_exit(&self, callback: Py<PyAny>, py: Python<'_>) -> PyResult<()> {
        crate::dispatch::register_handler(
            &self.handlers,
            (self.id, EventKind::HoverExit),
            callback,
            py,
        )
    }

    /// M14 Phase 3 (§16.7): registers `callback` to run on this node's
    /// own real `Change` -- a `Slider` drag genuinely ending (fired
    /// through the ordinary `Tree::dispatch` -> `DispatchOutcome::
    /// Changed` -> `run_dispatch_outcome` path, the same as `Click`/
    /// `HoverEnter`/`HoverExit`), or `Node.set_checked` being called on
    /// a `Checkbox` (fired directly there, not through `Tree::dispatch`
    /// at all -- see `set_checked`'s own doc comment for why). The real
    /// mechanism §16.7's own two-way binding sugar is built on.
    pub(crate) fn set_on_change(&self, callback: Py<PyAny>, py: Python<'_>) -> PyResult<()> {
        crate::dispatch::register_handler(
            &self.handlers,
            (self.id, EventKind::Change),
            callback,
            py,
        )
    }

    /// M55 (§10, §16.2): registers `callback` to run when this node
    /// becomes the keyboard-focused node -- real click-to-focus,
    /// Tab/Shift-Tab navigation, a real `Window.focus()`/`View.focus()`
    /// call, or a real AccessKit `Action::Focus` request, all firing
    /// through `dispatch::fire_focus_transition`, the same shared
    /// mechanism regardless of which real source caused it.
    pub(crate) fn set_on_focus_enter(&self, callback: Py<PyAny>, py: Python<'_>) -> PyResult<()> {
        crate::dispatch::register_handler(
            &self.handlers,
            (self.id, EventKind::FocusEnter),
            callback,
            py,
        )
    }

    /// The `FocusExit` counterpart to `set_on_focus_enter` -- fired
    /// when this node stops being the focused node.
    pub(crate) fn set_on_focus_exit(&self, callback: Py<PyAny>, py: Python<'_>) -> PyResult<()> {
        crate::dispatch::register_handler(
            &self.handlers,
            (self.id, EventKind::FocusExit),
            callback,
            py,
        )
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

    /// M13 Phase 2 (§11.2): the one missing half `add_child` already
    /// had a counterpart for at the `engine-core` level (`Tree::remove`,
    /// real since §5) but never a Python-facing one -- "navigating"
    /// (§11.2's own text) means replacing `content`'s own children, an
    /// ordinary remove-then-add, and `add_child` alone could only ever
    /// do the "add" half. Recursively removes this node and its whole
    /// subtree, unlinking it from its own parent first (`Tree::remove`'s
    /// own real behavior) -- no return value: a `Node` handle Python
    /// already holds always refers to a real, present `NodeId` at the
    /// point this is called, the same assumption every other `Node`
    /// method already makes, so `Tree::remove`'s own bare `bool` ("was
    /// it actually present") would be dead API surface here, not real
    /// information.
    fn remove(&self) {
        self.tree.borrow_mut().remove(self.id);
    }

    /// M14 Phase 1 (§5, §7.3): the real, plain (non-animated) write to
    /// `NodeKind::Checkbox`'s own `checked: bool` -- Design Principle 6
    /// ("selection/checked-state... depend on what the app's data
    /// means") is why the engine never flips this itself on click; the
    /// app's own `on_click` handler calls this, typically alongside
    /// `.animate("check_progress", ...)` for the real visual
    /// consequence, mirroring exactly how §7.3's own text separates the
    /// two ("checked-state... ordinary NodeKind-payload fields...
    /// animated through the same Animated<T> mechanism once set"). Also
    /// keeps the real accessibility tree correct for free -- `Tree::
    /// build_access_update` reads this same field directly, so there's
    /// nothing else to update.
    ///
    /// M14 Phase 3 (§16.7): also fires a real `Change` (`Node.set_on_
    /// change`'s own registered handler, if any) -- not mechanical the
    /// way a `Slider` drag ending is (`engine-core` never touches
    /// `checked` itself), so it fires directly here rather than through
    /// a `Tree::dispatch` outcome; this is the *only* place `checked`
    /// ever genuinely changes, so it's the one real place to fire from.
    pub(crate) fn set_checked(&self, checked: bool, py: Python<'_>) -> PyResult<()> {
        let mut tree = self.tree.borrow_mut();
        let node = tree.get_mut(self.id).expect(
            "Node holds a NodeId missing from its own Tree -- an engine-py bug, not a user error",
        );
        let kind = kind_name(&node.kind);
        match &mut node.kind {
            NodeKind::Checkbox(state) => {
                let old_checked = state.checked;
                state.checked = checked;
                drop(tree);
                let id = self.id;
                let ctx = NodeContext {
                    tree: &self.tree,
                    handlers: &self.handlers,
                    context_menus: &self.context_menus,
                    theme: &self.theme,
                    completions: &self.completions,
                };
                call_handler(&self.handlers, id, EventKind::Change, py, |py| {
                    Event::change(
                        py,
                        id,
                        &ctx,
                        Some(
                            old_checked
                                .into_pyobject(py)?
                                .to_owned()
                                .unbind()
                                .into_any(),
                        ),
                        Some(checked.into_pyobject(py)?.to_owned().unbind().into_any()),
                    )
                });
                Ok(())
            }
            _ => Err(EngineError::UnknownProperty {
                kind,
                property: "checked".to_string(),
            }
            .into()),
        }
    }

    /// M30 Phase 2 Step 1 (§8, §16.7): `set_checked`'s own real shape,
    /// mirrored exactly -- the engine never toggles `selected` itself
    /// (Design Principle 6), only reflects it once the app writes it,
    /// and always fires a real `Change` the same way.
    pub(crate) fn set_selected(&self, selected: bool, py: Python<'_>) -> PyResult<()> {
        let mut tree = self.tree.borrow_mut();
        let node = tree.get_mut(self.id).expect(
            "Node holds a NodeId missing from its own Tree -- an engine-py bug, not a user error",
        );
        let kind = kind_name(&node.kind);
        match &mut node.kind {
            NodeKind::RadioButton(_) | NodeKind::Switch(_) => {
                // M90: `Switch` shares `selected` with `RadioButton` (MD3's
                // own term for both); its state struct still calls it `on`.
                let old_selected = match &mut node.kind {
                    NodeKind::RadioButton(state) => {
                        std::mem::replace(&mut state.selected, selected)
                    }
                    NodeKind::Switch(state) => std::mem::replace(&mut state.on, selected),
                    _ => unreachable!(),
                };
                drop(tree);
                let id = self.id;
                let ctx = NodeContext {
                    tree: &self.tree,
                    handlers: &self.handlers,
                    context_menus: &self.context_menus,
                    theme: &self.theme,
                    completions: &self.completions,
                };
                call_handler(&self.handlers, id, EventKind::Change, py, |py| {
                    Event::change(
                        py,
                        id,
                        &ctx,
                        Some(
                            old_selected
                                .into_pyobject(py)?
                                .to_owned()
                                .unbind()
                                .into_any(),
                        ),
                        Some(selected.into_pyobject(py)?.to_owned().unbind().into_any()),
                    )
                });
                Ok(())
            }
            _ => Err(EngineError::UnknownProperty {
                kind,
                property: "selected".to_string(),
            }
            .into()),
        }
    }

    /// M15 Phase 2 (§8, §16.7): the plain, non-animated, programmatic
    /// write `set_checked`'s own real shape mirrors exactly (including
    /// always firing a real `Change`, the same established convention
    /// `apply_binding_value`'s own forward-bind path will need for a
    /// real `two_way: text` round trip -- `Signal`'s own change-
    /// detection, M14 Phase 3, already protects against a feedback
    /// loop here too, no new fix needed). `cursor` resets to the new
    /// content's own real end, the same "fresh content, fresh cursor"
    /// convention `TextFieldState::new` already establishes -- an old
    /// byte offset could land mid-character or past the new content's
    /// own end otherwise.
    pub(crate) fn set_text(&self, content: &str, py: Python<'_>) -> PyResult<()> {
        let mut tree = self.tree.borrow_mut();
        let node = tree.get_mut(self.id).expect(
            "Node holds a NodeId missing from its own Tree -- an engine-py bug, not a user error",
        );
        let kind = kind_name(&node.kind);
        match &mut node.kind {
            NodeKind::TextField(state) => {
                let old_content = state.content.clone();
                state.content = content.to_string();
                state.cursor = state.content.len();
                state.selection_anchor = None;
                drop(tree);
                let id = self.id;
                let new_content = content.to_string();
                let ctx = NodeContext {
                    tree: &self.tree,
                    handlers: &self.handlers,
                    context_menus: &self.context_menus,
                    theme: &self.theme,
                    completions: &self.completions,
                };
                call_handler(&self.handlers, id, EventKind::Change, py, |py| {
                    Event::change(
                        py,
                        id,
                        &ctx,
                        Some(old_content.into_pyobject(py)?.unbind().into_any()),
                        Some(new_content.into_pyobject(py)?.unbind().into_any()),
                    )
                });
                Ok(())
            }
            // M27 Phase 4: a real, genuine gap found while building the
            // showcase demo's data screen -- `Window.add_text` (M27
            // Phase 2) creates a real, plain `NodeKind::Text` label,
            // but nothing could ever update one's own content
            // afterward (`set_text` only ever handled `TextField`,
            // confirmed by a real `ValueError` from actually calling
            // it, not assumed). A plain label has no cursor/selection
            // concept and isn't interactive, so this arm is simpler
            // than `TextField`'s own real one -- no cursor reset, no
            // `Change` fired (nothing has ever registered a change
            // handler on a label, since it has no real `Change` source
            // of its own the way a `TextField` edit or `Checkbox`
            // toggle does).
            NodeKind::Text(state) => {
                state.content = content.to_string();
                Ok(())
            }
            _ => Err(EngineError::UnknownProperty {
                kind,
                property: "text".to_string(),
            }
            .into()),
        }
    }

    /// M30 Phase 9 Step 1 (§5): `Video`'s own real update, on whatever
    /// `Window.add_video`-created `Node` the app is displaying a live
    /// stream through. **Real, deliberate reuse of `NodeKind::Image`
    /// directly, not a new `NodeKind`** -- grounded in real, directly-
    /// applicable precedent from the sibling `pyCopper` project's own
    /// `Video` widget (same author, same explicit desktop-only design
    /// goal, confirmed via direct source read): `Video` is a **frame
    /// sink**, not a decoder -- nothing in this codebase depends on a
    /// codec library, and adding one (realistically PyAV, wrapping
    /// FFmpeg) would mean taking on its install size and licensing
    /// considerations for every TRE application, not just the ones
    /// that show video. The application decodes however it likes
    /// (PyAV, OpenCV, a camera driver, frames generated on the fly)
    /// and calls `push_frame(rgba, width, height)` at whatever cadence
    /// it decides; this method only replaces what the node currently
    /// displays. `rgba` is straight-alpha 8-bit RGBA pixels, the
    /// identical real convention `Window.add_image`'s own `image::
    /// open(...).to_rgba8()` already produces -- `len` must be exactly
    /// `width * height * 4`, a real, clear `PyValueError` otherwise
    /// (`add_icon`'s own established "fail loudly on pure validation,
    /// no I/O involved" convention, not routed through `EngineError`).
    ///
    /// **Real, confirmed architecture gap found and fixed alongside
    /// this method, not silently missed:** `engine-render`'s own
    /// `ImageTextureCache::sync` originally uploaded a real `Image`
    /// node's GPU texture exactly once, keyed only on node presence --
    /// correct when `add_image`'s own pixel data never changed after
    /// creation (true before this phase), but would have silently kept
    /// painting a video's very first frame forever otherwise. Fixed at
    /// the render layer (`image_cache.rs`'s own doc comment) to key
    /// re-upload on real content identity instead.
    ///
    /// Unlike pyCopper's own `Video` (which lays out `0x0` until the
    /// first frame arrives, since its own `Image` intrinsically sizes
    /// to its decoded pixel dimensions), a TRE `Image`/`Video` node's
    /// box is always the real, fixed `width`/`height` `add_video` was
    /// given -- `ImageState.content_fit` already resolves any mismatch
    /// between that box and a pushed frame's own real pixel dimensions
    /// at paint time, so a resolution change between frames needs no
    /// layout involvement at all here, only a real, ordinary content
    /// replacement.
    fn push_frame(&self, rgba: Vec<u8>, width: u32, height: u32) -> PyResult<()> {
        validate_rgba_frame_len("push_frame", rgba.len(), width, height)?;

        let mut tree = self.tree.borrow_mut();
        let node = tree.get_mut(self.id).expect(
            "Node holds a NodeId missing from its own Tree -- an engine-py bug, not a user error",
        );
        let kind = kind_name(&node.kind);
        match &mut node.kind {
            NodeKind::Image(state) => {
                state.image = peniko::ImageData {
                    data: peniko::Blob::from(rgba),
                    format: peniko::ImageFormat::Rgba8,
                    alpha_type: peniko::ImageAlphaType::Alpha,
                    width,
                    height,
                };
                Ok(())
            }
            _ => Err(EngineError::UnknownProperty {
                kind,
                property: "push_frame".to_string(),
            }
            .into()),
        }
    }

    /// M14 Phase 3 (§16.7): the missing read-back half of `set_checked`
    /// -- real two-way binding sugar needs to read a `Checkbox`'s own
    /// current `checked` to write it back into a bound `Signal` on a
    /// real `Change`; `check_progress` (the animated visual half) was
    /// already readable via `Node.get`, but `checked` itself (a plain
    /// `bool`, not an `f64` `Animated<T>` field `get`'s own real
    /// contract returns) needed its own dedicated getter.
    pub(crate) fn get_checked(&self) -> PyResult<bool> {
        let tree = self.tree.borrow();
        let node = tree.get(self.id).expect(
            "Node holds a NodeId missing from its own Tree -- an engine-py bug, not a user error",
        );
        match &node.kind {
            NodeKind::Checkbox(state) => Ok(state.checked),
            _ => Err(EngineError::UnknownProperty {
                kind: kind_name(&node.kind),
                property: "checked".to_string(),
            }
            .into()),
        }
    }

    /// M30 Phase 2 Step 1 (§5, §16.7): `get_checked`'s own real shape,
    /// mirrored exactly.
    pub(crate) fn get_selected(&self) -> PyResult<bool> {
        let tree = self.tree.borrow();
        let node = tree.get(self.id).expect(
            "Node holds a NodeId missing from its own Tree -- an engine-py bug, not a user error",
        );
        match &node.kind {
            NodeKind::RadioButton(state) => Ok(state.selected),
            NodeKind::Switch(state) => Ok(state.on),
            _ => Err(EngineError::UnknownProperty {
                kind: kind_name(&node.kind),
                property: "selected".to_string(),
            }
            .into()),
        }
    }

    /// M30 Phase 9 Step 5 (§5, §7, §11.7): moves a real `NodeKind::
    /// Carousel` to `index`, starting (or retargeting) its own real
    /// eased snap -- a thin real wrapper around `Tree::set_carousel_
    /// index`, the same "engine-core owns the mechanism, this is just
    /// the real Python entry point" shape `set_splitter_position`'s own
    /// real Python callers already use elsewhere. `index` is a plain
    /// `usize`, not an `f64` `Animated<T>` value -- the identical real
    /// reason `checked`/`selected`/`on` each needed their own dedicated
    /// setter instead of the generic `Node.animate()`.
    pub(crate) fn set_carousel_index(&self, index: usize) -> PyResult<()> {
        let mut tree = self.tree.borrow_mut();
        let kind = kind_name(&tree.get(self.id).expect(
            "Node holds a NodeId missing from its own Tree -- an engine-py bug, not a user error",
        ).kind);
        if !matches!(
            tree.get(self.id).map(|n| &n.kind),
            Some(NodeKind::Carousel(_))
        ) {
            return Err(EngineError::UnknownProperty {
                kind,
                property: "index".to_string(),
            }
            .into());
        }
        tree.set_carousel_index(self.id, index, Instant::now());
        Ok(())
    }

    /// `set_carousel_index`'s own real read-back getter -- the item the
    /// carousel is *settling on* (its real destination, not necessarily
    /// where it's currently drawn mid-snap; see `get_carousel_position`
    /// for that).
    pub(crate) fn get_carousel_index(&self) -> PyResult<usize> {
        let tree = self.tree.borrow();
        let node = tree.get(self.id).expect(
            "Node holds a NodeId missing from its own Tree -- an engine-py bug, not a user error",
        );
        match &node.kind {
            NodeKind::Carousel(state) => Ok(state.index),
            _ => Err(EngineError::UnknownProperty {
                kind: kind_name(&node.kind),
                property: "index".to_string(),
            }
            .into()),
        }
    }

    /// The real, currently-animating strip position -- an integer at
    /// rest, fractional mid-snap. Exposed for the same real reason
    /// `thumb_position` is readable via `Node.get`: `position` isn't a
    /// plain `f64` `Animated<T>` field reachable through that generic
    /// mechanism here (it's `CarouselState`-specific, not universal),
    /// so it gets its own dedicated getter instead, matching `get_
    /// checked`'s own real precedent for a kind-specific field.
    pub(crate) fn get_carousel_position(&self) -> PyResult<f64> {
        let tree = self.tree.borrow();
        let node = tree.get(self.id).expect(
            "Node holds a NodeId missing from its own Tree -- an engine-py bug, not a user error",
        );
        match &node.kind {
            NodeKind::Carousel(state) => Ok(state.position.current),
            _ => Err(EngineError::UnknownProperty {
                kind: kind_name(&node.kind),
                property: "position".to_string(),
            }
            .into()),
        }
    }

    /// `Uncontained`'s own real free-scroll counterpart to `set_
    /// carousel_index` -- a thin wrapper around `Tree::set_carousel_
    /// scroll`.
    pub(crate) fn set_carousel_scroll(&self, value: f64) -> PyResult<()> {
        let mut tree = self.tree.borrow_mut();
        let kind = kind_name(&tree.get(self.id).expect(
            "Node holds a NodeId missing from its own Tree -- an engine-py bug, not a user error",
        ).kind);
        if !matches!(
            tree.get(self.id).map(|n| &n.kind),
            Some(NodeKind::Carousel(_))
        ) {
            return Err(EngineError::UnknownProperty {
                kind,
                property: "scroll_x".to_string(),
            }
            .into());
        }
        tree.set_carousel_scroll(self.id, value);
        Ok(())
    }

    /// `set_carousel_scroll`'s own real read-back getter.
    pub(crate) fn get_carousel_scroll(&self) -> PyResult<f64> {
        let tree = self.tree.borrow();
        let node = tree.get(self.id).expect(
            "Node holds a NodeId missing from its own Tree -- an engine-py bug, not a user error",
        );
        match &node.kind {
            NodeKind::Carousel(state) => Ok(state.scroll_x),
            _ => Err(EngineError::UnknownProperty {
                kind: kind_name(&node.kind),
                property: "scroll_x".to_string(),
            }
            .into()),
        }
    }

    /// M39 Phase 2 Step 2 (§5, §7): a real, direct programmatic move of
    /// a `NodeKind::TimePickerDial`'s own hand positions -- the same
    /// "engine-core owns the mechanism, this is just the real Python
    /// entry point" shape `set_carousel_index` already establishes. A
    /// thin wrapper around `Tree::set_time_picker_dial_time`, which
    /// itself clamps `hour`/`minute` into range -- this method adds no
    /// further validation of its own.
    pub(crate) fn set_time_picker_dial_time(&self, hour: u8, minute: u8) -> PyResult<()> {
        let mut tree = self.tree.borrow_mut();
        let kind = kind_name(
            &tree
                .get(self.id)
                .expect(
                    "Node holds a NodeId missing from its own Tree -- an engine-py bug, not a user error",
                )
                .kind,
        );
        if !matches!(
            tree.get(self.id).map(|n| &n.kind),
            Some(NodeKind::TimePickerDial(_))
        ) {
            return Err(EngineError::UnknownProperty {
                kind,
                property: "hour/minute".to_string(),
            }
            .into());
        }
        tree.set_time_picker_dial_time(self.id, hour, minute);
        Ok(())
    }

    /// `set_time_picker_dial_time`'s own real read-back getter.
    pub(crate) fn get_time_picker_dial_time(&self) -> PyResult<(u8, u8)> {
        let tree = self.tree.borrow();
        let node = tree.get(self.id).expect(
            "Node holds a NodeId missing from its own Tree -- an engine-py bug, not a user error",
        );
        match &node.kind {
            NodeKind::TimePickerDial(state) => Ok((state.hour, state.minute)),
            _ => Err(EngineError::UnknownProperty {
                kind: kind_name(&node.kind),
                property: "hour/minute".to_string(),
            }
            .into()),
        }
    }

    /// M39 Phase 2 Step 2 (§5, §7): switches which real hand a drag on
    /// this dial moves next -- the string-vocabulary convention
    /// `parse_content_fit`/`parse_dock_side` already establish for a
    /// small, closed real Rust enum exposed to Python, rather than a
    /// dedicated pyo3-native enum type for just these two variants.
    pub(crate) fn set_time_picker_dial_mode(&self, mode: &str) -> PyResult<()> {
        let mode = match mode {
            "hour" => TimePickerDialMode::Hour,
            "minute" => TimePickerDialMode::Minute,
            other => {
                return Err(pyo3::exceptions::PyValueError::new_err(format!(
                    "unknown time picker dial mode {other:?} -- expected \"hour\" or \"minute\""
                )));
            }
        };
        let mut tree = self.tree.borrow_mut();
        let kind = kind_name(
            &tree
                .get(self.id)
                .expect(
                    "Node holds a NodeId missing from its own Tree -- an engine-py bug, not a user error",
                )
                .kind,
        );
        if !matches!(
            tree.get(self.id).map(|n| &n.kind),
            Some(NodeKind::TimePickerDial(_))
        ) {
            return Err(EngineError::UnknownProperty {
                kind,
                property: "mode".to_string(),
            }
            .into());
        }
        tree.set_time_picker_dial_mode(self.id, mode);
        Ok(())
    }

    /// `set_time_picker_dial_mode`'s own real read-back getter.
    pub(crate) fn get_time_picker_dial_mode(&self) -> PyResult<&'static str> {
        let tree = self.tree.borrow();
        let node = tree.get(self.id).expect(
            "Node holds a NodeId missing from its own Tree -- an engine-py bug, not a user error",
        );
        match &node.kind {
            NodeKind::TimePickerDial(state) => Ok(match state.mode {
                TimePickerDialMode::Hour => "hour",
                TimePickerDialMode::Minute => "minute",
            }),
            _ => Err(EngineError::UnknownProperty {
                kind: kind_name(&node.kind),
                property: "mode".to_string(),
            }
            .into()),
        }
    }

    /// M15 Phase 1 (§5, §16.7): the real read-back getter for a
    /// `TextField`'s own current `content` -- mirrors `get_checked`'s
    /// own exact shape (rejecting a non-`TextField` node the same way).
    pub(crate) fn get_text(&self) -> PyResult<String> {
        let tree = self.tree.borrow();
        let node = tree.get(self.id).expect(
            "Node holds a NodeId missing from its own Tree -- an engine-py bug, not a user error",
        );
        match &node.kind {
            NodeKind::TextField(state) => Ok(state.content.clone()),
            // The real read-back counterpart to `set_text`'s own new
            // `NodeKind::Text` arm, above -- the identical real need
            // (verifying a label's content actually changed) surfaced
            // by the exact same phase.
            NodeKind::Text(state) => Ok(state.content.clone()),
            // Tesserae M27 (`tre` M84): `Link` wraps the identical
            // `TextState` `Text` does (`window_factory.rs`'s own real
            // `add_link` body confirms this directly) -- the same real
            // read-back need `Text`'s own arm above exists for applies
            // here too, previously missing entirely (Link had no text
            // read-back path from Python at all).
            NodeKind::Link(state) => Ok(state.content.clone()),
            // M30 Phase 9 Step 4 (§5, §8, §10): a real terminal's own
            // "text content" is its whole cell grid, not one string --
            // joined here row by row (`\n`-separated, each row's own
            // trailing spaces trimmed, the real, expected shape for a
            // test to assert against, e.g. `"hello" in node.get_text()`
            // after a real shell echoes it) purely for real read-back
            // testing, mirroring `Text`/`TextField`'s own established
            // "one plain string" contract rather than exposing the raw
            // per-cell color/bold data Python has no real use for yet.
            NodeKind::Terminal(state) => {
                let mut lines = Vec::with_capacity(usize::from(state.rows));
                for row in 0..state.rows {
                    let line: String = (0..state.cols).map(|col| state.cell(row, col).ch).collect();
                    lines.push(line.trim_end().to_string());
                }
                Ok(lines.join("\n"))
            }
            _ => Err(EngineError::UnknownProperty {
                kind: kind_name(&node.kind),
                property: "text".to_string(),
            }
            .into()),
        }
    }

    /// M15 Phase 1 (§10): the real, missing "is this node currently
    /// focused" query -- confirmed via grep, no Python-facing way to
    /// read `Tree::focused()` existed anywhere before this. Works for
    /// any `NodeKind`, not just `TextField` (§10's own focus model is
    /// generic), the same "no stricter rule for one kind than another"
    /// precedent `Tree::activate`/`set_focus_to` already state.
    fn is_focused(&self) -> bool {
        self.tree.borrow().focused() == Some(self.id)
    }

    /// M31 Phase 4 (§5, §8): sets a Code Editor's own real per-byte-
    /// range syntax coloring -- `spans` is a list of `(start, end,
    /// (r, g, b, a))` tuples, each naming a real byte range into
    /// `get_text()`'s own content and the real color to paint it.
    /// `TextField`-only, mirroring `set_checked`'s own real "raises
    /// for any other kind" contract. Replaces the whole list on every
    /// call -- an app re-tokenizing its own buffer on every real
    /// `Change` (Design Principle 6: app-side tokenization only, no
    /// engine-bundled lexer) is expected to call this again with the
    /// new spans each time, not diff them; `engine-core` never
    /// interprets these ranges itself, so overlapping or out-of-order
    /// spans are the app's own concern, not validated here.
    pub(crate) fn set_syntax_spans(&self, spans: Vec<SyntaxSpanInput>) -> PyResult<()> {
        let mut tree = self.tree.borrow_mut();
        let node = tree.get_mut(self.id).expect(
            "Node holds a NodeId missing from its own Tree -- an engine-py bug, not a user error",
        );
        let kind = kind_name(&node.kind);
        match &mut node.kind {
            NodeKind::TextField(state) => {
                state.syntax_spans = spans
                    .into_iter()
                    .map(|(start, end, (r, g, b, a))| (start..end, Color::from_rgba8(r, g, b, a)))
                    .collect();
                Ok(())
            }
            _ => Err(EngineError::UnknownProperty {
                kind,
                property: "syntax_spans".to_string(),
            }
            .into()),
        }
    }

    /// M31 Phase 5 (§5, §8): sets a Code Editor's own real, paint-only
    /// content folding -- `ranges` is a list of `(start, end)` tuples,
    /// each naming a real byte range of `get_text()`'s own content to
    /// collapse into one visible "⋯" marker. `TextField`-only, the
    /// identical real contract `set_syntax_spans` already has.
    /// Replaces the whole list on every call; `engine-core` never
    /// validates these ranges itself -- deciding *which* real lines
    /// are foldable/currently folded is the app's own concern (this
    /// codebase has no code-structure awareness of its own to decide
    /// that itself), the identical real "app's own concern" split
    /// `set_syntax_spans` already has for overlapping/out-of-order
    /// input. `Home`/`End`/`ArrowUp`/`ArrowDown` are fold-aware (M38
    /// Phase 3): a real cursor move that would land strictly inside a
    /// folded range snaps forward to right after that fold's own real
    /// marker instead, the identical clamp `engine-render`'s own paint
    /// code already applies to the *displayed* caret
    /// (`TextFieldState.folded_ranges`'s own doc comment has the full
    /// real reasoning).
    pub(crate) fn set_folded_ranges(&self, ranges: Vec<(usize, usize)>) -> PyResult<()> {
        let mut tree = self.tree.borrow_mut();
        let node = tree.get_mut(self.id).expect(
            "Node holds a NodeId missing from its own Tree -- an engine-py bug, not a user error",
        );
        let kind = kind_name(&node.kind);
        match &mut node.kind {
            NodeKind::TextField(state) => {
                state.folded_ranges = ranges.into_iter().map(|(start, end)| start..end).collect();
                Ok(())
            }
            _ => Err(EngineError::UnknownProperty {
                kind,
                property: "folded_ranges".to_string(),
            }
            .into()),
        }
    }

    /// M32 Phase 3 (§5, §7, §11.7/§11.8): opts this node into clipping
    /// its own real children to its own box -- the real, general form
    /// of the clip `VirtualList`/`Carousel` each already have built
    /// into their own paint, closing "no `NodeKind` besides
    /// `VirtualList` clips its own children today" (`Code Editor`'s
    /// own stated gap, M30 Phase 9 Step 3). Universal, not `NodeKind`-
    /// specific, unlike `set_syntax_spans`/`set_folded_ranges` just
    /// above -- `PaintProperties.clip_children` lives on every real
    /// node already, the identical "no per-kind rejection needed"
    /// shape `set_corner_radius`/`set_border` already have. **Real,
    /// honest v1 limit, not silently glossed over:** clipping only --
    /// this does not give a container real scroll input or a scroll
    /// offset of its own; content past the box is genuinely hidden,
    /// not scrollable into view, the identical real limit `PaintProperties.
    /// clip_children`'s own Rust doc comment states.
    fn set_clip_children(&self, clip: bool) {
        self.tree.borrow_mut().get_mut(self.id).expect(
            "Node holds a NodeId missing from its own Tree -- an engine-py bug, not a user error",
        ).paint.clip_children = clip;
    }

    /// M32 Phase 6 (§4, §5, §8): a real, direct way to seed a
    /// `Terminal`'s own selection without a live mouse drag -- the
    /// identical "raw setter, no event simulation needed" shape
    /// `set_syntax_spans`/`set_folded_ranges` above already establish
    /// for `TextField`. `(start_row, start_col)`/`(end_row, end_col)`
    /// are real cell coordinates, normalized/clamped by `Tree::
    /// terminal_selected_text`/`draw_terminal` themselves -- this
    /// setter performs no validation of its own, mirroring `set_
    /// syntax_spans`'s own real "app's own concern" contract. Raises
    /// `ValueError` for any other kind.
    pub(crate) fn set_terminal_selection(
        &self,
        start_row: u16,
        start_col: u16,
        end_row: u16,
        end_col: u16,
    ) -> PyResult<()> {
        let mut tree = self.tree.borrow_mut();
        let node = tree.get_mut(self.id).expect(
            "Node holds a NodeId missing from its own Tree -- an engine-py bug, not a user error",
        );
        let kind = kind_name(&node.kind);
        match &mut node.kind {
            NodeKind::Terminal(state) => {
                state.selection_start = Some((start_row, start_col));
                state.selection_end = Some((end_row, end_col));
                Ok(())
            }
            _ => Err(EngineError::UnknownProperty {
                kind,
                property: "terminal_selection".to_string(),
            }
            .into()),
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
    matches!(
        kind,
        NodeKind::Text(_) | NodeKind::Link(_) | NodeKind::Icon(_) | NodeKind::LoadingIndicator(_)
    )
}

fn parse_flex_direction(value: &str) -> PyResult<FlexDirection> {
    match value {
        "horizontal" => Ok(FlexDirection::Row),
        "vertical" => Ok(FlexDirection::Column),
        other => Err(pyo3::exceptions::PyValueError::new_err(format!(
            "set_layout: unknown flex_direction {other:?} -- expected one of \"horizontal\", \"vertical\""
        ))),
    }
}

fn parse_align_items(value: &str) -> PyResult<AlignItems> {
    match value {
        "start" => Ok(AlignItems::START),
        "end" => Ok(AlignItems::END),
        "flex_start" => Ok(AlignItems::FLEX_START),
        "flex_end" => Ok(AlignItems::FLEX_END),
        "center" => Ok(AlignItems::CENTER),
        "baseline" => Ok(AlignItems::BASELINE),
        "stretch" => Ok(AlignItems::STRETCH),
        other => Err(pyo3::exceptions::PyValueError::new_err(format!(
            "set_layout: unknown align_items {other:?} -- expected one of \"start\", \"end\", \
             \"flex_start\", \"flex_end\", \"center\", \"baseline\", \"stretch\""
        ))),
    }
}

/// `parse_align_items`'s own real `justify_content=` sibling -- a
/// superset vocabulary (adds the real space-distribution keywords
/// `align_items` doesn't have), mirroring `engine-spec::
/// JustifyContentSpec`'s identical real shape.
fn parse_justify_content(value: &str) -> PyResult<JustifyContent> {
    match value {
        "start" => Ok(JustifyContent::START),
        "end" => Ok(JustifyContent::END),
        "flex_start" => Ok(JustifyContent::FLEX_START),
        "flex_end" => Ok(JustifyContent::FLEX_END),
        "center" => Ok(JustifyContent::CENTER),
        "stretch" => Ok(JustifyContent::STRETCH),
        "space_between" => Ok(JustifyContent::SPACE_BETWEEN),
        "space_around" => Ok(JustifyContent::SPACE_AROUND),
        "space_evenly" => Ok(JustifyContent::SPACE_EVENLY),
        other => Err(pyo3::exceptions::PyValueError::new_err(format!(
            "set_layout: unknown justify_content {other:?} -- expected one of \"start\", \"end\", \
             \"flex_start\", \"flex_end\", \"center\", \"stretch\", \"space_between\", \
             \"space_around\", \"space_evenly\""
        ))),
    }
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
        NodeKind::Splitter(_) => "Splitter",
        NodeKind::VirtualList(_) => "VirtualList",
        NodeKind::Canvas(_) => "Canvas",
        NodeKind::Checkbox(_) => "Checkbox",
        NodeKind::RadioButton(_) => "RadioButton",
        NodeKind::Switch(_) => "Switch",
        NodeKind::LinearProgress(_) => "LinearProgress",
        NodeKind::CircularProgress(_) => "CircularProgress",
        NodeKind::Slider(_) => "Slider",
        NodeKind::TextField(_) => "TextField",
        NodeKind::Image(_) => "Image",
        NodeKind::Icon(_) => "Icon",
        NodeKind::Link(_) => "Link",
        NodeKind::Terminal(_) => "Terminal",
        NodeKind::Carousel(_) => "Carousel",
        NodeKind::ScrollView(_) => "ScrollView",
        NodeKind::LoadingIndicator(_) => "LoadingIndicator",
        NodeKind::TimePickerDial(_) => "TimePickerDial",
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
