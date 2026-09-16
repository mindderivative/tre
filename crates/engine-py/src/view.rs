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
//! - Handler wiring stops at eager validation (`getattr(viewmodel,
//!   name)` must resolve to something callable, checked at `_attach()`
//!   time, matching "validated eagerly... not discovered on first
//!   use"). Actually *firing* a handler from a real click needs
//!   `InputEvent`/`AppHandler` pointer dispatch, which -- checked
//!   directly -- doesn't exist anywhere in this codebase yet (the same
//!   finding steps 7/9/11 already made for keyboard/pointer/theme
//!   dispatch respectively).
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

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use engine_core::{NodeId, Tree};
use engine_spec::{Expression, Reconciler, WidgetSpec, evaluate, parse_binding, parse_view};
use pyo3::IntoPyObjectExt;
use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;

use crate::binding::PyViewModelResolver;
use crate::node::Node;

thread_local! {
    static RECORDING: RefCell<Option<Vec<Py<PyAny>>>> = const { RefCell::new(None) };
}

/// Called from Python's `Signal.get()` on every read. If a binding
/// evaluation is currently recording (see `begin_recording`/
/// `end_recording`), records this signal as one of its dependencies --
/// by object identity, deduplicated, since one binding can legitimately
/// read the same `Signal` more than once in a single evaluation.
#[pyfunction]
pub(crate) fn _record_read(py: Python<'_>, signal: Py<PyAny>) {
    RECORDING.with(|cell| {
        if let Some(list) = cell.borrow_mut().as_mut() {
            let already_present = list.iter().any(|s| s.bind(py).is(signal.bind(py)));
            if !already_present {
                list.push(signal);
            }
        }
    });
}

fn begin_recording() {
    RECORDING.with(|cell| *cell.borrow_mut() = Some(Vec::new()));
}

fn end_recording() -> Vec<Py<PyAny>> {
    RECORDING.with(|cell| cell.borrow_mut().take().unwrap_or_default())
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
fn apply_binding_value(
    tree: &Rc<RefCell<Tree>>,
    node_id: NodeId,
    property: &str,
    py: Python<'_>,
    value: &engine_spec::Value,
) -> PyResult<()> {
    let bound: Bound<'_, PyAny> = match value {
        engine_spec::Value::Int(i) => (*i as f64).into_bound_py_any(py)?,
        engine_spec::Value::Float(f) => (*f).into_bound_py_any(py)?,
        other => {
            return Err(PyValueError::new_err(format!(
                "binding for property {property:?} resolved to {other:?} -- only numeric \
                 (opacity/corner_radius) bindings are supported today"
            )));
        }
    };
    // Never exposed to Python -- only `animate()` is called on it below
    // -- so an empty, throwaway `click_handlers` map is fine here; a
    // real `View`-created `Node` (returned from `View::node`, below)
    // shares `View`'s own persistent one instead.
    let temp_node = Node {
        id: node_id,
        tree: tree.clone(),
        click_handlers: Rc::new(RefCell::new(HashMap::new())),
    };
    temp_node.animate(property, bound, 0)?;
    tree.borrow_mut().tick_all(std::time::Instant::now());
    Ok(())
}

fn collect_bindings(spec: &WidgetSpec, out: &mut Vec<(String, String, String)>) {
    for (property, expr) in &spec.bindings {
        out.push((spec.id.clone(), property.clone(), expr.clone()));
    }
    for child in &spec.children {
        collect_bindings(child, out);
    }
}

fn collect_handlers(spec: &WidgetSpec, out: &mut Vec<(String, String, String)>) {
    for (event, method) in &spec.handlers {
        out.push((spec.id.clone(), event.clone(), method.clone()));
    }
    for child in &spec.children {
        collect_handlers(child, out);
    }
}

