# LOG — M54: Real `Event` Payload for Handlers

- User: "What do you recommend next?" -> I recommended scoping the
  real `Event` payload gap (`BUILD_TRACKER.md`'s own "Known gaps"
  section names it directly). User: "Scope the event payload work."
- Dispatched a dedicated Explore agent for an exhaustive read of every
  handler-invocation site (`call_handler`, `HandlerMap`, `EventKind`),
  every real dispatch-site data source, and existing FFI-boundary
  precedent (`CanvasContext`'s own `call1` pattern) before designing
  anything.
- **Real finding:** exactly one function matters -- `call_handler`
  (`dispatch.rs:299-319`), `handler.call0(py)` for all four `EventKind`
  variants. Nothing is structurally lost: position/button (`Click`)
  and position (`HoverChanged`) are genuinely alive in Rust right up
  until `DispatchOutcome`'s narrow shape discards them; `old_value`
  for `Change` is the one genuine data-loss case (four `node.rs`
  setters overwrite state before calling `call_handler`; `Tree::
  dispatch`'s own `Changed(NodeId)` never snapshots the pre-mutation
  value either). Every real handler in `tests`/`examples`/`demo/
  showcase.py` is zero-argument today -- 133+ confirmed call sites, a
  naive switch to unconditional `call1` breaks all of them.
- Four real design forks resolved via `AskUserQuestion`: (1) arity-sniff
  at registration time for backward compatibility, not a breaking
  change; (2) capture `old_value` (the one thing no existing workaround
  can produce); (3) `Event.source` stays a plain `NodeId` for now, not
  a live `Node` handle; (4) scope stays the existing four `EventKind`s
  (`Click`/`HoverEnter`/`HoverExit`/`Change`), no new kinds this
  milestone.
- Entered Plan Mode with this real, grounded scope before implementing.

## Phase 1 — `engine-core`: Widen `DispatchOutcome::Changed` for Real Old-Value Capture

- **Real correction found while implementing, not assumed in advance:**
  `Activated`/`HoverChanged` need NO `engine-core` widening at all --
  every real `engine-py` call site already holds the originating
  `InputEvent` (or its constituent `position`/`button`) in scope right
  where it calls `run_dispatch_outcome`. Extracting `Click`'s
  position/button and `HoverEnter`/`HoverExit`'s position there
  directly needs zero new `engine-core` data -- confirmed by reading
  `app.rs:765-777`, `window_input.rs`'s six synthetic entry points, and
  `view.rs`'s equivalents.
- **A second real correction, also found while implementing:** `Changed`
  is not uniformly numeric, contrary to the original plan's own
  assumption. Tracing every real producer in `tree.rs` found three
  genuinely different value shapes -- `Slider` drag-end/arrow-nudge
  (`f64`), `TextField` edits (`String`, the most common `Change` in
  practice), and `TimePickerDial` (`{hour: u8, minute: u8}`, not a
  single number at all). Resolved via a second `AskUserQuestion`: a
  small, exact `ChangedValue` enum covering all three (`Text`/`Number`/
  `Time`), not a generic/open-ended shape or a bare `f64`.
- New `engine_core::ChangedValue` enum (`input.rs`). `DispatchOutcome::
  Changed(NodeId)` widened to `Changed { node: NodeId, old_value:
  ChangedValue }` -- `new_value` deliberately not carried here, since
  it's cheaply, correctly recoverable by `engine-py` reading the node's
  current state *after* `dispatch()` returns (the same `get_text()`/
  `get_checked()`-style read every existing workaround in `tests`/
  `examples` already does). `DispatchOutcome` dropped `Eq` from its
  derive (kept `PartialEq`/`Debug`/`Clone`) -- an `f64` field can't
  derive `Eq`; confirmed via grep that every real use across the
  workspace is `assert_eq!`/pattern matching, needing only `PartialEq`.
- Every real `Changed`-producing site in `tree.rs` snapshots the old
  value immediately before mutating: `dispatch_text_field_key`'s
  Backspace/Delete/Space/Enter/Tab arms and the top-level `InputEvent::
  TextInput` arm snapshot `state.content.clone()`; `dispatch_slider_
  key`'s arrow-nudge snapshots `state.thumb_position.current`. The
  pointer-driven Slider/TimePickerDial drag-end case needed the value
  snapshotted at drag *start*, not release (a drag continuously
  overwrites the live value as the pointer moves) -- a new `Tree`
  field, `drag_start_value: Option<ChangedValue>`, set only in the
  Slider/TimePickerDial branch of the real drag-start site (deliberately
  not folded into the shared `dragging: Option<NodeId>` field itself,
  which `Splitter`/`Carousel`/`ScrollView`/`VirtualList` thumb drags
  also use and must stay completely unaffected), read and cleared at
  drag-end.
- Updated 20 existing Rust unit tests in `tree.rs` to assert the real,
  correct `old_value` for every `Changed`-producing scenario already
  covered (Backspace/Delete/Space/Enter/Tab/typed-character TextField
  edits including a real multi-byte UTF-8 case and an active-selection
  replace case; Slider arrow-nudge both directions plus the
  at-minimum-clamp case; a real pointer-driven Slider drag-end,
  confirmed pre-drag not post-release; a real pointer-driven
  TimePickerDial drag-end) -- extending the existing, already-thorough
  suite rather than duplicating scene setups with new tests, this
  codebase's own established convention.
- Full `engine-core` chain green: `cargo check`/`clippy -D warnings`/
  `fmt --check` clean, `cargo test -p engine-core --release` (224
  passed, unchanged count -- every new assertion added to an existing
  test, no new `#[test]` functions needed since the existing coverage
  already exercised every real `Changed`-producing code path).
- `cargo check --workspace --all-targets` confirms the only remaining
  breakage is in `engine-py` (`dispatch.rs`'s `run_dispatch_outcome`
  pattern-matching the old tuple shape; `app.rs`/`view.rs`/
  `window_input.rs` each using `outcome` twice, now blocked by
  `DispatchOutcome` losing `Copy`) -- exactly Phase 2's own scope, not
  a Phase 1 regression.

## Phase 2 — `engine-py`: `Event` Pyclass + Arity-Sniffing Dispatch

- New `Event` pyclass (`crates/engine-py/src/event.rs`, new file):
  `kind: String`, `source: u64` (`engine_core::node_id_as_u64`, the
  exact same `slotmap::KeyData::as_ffi()` encoding this crate already
  uses for `accesskit`/`engine-render`'s own GPU texture cache keys --
  reused, not a third id scheme), `position: Option<(f64, f64)>`,
  `button: Option<String>`, `old_value`/`new_value: Option<Py<PyAny>>`.
  Plain `#[pyo3(get)]` field access -- a deliberate, justified
  departure from this crate's own usual explicit-getter-method
  convention (`Node.get_checked()`/etc.), since `Event` is an
  immutable snapshot handed to exactly one callback invocation, not
  live, mutable `Tree` state.
- `dispatch::wants_event_payload`: arity-sniffs a handler's real
  *required* positional parameter count at registration time, via
  Python's own `inspect.signature` (not `__code__.co_argcount`, which
  only exists on plain functions). Correctly treats `VAR_POSITIONAL`/
  `VAR_KEYWORD`/`KEYWORD_ONLY` parameters and any parameter with a real
  default (`lambda i=i: ...`, used pervasively in this catalog to close
  over a loop index) as not requiring the new argument -- confirmed
  against the real, pervasive pattern this codebase's own examples/
  tests already use. `HandlerMap`'s stored value widens from `Py<PyAny>`
  to `(Py<PyAny>, bool)`; a new `dispatch::register_handler` funnels
  all four `Node.set_on_*` registrations (and `view.rs`'s declarative
  equivalent) through the identical arity-sniff, once.
