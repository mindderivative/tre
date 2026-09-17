# Plan: M14 Phase 3 — Real `Change` EventKind + Two-Way Binding Sugar (§5, §7.3, §16.7)

Corresponds to `BUILD_TRACKER.md` M14 Phase 3's own scoping: a real
`EventKind::Change`, fired two different ways (mechanically for a
Slider drag-release inside `Tree::dispatch`; directly for a Checkbox
via `Node.set_checked`), plus §16.7's two-way binding sugar wiring it
into `View`/`ViewModel` so a bound `Signal` writes back automatically.
Closes M14 entirely (3 of 3 phases).

## Investigation before writing code

- `EventKind`/`DispatchOutcome` (`engine-core/src/input.rs`) currently
  have `Click`/`HoverEnter`/`HoverExit` and `Activated`/
  `SecondaryActivated`/`HoverChanged` respectively — no `Change`/
  `Changed` at all, confirmed by direct read. A Slider drag-release
  today produces no distinguishable outcome (falls through to
  `Activated`, the same as any other release) — the real, missing
  signal this phase adds.
- Design Principle 6 (engine-core never knows "meaning," only
  mechanics) rules out a single uniform `Change`-firing path: a Slider
  drag genuinely completes *inside* `Tree::dispatch` (mechanical), but
  a Checkbox's `checked` is app-owned data `engine-core` never touches
  at all (§5) — its own `Change` can only originate from wherever
  `checked` is actually written, i.e. `Node.set_checked` in
  `engine-py`, never through `Tree::dispatch`.
- `call_handler` (`engine-py/src/dispatch.rs`) is currently a private
  `fn`, confirmed by direct read — `Node.set_checked` needs to reuse it
  directly (not invent a second dispatch path) once it fires `Change`,
  so it needs `pub(crate)`.
- `Node.get`/`Node.animate` (`engine-py/src/node.rs`) already do the
  real "two-level dispatch" ARCHITECTURE.md §8 describes for
  `check_progress`/`thumb_position` (M14 Phases 1/2) — no changes
  needed there for this phase; `checked` itself has no getter yet
  (`get_checked` is new, needed for two-way write-back to read the
  node's own current value back).
- ARCHITECTURE.md §16.7's own inline illustration mixes a `two_way`
  boolean into the same `bindings:` map as per-property expressions
  (`bindings: {text: "{{ username }}", two_way: true}`) — doesn't
  cleanly deserialize into `WidgetSpec.bindings: HashMap<String,
  String>` without either a mixed-type value enum or losing
  `deny_unknown_fields`'s own guarantee across the whole map. A
  separate `WidgetSpec.two_way: Option<String>` field, naming which
  property is two-way, is simpler and fully backward-compatible — a
  deliberate, documented deviation.
- Two-way binding is a `View`/YAML-only concept (`Window` has no
  `_attach`/binding mechanism at all) — testing/demonstrating it needs
  `Checkbox`/`Slider` declarable in `view.yaml`, which they aren't yet
  (`NodeKindSpec` only has `Rect`/`Container`/`Text`, confirmed by
  direct read) — real, additional scope this phase must also cover.

## Design

- `EventKind::Change` (new variant) + `DispatchOutcome::Changed
  (NodeId)` (new variant), both in `engine-core`.
- `Tree::dispatch`'s `PointerReleased` arm: check for `Changed` (a real
  Slider drag ending) *before* clearing `self.dragging`, taking
  priority over `Activated`/`SecondaryActivated`.
- `Node.set_checked` fires `Change` directly via `call_handler` after
  writing `state.checked`, once `call_handler` is `pub(crate)`.
- `Node.get_checked` (new): the missing read-back getter.
- `Node.set_on_change` (new): registers into `handlers[(id,
  EventKind::Change)]`, mirroring `set_on_click`/`set_on_hover_*`.
- `engine-py/src/dispatch.rs::run_dispatch_outcome` gains a `Changed`
  arm calling `call_handler`.
- `WidgetSpec.checked: bool`/`value: f64`/`two_way: Option<String>`
  (all `#[serde(default)]`, additive) + `NodeKindSpec::Checkbox`/
  `Slider` (unit variants) in `engine-spec`; `build.rs::
  node_kind_and_paint` gains matching arms using the existing
  `required_background` helper.
- `View._attach`: `on_change` handler-name mapping; `apply_binding_
  value` gains a `Value::Bool` branch calling `set_checked` (its own
  stale "only numeric" doc comment corrected); a new `TwoWayCallback`
  pyclass registered as the widget's own `Change` handler, reading the
  node's current value back (`get_checked()` or `get(property)`) and
  calling `signal.set(value)`. Restricted to a plain Signal `.get()`
  call (`Expression::Call(Expression::Ident(name), "get")`) — matches
  every other binding's own established `.get()` convention, not a
  bare identifier (ARCHITECTURE.md §16.7's own illustration shows a
  bare one, but every real binding in this codebase already needs
  `.get()` to extract a primitive; a bare identifier resolves to the
  raw `Signal` object itself through `PyViewModelResolver`, an opaque
  `Value::Handle`, not anything `apply_binding_value` can apply
  forward — confirmed only once actual testing below caught it).

## Verification plan

Same discipline as every prior phase: `cargo test --workspace
--release`/`clippy -D warnings`/`fmt --check`, `maturin develop
--release`, full `pytest tests/`, every example script run, then
`LOG.md`/`BUILD_TRACKER.md`/tracker artifact/commit/memory.
