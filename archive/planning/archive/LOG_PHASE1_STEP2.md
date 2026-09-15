# Log: Phase 1, Step 2 -- Decoupled Event & Signal Pipeline

## Doc gaps found

None in the four core documents beyond the one already fixed during planning (IMPLEMENTATION.md Step 1.2 restating "SPMC" -- see the archived `PLAN_PHASE1_STEP2.md` and REVIEW.md finding #47). TECHNICAL.md Section 8's SPSC design and ARCHITECTURE.md Section 1's Platform & Event Layer sketch both matched what got built without further correction, beyond adding "implemented as of" notes pointing at the real types.

## Design decision made before writing code (not a bug -- a hazard avoided)

### Coalescing-in-the-ring-buffer would race a concurrent consumer
`PLAN.md`'s task 5 wording ("a new `PointerMoved` event... overwrites the ring buffer's existing pending entry") reads naturally as "mutate an already-published ring-buffer slot in place." Working through the concurrency implications before writing any code: whenever the queue holds exactly one unconsumed item, the slot the producer would want to overwrite (`head - 1`) is the exact same slot as `tail` -- the one slot the consumer might be mid-read of via `assume_init_read()`. Overwriting that slot in place is a genuine data race, not just an inelegant design, and one that a single-threaded Step 2 integration test could never catch (both ends run on the same call stack right now), only to bite whoever adds a real second thread later.

**Resolution:** `tre_engine::InputEventQueue` stages the pending move in a plain (non-atomic) struct field, producer-exclusive until an ordinary `push()` publishes it into the underlying `tre_memory::SpscRingBuffer`. The ring buffer itself is never touched by the coalescing logic, so it stays correct if a real second consumer thread shows up later -- matching the ring buffer's own "no redesign needed" design goal (TECHNICAL.md Section 8).

## Real bugs found (all caught by actually building/running the code, not by inspection alone)

### Borrow-checker error from double-borrowing `self` in `X11Connection::poll_events`
The first draft of `Event::ConfigureNotify` handling held a `&mut WindowState` from `self.windows.get_mut(&id)` across a call to `self.flush_pending_move(&mut events)`, which itself needs `&mut self`. Caught immediately by `cargo build` (E0499), before it ever ran. Fixed by computing whether the size actually changed with a short-lived `&self` borrow first, dropping it, then taking the `&mut self` borrow only for the parts that need it.

### A merged `Event::ButtonPress(btn) | Event::ButtonRelease(btn)` match arm lost which one fired
An early draft of the X11 button-event handling merged the press/release cases into one match arm (since both carry an identical struct shape) and then tried to disambiguate with a broken placeholder helper function that didn't compile. Caught by re-reading the code before ever running `cargo build` on it. Fixed by keeping the press and release cases as separate match arms, each hardcoding its own `ElementState`, which is simpler and more obviously correct than trying to recover "which event fired" after the fact.

## Verification performed

- `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets` (zero warnings), `cargo build --workspace --all-targets`, `cargo test --workspace`: all clean, including 5 new `tre-memory` unit tests for `SpscRingBuffer` (FIFO order, capacity-is-reported-not-grown, wraparound, drop cleanup, and a genuinely concurrent 100,000-item producer/consumer stress test on real OS threads) and 4 new `tre-engine` unit tests for `InputEventQueue`'s coalescing behavior, independent of any windowing.
- All three migrated Vulkan examples (`walking_skeleton`, `multi_window`, `headless`) plus the new `input_demo` ran against real hardware (AMD Radeon 890M) with `VK_LAYER_KHRONOS_validation` enabled: zero validation errors across every run, both before and after centralizing coalescing into `InputEventQueue`.
- `headless`'s output PNG re-inspected pixel-by-pixel after the `PlatformConnection` refactor: green rect present at the expected location, dark background elsewhere -- confirms the probe-window path through the new consolidated connection still produces a correct device.
- Real input synthesized against the X11 backend via the XTEST extension (the same mechanism `xdotool`/`ydotool` use), built as a standalone scratch harness (not part of the repo): pointer motion, a button click, and a key press/release all showed up in `smoke_test`'s printed output translated correctly (`PointerMoved`, `PointerButton { button: Left, state: Pressed/Released }`, `KeyboardKey { key_code: 30 (KEY_A), state: Pressed/Released }`), both before and after the `InputEventQueue` centralization refactor.
- Multi-window routing specifically verified against `input_demo`: synthesized input into window A, then window B, then window A again, with the harness explicitly raising/focusing each target first (KWin's default placement stacked both same-size windows at the same screen location, so without this the topmost window absorbed every click regardless of target -- a window-manager/test-harness quirk, not a `tre-platform` bug). Result: every event was correctly tagged `[A]`, `[B]`, `[A]` in the same order the input was applied, zero cross-window leakage in either direction.
- Wayland input translation was **not** verified via live synthesized input in this session: KWin (the session's compositor) does not advertise `org_kde_kwin_fake_input`, and wlroots-specific virtual-input protocols don't apply to KWin. Verified instead by code review and structural parity with the XTEST-verified X11 path (same event model, same `InputEventQueue` coalescing). Recorded honestly as a verification gap, not papered over.