- `call_handler` widened to take a `impl FnOnce(Python<'_>) ->
  PyResult<Event>` builder, invoked lazily -- only when a handler is
  genuinely found *and* arity-sniffed as wanting one, so building an
  `Event` never happens on a dispatch nothing is listening for.
  `run_dispatch_outcome` widened to also take `tree: &Rc<RefCell<
  Tree>>` and `event: Option<&InputEvent>` -- `Activated`/
  `HoverChanged` build their own `Event.position`/`button` by
  pattern-matching `event` directly (Phase 1's own real correction: no
  `engine-core` data needed); `Changed` builds `old_value` from
  `ChangedValue` and reads `new_value` fresh from `tree`. `Option`, not
  a bare reference -- the one real caller with no originating
  `InputEvent` at all (`app.rs`'s real AccessKit `Action::Click`
  handling, which calls `Tree::activate` directly, a screen reader's
  own semantic request) correctly yields `None`/`None` rather than a
  fabricated position.
- Every real call site rewired: `node.rs`'s four setters
  (`set_checked`/`set_selected`/`set_on`/`set_text`) snapshot the old
  value before mutating and build a real `Change` `Event` with both
  values; `window_input.rs`'s `cut()` (hermetic) gets the identical
  treatment via the newly-`pub(crate)` `read_new_changed_value`;
  `app.rs`'s real winit path, `window_input.rs`'s six synthetic entry
  points, and `view.rs`'s declarative equivalents all thread the real
  `InputEvent` through. `PyWindow`/`View`'s `__traverse__` GC-traversal
  loops updated for the widened `HandlerMap` tuple value.
