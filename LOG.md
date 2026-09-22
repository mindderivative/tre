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

## Status

**M54 Phase 1 of 3 is complete.** `engine-core`'s own real mechanical
data (position/button already reachable from `engine-py`'s own
in-scope `InputEvent`; `Changed`'s real, correctly-typed `old_value`)
is proven. Committing locally now. Up next: Phase 2, the `engine-py`
`Event` pyclass, arity-sniffing dispatch, and threading real data
through every call site.
