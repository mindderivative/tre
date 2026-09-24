//! §16.2's real Python-facing entry point: `View` loads a `view.yaml`
//! into its own `Tree`, and `View._attach(viewmodel)` is "the one place
//! the inversion resolves" -- handlers validated eagerly by name,
//! bindings resolved once and re-evaluated automatically whenever a
//! `Signal` they read from is written to (real dependency tracking: a
//! binding's evaluation runs inside a recording scope, and only the
//! `Signal`s it actually read get subscribed).
//!
//! Scoped narrower than §16.2's full picture in two stated ways:
//!
//! - **Updated, M4 Phase 4:** `on_click` handlers are now wired for
//!   real, not just eagerly validated -- `_attach` registers the
//!   validated method into the same `handlers`/`Tree::dispatch`
//!   mechanism `Node.set_on_click` already uses (M4 Phase 1 step 3),
//!   and `View.click(node)` (below) proves it fires without needing a
//!   live window, the same pattern `Window.click` established.
//! - **Updated, M4 Phase 6:** `on_hover_enter`/`on_hover_exit` are now
//!   also wired the same way, through the new `EventKind`/
//!   `DispatchOutcome::HoverChanged` mechanism (§7.3) -- `_attach`'s
//!   single `on_click`-only special case generalized into a small match
//!   over the three real event kinds that exist today. Other declared
//!   event names still validate but reach no real mechanism, matching
//!   §16.2's own "generalizing to whatever named events a `NodeKind`
//!   exposes" -- not manufactured ahead of a real need.
//! - **Updated, M42 Phase 1 (§4, §5, §8, §16.2, §16.4):** `View` can now
//!   be shown in a real, live, `winit`-driven window --
//!   `crate::window::PyWindow::from_view` shares this `View`'s own
//!   `Rc<RefCell<Tree>>`, root, `handlers`/`context_menus`, and (new
//!   this phase) persistent `theme`/`completions`/`width`/`height`
//!   fields directly into a real `Window`, the same `Rc`-clone pattern
//!   `PyWindow::wrap_node` already uses for every `Node` it hands out.
//!   `click()`/`hover()`/`right_click()` below now lay out against the
//!   real `Definite` size once a `View` has been shown this way (see
//!   `available_space()`), falling back to the original `MaxContent`
//!   behavior -- byte-for-byte unchanged -- for a `View` never shown
//!   live. Live theme-switching, `on_complete` callbacks firing on a
//!   View-sourced node's live render loop, and hot-reload *while shown*
//!   remain real, separate, stated gaps -- not silently dropped, just
//!   not this phase's own scope (see `BUILD_TRACKER.md` M42).
//! - Binding application supports `opacity`/`corner_radius` -- the two
//!   numeric `Animated<f64>` properties a resolved `engine_spec::
//!   Value::Int`/`Float` maps onto directly through the existing
//!   `Node::animate` dispatch (reused verbatim, not reimplemented).
//!   `background` needs a color-string (or MD3-token) parse a bound
//!   value hasn't gone through yet -- real, additive work for whenever
//!   a binding actually needs a dynamic color, not manufactured ahead
//!   of that need.
//! - A binding's dependency set is captured once, at its first
//!   evaluation (attach time), not re-tracked on every re-evaluation --
//!   correct for every binding shape §16.2's own examples show (a
//!   `Signal.get()` read unconditionally), and the same "simplest thing
//!   that works, revisit if a real case needs more" calibration this
//!   project applies throughout.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use engine_core::{EventKind, InputEvent, NodeId, PointerButton, Tree};
use engine_md3::{ColorScheme, DynamicTheme};
use engine_spec::{
    Expression, Reconciler, Stylesheet, ThemeSpec, ViewWatcher, WidgetSpec, evaluate,
    parse_binding, parse_stylesheet, parse_theme, parse_view_with_includes,
};
use peniko::Color;
use pyo3::IntoPyObjectExt;
use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use taffy::prelude::{AvailableSpace, Size};

use crate::binding::PyViewModelResolver;
use crate::dispatch::CompletionRegistry;
use crate::dispatch::{
    HandlerMap, SharedCompletions, interaction_config, node_center, open_context_menu,
    run_dispatch_outcome,
};
use crate::node::Node;
use crate::window::{SharedSize, SharedTheme, ThemeState};

thread_local! {
    /// M45 (§16.2): a real *stack* of recording frames, not a single
    /// flat slot -- `_begin_recording`/`_end_recording` push/pop a
    /// frame each, and `_record_read` only ever touches the top one.
    /// **Real, load-bearing correctness fix, not a hypothetical
    /// hardening:** the binding grammar (`engine_spec::binding::
    /// Expression::Call`) already permits a zero-arg method call with
    /// real side effects -- `{{ vm.trigger().get() }}` could call a
    /// method that does `some_signal.set(...)`, firing `_notify()`
    /// *synchronously* while the outer binding's own recording scope is
    /// still open. Before this fix (a flat `Option<Vec<...>>`), any
    /// subscriber that opened its *own* nested recording scope from
    /// inside that `_notify()` call -- exactly what M45's own `Computed`
    /// /`Effect` do -- would silently destroy the outer scope's already-
    /// recorded dependencies (the inner `end_recording` cleared the
    /// whole slot to `None`), permanently losing that binding's own
    /// reactivity with no error at all. Unreachable before M45 (nothing
    /// previously opened a nested recording scope), but `Computed`/
    /// `Effect`'s own eager re-tracking is exactly the mechanism that
    /// would first make it reachable, so this is fixed as a real
    /// prerequisite here, not deferred.
    static RECORDING: RefCell<Vec<Vec<Py<PyAny>>>> = const { RefCell::new(Vec::new()) };
}

/// Called from Python's `Signal.get()` on every read. If a binding
/// evaluation is currently recording (see `_begin_recording`/`_end_
/// recording`), records this signal as one of its dependencies -- by
/// object identity, deduplicated, since one binding can legitimately
/// read the same `Signal` more than once in a single evaluation. Only
/// ever touches the *top* frame of the stack -- a nested recording
/// scope (M45's `Computed`/`Effect`) never disturbs an outer one.
#[pyfunction]
pub(crate) fn _record_read(py: Python<'_>, signal: Py<PyAny>) {
    RECORDING.with(|cell| {
        if let Some(list) = cell.borrow_mut().last_mut() {
            let already_present = list.iter().any(|s| s.bind(py).is(signal.bind(py)));
            if !already_present {
                list.push(signal);
            }
        }
    });
}

/// M45 (§16.2): `pub(crate)` + `#[pyfunction]`, not private -- `Computed`
/// /`Effect`/`untrack` (`python/tre/__init__.py`) need to open/close a
/// recording scope around their own `fn` the same way a YAML binding's
/// evaluation already does via `attach_bindings_and_handlers` (which
/// calls this directly, not just through the Python wrapper -- a
/// `#[pyfunction]`-annotated fn stays a normal, directly-callable Rust
/// item under its own name, the same pattern already proven safe by
/// this file's own `_record_read`, called only from Python, and this
/// pair, called from both sides).
#[pyfunction]
pub(crate) fn _begin_recording() {
    RECORDING.with(|cell| cell.borrow_mut().push(Vec::new()));
}

/// The `_begin_recording` counterpart -- pops the top frame and returns
/// it. **`untrack(fn)` (Python) needs no third primitive:** calling this
/// pair around `fn()` and discarding the returned list already hides
/// `fn`'s own reads from whatever frame is beneath it on the stack,
/// since `_record_read` only ever touches the top -- exactly `untrack`'s
/// contract, for free.
#[pyfunction]
pub(crate) fn _end_recording() -> Vec<Py<PyAny>> {
    RECORDING.with(|cell| cell.borrow_mut().pop().unwrap_or_default())
}

/// Real review finding: `apply_binding_value` and `TwoWayCallback::
/// __call__` each built an identical throwaway `Node` -- real `tree`/
/// `id`, but empty/fresh `context_menus`/`theme`/`completions` (never
/// exposed to Python beyond that one call, so nothing real is lost by
/// sharing none of `View`'s own persistent ones) -- differing only in
/// which `handlers` map to give it. Factored out once.
fn throwaway_node(tree: &Rc<RefCell<Tree>>, id: NodeId, handlers: HandlerMap) -> Node {
    Node {
        id,
        tree: tree.clone(),
        handlers,
        context_menus: Rc::new(RefCell::new(HashMap::new())),
        theme: Rc::new(RefCell::new(ThemeState::default())),
        completions: Rc::new(RefCell::new(CompletionRegistry::new())),
    }
}

/// Parses a bound `background:` string the same way `engine_spec::
/// build::resolve_color` parses a *static* YAML color -- hex (`#rrggbb`/
/// `#rrggbbaa`) or a CSS-named color, via `peniko::color::parse_color`
/// -- returning the `(r, g, b, a)` u8 tuple `Node::animate`'s own
/// `extract_color` already accepts for `background` imperatively.
///
/// Returns a plain `Result<_, String>`, not a `PyResult` -- deliberately
/// keeps this function 100% free of `pyo3`/GIL concerns (it never
/// touches `Python<'_>` or `PyErr`) so it gets a real, unconditional
/// Rust unit test below (no interpreter to initialize, no ambiguity
/// about whether `PyErr::to_string()` itself needs the GIL) rather than
/// only indirect pytest coverage -- matching this crate's own
/// established split (§16.2). The caller (`apply_binding_value`, right
/// below) wraps the `Err` string into a real `PyValueError` at its own
/// call site, where a GIL token is already in scope. MD3 theme-role
/// token strings (e.g. `"primary"`) are explicitly not handled here:
/// that needs a live `ColorScheme`, which `apply_binding_value`'s own
/// `throwaway_node` doesn't carry -- a real, named, deferred follow-up,
/// not silently unsupported.
pub(crate) fn parse_background_color(raw: &str) -> Result<(u8, u8, u8, u8), String> {
    let color = peniko::color::parse_color(raw)
        .map(|c| c.to_alpha_color::<peniko::color::Srgb>())
        .map_err(|e| format!("{raw:?} isn't a valid color: {e}"))?;
    let [r, g, b, a] = color.to_rgba8().to_u8_array();
    Ok((r, g, b, a))
}

