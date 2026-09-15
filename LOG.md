# Log: M3 Phase 7, Step 14 — Multi-Window (§14 step 14, §11.1)

Corresponds to `PLAN.md` / `BUILD_TRACKER.md` M3 Phase 7, step 14 of 3 (steps 13-15).

## What happened

**Checked what §11.1's own text actually assumes before touching
anything.** Its "routing an event to the right `Tree` before
translating it into `InputEvent`... is a lookup" phrasing presumes real
pointer/keyboard `InputEvent`/`AppHandler` dispatch already exists —
checked directly, it still doesn't, anywhere in this codebase, the same
finding as steps 7/9/11/12/13. What genuinely does exist and get
dispatched per-window today is `winit`'s own window-level events
(`RedrawRequested`, `CloseRequested`) and `accesskit_winit`'s own events
(which already carry a real `window_id`, confirmed directly in its
source) — this step's real, provable claim is routing *those*
correctly via `WindowId`, not input dispatch.

**`engine-platform`**: `run_windowed_multi` replaces the single-window
`MultiWindowApp`'s implicit "there is exactly one window" assumption
with a real `HashMap<WindowId, PerWindow>`. Windows are opened only via
a `WindowOpener` handle (wrapping the existing `EventLoopProxy`), which
a `setup` closure uses once, before the blocking loop starts — the
identical "only thin, `Send` data crosses into `winit`'s own callback
world" pattern the crate already used for `accesskit`, now reused for
window-open requests too. Each window-open request carries a
caller-assigned `token: u64` so `on_window_created(WindowId, token,
Arc<Window>)` lets the caller correlate the real, only-now-known
`WindowId` with whatever it originally asked for. `on_frame`/
`build_access_update` dropped their `&Arc<Window>` parameter entirely —
now that GPU/render state is built once, eagerly, at `on_window_created`
time (not lazily on first frame, as the old single-window code did),
neither callback needs the window handle again.

**The original `run_windowed` is now a thin wrapper** over
`run_windowed_multi` — it stashes the one `Arc<Window>` from
`on_window_created` and hands it back to the caller's old-style
`on_frame` closure every redraw, so its two existing callers
(`rect_window.rs`, `access_button.rs`) needed zero source changes.
Confirmed by building both unchanged: they compiled immediately.

**`engine-py`**: `PyWindow` split back out of `App`, at exactly the
step every earlier module doc comment (going back to step 6) predicted
— `App::new`/`App::add_rect`'s old body moved to `window.rs` verbatim,
renamed `Window` on the Python side (matching `Node`'s own "renamed to
match what Python sees" precedent). `App` is now a thin collector:
`add_window(Py<PyWindow>)` registers, `run(max_frames)` extracts each
window's plain Rust data once (while the GIL is already held) into a
`Vec<WindowSetup>`, then drives them all through
`run_windowed_multi` — the `winit` closures below that point never
touch a Python object again, matching the same "only thin data crosses
the boundary" discipline `engine-platform` already established.
`App.run()` with no registered windows now raises a clear `RuntimeError`
naming `add_window`, rather than silently doing nothing.

**Every borrow-checker assumption behind this design (disjoint field
destructuring so `windows`/`on_frame`/`build_access_update` can all be
mutably accessed within the same `window_event`/`user_event` match)
compiled correctly on the first real build** — verified for real, not
assumed from reasoning about the borrow checker in the abstract.

**The actual proof, at two levels.** `crates/engine-platform/tests/
multi_window.rs` (new, `harness = false`, matching `access_button.rs`'s
precedent) opens two windows directly through `run_windowed_multi` with
no `pyo3` involved at all, and asserts the two real `WindowId`s are
genuinely distinct and each window's frame counter reached exactly its
own `max_frames` independently — this ran and passed on the first try.
`examples/two_windows.py` proves the same mechanism end to end through
the real Python API most app authors will actually use: two `Window`s
with distinct sizes/titles/content/animations, one `App.run()` driving
both — it also ran and exited cleanly on the first try.

## Verification

```
$ cargo test -p engine-platform --test multi_window
engine-platform §14 step 14: window 0 created as WindowId(...)
engine-platform §14 step 14: window 1 created as WindowId(...)
engine-platform §14 step 14: both windows opened with distinct WindowIds,
  ticked independently, and both exited cleanly at their own max_frames

$ cargo test --workspace             # all green, incl. unchanged rect_window.rs/access_button.rs
$ cargo clippy --workspace --all-targets -- -D warnings    # clean
$ cargo fmt --check                  # clean

$ maturin develop && python -m pytest tests/ -v
14 passed (13 pre-existing/updated + 1 new App-requires-a-window test)

$ python examples/animate_rect.py    # exited cleanly after 60 frames (Window/App split, unchanged behavior)
$ python examples/two_windows.py     # exited cleanly after 60 frames, both windows
```

## Next

`BUILD_TRACKER.md` updated: Phase 7 step 14 done (2 of 3 steps in this
phase). Next: step 15, M3's final step — docking (§11.4) + virtualization
(§11.7), sequenced last since both build on the overlay mechanism (step
13) and the accepted multi-window model (this step).
