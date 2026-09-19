# LOG — M31 Phase 6: Real Event-Loop Wake (EventLoopProxy)

- Confirmed the real, existing precedent before designing anything, by
  direct source read: `engine-platform::run_windowed_multi` already
  owns a real `EventLoopProxy<PlatformEvent>` (`event_loop.
  create_proxy()`), already used for `accesskit_winit::Adapter::
  with_event_loop_proxy`'s own cross-thread `AccessKit` event delivery
  and `WindowOpener`'s own "request a new window" mechanism.
- Widened the private `PlatformEvent` enum (`AccessKit`/`OpenWindow`)
  with a real, third, untargeted `Wake` variant -- no `WindowId`
  payload, resolving the phase's own left-open design question: redraw
  every real open window on a wake, matching `any_active`'s own real
  "whole-loop signal, not per-window" shape.
- Added a new public `EventLoopWaker` handle (`#[derive(Clone)]`,
  wrapping the identical `EventLoopProxy<PlatformEvent>` `WindowOpener`
  already wraps) with one real method, `wake()`, mirroring
  `WindowOpener::open_window`'s own real "silently no-op via `let _ =
  ...` if the loop has already exited" error-handling convention
  verbatim.
- Widened `run_windowed_multi`'s own `setup: S` bound from `FnOnce(&
  WindowOpener)` to `FnOnce(&WindowOpener, &EventLoopWaker)` -- a real,
  source-breaking signature change, fixed at all three real call sites
  (`engine-py::app.rs`'s own `App.run()`, `engine-platform::
  run_windowed`'s internal wrapper, and `multi_window.rs`'s own
  integration test).
- Handled `PlatformEvent::Wake` in `user_event`: requests a real
  redraw on every currently open window.
- Confirmed via direct read that every real `Window.add_terminal` call
  happens before `App.run()` ever starts, so every real
  `TerminalSession` already exists by the time `setup` runs -- no need
  to thread the waker through `TerminalSession::spawn` itself.
- Added `TerminalSession::set_waker`, called once per real session
  from `engine-py::app.rs`'s own `setup` closure (the one real place
  able to reach a fresh `EventLoopWaker` at all). The waker is shared
  with the session's own background PTY reader thread via the
  identical `Arc<Mutex<Option<EventLoopWaker>>>` pattern `incoming`
  (`Arc<Mutex<Vec<u8>>>`) already uses -- a waker registered *after*
  the thread started is still visible to it. The reader thread now
  calls `waker.wake()` the instant real new PTY bytes actually arrive,
  not on any polling interval.
- **Removed the old `any_active` widening entirely** in `engine-py::
  app.rs` (`if !terminals.is_empty() { any_active = true; }`) -- the
  real point of this phase, not an optional cleanup left for later. A
  window with a real but genuinely quiet live terminal can now go
  fully idle exactly like any other window, closing the real, stated
  v1 cost M30 Phase 9 Step 4 (Terminal) left open.
- Full Rust verification chain green on the first pass: `cargo check`/
  `clippy -D warnings`/`fmt --check`/`cargo test --release` all clean.
- Wrote a new, dedicated `engine-platform` integration test
  (`wake_event.rs`, `harness = false`, the identical real
  main-thread-only requirement `access_button.rs`/`multi_window.rs`
  already have): a genuinely separate OS thread, holding only a clone
  of the real waker and nothing else this crate owns, calls `wake()`
  three times while a real `winit` event loop is genuinely running --
  the window still completes its own real `max_frames` (10) cleanly,
  with no panic and no hang. Passed on the first run.
- Rebuilt the Python extension. **Ran a real, direct empirical
  end-to-end script re-confirming the terminal's own real shell-
  response behavior survives the `any_active` removal**: spawned a
  real shell, typed a real command, ran 60 real frames -- the real
  response still appeared in the returned cell-grid text. Passed.
- Ran the full pytest suite three times in a row to check for any new
  timing-related flakiness from the real behavioral change (wake-driven
  instead of continuously-polled redraws) -- stable at 506 passed/1
  skipped every time, unchanged from before this phase (a pure
  internal wiring change needed no new/removed tests at the Python
  layer).
- Full verification: `cargo check`/`clippy -D warnings`/`fmt --check`
  clean, `cargo test --release` (`engine-platform` +1 real integration
  test binary), `maturin develop --release`, `pytest tests/` (506
  passed, 1 skipped, unchanged), all 69 examples (including
  `examples/terminal.py`, now exercised through the new wake path) and
  the showcase demo re-run clean.
- Updated `BUILD_TRACKER.md` (Top Metrics row now 100%, Phase 6
  heading ✅, Step 1 marked done, M31 itself marked fully complete) --
  verified the parser's own reported item count before/after (191,
  unchanged, since no bullets were added or removed, only an existing
  one filled in), regenerated and republished the Build Tracker
  artifact at https://claude.ai/artifact/CaPkWjpd91oR7YFbcqC9ty.