/// M49 Phase 4: the engine's own shipped default theme (`M49 Phase 4`'s
/// own `default_theme.yaml` doc comment) -- embedded at compile time,
/// not read from the installed package's own filesystem location at
/// runtime, so it can never go missing/stale relative to the compiled
/// extension. Loaded whenever a caller's `default_theme` param is
/// `None`, both here (`View::new`) and in `Window.set_theme`
/// (`window.rs`).
pub(crate) const SHIPPED_DEFAULT_THEME_YAML: &str = include_str!("../assets/default_theme.yaml");

/// M49 Phase 4: reads and parses one theme YAML file -- shared by
/// `View::new`'s `default_theme`/`custom_theme` params and `Window.
/// set_theme`'s `custom_theme` param, rather than three independent
/// copies of "read this file, call `parse_theme`, wrap the errors."
pub(crate) fn load_theme_spec(path: &str) -> PyResult<ThemeSpec> {
    let yaml = std::fs::read_to_string(path)
        .map_err(|e| PyRuntimeError::new_err(format!("failed to read theme {path:?}: {e}")))?;
    parse_theme(&yaml).map_err(|e| PyValueError::new_err(e.to_string()))
}

/// M49 Phase 4: a `ThemeSpec`'s `styles:` section, wrapped as a
/// `Stylesheet` -- the exact shape `resolve_style_layered`'s own
/// `default_theme`/`custom_theme` parameters expect (`cascade.rs`).
pub(crate) fn theme_spec_to_stylesheet(theme: &ThemeSpec) -> Stylesheet {
    Stylesheet {
        styles: theme.styles.clone(),
    }
}

/// M49 Phase 4: a `ThemeSpec`'s own optional `seed:` color string,
/// parsed the same real hex/CSS way every other color string in this
/// codebase already is -- `None` when the theme doesn't name its own
/// seed (the caller's own directly-passed seed, if any, applies
/// instead).
pub(crate) fn theme_spec_seed(theme: &ThemeSpec) -> PyResult<Option<(u8, u8, u8, u8)>> {
    theme
        .seed
        .as_deref()
        .map(|raw| {
            parse_background_color(raw)
                .map_err(|e| PyValueError::new_err(format!("theme seed color: {e}")))
        })
        .transpose()
}

/// M51: `View::new`'s own real theme-resolution logic (default_theme/
/// custom_theme -> the `(default_sheet, custom_sheet, scheme)` triple
/// `build_tree`/`patch_node`/`retheme` all need), factored out once a
/// second real call site (`View.set_theme`, below) needed the identical
/// logic -- the same "two real call sites justify factoring out"
/// precedent `resolve_button_colors`/`fill_scrollbar_thumb` already
/// established elsewhere in this codebase. `default_theme` omitted
/// loads the engine's own shipped default; seed precedence: explicit
/// `theme_seed` > `custom_theme`'s own `seed:` > `default_theme`'s.
/// `colors:` overrides apply to whichever scheme that precedence
/// resolves to, default's first then custom's.
fn resolve_theme_layers(
    default_theme: Option<&str>,
    custom_theme: Option<&str>,
    theme_seed: Option<(u8, u8, u8, u8)>,
    dark: bool,
) -> PyResult<(Stylesheet, Option<Stylesheet>, Option<ColorScheme>)> {
    let default_theme_spec = match default_theme {
        Some(theme_path) => load_theme_spec(theme_path)?,
        None => parse_theme(SHIPPED_DEFAULT_THEME_YAML)
            .expect("the engine's own shipped default_theme.yaml must always parse"),
    };
    let custom_theme_spec = custom_theme.map(load_theme_spec).transpose()?;

    let default_theme_sheet = theme_spec_to_stylesheet(&default_theme_spec);
    let custom_theme_sheet = custom_theme_spec.as_ref().map(theme_spec_to_stylesheet);

    let default_seed = theme_spec_seed(&default_theme_spec)?;
    let custom_seed = match &custom_theme_spec {
        Some(t) => theme_spec_seed(t)?,
        None => None,
    };
    let effective_seed = theme_seed.or(custom_seed).or(default_seed);

    let mut scheme = effective_seed.map(|(r, g, b, a)| {
        let theme = DynamicTheme::from_seed(Color::from_rgba8(r, g, b, a));
        if dark { theme.dark } else { theme.light }
    });
    if let Some(scheme) = scheme.as_mut() {
        if !default_theme_spec.colors.is_empty() {
            scheme
                .apply_overrides(&default_theme_spec.colors)
                .map_err(PyValueError::new_err)?;
        }
        if let Some(custom) = &custom_theme_spec
            && !custom.colors.is_empty()
        {
            scheme
                .apply_overrides(&custom.colors)
                .map_err(PyValueError::new_err)?;
        }
    }
    Ok((default_theme_sheet, custom_theme_sheet, scheme))
}

/// Applies a resolved binding value to `node_id`'s corresponding
/// property by constructing a temporary `Node` and reusing its own
/// `animate` dispatch (§8) verbatim -- the exact same property-name
/// validation and type-mismatch errors a `node.animate(...)` call
/// already gives, not a second, parallel dispatch implementation.
///
/// **Real finding, not obvious from `animate`'s own contract:**
/// `animate(property, value, duration_ms=0)` only *registers* a
/// zero-duration `ActiveAnimation` (§5's `animate_to` never eagerly
/// writes `current` itself) -- it only actually snaps to the target
/// value the next time something ticks this node, which is normally
/// the render loop's own per-frame `Tree::tick_all` call. A `View` with
/// no running render loop attached (this crate's own real scope today
/// -- see this module's doc comment) never calls that, so a binding's
/// "instant" value would otherwise never actually become observable.
/// Ticking immediately after registering makes `duration_ms=0` mean
/// what it says regardless of whether a frame loop happens to be
/// running, rather than only working correctly by accident once one
/// eventually is.
///
/// **M44 (§16.2):** dispatch is now *property-name-first*, not
/// value-type-first. The old version keyed entirely off `value`'s own
/// runtime `Value` variant (`Bool` -> unconditionally `set_checked`,
/// `Str` -> unconditionally `set_text`, regardless of what `property`
/// actually named) -- real bug: a `background:` binding resolving to a
/// `Str` (a hex color) was silently misrouted into `set_text`, and any
/// binding resolving to a non-primitive Python value (`Value::Handle`,
/// e.g. an `(r,g,b,a)` tuple) was rejected outright even though `Node.
/// animate` already accepts exactly that shape for `background`/
/// `transform`/`shape` imperatively. `checked`/`text` are still the two
/// genuinely non-numeric properties that bypass `animate()` entirely
/// (a real `bool`/`String`, not a numeric `Animated<T>` field `animate
/// ()`'s own contract can ever reach) -- gated by `property` now, so a
/// binding declared on `checked` that resolves to the wrong shape is a
/// real type-mismatch error instead of silently accepting whatever
/// value happened to be a `Bool`. Every other property (numeric --
/// `opacity`/`corner_radius`/`elevation`/`rotation`/`check_progress`/
/// `thumb_position`/`select_progress`/`toggle_progress` -- and
/// composite -- `background`/`transform`/`shape`) forwards to `animate
/// ()`, which already knows how to validate/extract whatever Python
/// value shape each one needs; a bound `Value::Str` is parsed as a
/// color only for `property == "background"` (the same real hex/CSS-
/// named parser `engine_spec::build::resolve_color` already uses for
/// *static* YAML colors -- MD3 theme-role token strings are explicitly
/// out of scope here, since this function's own `throwaway_node` below
/// has no access to the view's real, live `ColorScheme`), and a
/// `Value::Handle` is recovered via `resolver.to_pyobject` and handed
/// to `animate()` verbatim.
fn apply_binding_value(
    tree: &Rc<RefCell<Tree>>,
    handlers: &HandlerMap,
    node_id: NodeId,
    property: &str,
    py: Python<'_>,
    value: &engine_spec::Value,
    resolver: &PyViewModelResolver,
) -> PyResult<()> {
    // Never exposed to Python beyond this function's own real calls
    // below -- an empty, throwaway `context_menus`/`theme`/`completions`
    // is fine here; a real `View`-created `Node` (returned from `View::
    // node`, below) shares `View`'s own persistent ones instead.
    // `handlers` is the one real exception -- `set_checked` fires a
    // real `Change` through it (M14 Phase 3), so this reuses `View`'s
    // own persistent map, not a throwaway one, or a registered `on_
    // change` handler would never see a binding-applied `checked` value.
    let temp_node = throwaway_node(tree, node_id, handlers.clone());

    if property == "checked" {
        let engine_spec::Value::Bool(checked) = value else {
            return Err(PyValueError::new_err(format!(
                "widget property {property:?} expects a boolean binding, got {value:?}"
            )));
        };
        return temp_node.set_checked(*checked, py);
    }
    // M15 Phase 3 (§16.7): the same real, direct dispatch the `checked`
    // branch above already established -- `text` is the one other
    // genuinely non-numeric bindable property, so it goes through
    // `Node.set_text` directly rather than `animate()`'s own `Animated
    // <f64>` contract, which a `String` can never satisfy.
    if property == "text" {
        let engine_spec::Value::Str(text) = value else {
            return Err(PyValueError::new_err(format!(
                "widget property {property:?} expects a string binding, got {value:?}"
            )));
        };
        return temp_node.set_text(text, py);
    }
    // M48 (§16.2): `width`/`height`/`padding`/`gap` are real, live-
    // mutable via `Node.set_layout` since M48, but -- unlike every
    // other bindable property -- they are NOT `Animated<T>` fields, so
    // they can never reach `animate()`'s own dispatch at all (see `Node
    // ::set_layout`'s own doc comment). Routed here, before the generic
    // `animate()` forwarding below, the same way `checked`/`text`
    // already are for their own non-numeric reasons.
    if matches!(property, "width" | "height" | "padding" | "gap") {
        let numeric = match value {
            engine_spec::Value::Int(i) => *i as f32,
            engine_spec::Value::Float(f) => *f as f32,
            other => {
                return Err(PyValueError::new_err(format!(
                    "widget property {property:?} expects a numeric binding, got {other:?}"
                )));
            }
        };
        let (width, height, padding, gap) = match property {
            "width" => (Some(numeric), None, None, None),
            "height" => (None, Some(numeric), None, None),
            "padding" => (None, None, Some(numeric), None),
            "gap" => (None, None, None, Some(numeric)),
            _ => unreachable!("matched by the outer `matches!` above"),
        };
        // M59/M71 (§5, §16.3): `set_layout` widened with per-side padding/
        // margin, flex-grow/shrink/basis, align-items/justify-content,
        // and flex-direction -- none of those are reachable from a `{{ }}` binding
        // (this call site's own real scope stays `width`/`height`/
        // `padding`/`gap`, matching the `matches!` guard above), so
        // every new parameter is `None` here.
        temp_node.set_layout(
            width, height, padding, None, None, None, None, None, None, None, None, None, gap,
            None, None, None, None, None, None,
        )?;
        return Ok(());
    }

    let bound: Bound<'_, PyAny> = match value {
        engine_spec::Value::Int(i) => (*i as f64).into_bound_py_any(py)?,
        engine_spec::Value::Float(f) => (*f).into_bound_py_any(py)?,
        // M44: a string bound to `background` is parsed as a color
        // (hex/CSS-named) rather than falling into the `other => Err`
        // arm below -- see this function's own doc comment above for
        // why MD3 theme-role tokens aren't handled here. M48: `border_
        // color` gets the identical treatment -- same `(u8,u8,u8,u8)`
        // shape `Node::animate`'s own new `"border_color"` arm expects.
        engine_spec::Value::Str(s) if matches!(property, "background" | "border_color") => {
            let rgba = parse_background_color(s).map_err(|e| {
                PyValueError::new_err(format!("binding for property {property:?} resolved to {e}"))
            })?;
            rgba.into_bound_py_any(py)?
        }
        // M44: a non-primitive resolved value (e.g. a Signal holding an
        // `(r,g,b,a)` tuple for `background`, or a translate/scale
        // tuple for `transform`, or a point list for `shape`) is
        // recovered via the resolver's own real Handle -> Python object
        // mapping and forwarded verbatim -- `animate()` does the actual
        // per-property validation, identical to its own imperative path.
        engine_spec::Value::Handle(_) => resolver.to_pyobject(py, value)?,
        other => {
            return Err(PyValueError::new_err(format!(
                "binding for property {property:?} resolved to {other:?} -- only numeric, \
                 boolean (checked), string (text), and background-color bindings are \
                 supported today"
            )));
        }
    };
    temp_node.animate(property, bound, 0, None)?;
    tree.borrow_mut().tick_all(std::time::Instant::now());
    Ok(())
}

