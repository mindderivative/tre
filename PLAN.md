# Plan: M3 Phase 6, Step 12 — `engine-spec`, Full (§14 step 12)

Corresponds to `BUILD_TRACKER.md` M3 Phase 6, step 12 of 1 -- closes
Phase 6 (a single build-order step, but the largest bundled scope in
§14: §16.2 + §16.3 + §16.4 together, explicitly sequenced here because
it needs both `engine-py` (step 6) and MD3 theming (step 11)).

## Goal

Per §14 step 12: "`engine-spec`, full: `BindingResolver` + the
`ViewModel`/`View._attach()` model (§16.2), the stylesheet cascade with
real MD3 token resolution (§16.3, now that step 11 gives it an actual
color scheme), and reconciliation/hot-reload (§16.4)."

This is by far the largest single build-order step so far -- three
substantial sub-specs bundled into one step. Broken into three
sequential, separately-committed **stages** (matching the nested
Stage-level tracking `BUILD_TRACKER.md` already uses for M2's phases),
each with its own real verification, rather than one unreviewable
change:

## Stage A — §16.3 Stylesheet cascade + MD3 token resolution

Self-contained, no missing dependencies -- buildable in full today.

- `WidgetSpec` gains `classes: Vec<String>` (a widget's own applied
  classes -- needed for the cascade's `classes:` selector to match
  against).
- `engine_spec::cascade`: `Stylesheet`/`StyleRule` (`kind`/`classes`/
  `id` selectors + a `StyleSpec` payload), `parse_stylesheet`, and
  `resolve_style(spec, sheet) -> StyleSpec` implementing §16.3's exact
  precedence: baseline (no selector) → `kind:` → `classes:` (more
  classes beat fewer -- applied in ascending specificity order so the
  most-specific match wins) → `id:` → the widget's own inline `style:`.
  Per-field merge (a `StyleSpec` field already set earlier in the
  cascade survives if a later rule leaves it unset), not whole-struct
  replacement.
- `engine_md3::ColorScheme::role(&self, name: &str) -> Option<Color>`:
  centralizes MD3 token-name lookup in the crate that owns the roles,
  not duplicated in `engine-spec`. `engine-spec`'s color resolution
  tries this first, falls back to `peniko::color::parse_color` for
  literal values -- both `background: primary` and `background:
  "#6750A4"` work, with or without an active scheme.
- `build_tree`/`load_view` gain an optional stylesheet + `ColorScheme`
  path (`load_styled_view`), keeping the existing plain `load_view`
  (no cascade, no MD3 tokens) working unchanged for its one existing
  caller (`engine-render/tests/spec_view.rs`).

## Stage B — §16.4 Reconciliation & hot-reload

- `engine_core::Tree::remove(id)`: real, recursive subtree removal
  (`taffy::TaffyTree::remove` only detaches one node and orphans its
  children, per its own doc comment -- checked directly; this crate's
  own `Tree::remove` recurses to actually drop a whole disappeared
  subtree, matching what reconciliation needs). New engine-core surface,
  small and additive.
- `engine_spec::reconcile`: keyed diff between an old and new
  `WidgetSpec` tree, matched by `id` + `NodeKindSpec` (§16.1's own
  stated matching rule) against a live `id -> NodeId` map that
  `build_tree` now returns alongside the root `NodeId`. A matched,
  same-kind node's `PaintProperties`/`layout_style` are patched in
  place (its `NodeId` never changes -- focus/scroll/in-flight
  animations survive, per §16.4's own claim); a node whose `id` or kind
  changed is treated as a removal + fresh insertion; a genuinely new
  `id` is inserted; a disappeared `id` is removed via the new
  `Tree::remove`.
- `engine_spec::watch`: a thin `notify`-crate file watcher
  (`ViewWatcher`), the crate's own direct ownership of file-watching per
  §16.4's text ("no `pyo3` needed to detect a file change"). Verified
  against a real temp-file write, not just "the API compiles."
  Deliberately **not** wired into a live running `engine-py` `App` this
  step -- that needs `App` to gain "this app was loaded from a
  `view.yaml`" as a new concept, real net-new `engine-py` surface with
  no current caller, and no component depends on it existing yet
  (matching every prior step's "prove the mechanism standalone, wire
  into a real running app only once something needs it" discipline).

## Stage C — §16.2 `BindingResolver` + expression grammar + `ViewModel`

- `engine_spec::binding`: `Expression` (a small AST for the whitelisted
  grammar §16.2 names explicitly -- attribute access, indexing,
  comparison, arithmetic, boolean logic, zero-arg method calls) plus a
  hand-written recursive-descent parser for `{{ ... }}` strings
  (no parser-generator dependency for a grammar this small). The
  generic `BindingResolver` trait (the same dependency-inversion shape
  as `AppHandler`, §4): `engine-spec` defines it and evaluates
  `Expression`s against `&dyn BindingResolver`, with zero `pyo3`
  knowledge -- tested here with a fake in-crate resolver before
  `engine-py` ever implements the real one.
- `engine-py`: a `PyViewModelResolver` implementing `BindingResolver`
  via real `pyo3` attribute access (`getattr`, index access, zero-arg
  method calls) against a stored `Py<PyAny>` ViewModel.
- `python/tre/`: a real `Signal[T]` (value + subscriber list),
  `ViewModel` base class, and `View`/`View._attach()` implementing the
  dependency-tracking "evaluate once inside a recording scope, subscribe
  to whatever was read" mechanism §16.2 describes, plus eager
  validation at `_attach()` time (a bad handler/binding name fails at
  startup, not on first click).

Handlers/two-way bindings' underlying event dispatch (§16.2's
`set_on_click`-style wiring, §16.7) still needs real pointer/keyboard
`InputEvent`/`AppHandler` dispatch, which -- checked directly, same
finding as steps 7/9/11 -- does not exist anywhere in this codebase yet.
This stage builds and proves the resolution/wiring machinery up to
that point (a handler name resolves to a real bound method, a binding
evaluates and re-evaluates on `Signal` writes); actually *firing* a
handler from a real click is deferred to whenever pointer dispatch
lands, the same scope boundary every MD3-interaction step this session
has drawn.

## Verification (per stage)

Each stage: its own new/extended tests pass, plus `cargo test
--workspace`, `cargo clippy --workspace --all-targets -- -D warnings`,
`cargo fmt --check` all clean, plus (Stage C) the Python test suite
(`pytest tests/ -v`) still passes after any `engine-py`/`tre` package
changes. Each stage gets its own local commit as it completes.
