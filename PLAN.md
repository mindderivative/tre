# PLAN — M31 Phase 6: Real Event-Loop Wake (EventLoopProxy)

## Goal
A real, generic wake mechanism for `App.run`'s own idle event loop,
closing the real, stated v1 cost M30 Phase 9 Step 4 (Terminal) left
open (continuously widening `any_active` to keep polling instead of
genuinely waking on new PTY output). This closes M31 itself, all 6
phases.

## Steps
1. Confirmed the real, existing precedent via direct source read
   before designing anything: `engine-platform::run_windowed_multi`
   already owns a real `EventLoopProxy<PlatformEvent>`, already used
   for AccessKit's own cross-thread event delivery and `WindowOpener`'s
   own "request a window" mechanism.
2. Widened the private `PlatformEvent` enum with a real, untargeted
   `Wake` variant (no `WindowId` payload — confirmed the correct v1
   answer the scoping note left open: redraws every open window, the
   same whole-loop shape `any_active` already had).
3. Added a new public `EventLoopWaker` handle (`Send` + `Clone`,
   wrapping the identical `EventLoopProxy`), exposed via
   `run_windowed_multi`'s own `setup` closure alongside the existing
   `WindowOpener` — the one real place able to reach a fresh proxy and
   hand a clone to an already-constructed background producer.
4. Confirmed every real `add_terminal` call happens before `App.run()`
   starts, so every real `TerminalSession` already exists by the time
   `setup` runs — no need to thread the waker through construction.
5. Added `TerminalSession::set_waker`; the waker is shared with the
   session's own background PTY reader thread via the identical
   `Arc<Mutex<...>>` pattern `incoming` already uses, so a waker
   registered later is still visible to an already-running thread.
   The reader thread now calls `waker.wake()` the instant real new PTY
   bytes arrive.
6. Removed the old `any_active` widening in `engine-py::app.rs` (the
   real point of this phase, not an optional cleanup) — a window with
   a genuinely quiet live terminal can now go fully idle.
7. Wrote a new, dedicated `engine-platform` integration test
   (`wake_event.rs`): a genuinely separate OS thread holding only a
   waker clone calls `wake()` three times while a real event loop
   runs; the window still completes its own real `max_frames` cleanly.
8. Ran a real, direct empirical script re-confirming the terminal's
   own real shell-response behavior survives the `any_active` removal,
   then the full pytest suite (3x, checking stability).
9. Full verification chain: cargo check/clippy/fmt/test, maturin
   develop, pytest (full suite), all 69 examples, showcase demo.
10. Update `BUILD_TRACKER.md` — verified the parser's own reported
    item count before/after (191, unchanged), regenerate + republish
    the Build Tracker artifact. Closes M31 itself, all 6 phases.
11. Update memory, commit, push.

## Status
Complete. All steps done; full verification chain green (`engine-platform`
gains 1 new integration test binary, `pytest tests/` 506 passed/1
skipped unchanged, all 69 examples, showcase demo). A real background
PTY thread genuinely wakes an idle event loop via a real cross-thread
`EventLoopProxy` send, confirmed by a dedicated test exercising a real
separate OS thread against a real running loop, not assumed. **M31 —
Code Editor: Real IDE Functionality (Second Pass) is now fully
complete, all 6 phases.**