/// `pub(crate)`, not private: M43 Phase 1's own new `component.rs`
/// needs this identical spec-walking logic to collect a *component*
/// instance's own bindings, scoped separately from whatever `View`/
/// `Component` it's embedded into -- reused verbatim, not duplicated.
pub(crate) fn collect_bindings(spec: &WidgetSpec, out: &mut Vec<(String, String, String)>) {
    for (property, expr) in &spec.bindings {
        out.push((spec.id.clone(), property.clone(), expr.clone()));
    }
    for child in &spec.children {
        collect_bindings(child, out);
    }
}

/// `pub(crate)` for the same M43 Phase 1 reason as `collect_bindings`.
pub(crate) fn collect_handlers(spec: &WidgetSpec, out: &mut Vec<(String, String, String)>) {
    for (event, method) in &spec.handlers {
        out.push((spec.id.clone(), event.clone(), method.clone()));
    }
    for child in &spec.children {
        collect_handlers(child, out);
    }
}

/// M14 Phase 3 (§16.7): mirrors `collect_bindings`/`collect_handlers`'
/// own shape exactly -- `(widget_id, property)` for every widget that
/// named a real `two_way:` property. `pub(crate)` for the same M43
/// Phase 1 reason as `collect_bindings`.
pub(crate) fn collect_two_way(spec: &WidgetSpec, out: &mut Vec<(String, String)>) {
    if let Some(property) = &spec.two_way {
        out.push((spec.id.clone(), property.clone()));
    }
    for child in &spec.children {
        collect_two_way(child, out);
    }
}

/// A binding's own re-evaluation trigger, subscribed onto every
/// `Signal` its expression read during its initial evaluation.
/// `Signal._notify` calls this like any other zero-arg Python callable.
#[pyclass(unsendable)]
struct BindingCallback {
    tree: Rc<RefCell<Tree>>,
    handlers: HandlerMap,
    node_id: NodeId,
    property: String,
    expr: Expression,
    viewmodel: Py<PyAny>,
}

#[pymethods]
impl BindingCallback {
    fn __call__(&self, py: Python<'_>) -> PyResult<()> {
        let resolver = PyViewModelResolver::new(self.viewmodel.clone_ref(py));
        let value =
            evaluate(&self.expr, &resolver).map_err(|e| PyValueError::new_err(e.to_string()))?;
        apply_binding_value(
            &self.tree,
            &self.handlers,
            self.node_id,
            &self.property,
            py,
            &value,
            &resolver,
        )
    }
}

/// M14 Phase 3 (§16.7): a two-way binding's own real write-back --
/// registered as this widget's own `EventKind::Change` handler
/// (`Node.set_on_change`'s own real storage, reused directly rather
/// than inventing a second callback-registration path). Reads the
/// node's own current, real value for `property` (the exact reverse of
/// `apply_binding_value`'s own forward direction: `checked` via `Node.
/// get_checked`, everything else via `Node.get`) and writes it into
/// the bound `Signal` via its own real, public `.set(value)`.
#[pyclass(unsendable)]
struct TwoWayCallback {
    tree: Rc<RefCell<Tree>>,
    node_id: NodeId,
    property: String,
    signal: Py<PyAny>,
}

#[pymethods]
impl TwoWayCallback {
    fn __call__(&self, py: Python<'_>) -> PyResult<()> {
        let temp_node = throwaway_node(
            &self.tree,
            self.node_id,
            Rc::new(RefCell::new(HashMap::new())),
        );
        let value: Bound<'_, PyAny> = if self.property == "checked" {
            temp_node.get_checked()?.into_bound_py_any(py)?
        } else if self.property == "text" {
            // M15 Phase 3 (§16.7): the same real read-back split
            // `checked` already established -- `text` isn't an
            // `Animated<f64>` property `Node.get` dispatches to.
            temp_node.get_text()?.into_bound_py_any(py)?
        } else {
            temp_node.get(&self.property)?.into_bound_py_any(py)?
        };
        self.signal.bind(py).call_method1("set", (value,))?;
        Ok(())
    }
}

