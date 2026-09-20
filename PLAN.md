# PLAN — M43 Phase 1: Real Component Instantiation with an Independent `ViewModel`

## Goal
Embed a view inside another view where the embedded content gets its
own, separate `ViewModel`, supporting multiple simultaneous instances
-- "the essence of MVVM and single page applications," per the user's
own words. Option 1 of two weighed architectures (one shared `Tree`,
multiple `ViewModel`-scoped regions), confirmed and approved via a
formal plan (`EnterPlanMode`/`ExitPlanMode`).

## Steps
1. Investigated before designing: `engine_spec::build_tree` (already
   `pub`, re-exported) inserts a `WidgetSpec` into an *existing* `Tree`,
   returning a parentless subtree root; `Reconciler::load` does parse+
   build+id-recording in one call; `Tree::add_child`/`Tree::remove`
   (the latter confirmed, by reading its body, to already recurse a
   whole subtree deepest-first) are exactly the primitives needed.
   **`engine-spec`/`engine-core` needed zero changes.**
2. Widened `collect_bindings`/`collect_handlers`/`collect_two_way`
   (`view.rs`) from private `fn` to `pub(crate) fn` so the new
   `component.rs` could reuse them verbatim.
3. Factored `View::_attach`'s own ~150-line body into a new, shared
   `pub(crate) fn attach_bindings_and_handlers` (`view.rs`) -- `View::
   _attach` became a thin wrapper over it; behavior confirmed byte-for-
   byte unchanged by the full pre-existing `_attach`-heavy pytest suite
   passing unmodified.
4. New `crates/engine-py/src/component.rs`: `Component { tree, reconciler,
   bindings, declared_handlers, two_way, handlers, context_menus, theme,
   completions }` -- deliberately mirrors `View`'s own shape (holding a
   real `Reconciler`, not a separately-extracted id map). New
   `instantiate_component` helper, called from both `View.instantiate`
   and `Component.instantiate` (so components nest recursively for
   free) -- reads the component's own YAML, `Reconciler::load`s it into
   the shared `Tree`, `Tree::add_child`s its root under the target
   node, collects its own scoped bindings/handlers via the
   now-`pub(crate)` collection functions.
5. **Real design correction, found during implementation, not in the
   plan text:** `Component` has no `click`/`hover`/`right_click` of its
   own. `Tree::compute_layout(root, available_space)` called from a
   component's own root would compute a fresh layout as if that root
   were the whole tree's top level -- silently distorting its real,
   parent-constrained size. Confirmed via reading `Tree::
   absolute_position`'s own body: it already walks a node's real parent
   chain up to whatever ancestor has no parent, so dispatch on an
   embedded node is already fully correct through the *owning* `View`/
   `Window`'s existing `click`/`hover`/`right_click`
   (`view.click(component.node("button"))`) -- zero new dispatch code
   needed.
6. Registered `Component` in `lib.rs`'s pymodule; re-exported from
   `python/tre/__init__.py`; updated `_core.pyi`.
7. New pytest tests (`tests/test_component.py`, 7): instantiate returns
   a usable component; multiple instances resolve to distinct real
   state (`Node.set_text`'s immediate effect, since `Node` has no `.id`
   getter); each instance's own `ViewModel` is genuinely independent; a
   click via the owning `View` reaches only the right instance's
   handler (3 simultaneous instances); components nest recursively; bad
   path and unknown widget id raise clearly.
8. New live example (`examples/component_list.py` + 2 yaml files): 3
   real `Card` component instances in one container, each its own
   `Signal`-bound counter, a real dispatched click on only one
   instance's button, then a genuine 20-frame `App.run()`.

## Status
Complete. Full verification chain green: `cargo check`/`clippy -D
warnings`/`fmt`, `cargo test --workspace --release` (213 unchanged --
`Component`'s own methods need a live Python interpreter to call
through pyo3, so no new Rust-level `#[test]`s were added, matching this
crate's established GIL-needed/GIL-free split), `maturin develop
--release`, `pytest tests/` (596 passed, +7, 1 skipped unchanged), all
80 examples (+1), showcase demo. **M43 Phase 1 is complete.** Phase 2
(real removal, with automatic `Signal` unsubscription) remains open.