/// A binding's own re-evaluation trigger, subscribed onto every
/// `Signal` its expression read during its initial evaluation.
/// `Signal._notify` calls this like any other zero-arg Python callable.
#[pyclass(unsendable)]
struct BindingCallback {
    tree: Rc<RefCell<Tree>>,
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
        apply_binding_value(&self.tree, self.node_id, &self.property, py, &value)
    }
}

#[pyclass(unsendable)]
pub struct View {
    tree: Rc<RefCell<Tree>>,
    reconciler: Reconciler,
    bindings: Vec<(String, String, String)>, // (widget_id, property, raw "{{ expr }}")
    handlers: Vec<(String, String, String)>, // (widget_id, event, method_name)
    /// Mirrors `PyWindow`'s own `click_handlers` (M4 Phase 1 step 3) --
    /// shared with every `Node` this `View` hands out via `node()`, so
    /// `set_on_click` is structurally available on a `View`'s widgets
    /// too. Not yet reachable by any real dispatch: `View` has no event
    /// loop wired to it (this module's own doc comment names that as
    /// this crate's stated scope boundary) -- a future step that gives
    /// `View` a real render loop is what would actually invoke these.
    click_handlers: Rc<RefCell<HashMap<NodeId, Py<PyAny>>>>,
}

#[pymethods]
impl View {
    #[new]
    fn new(path: String) -> PyResult<Self> {
        let yaml = std::fs::read_to_string(&path)
            .map_err(|e| PyRuntimeError::new_err(format!("failed to read view {path:?}: {e}")))?;
        let mut tree = Tree::new();
        let reconciler = Reconciler::load(&mut tree, &yaml, None, None)
            .map_err(|e| PyValueError::new_err(e.to_string()))?;
        let spec = parse_view(&yaml).map_err(|e| PyValueError::new_err(e.to_string()))?;

        let mut bindings = Vec::new();
        collect_bindings(&spec, &mut bindings);
        let mut handlers = Vec::new();
        collect_handlers(&spec, &mut handlers);

        Ok(Self {
            tree: Rc::new(RefCell::new(tree)),
            reconciler,
            bindings,
            handlers,
            click_handlers: Rc::new(RefCell::new(HashMap::new())),
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
            click_handlers: self.click_handlers.clone(),
        })
    }

    /// §16.2's real inversion point. Validates every declared handler
    /// eagerly (a bad name fails here, not on first click), then
    /// resolves every declared binding once against `viewmodel`,
    /// applies its initial value, and subscribes a re-evaluation
    /// callback onto every `Signal` that evaluation actually read.
    fn _attach(&mut self, py: Python<'_>, viewmodel: Py<PyAny>) -> PyResult<()> {
        for (widget_id, event, method_name) in &self.handlers {
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
        }

        for (widget_id, property, raw_expr) in &self.bindings {
            let node_id = self.reconciler.id_of(widget_id).ok_or_else(|| {
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

            begin_recording();
            let evaluated = evaluate(&expr, &resolver);
            let touched = end_recording();
            let value = evaluated.map_err(|e| {
                PyValueError::new_err(format!(
                    "widget {widget_id:?} binding on {property:?} ({raw_expr:?}): {e}"
                ))
            })?;

            apply_binding_value(&self.tree, node_id, property, py, &value)?;

            let callback = Py::new(
                py,
                BindingCallback {
                    tree: self.tree.clone(),
                    node_id,
                    property: property.clone(),
                    expr,
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
            }
        }

        Ok(())
    }

    /// Same real GC-cycle-safety obligation `PyWindow` already carries
    /// for its own `click_handlers` (M4 Phase 1 step 3) -- `View` stores
    /// `Py<PyAny>` callbacks too now, so it needs to make them visible
    /// to CPython's cyclic collector the same way.
    fn __traverse__(&self, visit: pyo3::PyVisit<'_>) -> Result<(), pyo3::PyTraverseError> {
        for click_handler in self.click_handlers.borrow().values() {
            visit.call(click_handler)?;
        }
        Ok(())
    }

    fn __clear__(&mut self) {
        self.click_handlers.borrow_mut().clear();
    }
}