/// M43 Phase 1 (§4, §5, §8, §16.2, §16.6): the real, shared "wire a
/// `ViewModel` onto a set of declared bindings/handlers" logic --
/// factored out of `View::_attach`'s own original body (byte-for-byte
/// unchanged behavior there, verified by the existing `test_view_
/// binding.py`/`test_view_handlers.py`/`test_view_in_window.py` suites)
/// so the new `Component::_attach` (`component.rs`) can reuse it
/// verbatim for an embedded component's own, independently-scoped
/// bindings/handlers, rather than duplicating ~150 lines -- the
/// identical "two real call sites justify factoring out" pattern this
/// codebase already applies elsewhere (`throwaway_node`, `node_center`).
///
/// `id_of` is a closure rather than a `&Reconciler` taken directly,
/// since `View` looks a widget id up via `self.reconciler.id_of(...)`
/// and `Component` via its own, separately-instantiated `Reconciler`
/// the identical way -- both satisfy the same `Fn(&str) -> Option
/// <NodeId>` shape without this function needing to know which kind of
/// caller it's serving.
///
/// Returns every `(signal, callback)` pair this call subscribed onto --
/// `View::_attach` discards it (a `View` lives as long as the whole
/// script does, never needs to unsubscribe); `Component::_attach`
/// (Phase 2) keeps it, so a later `Component.remove()` can unsubscribe
/// them again before tearing down the subtree those callbacks reference
/// -- without this, a `Signal` write after removal would panic (`apply_
/// binding_value`'s own `tree.borrow_mut()...` calls expect a `NodeId`
/// still present in the `Tree`).
#[allow(clippy::too_many_arguments)]
pub(crate) fn attach_bindings_and_handlers(
    tree: &Rc<RefCell<Tree>>,
    handlers: &HandlerMap,
    context_menus: &Rc<RefCell<HashMap<NodeId, NodeId>>>,
    theme: &SharedTheme,
    completions: &SharedCompletions,
    id_of: impl Fn(&str) -> Option<NodeId>,
    declared_handlers: &[(String, String, String)],
    bindings: &[(String, String, String)],
    two_way: &[(String, String)],
    py: Python<'_>,
    viewmodel: Py<PyAny>,
) -> PyResult<Vec<(Py<PyAny>, Py<PyAny>)>> {
    for (widget_id, event, method_name) in declared_handlers {
        let attr = viewmodel
            .bind(py)
            .getattr(method_name.as_str())
            .map_err(|_| {
                PyValueError::new_err(format!(
                    "widget {widget_id:?}: handler {event:?} names {method_name:?}, which has \
                     no matching attribute on the ViewModel"
                ))
            })?;
        if !attr.is_callable() {
            return Err(PyValueError::new_err(format!(
                "widget {widget_id:?}: handler {event:?} names {method_name:?}, which is \
                     not callable"
            )));
        }

        // M4 Phase 4/6: wires the validated method into the same
        // real dispatch mechanism `Node.set_on_click`/
        // `set_on_hover_enter`/`set_on_hover_exit` already use --
        // `_attach` used to stop at validation, so a real click/hover
        // on this widget did nothing. Only these three real event
        // kinds are wired today, matching §16.2's own "generalizing
        // to whatever named events a NodeKind exposes" -- other
        // declared event names still validate (so a typo still
        // fails at `_attach()` time) but have no real mechanism to
        // reach yet. M54 Phase 2 (§8, §16.2): the real `Event`
        // argument this comment used to defer now exists -- `method_
        // name` is arity-sniffed the identical way `Node.set_on_
        // click`/etc. already are (`dispatch::register_handler`,
        // reached through these same four real setters), so a
        // declaratively-bound handler can opt into it exactly like an
        // imperatively-registered one.
        let kind = match event.as_str() {
            "on_click" => Some(EventKind::Click),
            "on_hover_enter" => Some(EventKind::HoverEnter),
            "on_hover_exit" => Some(EventKind::HoverExit),
            // M14 Phase 3 (§16.7): the real handler-name counterpart
            // to `EventKind::Change` -- wires a declared `on_change:`
            // the same way every other real event kind here already
            // is.
            "on_change" => Some(EventKind::Change),
            // M55 (§10, §16.2): the real handler-name counterpart to
            // `EventKind::FocusEnter`/`FocusExit` -- the same real
            // extension pattern `on_change` already established.
            "on_focus_enter" => Some(EventKind::FocusEnter),
            "on_focus_exit" => Some(EventKind::FocusExit),
            _ => None,
        };
        if let Some(kind) = kind {
            let node_id = id_of(widget_id).ok_or_else(|| {
                PyValueError::new_err(format!(
                    "widget {widget_id:?}: handler {event:?} names a widget id never built \
                         into the Tree"
                ))
            })?;
            let node = Node {
                id: node_id,
                tree: tree.clone(),
                handlers: handlers.clone(),
                context_menus: context_menus.clone(),
                theme: theme.clone(),
                completions: completions.clone(),
            };
            // Reuses `Node`'s own real setters verbatim (same
            // construction `apply_binding_value` already uses for
            // `animate`) rather than inserting into `handlers`
            // directly -- `set_on_click` also adds `Action::Click`
            // to the node's `access.actions` (§10, Tab-reachability),
            // a real side effect only the real method carries.
            match kind {
                EventKind::Click => node.set_on_click(attr.unbind(), py),
                EventKind::HoverEnter => node.set_on_hover_enter(attr.unbind(), py),
                EventKind::HoverExit => node.set_on_hover_exit(attr.unbind(), py),
                EventKind::Change => node.set_on_change(attr.unbind(), py),
                EventKind::FocusEnter => node.set_on_focus_enter(attr.unbind(), py),
                EventKind::FocusExit => node.set_on_focus_exit(attr.unbind(), py),
            }?;
        }
    }

    let mut subscriptions = Vec::new();

    for (widget_id, property, raw_expr) in bindings {
        let node_id = id_of(widget_id).ok_or_else(|| {
            PyValueError::new_err(format!(
                "binding on unknown widget id {widget_id:?} (never built into the Tree)"
            ))
        })?;
        let expr = parse_binding(raw_expr).map_err(|e| {
            PyValueError::new_err(format!(
                "widget {widget_id:?} binding on {property:?} ({raw_expr:?}): {e}"
            ))
        })?;
        let resolver = PyViewModelResolver::new(viewmodel.clone_ref(py));

        _begin_recording();
        let evaluated = evaluate(&expr, &resolver);
        let touched = _end_recording();
        let value = evaluated.map_err(|e| {
            PyValueError::new_err(format!(
                "widget {widget_id:?} binding on {property:?} ({raw_expr:?}): {e}"
            ))
        })?;

        apply_binding_value(tree, handlers, node_id, property, py, &value, &resolver)?;

        let callback = Py::new(
            py,
            BindingCallback {
                tree: tree.clone(),
                handlers: handlers.clone(),
                node_id,
                property: property.clone(),
                expr: expr.clone(),
                viewmodel: viewmodel.clone_ref(py),
            },
        )?;
        for signal in &touched {
            signal
                .bind(py)
                .call_method1("_subscribe", (callback.clone_ref(py),))
                .map_err(|e| {
                    PyValueError::new_err(format!(
                        "widget {widget_id:?} binding on {property:?}: failed to subscribe \
                             to a Signal it read: {e}"
                    ))
                })?;
            subscriptions.push((signal.clone_ref(py), callback.clone_ref(py).into_any()));
        }

        // M14 Phase 3 (§16.7): the real write-back half -- only for
        // a widget/property pair the author actually named `two_
        // way:`. ARCHITECTURE.md §16.7's own text: "only for a plain
        // Signal reference -- never a computed expression, since
        // there's no way to reverse `{{ f"{first} {last}" }}` back
        // into two Signals," enforced here by requiring the parsed
        // `expr` to be exactly a bare Signal's own `.get()` call
        // (`Expression::Call(Expression::Ident(signal_name), "get")`)
        // -- a real, load-time-checked error otherwise, not a silent
        // no-op. **Real finding:** a first draft of this check
        // required a *bare* `Expression::Ident` instead (matching
        // §16.7's own inline illustration, `{{ username }}` with no
        // `.get()`), but every binding's forward direction resolves
        // through `PyViewModelResolver::ident`, which reads the raw
        // Python attribute unmodified -- for a `Signal`, that's the
        // `Signal` object itself, not its value, so it always
        // resolved to an opaque `Value::Handle` and `apply_binding_
        // value` (above, forward direction) rejected it before this
        // code ever ran. `.get()` is the one shape that both
        // resolves to the real primitive forward (identical to every
        // other binding in this codebase -- see `test_view_binding.
        // py`) and still names the exact Signal to write back to.
        if two_way.iter().any(|(w, p)| w == widget_id && p == property) {
            let Expression::Call(receiver, method) = &expr else {
                return Err(PyValueError::new_err(format!(
                    "widget {widget_id:?}: two_way binding on {property:?} ({raw_expr:?}) \
                         must be a plain Signal's own .get() call (e.g. \"{{{{ username.get() \
                         }}}}\"), not a computed expression -- there's no way to reverse it back \
                         into a Signal"
                )));
            };
            let (Expression::Ident(signal_name), true) = (receiver.as_ref(), method == "get")
            else {
                return Err(PyValueError::new_err(format!(
                    "widget {widget_id:?}: two_way binding on {property:?} ({raw_expr:?}) \
                         must be a plain Signal's own .get() call (e.g. \"{{{{ username.get() \
                         }}}}\"), not a computed expression -- there's no way to reverse it back \
                         into a Signal"
                )));
            };
            let signal = viewmodel
                .bind(py)
                .getattr(signal_name.as_str())
                .map_err(|_| {
                    PyValueError::new_err(format!(
                        "widget {widget_id:?}: two_way binding names {signal_name:?}, which has \
                         no matching attribute on the ViewModel"
                    ))
                })?;
            let two_way_callback = Py::new(
                py,
                TwoWayCallback {
                    tree: tree.clone(),
                    node_id,
                    property: property.clone(),
                    signal: signal.unbind(),
                },
            )?;
            // M54 Phase 2: `TwoWayCallback` is a Rust-implemented
            // `__call__`, not an app-defined Python function -- always
            // zero-argument, inserted directly rather than through
            // `register_handler`'s own Python-side `inspect.signature`
            // introspection (real, but unnecessary indirection for a
            // callable whose own arity is already known here).
            handlers.borrow_mut().insert(
                (node_id, EventKind::Change),
                (two_way_callback.into_any(), false),
            );
        }
    }

    Ok(subscriptions)
}

#[pyclass(unsendable)]
pub struct View {
    /// `pub(crate)`, unlike most of this struct's other fields: M42
    /// Phase 1's own `crate::window::PyWindow::from_view` (a different
    /// module) needs to clone this same `Rc<RefCell<Tree>>` into a real,
    /// live `Window` -- mirrors `PyWindow`'s own fields, all `pub
    /// (crate)` for the identical reason (`wrap_node`'s own doc
    /// comment).
    pub(crate) tree: Rc<RefCell<Tree>>,
    pub(crate) reconciler: Reconciler,
    bindings: Vec<(String, String, String)>, // (widget_id, property, raw "{{ expr }}")
    declared_handlers: Vec<(String, String, String)>, // (widget_id, event, method_name)
    /// M14 Phase 3 (§16.7): `(widget_id, property)` for every widget
    /// with a real `two_way:` name -- see `WidgetSpec.two_way`'s own
    /// doc comment for why this is a separate field, not folded into
    /// `bindings` above.
    two_way: Vec<(String, String)>,
    /// Mirrors `PyWindow`'s own `handlers` (M4 Phase 1 step 3, re-keyed
    /// by `(NodeId, EventKind)` at M4 Phase 6) -- shared with every
    /// `Node` this `View` hands out via `node()`, so `set_on_click`/
    /// `set_on_hover_enter`/`set_on_hover_exit` are all structurally
    /// available on a `View`'s widgets too. `pub(crate)` for the same
    /// `from_view` reason as `tree` above.
    pub(crate) handlers: HandlerMap,
    /// M4 Phase 7 (§11.3): mirrors `PyWindow`'s own `context_menus`.
    /// `pub(crate)` for the same `from_view` reason as `tree` above.
    pub(crate) context_menus: Rc<RefCell<HashMap<NodeId, NodeId>>>,
    /// M42 Phase 1 (§4, §5, §8): a `View`'s own persistent theme,
    /// mirroring `PyWindow`'s own `theme: SharedTheme` field exactly --
    /// replaces the fresh, private `ThemeState::default()` instance
    /// `node()`/`_attach()` used to build per call (never shared with
    /// each other, and orphaned the instant each call returned). Needed
    /// so a `View` shown live via `PyWindow::from_view` has real,
    /// persistent theme state for the shared `Window` to read/mutate,
    /// not throwaway instances a live render loop can never reach.
    pub(crate) theme: SharedTheme,
    /// M42 Phase 1 (§4, §5, §8): same real reasoning as `theme` above,
    /// for `PyWindow`'s own `completions: SharedCompletions` --
    /// `Node.animate(..., on_complete=...)`'s registry needs a real,
    /// persistent instance a live `Window`'s per-frame loop can drain,
    /// not a fresh one built and discarded per `node()`/`_attach()` call.
    pub(crate) completions: SharedCompletions,
    /// M42 Phase 1 (§4, §5, §8): `0` (the default) means "never shown
    /// live" -- `available_space()` below falls back to `AvailableSpace
    /// ::MaxContent` on both axes in that case, byte-for-byte this
    /// struct's own pre-M42 `click()`/`hover()`/`right_click()`
    /// behavior. `PyWindow::from_view` sets a real nonzero value (and
    /// clones this exact `Rc<Cell<u32>>` into the new `Window` it
    /// returns), the same shared-`Cell` pattern `PyWindow`'s own
    /// `width`/`height` already establish (M33 Phase 2) -- a live
    /// resize's `.set()` call is then immediately visible here too.
    pub(crate) width: SharedSize,
    pub(crate) height: SharedSize,
    /// M19 Phase 1 (§16.4): remembered so `poll_reload` can re-read the
    /// same file later -- `View::new` used to discard it the instant
    /// the initial read finished.
    path: String,
    /// M19 Phase 1 (§16.4): `None` only if the initial `ViewWatcher::
    /// watch` genuinely failed (an unusual filesystem with no real
    /// inotify-equivalent) -- non-fatal, the same "real, expected,
    /// gracefully-handled" policy this codebase already applies to
    /// no-GPU/no-display (TRE v1 finding #261); `View` still works,
    /// `poll_reload` just always reports no change.
    watcher: Option<ViewWatcher>,
    /// M26 Phase 1 (§16.3): remembered so `poll_reload` re-resolves
    /// against the same real stylesheet/scheme on every future
    /// reconcile, not just the initial `Reconciler::load` -- both
    /// `None` (the default) is byte-for-byte the prior "no styling,
    /// literal colors only" behavior every existing `view.yaml` still
    /// gets.
    stylesheet: Option<Stylesheet>,
    scheme: Option<ColorScheme>,
    /// M49 Phase 4: the same real "remembered for every future
    /// reconcile" reasoning `stylesheet`/`scheme` above already
    /// establish -- the `styles:` half of the theme(s) this `View` was
    /// constructed with, already converted to a `Stylesheet` (`theme_
    /// spec_to_stylesheet`) so `poll_reload` never needs to re-parse
    /// the original YAML file on every reload.
    default_theme: Option<Stylesheet>,
    custom_theme: Option<Stylesheet>,
}

