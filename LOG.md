# LOG — Branch `0.3.4`: Milestone 94

- User-directed: "Approved, start M94."

## Done

1. M93 closed: revision 2 of `docs/design/target-api.md` approved, with
   R1–R12 as written.
2. M94 scoped against the source. The engine already receives raw
   pointer, key, text, wheel, theme and resize input, but only five
   dispatch outcomes reach Python. `Key` has 12 keys, and Shift is the
   only modifier. Legacy handlers live in one map keyed
   `(NodeId, EventKind)`, touched in a handful of places. `Text` and
   `Icon` are never hit targets.

3. Phase 1 done:
   - `node.on`/`off` listeners share the legacy handler map under a
     widened key.
   - `listeners.rs` routes raw input from a target resolved before
     dispatch, and outcome events from the same places the legacy
     handlers fire.
   - `dispatch::process_input` is the one pipeline used by both
     `App.run()` and `window.simulate`.
   - Pointer capture, and `PointerLeft` for leaving the window.
   - Full key names, and modifiers.
   - Window events, with a cancellable `close_requested`.
   - `window.set`/`get`/`root`.
   - `Node` equality and hashing.

   Verification:
   - `tests/test_listeners.py`: 30 tests.
   - pytest: 1003 passed.
   - Rust, release: 553 passed.
   - All 89 examples and the showcase run cleanly.
   - clippy is clean and `mkdocs --strict` passes.
   - The new `docs/api/python/events.md` documents all of it.

## Status

**M94 Phase 2 in progress.**
