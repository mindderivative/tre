# PLAN — M54: Real `Event` Payload for Handlers

## Goal
`BUILD_TRACKER.md`'s own "Known gaps" section names one real, still-open
capability gap: every registered handler is invoked with zero arguments
(`call_handler`'s own `handler.call0(py)`) -- no real `Event` object
carrying payload data exists anywhere. User: "What do you recommend
next?" -> I recommended scoping this. User: "Scope the event payload
work."

## Real investigation
Exactly one function matters: `call_handler` (`dispatch.rs:299-319`).
`HandlerMap` is keyed `(NodeId, EventKind)`, four registration methods
in `node.rs`, also reached via `view.rs`'s declarative bindings -- one
map, one call site. Nothing is structurally lost for `Click`/
`HoverChanged` -- position/button/hover-position are genuinely alive in
`engine-py`'s own already-in-scope `InputEvent` at every real dispatch
call site. `old_value` for `Change` is the one genuine data-loss case,
and it comes from three genuinely different value shapes (`Slider`:
`f64`, `TextField`: `String`, `TimePickerDial`: `{hour, minute}`), found
while implementing Phase 1, not assumed in advance. Every real handler
in `tests`/`examples`/`demo/showcase.py` is zero-argument today --
133+ confirmed call sites, a naive switch to unconditional `call1`
breaks all of them.

## Design (3 phases)
1. `engine-core`: widen `DispatchOutcome::Changed` to carry a real,
   correctly-typed `old_value` (`ChangedValue::Text`/`Number`/`Time`).
   `Activated`/`HoverChanged` need no widening at all -- `engine-py`
   already holds the data.
2. `engine-py`: new `Event` pyclass, arity-sniffing at registration
   (backward-compatible with every existing zero-arg handler), `call_
   handler`/`run_dispatch_outcome` threaded to build and pass it.
3. Python-facing API, tests, examples, docs.

## Four real design forks, resolved via `AskUserQuestion`
1. Backward compatibility: arity-sniff at registration (not a breaking
   change).
2. `old_value`: capture it (the one thing no existing workaround can
   produce).
3. `Event.source`: a plain `NodeId` for now, not a live `Node` handle.
4. `EventKind` scope: the existing four kinds only -- no `Focus`/
   context-menu kind this milestone.

## Explicitly out of scope, named not silent
New `EventKind` variants (`Focus`, a distinct context-menu/secondary-
click kind). `Event.source` minting a live `Node` handle. Any change to
`SecondaryActivated`/`open_context_menu`'s own `HandlerMap`-bypassing
mechanism.

## Status

**Phase 1 of 3 complete.** `DispatchOutcome::Changed` widened to
`Changed { node, old_value: ChangedValue }`; every real producer in
`tree.rs` snapshots the old value before mutating (TextField edits via
`dispatch_text_field_key`/the top-level `TextInput` arm; Slider via
`dispatch_slider_key`'s arrow-nudge; Slider/TimePickerDial pointer
drag-end via a new `drag_start_value` field, snapshotted at drag-start
since the value moves continuously during the drag itself).
`Activated`/`HoverChanged` deliberately left untouched -- a real,
found-while-implementing correction to the original plan's own
assumption that they'd need widening too; `engine-py` already holds
the relevant `InputEvent` data at every real call site. 20 existing
Rust unit tests updated to assert the real, correct `old_value` for
every already-covered `Changed`-producing scenario. `cargo check -p
engine-core`/`clippy -D warnings`/`fmt --check` clean; `cargo test -p
engine-core --release` (224 passed, unchanged count). `cargo check
--workspace --all-targets` confirms the only remaining breakage is in
`engine-py`, exactly Phase 2's own scope. **Up next: Phase 2, the
`engine-py` `Event` pyclass, arity-sniffing dispatch, and threading
real data through every call site.**