- Full chain green: `cargo check`/`clippy -D warnings`/`fmt --check`
  clean across the whole workspace; `cargo test --workspace --release`
  (unchanged -- every new pymethod/pyclass is GIL-bound, pytest-covered
  instead). `maturin develop --release`; a standalone smoke script
  confirmed every real path end to end before writing formal tests:
  `Click` position/button, backward-compat zero-arg handlers, `lambda
  i=i:`-style defaulted params, `Checkbox`/`TextField` `Change` (both
  direct `set_text` and a real keyboard Backspace dispatch),
  `HoverEnter`/`HoverExit` position. `pytest tests/` (755 passed, 2
  skipped -- unchanged from before this phase, confirming zero
  regressions to any pre-existing zero-argument handler across the
  whole suite before Phase 3's own new tests were added).

## Phase 3 — Python-Facing API, Tests, Example, Docs

- `python/tre/_core.pyi`: new `Event` class stub with every field
  documented; `set_on_click`/`set_on_hover_enter`/`set_on_hover_exit`/
  `set_on_change` widened to `Callable[[], object] | Callable[[Event],
  object]`; the module's own stale "always zero arguments, no Event
  object exists yet" doc-comment claim corrected. `python/tre/
  __init__.py` re-exports `Event`.
- 12 new pytest tests: `tests/test_click_dispatch.py` (`Click`'s real
  position/button, and `None`/`None` for a real keyboard `Tab`+`Enter`
  activation), `tests/test_change_event.py` (`Checkbox` `Change`
  old/new `bool`, `TextField` `Change` old/new `str` via both `set_
  text` and a real keyboard Backspace, a `Slider` `Change` old/new
  `float`), `tests/test_hover_events.py` (`HoverEnter`/`HoverExit`
  real shared position), and a new `tests/test_event_payload.py` for
  the cross-cutting arity-sniff mechanism itself (a bound method with
  only `self`, a bound method with one real param, `lambda i=i:`-style
  defaulted params, a keyword-only-param handler, `TimePickerDial`'s
  own `(hour, minute)` tuple value, `Event.source`'s stability across
  two distinct real nodes).
- **Real, found-while-testing correction, not assumed in advance:** an
  initial `test_hover_events.py` draft expected each hover transition's
  own `Event.position` to be that specific node's own real center --
  wrong. Both `HoverExit` and `HoverEnter` fire from the *identical*
  real `PointerMoved` dispatch (the move onto the new node), so they
  share the *same* real position (wherever the pointer now is), not
  each node's own former center. Fixed the test's own expectation to
  match the real, correct behavior.
- New `examples/event_payload.py`: a real, live window demonstrating
  `Click`/`HoverEnter`/`HoverExit`/`Change` all receiving a real
  `Event`, side by side with one plain zero-argument handler proving
  the two calling conventions genuinely coexist. **Deliberately does
  not rewrite `demo/showcase.py`'s own `toggle_checkbox`** -- a real,
  checked design point: that handler is on `Click`, not `Change`, so
  `Event.old_value`/`new_value` wouldn't actually help its own "what to
  toggle to" question: force-fitting it in would misrepresent what
  this milestone's payload is for.
- `BUILD_TRACKER.md`: Phase 2 and Phase 3 sections added, milestone
  marked ✅ complete, Top Metrics updated to 100%, the closed "Known
  gaps" bullet moved to "Fixed gaps". Regenerated: 54 milestones/162
  phases/291 items/1 known gap/20 fixed gaps. Artifact republished.
- Full chain green: `cargo check`/`clippy -D warnings`/`fmt --check`
  clean (no Rust changes this phase); `cargo test --workspace
  --release` (unchanged). `maturin develop --release`; `pytest tests/`
  (767 passed, up from 755, +12, 2 skipped unchanged); all 86 examples
  (+1, zero failures); `demo/showcase.py` (all 5 phases, exit 0).

## Status

**M54 is complete -- all 3 phases.** The real capability gap this
milestone exists to close -- every handler invoked zero-argument, no
real `Event` object with payload data anywhere -- is closed, with zero
breaking changes to any of the 755 pre-existing tests or 85 pre-
existing examples. Two real, found-while-implementing corrections to
the original plan (documented honestly, not glossed over): `Activated`/
`HoverChanged` needed no `engine-core` widening at all; `Changed`'s
value needed a 3-variant typed enum, not a bare `f64`. Committing
locally now; push deferred pending explicit user confirmation, per
this session's own established convention.
