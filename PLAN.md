# Plan: M4 Phase 4 — Wire `View`'s Declarative Handlers to Real Dispatch (§16.2)

## Context

`View::_attach` (`crates/engine-py/src/view.rs`) validates every declared
`handlers` entry eagerly (`getattr(viewmodel, method_name)` + a
callability check) but never registers the validated method anywhere
real dispatch reaches — confirmed by direct code reading, not
assumption. The module's own doc comment already named this as its
scope boundary before M4 existed: "Actually *firing* a handler from a
real click needs `InputEvent`/`AppHandler` pointer dispatch, which...
doesn't exist anywhere in this codebase yet." That dispatch now exists
(M4 Phases 1-3), but `_attach` was never revisited once it landed, so a
`view.yaml`'s `{on_click: "bump"}` still does nothing when actually
clicked today.

## Investigation before writing code

Re-read §16.2 in full. Confirmed directly against the current
codebase (not assumed):

- `View` owns its own standalone `Tree` (`View::new` calls `Tree::new()`
  directly) — it is **not** embedded into any `PyWindow`'s tree, and has
  no width/height/render-loop concept at all. Giving `View` a real,
  live winit-driven render loop (parity with `PyWindow`'s whole
  lifecycle) is real, separate, much larger work than this phase's
  actual, confirmed gap — not manufactured here ahead of a stated need.
- `View` already has its own `click_handlers: Rc<RefCell<HashMap<NodeId,
  Py<PyAny>>>>` (added in an earlier session for GC-safety consistency)
  and its own `__traverse__`/`__clear__` visiting it — the storage
  already exists, just never populated.
- `Node::set_on_click` (`node.rs`) is the exact existing mechanism that
  inserts into a `click_handlers` map and adds `Action::Click` to
  `access.actions` — already reused verbatim by `apply_binding_value`'s
  sibling code for `animate()`. The same reuse pattern applies here.
- `Reconciler::root() -> NodeId` already exists (`engine-spec`), giving
  `View` the same "one root `NodeId`" shape `PyWindow.root` has, without
  needing a new field.
- `PyWindow.click(node)` (M4 Phase 1 step 3) is the established,
  no-live-window-needed pattern for proving a dispatch path fires for
  real: compute layout, find the node's real center, dispatch a
  primary press+release pair there. `View` needs the identical method,
  using `AvailableSpace::MaxContent` in place of `PyWindow`'s own
  fixed width/height (every existing test `view.yaml` already declares
  an explicit `style.width`/`style.height` on its root widget, so
  `MaxContent` sizing is correct, not a workaround).

**Scope narrowed accordingly:** this phase does NOT give `View` a real
render loop or embed it into `PyWindow`. It wires the one thing that's
actually broken — `on_click` handlers never reaching real dispatch —
using the same no-window-needed proof pattern M4 Phase 1 already
established, and defers "make a YAML view actually run in a live
window" as separate, future, larger work (not yet named by any real
stated need).

## Approach

1. **`View::_attach`** — after validating a handler, if `event ==
   "on_click"`, look up the widget's `NodeId` via
   `self.reconciler.id_of(widget_id)` and call the exact same
   `Node::set_on_click` mechanism `Node.set_on_click` exposes, via a
   temporary `Node` value (the same construction `apply_binding_value`
   already uses for `animate`), passing the validated, already-`getattr`'d
   bound method. Other declared event names stay validate-only, matching
   §16.2's own "generalizing to whatever named events a `NodeKind`
   exposes" — not manufactured ahead of Phase 6's `EventKind` work.
2. **`View.click(node, py)`** — new method on `View`, mirroring
   `PyWindow.click` exactly: compute layout over `self.reconciler.root()`
   with `AvailableSpace::MaxContent` on both axes, find `node`'s real
   center via `absolute_position`/`layout()`, dispatch a primary
   press+release pair there, running `dispatch::run_activation` against
   `self.click_handlers` for each.
3. **Tests** — new `tests/test_view_handlers.py`: a `view.yaml` with
   `{on_click: "bump"}`, a `ViewModel` whose `bump` mutates a `Signal`,
   `view.click(view.node("root"))`, assert the `Signal`'s value actually
   changed — proving a real dispatched click invokes the bound method,
   not just that `_attach` validated it exists. Also cover: a handler
   for a non-`on_click` event name still validates but a click doesn't
   invoke it (no mechanism exists yet); an uncaught exception in a
   real `bump` is caught the same way `Window.click()`'s already is
   (`PyErr::print`, §9's policy), not propagated.

## Files to touch

- `crates/engine-py/src/view.rs` — `_attach`'s handler loop, new
  `click` method.
- `tests/test_view_handlers.py` — new.

## Verification

- `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D
  warnings`, `cargo fmt --check`.
- `maturin develop && python -m pytest tests/ -v` — full suite plus the
  new file.
