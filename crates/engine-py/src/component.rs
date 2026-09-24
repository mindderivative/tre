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
    /// M43 Phase 2 (§4, §5, §8, §16.2, §16.6): every `(signal,
    /// callback)` pair `_attach` subscribed onto, so `remove()` can
    /// unsubscribe them again before tearing down the subtree those
    /// callbacks reference by `NodeId` -- without this, a `Signal`
    /// write after removal would panic (`apply_binding_value`'s own
    /// `tree.borrow_mut()...` calls expect a `NodeId` still present in
    /// the `Tree`). `View` has no equivalent field: a `View` lives as
    /// long as the whole script does and is never removed, so it has
    /// nothing to unsubscribe from later.
    subscriptions: Vec<(Py<PyAny>, Py<PyAny>)>,
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
///
/// `source` (widened alongside `View::new`'s own M71 real precedent):
/// when given, used directly instead of reading `path` from disk, while
/// `path` still supplies the real base directory `include:` resolves
/// against below. `None` (the default) is the real, pre-existing
/// behavior -- read `path` directly -- unchanged for every existing
/// caller. Closes the real, confirmed gap `View::new`'s own M71 doc
/// comment already named: the sibling `Tesserae` project's `component:`
/// macro-expansion layer could reach a top-level `View` via this same
/// mechanism, but had no way to reach an *embedded* component, since
/// this function always read `path` straight from disk with no
/// override at all.
#[allow(clippy::too_many_arguments)]
pub(crate) fn instantiate_component(
    tree: &Rc<RefCell<Tree>>,
    into: NodeId,
    handlers: &HandlerMap,
    context_menus: &Rc<RefCell<HashMap<NodeId, NodeId>>>,
    theme: &SharedTheme,
    completions: &SharedCompletions,
    path: &str,
    source: Option<String>,
) -> PyResult<Component> {
    let yaml = match source {
        Some(text) => text,
        None => std::fs::read_to_string(path).map_err(|e| {
            PyRuntimeError::new_err(format!("failed to read component {path:?}: {e}"))
        })?,
    };
    let base_dir = std::path::Path::new(path).parent();

    let reconciler = {
        let mut tree_mut = tree.borrow_mut();
        Reconciler::load(&mut tree_mut, &yaml, None, None, None, None, base_dir)
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
        subscriptions: Vec::new(),
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
    /// shared `attach_bindings_and_handlers`, `view.rs`), but -- unlike
    /// `View`, which discards the return value -- keeps every
    /// `(signal, callback)` pair it subscribed, so a later `remove()`
    /// can unsubscribe them again.
    fn _attach(&mut self, py: Python<'_>, viewmodel: Py<PyAny>) -> PyResult<()> {
        let subscriptions = attach_bindings_and_handlers(
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
        self.subscriptions = subscriptions;
        Ok(())
    }

    /// Instantiates another component *inside* this one -- components
    /// nest for free, the same real `instantiate_component` helper
    /// `View.instantiate` itself calls. `source` mirrors `View.
    /// instantiate`'s own M71-style widening -- see `instantiate_
    /// component`'s own doc comment for the real reasoning.
    #[pyo3(signature = (path, into, source=None))]
    fn instantiate(
        &self,
        path: &str,
        into: PyRef<'_, Node>,
        source: Option<String>,
    ) -> PyResult<Component> {
        instantiate_component(
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

    /// M43 Phase 2 (§4, §5, §8, §16.2, §16.6): real, structural teardown
    /// -- unsubscribes every `(signal, callback)` pair `_attach`
    /// registered (via `Signal._unsubscribe`, `python/tre/__init__.py`),
    /// *then* removes this instance's whole subtree from the shared
    /// `Tree` (`Tree::remove`, confirmed by reading its own body to
    /// already recurse the whole subtree deepest-first, cleaning up
    /// `taffy`/the parent's children list/overlays/focus -- no new
    /// `engine-core` work needed).
    ///
    /// **Real, load-bearing ordering, not incidental:** unsubscribing
    /// *before* removing means a `Signal` write that happens to race
    /// with this call (unlikely in this single-threaded runtime, but a
    /// real ordering worth being deliberate about) can never reach a
    /// `BindingCallback` whose own `node_id` is already gone from the
    /// `Tree` -- the real bug this whole phase exists to prevent
    /// (`apply_binding_value`'s own `tree.borrow_mut()...` calls
    /// `.expect()` a `NodeId` still present).
    ///
    /// **Real, honestly-stated v1 limit, not silently glossed over:**
    /// this instance's own `on_click`/`on_change`/two-way `Change`
    /// handler entries stay in the *shared* `handlers`/`context_menus`
    /// maps -- harmless (a removed `NodeId` can never be hit-tested
    /// again, so they're never looked up), but not swept. A real,
    /// bounded follow-up if a long-running, high-churn app ever shows
    /// this mattering, not built ahead of a real need.
    fn remove(&mut self, py: Python<'_>) -> PyResult<()> {
        for (signal, callback) in self.subscriptions.drain(..) {
            signal.bind(py).call_method1("_unsubscribe", (callback,))?;
        }
        self.tree.borrow_mut().remove(self.reconciler.root());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Same real "genuinely fresh file per run" pattern `view.rs`'s own
    /// M71 tests already established -- `instantiate_component` reads a
    /// real path from disk absent `source`, so these tests need one.
    fn write_temp_component(yaml: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "engine_py_component_test_{}_{}.yaml",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(&path, yaml).expect("create test component file");
        path
    }

    /// A real outer `Tree` + root node to instantiate a component into
    /// -- built the identical way `View::new` itself builds these same
    /// fields (`view.rs`), not `View::new` directly (a private, pyo3-
    /// only constructor `component.rs`'s own module has no access to).
    struct OuterFixture {
        tree: Rc<RefCell<Tree>>,
        into: NodeId,
        handlers: HandlerMap,
        context_menus: Rc<RefCell<HashMap<NodeId, NodeId>>>,
        theme: SharedTheme,
        completions: SharedCompletions,
    }

    fn outer_fixture() -> OuterFixture {
        let mut tree = Tree::new();
        let reconciler = Reconciler::load(
            &mut tree,
            "id: root\nkind: Container\nstyle: {width: 100, height: 100}\n",
            None,
            None,
            None,
            None,
            None,
        )
        .expect("real outer Reconciler");
        OuterFixture {
            into: reconciler.root(),
            tree: Rc::new(RefCell::new(tree)),
            handlers: Rc::new(RefCell::new(HashMap::new())),
            context_menus: Rc::new(RefCell::new(HashMap::new())),
            theme: Rc::new(RefCell::new(crate::window::ThemeState::default())),
            completions: Rc::new(RefCell::new(crate::dispatch::CompletionRegistry::new())),
        }
    }

    /// `source=`, when given, is used instead of reading `path` from
    /// disk -- the real, confirmed gap this milestone closes: proven
    /// directly by writing a real on-disk component file with one
    /// `width`, then instantiating with `source=` naming a *different*
    /// `width` and confirming the live `Tree` reflects `source`'s own
    /// value, not the file's (the identical real proof `view.rs`'s own
    /// `source_override_is_used_instead_of_reading_path_from_disk`
    /// already uses for `View::new`).
    #[test]
    fn instantiate_component_source_override_is_used_instead_of_reading_path_from_disk() {
        let outer = outer_fixture();

        let component_path =
            write_temp_component("id: inner\nkind: Container\nstyle: {width: 40, height: 20}\n");
        let component = instantiate_component(
            &outer.tree,
            outer.into,
            &outer.handlers,
            &outer.context_menus,
            &outer.theme,
            &outer.completions,
            &component_path.to_string_lossy(),
            Some("id: inner\nkind: Container\nstyle: {width: 999, height: 20}\n".to_string()),
        )
        .expect("real Component");

        let tree = outer.tree.borrow();
        let inner = component
            .reconciler
            .id_of("inner")
            .expect("inner widget id");
        let style = &tree.get(inner).expect("inner node").layout_style;
        assert_eq!(
            style.size.width,
            taffy::prelude::length(999.0),
            "source= must be used instead of the real on-disk component file's own content"
        );

        let _ = std::fs::remove_file(&component_path);
    }

    /// `source=None` (the default) is the real, pre-existing behavior,
    /// unchanged -- a component instantiated with no override still
    /// reads its real file from disk exactly as it always has.
    #[test]
    fn instantiate_component_with_no_source_reads_the_real_file() {
        let outer = outer_fixture();

        let component_path =
            write_temp_component("id: inner\nkind: Container\nstyle: {width: 40, height: 20}\n");
        let component = instantiate_component(
            &outer.tree,
            outer.into,
            &outer.handlers,
            &outer.context_menus,
            &outer.theme,
            &outer.completions,
            &component_path.to_string_lossy(),
            None,
        )
        .expect("real Component");

        let tree = outer.tree.borrow();
        let inner = component
            .reconciler
            .id_of("inner")
            .expect("inner widget id");
        let style = &tree.get(inner).expect("inner node").layout_style;
        assert_eq!(style.size.width, taffy::prelude::length(40.0));

        let _ = std::fs::remove_file(&component_path);
    }
}
