# PLAN — Branch `0.3.4`: Milestone 94, Input and Accessibility Building Blocks

*(Replaces the M93 plan — the target API spec, `docs/design/target-api.md`
revision 2, was approved 2026-09-25. Every step is in `BUILD_TRACKER.md`.)*

## Goal

Everything a framework needs to build interactive widgets itself:
bubbling node events, pointer capture, full keyboard input, window events,
accessibility properties, focus order, cursor shape, and a headless
`window.simulate`. Additive only: the legacy `set_on_*` handlers keep
their exact behavior until M100.

## Design (from the source)

- **Listener storage.** `HandlerMap`'s key widens from
  `(NodeId, EventKind)` to `(NodeId, HandlerKey)`, where `HandlerKey` is
  `Legacy(EventKind)` or `Listener(EventType)`. `node.on` inserts a
  `Listener` key. No new field on `Node`, so none of the 34 construction
  sites change. M100 deletes the `Legacy` variant.
- **Routing.** A new `engine-py/src/listeners.rs`:
  - `Route::before(tree, root, &event)` captures the target before
    dispatch: the focused node for keys and text, and the captured or
    hit node for pointers and the wheel.
  - After `Tree::dispatch` and the legacy `run_dispatch_outcome`,
    `route(...)` delivers the raw event first (`pointer_*`, `wheel`,
    `key_*`, `input`), then the outcome events: enter/leave,
    `click`/`secondary_click`, `blur`/`focus`, `change`.
  - Bubbling walks from the target to the root. One `Py<Event>` is
    reused, with `current`, `x` and `y` updated at each step; `stop()`
    ends the walk.
  - No `Event` is built unless some node on the path is listening.
- **Engine input.** New `InputEvent` variants:
  - `Key { name, pressed, repeat }`, with every key named in snake_case
    from winit;
  - `ModifiersChanged(Modifiers)`;
  - `ScaleFactorChanged`;
  - `PointerLeft`, which clears hover.

  `Tree` gains `pointer_capture` and `window_to_local`, which is
  transform-aware.
- **Window events.** `run_windowed_multi` gains an `on_close_requested`
  callback, which returns whether to close. `closed` fires on every
  window removal. `PyWindow` shares the OS window with the runtime, for
  the title and scale factor.
- **Accessibility and focus.** `AccessNodeData` gains the M93
  accessibility fields plus `focusable` and `tab_index`.
  `collect_access_nodes` derives the offered actions from role and state.
  The Tab order uses HTML semantics: positive `tab_index` values first,
  then tree order, and `-1` is skipped.
- **`node.set`.** Atomic: parse every property into a typed change,
  then apply them all.

## Phases

1. Event routing, pointer, and keyboard.
2. Accessibility, focus, and `set`.
3. `simulate`, tests (including a proof slider), stubs, docs, and the
   full chain.

## Status

Phase 1 in progress.
