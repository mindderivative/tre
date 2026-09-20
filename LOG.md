# LOG — M43 Phase 1: Real Component Instantiation with an Independent `ViewModel`

- User's own governing instruction, reached through a real technical
  discussion, not assumed: "how do we make it so multiple views can
  use a single viewModel? Or no viewModel at all" -> "I want to be able
  to embed a view into another view and have the embedded view have a
  separate viewModel. The essence of MVVM and single page
  applications." Two architectural options were presented directly:
  (1) one shared `Tree`, multiple `ViewModel`-scoped regions -- the
  real Vue/React component pattern, zero `engine-core` rendering/
  dispatch changes needed; (2) genuinely separate `Tree`s composed via
  a cross-tree render -- would need `engine-core`/`engine-render` to
  become tree-of-trees aware, crossing crate boundaries §4 deliberately
  keeps separate. **User confirmed option 1, with multiple simultaneous
  instances required.** Approved via a formal plan (`EnterPlanMode`/
  `ExitPlanMode`).
- Real investigation before writing any code (grounding the whole plan,
  not assumed): `engine_spec::build_tree` (already `pub`, re-exported
  at `lib.rs:22`) inserts a `WidgetSpec` into an *existing* `Tree` and
  returns the new subtree's root as a parentless node ("attach it under
  another with `add_child`, or leave it as a root" -- `Tree::insert`'s
  own doc comment). `Reconciler::load` already does parse+build+id-
  recording in one call. `Tree::add_child` attaches a parentless node
  under an existing one. `Tree::remove` -- read directly, not assumed
  -- already recurses a whole subtree deepest-first, cleaning up
  `taffy`/the parent's children list/overlays/focus. **Real, decisive
  finding: `engine-spec` and `engine-core` needed zero changes.** Every
  primitive this feature needs already existed and was already `pub`.
- `python/tre/__init__.py`'s `Signal` was confirmed (by reading the
  file in full) to have `_subscribe` but no `_unsubscribe` -- a real
  gap relevant to Phase 2 (removal), noted but not yet fixed in this
  phase.
- `crates/engine-py/src/view.rs`: `collect_bindings`/`collect_handlers`/
  `collect_two_way` widened from private `fn` to `pub(crate) fn`.
  `View::_attach`'s own ~150-line body (handler validation/wiring +
  binding evaluation/subscription/two-way) was factored into a new,
  shared `pub(crate) fn attach_bindings_and_handlers` -- `id_of` taken
  as a closure (`impl Fn(&str) -> Option<NodeId>`) rather than a
  `&Reconciler` directly, since `View` and the new `Component` each
  look widget ids up through their own separately-owned `Reconciler`.
  Returns every `(signal, callback)` pair it subscribed -- `View::
  _attach` discards this (a `View` lives as long as the script does);
  `Component::_attach` keeps it, ready for Phase 2's own removal work.
  `View::_attach` itself became a thin wrapper; behavior confirmed
  byte-for-byte unchanged by the full pre-existing `test_view_binding.
  py`/`test_view_handlers.py`/`test_view_in_window.py` suites passing
  with zero modifications.
- New `crates/engine-py/src/component.rs`: `Component { tree: Rc<RefCell
  <Tree>>, reconciler: Reconciler, bindings, declared_handlers,
  two_way, handlers: HandlerMap, context_menus, theme: SharedTheme,
  completions: SharedCompletions }` -- deliberately mirrors `View`'s
  own struct shape (a real `Reconciler` field, not a separately-
  extracted `HashMap<String, NodeId>`) rather than the plan's own
  original sketch (`root: NodeId, ids: HashMap<...>`), since `Reconciler
  ::load` already returns exactly what's needed (`.root()`/`.id_of()`)
  -- a real, small simplification found while implementing, not a
  deviation from the plan's own intent. `tree`/`handlers`/
  `context_menus`/`theme`/`completions` are `Rc::clone`s shared with
  whatever it was instantiated into -- safe because it's the *same*
  `Tree`, so every `NodeId` is unique by construction (the opposite
  situation from M42 Phase 2's own `ActiveTree`, which had to bundle
  `handlers`/`context_menus` together specifically because *separate*
  Trees can collide).
