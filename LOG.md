# Log: M4 Phase 1, Steps 2-3 — Wire Real Input to `winit`/`engine-py`

Corresponds to `PLAN.md`/`BUILD_TRACKER.md` M4 Phase 1, steps 2-3 of 3. Step 1 (committed `c761d1c`) built the core mechanism entirely inside `engine-core` — `InputEvent`/`AppHandler`/`DispatchOutcome` and `Tree::hit_test`/`update_hover`/`move_focus`/`dispatch` — with no `winit`/`pyo3` involved. This closes the loop: real `winit` events now reach `dispatch`, and a real activation now calls a real registered Python callback. `Node.set_on_click` finally exists.

## What happened

### Step 2 — Real `winit` event translation (`engine-platform`)

**Real APIs verified directly first, not assumed**, against the pinned `winit = "0.30.13"`'s own source: `WindowEvent::CursorMoved { position: PhysicalPosition<f64>, .. }`, `MouseInput { state: ElementState, button: MouseButton, .. }`, `KeyboardInput { event: KeyEvent, .. }`, `ModifiersChanged(Modifiers)`. Two real corrections to prior assumptions surfaced this way: `MouseButton` has **six** real variants (`Left`/`Right`/`Middle`/`Back`/`Forward`/`Other(u16)`) — step 1's own `PointerButton` doc comment had claimed an unverified 1:1 three-variant match, now corrected in both `engine_core::PointerButton` and this crate's own `translate_pointer_button`; and shift state isn't carried on `KeyEvent` itself — `ModifiersChanged` is a separate event, so `PerWindow` gained a `modifiers: ModifiersState` field, updated there and read via `.shift_key()` when translating a `KeyboardInput`.

Two new standalone translation functions, `translate_pointer_button`/`translate_key`, map `winit`'s real types onto `engine_core::PointerButton`/`Key` — `Back`/`Forward`/`Other` mouse buttons and every key outside §10's own minimal `Tab`/`Enter`/`Space`/`Escape` vocabulary translate to `None` (no `InputEvent` at all), a stated narrowing, not silently mishandled cases.

`run_windowed_multi` gained a new `on_input: FnMut(WindowId, InputEvent)` closure parameter, matching `on_frame`/`build_access_update`'s own existing shape exactly — `engine-platform` never touches a `Tree` itself (it doesn't have one; generic over whatever the caller does with the translated event). `run_windowed` (the byte-compatible single-window wrapper) passes a no-op `|_, _| {}`, since neither existing caller (`rect_window.rs`, `access_button.rs`) needs input events. `PerWindow` also gained `last_cursor_position: Point`, tracked from `CursorMoved` — `winit`'s own `MouseInput` carries no position, it always corresponds to the cursor's last known location.

