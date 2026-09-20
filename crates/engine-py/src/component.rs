//! M43 Phase 1 (§4, §5, §8, §16.2, §16.6): real component composition
//! -- embedding a view inside another view where the embedded content
//! gets its own, separate `ViewModel`, and can be instantiated multiple
//! times simultaneously (a list where each row is its own independent
//! component instance). "The essence of MVVM and single page
//! applications," per the user's own words scoping this milestone.
//!
//! Deliberately *not* `include:` (`engine_spec::include`, §16.6): that
//! mechanism splices another YAML file's widgets in as ordinary
//! children *at parse time*, before the whole thing becomes one
//! `WidgetSpec` -- the included content shares the *same* single
//! `View`/`ViewModel` as its parent. `Component` is architecturally
//! closer to how Vue/React implement components: one shared `Tree`
//! (confirmed, by direct investigation before designing this, to need
//! zero `engine-core`/`engine-render` changes -- rendering, dispatch,
//! and hit-testing already walk an arbitrary node's real parent chain,
//! §4's own crate boundary stays untouched), but a *separate*
//! `ViewModel`-scoped set of bindings/handlers for just this
//! component's own subtree.
//!
//! **Real, load-bearing correctness point, confirmed before writing
//! any code:** `Component` shares `tree`/`handlers`/`context_menus`/
//! `theme`/`completions` directly (`Rc::clone`) with whatever `View`/
//! `Component` it was instantiated into -- safe *because* it's the
//! same `Tree`, so every `NodeId` involved is unique by construction
//! (a `slotmap` key scoped to one `Tree`). This is the opposite
//! situation from M42 Phase 2's own `window::ActiveTree`, which had to
//! bundle `handlers`/`context_menus` together specifically because
//! *separate* Trees can (and do) allocate colliding `NodeId` values --
//! that risk simply doesn't exist here, since there's only ever one
//! `Tree` involved in a single `View`'s own component subtree.
//!
//! **Real design correction, found during implementation, not
//! assumed:** `Component` does *not* get its own `click`/`hover`/
//! `right_click` methods. `Tree::compute_layout(root, available_space)`
//! computes a fresh layout *as if* `root` were the whole tree's own
//! top-level root, using `available_space` as its outer constraint --
//! calling it from a component's own root would silently distort its
//! real computed size (which is actually constrained by its real
//! parent container, not by some independent "available space" of its
//! own). Dispatching a synthetic click on an embedded component's own
//! node is already fully correct through the *owning* `View`/`Window`'s
//! existing `click`/`hover`/`right_click` methods (`view.click(component
//! .node("button"))`/`window.click(...)`) -- `Tree::absolute_position`
//! (confirmed by reading its own body) walks a node's real parent chain
//! all the way up to whatever ancestor has no parent, so it already
//! resolves correctly for a deeply-nested component node once the
//! *owning* View/Window's own `compute_layout` call has run against the
//! real top-level root.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use engine_core::{NodeId, Tree};
use engine_spec::{Reconciler, parse_view_with_includes};
use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;

use crate::dispatch::{HandlerMap, SharedCompletions};
use crate::node::Node;
use crate::view::{
    attach_bindings_and_handlers, collect_bindings, collect_handlers, collect_two_way,
};
use crate::window::SharedTheme;

#[pyclass(unsendable, name = "Component")]
pub struct Component {
    tree: Rc<RefCell<Tree>>,
    reconciler: Reconciler,
    bindings: Vec<(String, String, String)>,
    declared_handlers: Vec<(String, String, String)>,
    two_way: Vec<(String, String)>,
    handlers: HandlerMap,
    context_menus: Rc<RefCell<HashMap<NodeId, NodeId>>>,
    theme: SharedTheme,
    completions: SharedCompletions,
}

