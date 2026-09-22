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

**All 3 phases complete. Milestone closed.**

Phase 1: `DispatchOutcome::Changed` widened to `Changed { node,
old_value: ChangedValue }`; every real producer in `tree.rs` snapshots
the old value before mutating (TextField edits via `dispatch_text_
field_key`/the top-level `TextInput` arm; Slider via `dispatch_slider_
key`'s arrow-nudge; Slider/TimePickerDial pointer drag-end via a new
`drag_start_value` field, snapshotted at drag-start since the value
moves continuously during the drag itself). `Activated`/`HoverChanged`
deliberately left untouched -- a real, found-while-implementing
correction to the original plan's own assumption that they'd need
widening too; `engine-py` already holds the relevant `InputEvent` data
at every real call site. 20 existing Rust unit tests updated to assert
the real, correct `old_value` for every already-covered `Changed`-
producing scenario.

Phase 2: new `Event` pyclass (`crates/engine-py/src/event.rs`) --
`kind`/`source`/`position`/`button`/`old_value`/`new_value`, plain
`#[pyo3(get)]` attribute access (a deliberate, justified departure from
`Node`'s own explicit-getter convention, since `Event` is an immutable
snapshot, not live `Tree` state). `dispatch::wants_event_payload`
arity-sniffs a handler's real required-parameter count at registration
(`inspect.signature`, correctly treating `lambda i=i: ...`-style
defaulted params, `VAR_POSITIONAL`/`VAR_KEYWORD`/`KEYWORD_ONLY` as not
requiring the new argument) -- `HandlerMap`'s stored value widens to
`(Py<PyAny>, bool)`. `call_handler` takes a lazy `Event`-builder
closure, invoked only when a handler is found and wants one.
`run_dispatch_outcome` widened to take `tree`/`event: Option<&
InputEvent>` -- `Activated`/`HoverChanged` extract position/button by
pattern-matching `event` directly (no `engine-core` data needed, per
Phase 1's own correction); `Changed` builds `old_value` from
`ChangedValue` and reads `new_value` fresh from `tree`. Every real call
site rewired: `node.rs`'s four setters, `window_input.rs`'s `cut()`,
`app.rs`'s real winit path, six synthetic entry points, `view.rs`'s
declarative equivalents, GC traversal loops.

Phase 3: `python/tre/_core.pyi` gets a new `Event` class stub and
widened handler-parameter types; `python/tre/__init__.py` re-exports
`Event`. 12 new pytest tests across `test_click_dispatch.py`/`test_
change_event.py`/`test_hover_events.py` (extended) and a new `test_
event_payload.py` (the cross-cutting arity-sniff mechanism itself) --
one real, found-while-testing correction along the way: `HoverExit`/
`HoverEnter` share the *same* pointer position (wherever the pointer
now is), not each node's own former center, since both come from one
real `PointerMoved` dispatch. New `examples/event_payload.py` --
deliberately does *not* rewrite `demo/showcase.py`'s own `toggle_
checkbox` (a real `Click` handler, not `Change` -- `Event.old_value`
wouldn't actually help its own "what to toggle to" question, a
distinction the original investigation named up front and this phase
honored rather than force-fitting).

Full chain green: `cargo check`/`clippy -D warnings`/`fmt --check`
clean, `cargo test --workspace --release` (unchanged -- every new
pymethod/pyclass is GIL-bound, pytest-covered instead), `maturin
develop --release`, `pytest tests/` (767 passed, up from 755, +12, 2
skipped unchanged), all 86 examples (+1), showcase demo. Tracker
generator: 54 milestones/162 phases/291 items/1 known gap/20 fixed
gaps.
