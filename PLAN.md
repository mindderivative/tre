# Plan: M3 Phase 7, Step 14 — Multi-Window (§14 step 14, §11.1)

Corresponds to `BUILD_TRACKER.md` M3 Phase 7, step 14 of 3 (steps 13-15).

## Goal

Per §14 step 14: "Multi-window (§11.1) — a second `PyWindow` opened from
a running app, proving `WindowId`-routed event dispatch before
docking's 'detach into its own window' pattern needs it." §11.1's own
text: one `Tree` per OS window; `engine-platform` manages windows keyed
by `WindowId`; routing a `winit` event to the right window is "a
lookup, not new dispatch machinery."

## Scope note

§11.1's own text assumes real `InputEvent`/`AppHandler` dispatch
already exists ("unchanged from the single-window model already
designed"). Checked directly: it doesn't, anywhere in this codebase
(same finding as steps 7/9/11/12/13). What genuinely *does* already
exist and get dispatched per-window today is `winit`'s own window-level
events (`RedrawRequested`, `CloseRequested`) plus `accesskit_winit`'s
own events (which already carry a real `window_id`, confirmed directly
in its source) — this step's real, provable claim is routing *those*
correctly to independent per-window state via a `WindowId` lookup, not
pointer/keyboard input dispatch, which stays out of scope for the same
reason it has every prior step.

"A second `PyWindow` opened from a running app" is scoped to: windows
requested before `App.run()` starts, all opened together as part of one
continuously-running session (not a live Python call interleaving with
the blocking event loop mid-session — Python's single call stack
can't do that without a callback hook this step doesn't need to build,
since nothing requires *dynamically* adding a window after the loop is
already running).

## In scope

- `engine-platform`: `run_windowed` restructured for N windows.
  `WindowOpener` (wraps the existing `EventLoopProxy`) lets a `setup`
  closure request one or more windows before the blocking loop starts,
  each tagged with a caller-assigned `token: u64` for correlation.
  `on_window_created(WindowId, token, Arc<Window>)` fires once per
  window as it's actually created (letting the caller build its own
  per-window GPU/render state exactly once, keyed by the real
  `WindowId`). `on_frame(WindowId, frame_index)` and
  `build_access_update(WindowId) -> TreeUpdate` are now per-window,
  looked up via a `HashMap<WindowId, _>` `engine-platform` itself
  maintains internally for accesskit routing, and via the caller's own
  map (built from `on_window_created`) for rendering.
- `engine-py`: `PyWindow` split back out of `App` (`App`'s own module
  doc comment predicted this exact refactor at this exact step) —
  `PyWindow` owns one `Tree`/root/size, `App` collects registered
  `PyWindow`s and drives them together in one `run()` call.
- Real proof: two `PyWindow`s, distinct sizes/content, opened together,
  each independently ticking/laying-out/rendering its own `Tree` --
  verified via a real multi-window run (a `[[test]] harness = false`
  target in `engine-platform`, matching `access_button.rs`'s own
  precedent) confirming both windows receive independent `WindowId`s
  and both redraw loops run to completion.

## Verification

`cargo test --workspace`, `cargo clippy --workspace --all-targets --
-D warnings`, `cargo fmt --check` all clean, plus `maturin develop` +
the existing pytest suite (`App`'s public shape changes, so
`test_engine_py.py` needs updating for the new `PyWindow`/`App` split)
and a real two-window run.