- New `instantiate_component` helper (`component.rs`), called from both
  `View.instantiate(path, into)` (`view.rs`) and `Component.instantiate
  (...)` (so components nest recursively for free, with zero special-
  casing) -- reads the component's own YAML, `Reconciler::load`s it
  into the shared `Tree`, `Tree::add_child`s its root under the target
  node, then `parse_view_with_includes`'s the same yaml a second time
  (the identical "parse twice" pattern `View::new` itself already
  established) to collect this instance's own scoped bindings/handlers.
- **Real design correction, found during implementation, not in the
  plan's own text -- the single most load-bearing finding this phase
  made:** `Component` does *not* get `click`/`hover`/`right_click`
  methods of its own. Traced through exactly what `Tree::compute_layout
  (root, available_space)` does: it computes a fresh layout treating
  `root` as if it were the whole tree's own top-level root, using
  `available_space` as its outer constraint -- calling this from a
  component's own root would silently distort its real size (actually
  determined by its real parent container, not an independent
  "available space" of its own). Read `Tree::absolute_position`'s own
  body directly to confirm the real fix: it already walks a node's
  real parent chain all the way up to whatever ancestor has no parent,
  regardless of which root `compute_layout` was last called with --
  meaning dispatch on an embedded component's own node is *already*
  fully correct through the *owning* `View`/`Window`'s existing `click`/
  `hover`/`right_click` (`view.click(component.node("button"))`), with
  zero new dispatch code needed. This removed an entire planned layer
  of complexity (a `window_root`/`available_space` reference `Component`
  would otherwise have needed to thread through nested instantiation).
- `Component` registered in `lib.rs`'s `#[pymodule]`; re-exported from
  `python/tre/__init__.py` and its `__all__`; `_core.pyi` updated with
  `View.instantiate`/the new `Component` class (mirroring `View`'s own
  stub, minus `click`/`hover`/`right_click`/`__init__` -- not directly
  constructible from Python).
- New pytest tests (`tests/test_component.py`, 7 new): instantiate
  returns a usable component; multiple instances resolve to distinct
  real state (proven via `Node.set_text`'s own immediate, synchronous
  effect -- `Node` has no `.id` getter exposed to Python, an early
  draft of this test tried to compare `.id` directly and failed with a
  real `AttributeError`, fixed by switching to an observable-behavior
  proof instead); each instance's own `ViewModel` is genuinely
  independent; a click via the owning `View` reaches only the right
  instance's own handler, proven with 3 simultaneous instances;
  components nest recursively; a bad path and an unknown widget id
  both raise clearly.
- New live example (`examples/component_list.py` + `component_list.
  yaml` + `component_list_card.yaml`): 3 real `Card` component
  instances in one container, each its own `Signal`-bound counter, a
  real dispatched click on only the second instance's own button
  (twice) leaving the other two untouched, then a genuine 20-frame
  `App.run()`. Ran clean on the first real attempt.
- Full verification chain, all green: `cargo check --workspace --all-
  targets`; `cargo clippy --workspace --all-targets -- -D warnings`;
  `cargo fmt` + `cargo fmt --check`; `cargo test --workspace --release`
  (213 in engine-py's own lib suite, unchanged -- `Component`'s own
  methods need a live Python interpreter to call through pyo3's
  generated wrapper, so no new Rust-level `#[test]`s were added,
  matching this crate's established GIL-needed/GIL-free test-surface
  split); `maturin develop --release` rebuilt; `pytest tests/` (596
  passed, +7, 1 skipped, unchanged -- confirming zero regressions from
  the `View::_attach` refactor); all 80 examples (+1) and the showcase
  demo run clean.

**M43 Phase 1 -- Real Component Instantiation with an Independent
`ViewModel` -- is complete.** Phase 2 (real removal, with automatic
`Signal` unsubscription) remains open -- the milestone as a whole is
not yet closed, so no push yet per the standing "push only after a
full milestone closes" convention.