**Real, verified-first constraint that shaped the test strategy**: `winit` has no public API to inject a synthetic `WindowEvent` into a live event loop from test code (`EventLoopProxy::send_event` only carries the loop's own custom user-event type `T`, already used here for `PlatformEvent`) — checked directly before assuming this was testable the same way `multi_window.rs` tests window lifecycle. The real, honest proof is at the translation-function level instead: 4 new unit tests exercise `translate_pointer_button`/`translate_key` directly against real `winit` enum values, no live `EventLoop` needed (Design Principle 5's "validate standalone first," applied to a case where the *integration* genuinely isn't unit-testable, only the pure logic is).

### Step 3 — `engine-py` implements the meaning-dependent half

`Node.set_on_click(callback)` exists for real now — `node.rs`'s own long-standing forward-reference comment ("needs `PyWindow`'s cyclic-GC participation... until something actually stores one") is resolved, reusing exactly the real `__traverse__`/`__clear__` precedent M3 step 15 Stage C's materializer already established, not inventing a second GC-safety mechanism.

**Design choice, not the architecture's literal wording**: rather than giving `Node` a back-reference to its owning `PyWindow` (a new GC-cycle shape of its own), `click_handlers: Rc<RefCell<HashMap<NodeId, Py<PyAny>>>>` is created once in `PyWindow::new` and *shared* into every `Node` it hands out — the exact same sharing shape `tree: Rc<RefCell<Tree>>` already uses between a `Window` and its `Node`s. `set_on_click` also adds `Action::Click` to the node's own `access.actions` if not already present (idempotent) — the same signal `Tree::move_focus` already keys "interactive" off, so registering a click handler makes a node Tab-reachable for free, fulfilling §10's own "keyboard operability ships from day one" at the one call site that actually makes a node interactive for the first time. `View` (a separate `Tree` owner, `view.rs`) gained its own parallel `click_handlers` field and `__traverse__`/`__clear__` for the same reason, even though nothing wires a render loop to `View` yet — structurally consistent, not yet reachable by any dispatch.

New shared module `dispatch.rs` factors out `interaction_config()` (the MD3-value `InteractionConfig` constants — real MD3 spec values where expressible today, 0.08/0.12 hover/focus opacity; `ripple_radius` a flat approximation, not per-node-computed, matching `interaction.rs`'s own already-stated future scope) and `run_activation()` (look up and call a registered click handler for a `DispatchOutcome::Activated`, dropping the `click_handlers` `RefCell` borrow *before* calling into Python — a handler that itself calls `set_on_click` again, a real plausible pattern, would otherwise panic on a re-entrant borrow). Both `App::run()`'s real `on_input` closure (the actual `winit`-driven path) and the new `Window.click(node)` method use this same logic — one copy, not two.

`Window.click(node)` (M4 step 3's own real, no-window-needed proof mechanism) is a direct, programmatic "click this node" entry point — the same "expose a direct `Tree` method since real dispatch has nowhere else to originate outside a live window" pattern every prior interaction step used. It computes layout first (so `node`'s bounds are current — nothing else does this for a `Window` with no render loop attached), then dispatches a real primary-button press+release pair at `node`'s own computed center point, exactly what a real click there would produce.

An uncaught exception from a click handler is caught via `PyErr::print(py)` — the same real traceback CPython itself would print for an uncaught exception, matching §9's own stated policy ("caught, logged, and non-fatal") more faithfully than a one-line `eprintln!` would. `tracing` (§3's own eventual choice) isn't wired up anywhere in this codebase yet — not manufactured ahead of a step that actually sets up a subscriber.

## Verification

6 new pytest tests in `test_click_dispatch.py`: a click fires its registered handler; a click on a node with no handler is a safe no-op; two nodes' handlers stay independent (clicking one never fires the other's); a raising handler is caught, doesn't crash the process, and its real traceback reaches stderr (captured via `capsys`); `set_on_click` is confirmed not to raise (the observable proxy for "also touches `access.actions`," since nothing yet exposes that field to Python directly); and a real reference-cycle-collection test mirroring M3 step 15 Stage C's own, proving `click_handlers` (a second, independently-populated `Py<PyAny>` map on the same `Window`) is genuinely visited by `__traverse__` too, not just structurally present. All passed on the first run.

4 new `engine-platform` unit tests for `translate_pointer_button`/`translate_key`, all passed on the first run. Both real windowed tests (`multi_window.rs`, `access_button.rs`) and `engine-render`'s `rect_window.rs` still pass unchanged with the new `on_input` parameter threaded through. Both real end-to-end example scripts (`animate_rect.py`, `two_windows.py`) still exit cleanly through the full `App.run()` render loop with the new closure wired in.

```
$ cargo test --workspace              # all green: engine-core 32, engine-platform 4 (new), rest unchanged
$ cargo clippy --workspace --all-targets -- -D warnings    # clean
$ cargo fmt --check                   # clean

$ maturin develop
$ python -m pytest tests/ -v
26 passed, 1 skipped (the TRE_RUN_BENCHMARK-gated benchmark)

$ python examples/animate_rect.py     # exited cleanly after 60 frames
$ python examples/two_windows.py      # exited cleanly after 60 frames, both windows
```

## Known scope narrowing (stated, not silent)

- **Keyboard-triggered activation (Tab then Enter/Space) isn't separately end-to-end proven at the Python/FFI level** — only at the `engine-core` `Tree::dispatch` unit-test level (step 1) and, once wired to real `KeyboardInput` events, at the translation-function level (step 2). `Window` exposes no Python-facing "press this key" entry point yet, only `click()` (pointer press+release) — adding one is real, additive work for whenever something needs to test keyboard activation specifically, not manufactured here.
- **`ripple_radius` stays a flat, caller-supplied approximation**, not computed per-node from its own real size the way MD3's actual ripple covers a surface's diagonal from the press point — `interaction.rs`'s own doc comment already named the real two-phase/per-node ripple model as separate future scope; this step didn't touch that.
- **`tracing`-based structured logging (§3) isn't wired up anywhere in this codebase** — a raising click handler's traceback goes to stderr via `PyErr::print`, which is a real, complete traceback, just not routed through a `tracing` subscriber. Revisit when some other real need sets one up.

## Next

M4 Phase 1 is now complete (all 3 sketched steps done). `BUILD_TRACKER.md` updated at all three levels. No further M4 phase is yet planned in `ARCHITECTURE.md` — the framework now has a real, if minimal, end-to-end interaction path (pointer hover/click, keyboard focus/activation) for the first time in this project's history. Natural next real gaps: §11.9 transform composition (`PaintProperties.transform`, blocking transform-aware hit-testing), a real scrollable viewport for `VirtualList` (§11.8), and `NodeKind::Canvas` (blocking custom hit-testing, §11.10's own second half).