/// The real, shared "parse a component's own YAML and splice it into
/// an already-live `Tree` under `into`" logic -- called from both
/// `View::instantiate` (`view.rs`) and `Component::instantiate`
/// (below), so components nest recursively for free (a component
/// containing another component needs no special-casing here).
///
/// Mirrors `View::new`'s own real "parse twice" pattern (once inside
/// `Reconciler::load`, once via `parse_view_with_includes` again for
/// binding/handler collection) -- the same established precedent, not
/// a new one invented here; `Reconciler`'s own `spec: WidgetSpec` field
/// is private, so there's no way to reuse the *same* parse without
/// widening its own public API for this, and this file's real scope
/// keeps `engine-spec` untouched entirely.
///
/// **Real, stated v1 scope limit:** no stylesheet/MD3-token resolution
/// for a component's own styling (`Reconciler::load`'s `sheet`/`scheme`
/// params are `None`) -- matches `View`'s own real "literal colors
/// only" default when no stylesheet is given; a component-level
/// stylesheet is real, additive, deferred work if a real need surfaces,
/// not manufactured ahead of one.
pub(crate) fn instantiate_component(
    tree: &Rc<RefCell<Tree>>,
    into: NodeId,
    handlers: &HandlerMap,
    context_menus: &Rc<RefCell<HashMap<NodeId, NodeId>>>,
    theme: &SharedTheme,
    completions: &SharedCompletions,
    path: &str,
) -> PyResult<Component> {
    let yaml = std::fs::read_to_string(path)
        .map_err(|e| PyRuntimeError::new_err(format!("failed to read component {path:?}: {e}")))?;
    let base_dir = std::path::Path::new(path).parent();

    let reconciler = {
        let mut tree_mut = tree.borrow_mut();
        Reconciler::load(&mut tree_mut, &yaml, None, None, base_dir)
            .map_err(|e| PyValueError::new_err(e.to_string()))?
    };
    tree.borrow_mut().add_child(into, reconciler.root());

    let spec = parse_view_with_includes(&yaml, base_dir)
        .map_err(|e| PyValueError::new_err(e.to_string()))?;
    let mut bindings = Vec::new();
    collect_bindings(&spec, &mut bindings);
    let mut declared_handlers = Vec::new();
    collect_handlers(&spec, &mut declared_handlers);
    let mut two_way = Vec::new();
    collect_two_way(&spec, &mut two_way);

    Ok(Component {
        tree: tree.clone(),
        reconciler,
        bindings,
        declared_handlers,
        two_way,
        handlers: handlers.clone(),
        context_menus: context_menus.clone(),
        theme: theme.clone(),
        completions: completions.clone(),
    })
}

#[pymethods]
impl Component {
    /// The `Node` for one widget's author-assigned `id`, scoped to
    /// *this* component instance's own YAML -- mirrors `View::node`
    /// exactly. Three instances of the same component, each declaring
    /// `id: title` internally, each resolve to their own distinct real
    /// `NodeId` here -- confirmed correct by construction, since each
    /// `instantiate()` call built its own independent `Reconciler`.
    fn node(&self, widget_id: &str) -> PyResult<Node> {
        let id = self.reconciler.id_of(widget_id).ok_or_else(|| {
            PyValueError::new_err(format!("no widget with id {widget_id:?} in this component"))
        })?;
        Ok(Node {
            id,
            tree: self.tree.clone(),
            handlers: self.handlers.clone(),
            context_menus: self.context_menus.clone(),
            theme: self.theme.clone(),
            completions: self.completions.clone(),
        })
    }

    /// §16.2's real inversion point, scoped to this component instance
    /// -- mirrors `View::_attach` exactly (both call the identical
    /// shared `attach_bindings_and_handlers`, `view.rs`).
    fn _attach(&mut self, py: Python<'_>, viewmodel: Py<PyAny>) -> PyResult<()> {
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

    /// Instantiates another component *inside* this one -- components
    /// nest for free, the same real `instantiate_component` helper
    /// `View.instantiate` itself calls.
    fn instantiate(&self, path: &str, into: PyRef<'_, Node>) -> PyResult<Component> {
        instantiate_component(
            &self.tree,
            into.id,
            &self.handlers,
            &self.context_menus,
            &self.theme,
            &self.completions,
            path,
        )
    }
}