/// A plain, non-`#[pymethods]` block for a Rust-only helper, mirroring
/// `PyWindow::wrap_node`'s own identical split (`window.rs`) -- `pyo3`
/// exposes every fn in a `#[pymethods]` block as a Python method, so a
/// genuinely internal helper like this one needs its own separate,
/// ordinary `impl` block instead.
impl View {
    /// M42 Phase 1 (§4, §5, §8): `click()`/`hover()`/`right_click()`'s
    /// own real layout size -- `Definite` on both axes once a real
    /// nonzero `width`/`height` has been set (only `PyWindow::from_view`
    /// ever does that), otherwise `MaxContent` on both, byte-for-byte
    /// this struct's own original, pre-M42 behavior for a `View` that
    /// has never been shown live.
    fn available_space(&self) -> Size<AvailableSpace> {
        let (width, height) = (self.width.get(), self.height.get());
        if width == 0 || height == 0 {
            Size {
                width: AvailableSpace::MaxContent,
                height: AvailableSpace::MaxContent,
            }
        } else {
            Size {
                width: AvailableSpace::Definite(width as f32),
                height: AvailableSpace::Definite(height as f32),
            }
        }
    }
}

#[pymethods]
impl View {
    /// M26 Phase 1 (§16.3): `stylesheet` (a path to a real stylesheet
    /// YAML file, read and parsed the same direct way `path` itself
    /// already is -- a plain Python constructor argument, as trusted as
    /// `path`, not embedded YAML *content* the way `include:` paths
    /// are, so it needs none of `include:`'s own path-confinement
    /// machinery) and `theme_seed`/`dark` (the identical real
    /// `DynamicTheme::from_seed` mechanism `Window.set_theme` already
    /// uses, picking `light`/`dark` the same way) are both optional and
    /// both default to `None` -- a `View(path)` call with neither given
    /// is byte-for-byte today's existing "literal colors only, no
    /// stylesheet" behavior.
    /// M49 Phase 4: `default_theme`/`custom_theme` (paths to real theme
    /// YAML files, `ThemeSpec` -- `engine-spec/src/theme.rs`) are the
    /// two new layers `resolve_style_layered` cascades beneath
    /// `stylesheet`/inline (`cascade.rs`). `default_theme` omitted uses
    /// the engine's own shipped default (`SHIPPED_DEFAULT_THEME_YAML`,
    /// embedded at compile time) -- the user's own stated "a yaml style
    /// file loaded as the default." A theme's own `seed:` (if present)
    /// takes precedence over `default_theme`'s, which takes precedence
    /// over `custom_theme`'s -- an explicit, directly-passed `theme_
    /// seed` argument always wins, matching "an explicit argument beats
    /// file config." A theme's own `colors:` overrides are applied
    /// *onto* whichever scheme that precedence resolves to, default's
    /// first then custom's (custom wins on any overlapping role) --
    /// only meaningful when a real scheme exists at all (some `seed`
    /// was resolved); with none, colors stay silently unused, the same
    /// "no scheme, no MD3 token resolution, literal colors only"
    /// precedent this codebase already establishes for `theme_seed`
    /// alone being omitted.
    // M71 (§8, §16.1): `source` added -- when given, used directly
    // instead of reading `path` from disk, while `path` still supplies
    // the real base directory `include:`/`image.src:` resolve against
    // (below) and the real file `ViewWatcher` watches for hot-reload
    // (also below) -- so a caller that pre-processes a view's own raw
    // text (the sibling `Tesserae` project's own real, confirmed need:
    // expanding its own custom `component:` macro syntax into tre-
    // native primitive YAML *before* this constructor ever sees it) can
    // still have `poll_reload()` react to real edits of the file the
    // developer actually wrote, not a generated artifact with no
    // meaningful path of its own. `None` (the default) is the real,
    // pre-existing behavior -- read `path` directly -- unchanged for
    // every caller that doesn't pass it.
    // tre issue #3, Part A (Tier 1): `spec` is a real Python object
    // (a dict, shaped like `view.yaml`'s own `WidgetSpec` schema) built
    // directly by the caller -- Tesserae's own macro-expansion layer,
    // or any app that wants to compose a tree as native Python data --
    // with zero YAML-text round trip. Depythonized straight into a real
    // `WidgetSpec` (`pythonize::depythonize`) and handed to `Reconciler
    // ::load_spec` (M77), the identical real building/id-recording
    // logic the YAML-text path already uses. Mutually exclusive with
    // `source` (both name a real content source; only one is
    // meaningful). `path` becomes genuinely optional when `spec` is
    // given -- a purely programmatic view built from a Python dict may
    // have no real backing file at all, unlike `source`'s own M71
    // precedent (there, a real file always exists; only its *content*
    // is pre-processed before this constructor sees it). When `path`
    // is omitted, `base_dir` resolves to `None` (an `include:`/`image.
    // src:` inside the spec fails with the same real, clear error it
    // already would for any other `base_dir: None` case) and
    // `ViewWatcher` simply doesn't start -- the identical graceful
    // "no watcher" path an unwatchable real file already takes below,
    // not a new failure mode.
    #[new]
    #[pyo3(signature = (path=None, stylesheet=None, theme_seed=None, dark=false, default_theme=None, custom_theme=None, source=None, spec=None))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        py: Python<'_>,
        path: Option<String>,
        stylesheet: Option<String>,
        theme_seed: Option<(u8, u8, u8, u8)>,
        dark: bool,
        default_theme: Option<String>,
        custom_theme: Option<String>,
        source: Option<String>,
        spec: Option<Py<PyAny>>,
    ) -> PyResult<Self> {
        if spec.is_some() && source.is_some() {
            return Err(PyValueError::new_err(
                "View() cannot take both spec= and source= -- pass one real content source, not two",
            ));
        }
        if spec.is_none() && path.is_none() {
            return Err(PyValueError::new_err(
                "View() needs path= unless spec= is given -- a purely programmatic view has no file to name",
            ));
        }
        let path = path.unwrap_or_default();

        // M19 Phase 2 (§16.6): an `include:` path is only ever
        // meaningful relative to the file that named it -- `View`'s
        // own directory is the real base every include in this view
        // (and, recursively, every file it includes) resolves against.
        // `Path::new("").parent()` is `None`, the same real "no base
        // directory" outcome an explicitly omitted `path` already
        // means elsewhere in this codebase (`include:`'s own `base_
        // dir: None` contract).
        let base_dir = std::path::Path::new(&path).parent();

        let stylesheet = match stylesheet {
            Some(sheet_path) => {
                let sheet_yaml = std::fs::read_to_string(&sheet_path).map_err(|e| {
                    PyRuntimeError::new_err(format!(
                        "failed to read stylesheet {sheet_path:?}: {e}"
                    ))
                })?;
                Some(
                    parse_stylesheet(&sheet_yaml)
                        .map_err(|e| PyValueError::new_err(e.to_string()))?,
                )
            }
            None => None,
        };

        let (default_theme_sheet, custom_theme_sheet, scheme) = resolve_theme_layers(
            default_theme.as_deref(),
            custom_theme.as_deref(),
            theme_seed,
            dark,
        )?;
        let default_theme_sheet = Some(default_theme_sheet);

        let mut tree = Tree::new();
        let (reconciler, spec_for_bindings) = if let Some(spec_obj) = &spec {
            let widget_spec: WidgetSpec = pythonize::depythonize(spec_obj.bind(py))
                .map_err(|e| PyValueError::new_err(format!("spec=: {e}")))?;
            let spec_for_bindings = widget_spec.clone();
            let reconciler = Reconciler::load_spec(
                &mut tree,
                widget_spec,
                default_theme_sheet.as_ref(),
                custom_theme_sheet.as_ref(),
                stylesheet.as_ref(),
                scheme.as_ref(),
                base_dir,
            )
            .map_err(|e| PyValueError::new_err(e.to_string()))?;
            (reconciler, spec_for_bindings)
        } else {
            let yaml = match source {
                Some(text) => text,
                None => std::fs::read_to_string(&path).map_err(|e| {
                    PyRuntimeError::new_err(format!("failed to read view {path:?}: {e}"))
                })?,
            };
            let reconciler = Reconciler::load(
                &mut tree,
                &yaml,
                default_theme_sheet.as_ref(),
                custom_theme_sheet.as_ref(),
                stylesheet.as_ref(),
                scheme.as_ref(),
                base_dir,
            )
            .map_err(|e| PyValueError::new_err(e.to_string()))?;
            let spec_for_bindings = parse_view_with_includes(&yaml, base_dir)
                .map_err(|e| PyValueError::new_err(e.to_string()))?;
            (reconciler, spec_for_bindings)
        };

        let mut bindings = Vec::new();
        collect_bindings(&spec_for_bindings, &mut bindings);
        let mut declared_handlers = Vec::new();
        collect_handlers(&spec_for_bindings, &mut declared_handlers);
        let mut two_way = Vec::new();
        collect_two_way(&spec_for_bindings, &mut two_way);

        // M19 Phase 1 (§16.4): a real, additive capability -- `View`
        // worked fine without it before this phase, so a failure here
        // (an unusual filesystem with no real inotify-equivalent) is
        // non-fatal, logged and skipped, not propagated as a
        // constructor error.
        let watcher = match ViewWatcher::watch(std::path::Path::new(&path)) {
            Ok(watcher) => Some(watcher),
            Err(err) => {
                tracing::warn!(%err, path = %path, "failed to start watching this view file for hot-reload -- poll_reload will always report no change");
                None
            }
        };

        Ok(Self {
            tree: Rc::new(RefCell::new(tree)),
            reconciler,
            bindings,
            declared_handlers,
            two_way,
            handlers: Rc::new(RefCell::new(HashMap::new())),
            context_menus: Rc::new(RefCell::new(HashMap::new())),
            theme: Rc::new(RefCell::new(ThemeState::default())),
            completions: Rc::new(RefCell::new(CompletionRegistry::new())),
            width: Rc::new(Cell::new(0)),
            height: Rc::new(Cell::new(0)),
            path,
            watcher,
            stylesheet,
            scheme,
            default_theme: default_theme_sheet,
            custom_theme: custom_theme_sheet,
        })
    }

    /// The `Node` for one widget's author-assigned `id`, e.g. for a
    /// caller (or a test) to read back a property `Node` exposes a
    /// getter for after a binding has applied it.
    fn node(&self, widget_id: &str) -> PyResult<Node> {
        let id = self.reconciler.id_of(widget_id).ok_or_else(|| {
            PyValueError::new_err(format!("no widget with id {widget_id:?} in this view"))
        })?;
        Ok(Node {
            id,
            tree: self.tree.clone(),
            handlers: self.handlers.clone(),
            context_menus: self.context_menus.clone(),
            // M42 Phase 1 (§4, §5, §8): now this `View`'s own real,
            // persistent instance -- previously a fresh, private,
            // always-`None` `ThemeState::default()` built and discarded
            // on every call (M7 Phase 3's own original, narrower scope).
            // A `View` never shown live still sees byte-for-byte the
            // same behavior (this persistent instance also starts
            // `None`/un-themed); one shown via `PyWindow::from_view` now
            // shares the exact instance the live `Window` itself reads.
            theme: self.theme.clone(),
            // M42 Phase 1 (§4, §5, §8): same real reasoning as `theme`
            // above -- now shared with a live `Window`'s own per-frame
            // `on_complete` drain loop instead of a fresh, never-drained
            // instance (M9 Phase 2's own original, narrower scope).
            completions: self.completions.clone(),
        })
    }

    /// M43 Phase 1 (§4, §5, §8, §16.2, §16.6): instantiates another
    /// view's own YAML as a real, independent component -- its own
    /// `Reconciler`, its own scoped bindings/handlers, ready for its
    /// own separate `ViewModel` to `_attach` to -- spliced into this
    /// `View`'s own live `Tree` as a child of `into`. Calling this
    /// multiple times (e.g. once per item in a real list) gives each
    /// call its own independent `Component`, confirmed correct by
    /// construction: each instantiation builds its own `Reconciler`, so
    /// even identical widget ids declared inside the component's own
    /// YAML resolve to distinct real `NodeId`s per instance -- see
    /// `component.rs`'s own module doc comment for the full real
    /// design (and why `Component` has no `click`/`hover`/`right_click`
    /// of its own -- dispatch on an embedded node goes through this
    /// `View`'s own `click`/`hover`/`right_click` instead, e.g.
    /// `view.click(component.node("button"))`).
    ///
    /// `source` (widened alongside `View::new`'s own M71 real
    /// precedent, above): when given, used directly instead of reading
    /// `path` from disk -- see `instantiate_component`'s own doc
    /// comment (`component.rs`) for the real reasoning, and why this
    /// was a real, confirmed gap (not a hypothetical one) before now.
    #[pyo3(signature = (path, into, source=None))]
    fn instantiate(
        &self,
        path: &str,
        into: PyRef<'_, Node>,
        source: Option<String>,
    ) -> PyResult<crate::component::Component> {
        crate::component::instantiate_component(
            &self.tree,
            into.id,
            &self.handlers,
            &self.context_menus,
            &self.theme,
            &self.completions,
            path,
            source,
        )
    }

    /// M19 Phase 1 (§16.4): the real, first Python-facing entry point
    /// for reconciliation -- `ViewWatcher`/`Reconciler` both already
    /// existed as tested `engine-spec` primitives, but nothing ever
    /// called them from a real, live `View` before this. Returns
    /// `false` with no real change detected (no watcher, or nothing
    /// written to the file since the last call) -- `true` once a real
    /// change was actually reconciled into the live `Tree`.
    ///
    /// **Real, stated scope boundary:** patches structure/paint/layout
    /// only (`Reconciler::reconcile`'s own real diffing -- an unchanged
    /// widget keeps its real `NodeId`, so its focus/scroll/in-flight
    /// animation survive). `bindings:`/`handlers:`/`two_way:` are
    /// resolved entirely separately, by `_attach`'s own `BindingResolver`
    /// logic against a `viewmodel` this method has no access to -- a
    /// hot-reloaded view that adds a genuinely *new* binding or handler
    /// needs `_attach` called again, the caller's own responsibility,
    /// the same as it would be after adding one imperatively.
    ///
    /// `View` itself has no live-window/render-loop concept (this
    /// module's own doc comment) to call this automatically every
    /// "frame" the way §16.4's own text describes -- this is the real,
    /// honest translation of that intent given `View`'s own real,
    /// pre-existing architecture: an explicit method the caller invokes
    /// wherever its own script's equivalent of "between frames" is.
    ///
    /// M71 (§8, §16.1): `source` added -- `__new__`'s own real sibling
    /// parameter. The real change-detection gate above (`watcher.
    /// poll_changed()`) still watches `self.path` on disk regardless --
    /// a caller pre-processing this view's own raw text (see `__new__`'s
    /// doc comment) still needs to know *whether* the real underlying
    /// file changed at all before deciding it's worth re-expanding, so
    /// that real, cheap, inotify-backed check isn't bypassed. Only the
    /// *content actually reconciled* changes: `source`, when given,
    /// instead of a fresh `self.path` disk read.
    #[pyo3(signature = (source=None))]
    fn poll_reload(&mut self, source: Option<String>) -> PyResult<bool> {
        let Some(watcher) = &self.watcher else {
            return Ok(false);
        };
        if !watcher.poll_changed() {
            return Ok(false);
        }
        let yaml = match source {
            Some(text) => text,
            None => std::fs::read_to_string(&self.path).map_err(|e| {
                PyRuntimeError::new_err(format!("failed to re-read view {:?}: {e}", self.path))
            })?,
        };
        let base_dir = std::path::Path::new(&self.path).parent();
        let mut tree = self.tree.borrow_mut();
        self.reconciler
            .reconcile(
                &mut tree,
                &yaml,
                self.default_theme.as_ref(),
                self.custom_theme.as_ref(),
                self.stylesheet.as_ref(),
                self.scheme.as_ref(),
                base_dir,
            )
            .map_err(|e| PyValueError::new_err(e.to_string()))?;
        Ok(true)
    }

    /// tre issue #3, Part C: `poll_reload`'s own change-detection gate
    /// (`self.watcher`'s `poll_changed()`) is hard-wired to a real
    /// filesystem event -- a `View` built with no `path` at all (M78's
    /// own `spec=`-only construction) has no `watcher` and can *never*
    /// open that gate, so `poll_reload` would always report `false`
    /// for it, regardless of `source=`. This is not "teach `watch.rs`
    /// to detect programmatic changes" (a real, deliberately rejected
    /// design considered while scoping this) -- a caller with no
    /// backing file already knows precisely when its own data changed
    /// (typically via `tre.Effect`'s own real dependency tracking), so
    /// its own explicit call to this method already *is* the change
    /// signal. No new Rust-side dirty-flag/channel mechanism needed;
    /// `watch.rs`/`ViewWatcher` stay completely untouched by this
    /// milestone. Unconditional -- unlike `poll_reload`, there's no
    /// "did anything change" ambiguity to report, so this always
    /// reconciles when given valid input, matching `__new__`'s own
    /// `spec`/`source` shape and validation exactly (mutually
    /// exclusive; at least one required).
    #[pyo3(signature = (source=None, spec=None))]
    fn reconcile(
        &mut self,
        py: Python<'_>,
        source: Option<String>,
        spec: Option<Py<PyAny>>,
    ) -> PyResult<()> {
        if spec.is_some() && source.is_some() {
            return Err(PyValueError::new_err(
                "reconcile() cannot take both spec= and source= -- pass one real content source, not two",
            ));
        }
        if spec.is_none() && source.is_none() {
            return Err(PyValueError::new_err(
                "reconcile() needs source= or spec= -- nothing to reconcile against",
            ));
        }
        let base_dir = std::path::Path::new(&self.path).parent();
        let mut tree = self.tree.borrow_mut();
        if let Some(spec_obj) = &spec {
            let widget_spec: WidgetSpec = pythonize::depythonize(spec_obj.bind(py))
                .map_err(|e| PyValueError::new_err(format!("spec=: {e}")))?;
            self.reconciler
                .reconcile_spec(
                    &mut tree,
                    widget_spec,
                    self.default_theme.as_ref(),
                    self.custom_theme.as_ref(),
                    self.stylesheet.as_ref(),
                    self.scheme.as_ref(),
                    base_dir,
                )
                .map_err(|e| PyValueError::new_err(e.to_string()))?;
        } else {
            self.reconciler
                .reconcile(
                    &mut tree,
                    source
                        .as_deref()
                        .expect("validated above: source is Some when spec is None"),
                    self.default_theme.as_ref(),
                    self.custom_theme.as_ref(),
                    self.stylesheet.as_ref(),
                    self.scheme.as_ref(),
                    base_dir,
                )
                .map_err(|e| PyValueError::new_err(e.to_string()))?;
        }
        Ok(())
    }

    /// M51: live re-theme -- re-resolves *every* node's `PaintProperties`
    /// /`layout_style` against a new set of theme layers, in place
    /// (`Reconciler::retheme`, `reconcile.rs`), without needing the
    /// underlying `view.yaml` to have changed at all (unlike `poll_
    /// reload`, which reacts to a real file edit). Stores the new
    /// theme layers so a *later* `poll_reload()` keeps resolving
    /// against them, not the ones this `View` was originally
    /// constructed with.
    ///
    /// **Real, deliberate convention, matching `Window.set_theme`'s own
    /// already-shipped precedent, not a new one invented here:** each
    /// call is a complete, fresh theme selection -- omitting `default_
    /// theme`/`custom_theme` means the shipped default / no custom
    /// override, exactly like `View.__new__`'s own defaults, *not*
    /// "keep whatever the previous call used." A caller who only wants
    /// to change the seed re-passes the same `default_theme`/
    /// `custom_theme` path it already has in hand.
    ///
    /// `{{ }}` bindings are not re-applied by this call -- `patch_node`
    /// (inside `retheme`) only recomputes the *static* cascade, the
    /// identical real behavior a content-only `poll_reload()` already
    /// has today, not a new interaction this method introduces.
    #[pyo3(signature = (default_theme=None, custom_theme=None, theme_seed=None, dark=false))]
    fn set_theme(
        &mut self,
        default_theme: Option<String>,
        custom_theme: Option<String>,
        theme_seed: Option<(u8, u8, u8, u8)>,
        dark: bool,
    ) -> PyResult<()> {
        let (default_theme_sheet, custom_theme_sheet, scheme) = resolve_theme_layers(
            default_theme.as_deref(),
            custom_theme.as_deref(),
            theme_seed,
            dark,
        )?;
        let base_dir = std::path::Path::new(&self.path).parent();
        let mut tree = self.tree.borrow_mut();
        self.reconciler
            .retheme(
                &mut tree,
                Some(&default_theme_sheet),
                custom_theme_sheet.as_ref(),
                self.stylesheet.as_ref(),
                scheme.as_ref(),
                base_dir,
            )
            .map_err(|e| PyValueError::new_err(e.to_string()))?;
        drop(tree);
        self.default_theme = Some(default_theme_sheet);
        self.custom_theme = custom_theme_sheet;
        self.scheme = scheme;
        Ok(())
    }

    /// §16.2's real inversion point. Validates every declared handler
    /// eagerly (a bad name fails here, not on first click), then
    /// resolves every declared binding once against `viewmodel`,
    /// applies its initial value, and subscribes a re-evaluation
    /// callback onto every `Signal` that evaluation actually read.
    ///
    /// M43 Phase 1 (§4, §5, §8, §16.2, §16.6): the real body lives in
    /// `attach_bindings_and_handlers` now, shared with the new
    /// `Component::_attach` (`component.rs`) -- this is a thin wrapper
    /// over it, discarding the returned subscription list since a
    /// `View` lives as long as the whole script does and never needs
    /// to unsubscribe (unlike a removable `Component`, Phase 2).
    ///
    /// **`&self`, not `&mut self` -- a real bug found and fixed in this
    /// same phase, by actually running `examples/component_list.py`:**
    /// nothing in this method's own body (or `click`/`hover`/
    /// `right_click`, below) ever mutates one of `View`'s own plain
    /// struct fields directly -- every real mutation goes through an
    /// interior-mutable `Rc<RefCell<...>>`/`Cell` field instead. `&mut
    /// self` here was a real, pre-existing (if latent) bug: pyo3 holds
    /// an *exclusive* borrow on the whole `View` Python object for a
    /// `&mut self` method's entire duration, so a handler dispatched
    /// from inside `click()` that calls *any other* method on that
    /// same `view` object -- exactly what `view.instantiate(...)`
    /// inside an `on_click` handler does, the real scenario this
    /// milestone exists for -- panicked with "Already mutably
    /// borrowed." `&self` lets pyo3 allow that same reentrant call
    /// (many shared borrows can coexist; only one exclusive borrow
    /// can't coexist with anything), the same real reasoning `Window`'s
    /// own `click`/`hover`/`right_click` (`window_input.rs`) already
    /// use `&self` for.
    fn _attach(&self, py: Python<'_>, viewmodel: Py<PyAny>) -> PyResult<()> {
        attach_bindings_and_handlers(
            &self.tree,
            &self.handlers,
            &self.context_menus,
            &self.theme,
            &self.completions,
            |widget_id| self.reconciler.id_of(widget_id),
            &self.declared_handlers,
            &self.bindings,
            &self.two_way,
            py,
            viewmodel,
        )?;
        Ok(())
    }

    /// M4 Phase 4 (§16.2): the same no-live-window-needed proof pattern
    /// `Window.click` (M4 Phase 1 step 3) already established, adapted
    /// for a `View`'s own shape -- a `View` never shown live has no
    /// window width/height of its own, so layout is computed with
    /// `AvailableSpace::MaxContent` on both axes rather than a fixed
    /// size (`available_space()`, below). Every existing `view.yaml`
    /// already declares an explicit `style.width`/`style.height` on its
    /// root widget (see `tests/test_view_binding.py`), so this sizes
    /// correctly rather than needing a workaround. **Updated, M42 Phase
    /// 1:** once a `View` has been shown via `PyWindow::from_view`,
    /// `available_space()` instead returns the real, live `Definite`
    /// size -- matching what the on-screen window actually paints, not
    /// an unconstrained hypothetical layout. Computes layout, finds
    /// `node`'s real center, and dispatches a primary press+release pair
    /// there -- exactly what a real mouse click would produce, proving a
    /// handler `_attach` wired (above) actually fires, not just that it
    /// validated.
    fn click(&self, node: PyRef<'_, Node>, py: Python<'_>) {
        let root = self.reconciler.root();
        let point = node_center(&self.tree, root, self.available_space(), node.id);

        let now = std::time::Instant::now();
        let config = interaction_config();
        // Each `dispatch` call's own `self.tree.borrow_mut()` is a
        // short-lived temporary, released before `run_dispatch_outcome` runs
        // -- matches `Window.click`'s own reasoning: a handler that
        // itself touches this same `Tree` would otherwise panic on a
        // re-entrant borrow.
        let press_event = InputEvent::PointerPressed {
            position: point,
            button: PointerButton::Primary,
        };
        let press = self
            .tree
            .borrow_mut()
            .dispatch(root, press_event.clone(), &config, now);
        run_dispatch_outcome(
            &self.handlers,
            &self.tree,
            &self.context_menus,
            &self.theme,
            &self.completions,
            &press,
            Some(&press_event),
            py,
        );

        let release_event = InputEvent::PointerReleased {
            position: point,
            button: PointerButton::Primary,
        };
        let release = self
            .tree
            .borrow_mut()
            .dispatch(root, release_event.clone(), &config, now);
        run_dispatch_outcome(
            &self.handlers,
            &self.tree,
            &self.context_menus,
            &self.theme,
            &self.completions,
            &release,
            Some(&release_event),
            py,
        );
    }

    /// M4 Phase 6 (§7.3): `click()`'s own hover counterpart, mirroring
    /// `Window.hover` exactly -- dispatches a `PointerMoved` at `node`'s
    /// own real center, firing `HoverEnter`/`HoverExit` through the same
    /// `handlers` map `_attach` wires into (above).
    fn hover(&self, node: PyRef<'_, Node>, py: Python<'_>) {
        let root = self.reconciler.root();
        let point = node_center(&self.tree, root, self.available_space(), node.id);

        let event = InputEvent::PointerMoved { position: point };
        let outcome = self.tree.borrow_mut().dispatch(
            root,
            event.clone(),
            &interaction_config(),
            std::time::Instant::now(),
        );
        run_dispatch_outcome(
            &self.handlers,
            &self.tree,
            &self.context_menus,
            &self.theme,
            &self.completions,
            &outcome,
            Some(&event),
            py,
        );
    }

    /// M55 (§10, §16.2): `Window.focus`'s own real `View` sibling,
    /// mirroring it exactly -- no real `InputEvent` for "focus this
    /// specific node" exists, so this calls `Tree::set_focus_to`
    /// directly and fires the transition via the shared `dispatch::
    /// fire_focus_transition`.
    fn focus(&self, node: PyRef<'_, Node>, py: Python<'_>) {
        let config = interaction_config();
        let transition = self.tree.borrow_mut().set_focus_to(
            node.id,
            config.focus_ring_opacity,
            config.focus_ring_duration,
            std::time::Instant::now(),
        );
        if let Some((old, new)) = transition {
            crate::dispatch::fire_focus_transition(
                &self.handlers,
                &self.tree,
                &self.context_menus,
                &self.theme,
                &self.completions,
                old,
                new,
                py,
            );
        }
    }

    /// M4 Phase 7 (§11.3): `click()`'s own secondary-button (right-click)
    /// counterpart, mirroring `Window.right_click` exactly.
    fn right_click(&self, node: PyRef<'_, Node>, py: Python<'_>) {
        let root = self.reconciler.root();
        let point = node_center(&self.tree, root, self.available_space(), node.id);

        let now = std::time::Instant::now();
        let config = interaction_config();
        // M55 (§10, §16.2): a real gap found while scoping `Focus`
        // events -- this press's own outcome used to be discarded
        // with no variable at all, so a real right-click-to-focus
        // (M53) on `TextField`/`Terminal` was structurally
        // unobservable from this entry point. Captured and forwarded
        // now, the same way every other real dispatch call site
        // already does.
        let press_event = InputEvent::PointerPressed {
            position: point,
            button: PointerButton::Secondary,
        };
        let press = self
            .tree
            .borrow_mut()
            .dispatch(root, press_event.clone(), &config, now);
        run_dispatch_outcome(
            &self.handlers,
            &self.tree,
            &self.context_menus,
            &self.theme,
            &self.completions,
            &press,
            Some(&press_event),
            py,
        );
        let release_event = InputEvent::PointerReleased {
            position: point,
            button: PointerButton::Secondary,
        };
        let outcome = self
            .tree
            .borrow_mut()
            .dispatch(root, release_event.clone(), &config, now);
        run_dispatch_outcome(
            &self.handlers,
            &self.tree,
            &self.context_menus,
            &self.theme,
            &self.completions,
            &outcome,
            Some(&release_event),
            py,
        );
        open_context_menu(&self.tree, &self.context_menus, root, &outcome);
    }

    /// Same real GC-cycle-safety obligation `PyWindow` already carries
    /// for its own `handlers` (M4 Phase 1 step 3) -- `View` stores
    /// `Py<PyAny>` callbacks too now, so it needs to make them visible
    /// to CPython's cyclic collector the same way.
    fn __traverse__(&self, visit: pyo3::PyVisit<'_>) -> Result<(), pyo3::PyTraverseError> {
        for (handler, _wants_event) in self.handlers.borrow().values() {
            visit.call(handler)?;
        }
        Ok(())
    }

    fn __clear__(&mut self) {
        self.handlers.borrow_mut().clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Same real "genuinely fresh file per run" pattern `engine-spec::
    /// watch::tests::watcher_detects_a_real_write_to_the_watched_file`
    /// already established -- `View::new` reads a real path from disk,
    /// so these tests need one, not an in-memory fixture.
    fn write_temp_view(yaml: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "engine_py_view_test_{}_{}.yaml",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(&path, yaml).expect("create test view file");
        path
    }

    /// M71 (§8, §16.1): `source=`, when given, is used instead of
    /// reading `path` from disk -- proven directly by writing a real
    /// on-disk file with one `width`, then constructing with `source=`
    /// naming a *different* `width` and confirming the live `Tree`
    /// reflects `source`'s own value, not the file's.
    #[test]
    fn source_override_is_used_instead_of_reading_path_from_disk() {
        let path = write_temp_view("id: root\nkind: Container\nstyle: {width: 40, height: 20}\n");
        let view = Python::attach(|py| {
            View::new(
                py,
                Some(path.to_string_lossy().into_owned()),
                None,
                None,
                false,
                None,
                None,
                Some("id: root\nkind: Container\nstyle: {width: 999, height: 20}\n".to_string()),
                None,
            )
        })
        .expect("real View");

        let tree = view.tree.borrow();
        let root = view.reconciler.id_of("root").expect("root widget id");
        let style = &tree.get(root).expect("root node").layout_style;
        assert_eq!(
            style.size.width,
            taffy::prelude::length(999.0),
            "source= must be used instead of the real on-disk file's own content"
        );

        let _ = std::fs::remove_file(&path);
    }

    /// M71 (§8, §16.1): `poll_reload(source=...)`'s own real sibling
    /// behavior -- the change-detection gate still watches the real
    /// on-disk `path` (a real write to it is what makes `poll_changed()`
    /// report `true` at all), but the content actually reconciled is
    /// `source`, not a fresh read of `path`.
    #[test]
    fn poll_reload_source_override_is_reconciled_instead_of_the_file() {
        let path = write_temp_view("id: root\nkind: Container\nstyle: {width: 40, height: 20}\n");
        let mut view = Python::attach(|py| {
            View::new(
                py,
                Some(path.to_string_lossy().into_owned()),
                None,
                None,
                false,
                None,
                None,
                None,
                None,
            )
        })
        .expect("real View");

        // A real write to the watched file -- content doesn't matter,
        // only that the watcher's own inotify-backed `poll_changed()`
        // has something real to report; `source=` below overrides what
        // actually gets reconciled regardless of what this write says.
        std::fs::write(
            &path,
            "id: root\nkind: Container\nstyle: {width: 40, height: 20}\n",
        )
        .expect("real rewrite of the watched file");
        // Real filesystem watchers need a moment to deliver the event.
        std::thread::sleep(std::time::Duration::from_millis(200));

        let reloaded = view
            .poll_reload(Some(
                "id: root\nkind: Container\nstyle: {width: 777, height: 20}\n".to_string(),
            ))
            .expect("poll_reload must not error");
        assert!(reloaded, "a real file change must be detected");

        let tree = view.tree.borrow();
        let root = view.reconciler.id_of("root").expect("root widget id");
        let style = &tree.get(root).expect("root node").layout_style;
        assert_eq!(
            style.size.width,
            taffy::prelude::length(777.0),
            "poll_reload's own source= must be reconciled instead of a fresh disk read"
        );
        drop(tree);

        let _ = std::fs::remove_file(&path);
    }

    /// M42 Phase 1's own real, new behavior, updated for tre issue #3
    /// Tier 1: `View::new` now takes a real `Python<'_>` (needed for
    /// `pythonize::depythonize`'s own `spec_obj.bind(py)` when `spec=`
    /// is given), so this Rust-only test wraps the call in `Python::
    /// attach` (pyo3 0.29's real `with_gil` replacement -- confirmed
    /// directly against its own source, not assumed; `[dev-dependencies]`
    /// 's own `pyo3/auto-initialize` starts a real embedded interpreter
    /// for it) -- previously callable with no GIL at all. Still matches
    /// this crate's own established
    /// real test-surface split: pyo3-facing *Python* API behavior is
    /// covered by `tests/*.py` (needs a real interpreter), while
    /// plain-Rust logic reachable without touching a live `Py<PyAny>` --
    /// like
    /// `available_space()`'s own new branch, below -- gets a real Rust
    /// unit test the same as any other crate in this workspace.
    #[test]
    fn a_view_never_shown_live_still_lays_out_with_max_content() {
        let path = write_temp_view("id: root\nkind: Container\nstyle: {width: 40, height: 20}\n");
        let view = Python::attach(|py| {
            View::new(
                py,
                Some(path.to_string_lossy().into_owned()),
                None,
                None,
                false,
                None,
                None,
                None,
                None,
            )
        })
        .expect("real View");

        assert_eq!(
            view.available_space(),
            Size {
                width: AvailableSpace::MaxContent,
                height: AvailableSpace::MaxContent,
            },
            "a View PyWindow::from_view has never touched must keep its original, pre-M42 \
             MaxContent layout behavior byte-for-byte"
        );

        let _ = std::fs::remove_file(&path);
    }

    /// The exact real mutation `PyWindow::from_view` performs on a
    /// `View` (`view.width.set(width); view.height.set(height);`,
    /// `window.rs`) -- proven directly here since `from_view` itself is
    /// a `#[pymethods]` staticmethod whose generated pyo3 wrapper needs
    /// a live Python interpreter to call through, but the real,
    /// load-bearing logic this phase adds (`available_space()`'s own
    /// branch) needs none of that to verify.
    #[test]
    fn setting_a_real_size_switches_available_space_to_definite() {
        let path = write_temp_view("id: root\nkind: Container\nstyle: {width: 40, height: 20}\n");
        let view = Python::attach(|py| {
            View::new(
                py,
                Some(path.to_string_lossy().into_owned()),
                None,
                None,
                false,
                None,
                None,
                None,
                None,
            )
        })
        .expect("real View");

        view.width.set(300);
        view.height.set(150);

        assert_eq!(
            view.available_space(),
            Size {
                width: AvailableSpace::Definite(300.0),
                height: AvailableSpace::Definite(150.0),
            },
            "once a real nonzero size is set (what from_view does), layout must switch to the \
             real Definite size a live window actually paints at, not stay unconstrained"
        );

        let _ = std::fs::remove_file(&path);
    }

    /// A real, load-bearing edge case `available_space()`'s own `||`
    /// branch condition depends on: only one axis ever getting set
    /// (impossible through `from_view`'s own real call site, which
    /// always sets both together, but not structurally impossible for
    /// some future caller) must still fall back to MaxContent on *both*
    /// axes rather than mixing a Definite width with a MaxContent
    /// height -- taffy has no real "mixed" `AvailableSpace` convention
    /// this codebase relies on anywhere else.
    #[test]
    fn only_one_axis_set_still_falls_back_to_max_content_on_both() {
        let path = write_temp_view("id: root\nkind: Container\nstyle: {width: 40, height: 20}\n");
        let view = Python::attach(|py| {
            View::new(
                py,
                Some(path.to_string_lossy().into_owned()),
                None,
                None,
                false,
                None,
                None,
                None,
                None,
            )
        })
        .expect("real View");

        view.width.set(300);
        // height left at its real default, 0.

        assert_eq!(
            view.available_space(),
            Size {
                width: AvailableSpace::MaxContent,
                height: AvailableSpace::MaxContent,
            }
        );

        let _ = std::fs::remove_file(&path);
    }

    /// M44 (§16.2): `parse_background_color` is the real, GIL-free
    /// conversion a `background:` string binding now goes through --
    /// exact-value coverage here, since pytest has no way to read a
    /// node's applied color back (`Node.get` only returns `f64`; there's
    /// no `background`-reading getter at all, confirmed by grep).
    #[test]
    fn parse_background_color_accepts_a_hex_string() {
        assert_eq!(
            parse_background_color("#112233").unwrap(),
            (0x11, 0x22, 0x33, 0xff),
        );
    }

    #[test]
    fn parse_background_color_accepts_hex_with_alpha() {
        assert_eq!(
            parse_background_color("#11223344").unwrap(),
            (0x11, 0x22, 0x33, 0x44),
        );
    }

    #[test]
    fn parse_background_color_accepts_a_css_named_color() {
        assert_eq!(parse_background_color("red").unwrap(), (0xff, 0, 0, 0xff));
    }

    #[test]
    fn parse_background_color_rejects_nonsense() {
        let err = parse_background_color("not a color").unwrap_err();
        assert!(
            err.contains("not a color"),
            "the real bad input must appear in the error message, not a generic failure: {err}"
        );
    }
}
