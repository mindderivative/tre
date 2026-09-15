# Plan: Phase 1, Step 2 -- Decoupled Event & Signal Pipeline

## Scope decision (confirmed with project owner 2026-09-05)

Step 1.1 gave each window its own Wayland/X11 connection. The docs (DESIGN.md Section 5.1, TECHNICAL.md Section 8) describe input flowing through **one** SPSC queue fed by a **single** OS-event-pump producer across all windows -- which doesn't fit cleanly with per-window connections. This step **consolidates `tre-platform` to one shared connection per backend**, multiplexing multiple windows over it, rather than carrying the Step 1.1 per-window-connection design forward as a permanent architectural inconsistency.

Also scoped out, consistent with Step 1.1's precedent of deferring what can't be verified here: **touch input** (no touchscreen on this machine) and **genuine OS-thread separation** of the event pump from the render loop (the SPSC ring buffer is built correctly -- real lock-free, atomic-based -- so it's ready for a second thread whenever one is actually introduced, but this step still drains it from the same loop that renders, matching the "don't build ahead of a need you can't yet verify" precedent from Phase 0/1.1). Pointer (mouse) and keyboard input, on Wayland and X11, are in scope.

## Doc fix already applied

IMPLEMENTATION.md Step 1.2 task 1 said "SPMC," never updated when TECHNICAL.md Section 8 was corrected to SPSC in the original review -- undetected until this step's planning, since nothing had implemented it yet. Fixed to reference TECHNICAL.md Section 8 as canonical instead of restating (REVIEW.md finding #47).

## Goal

Build the real input event pipeline IMPLEMENTATION.md Step 1.2 and DESIGN.md Section 5 describe: OS pointer/keyboard/window events flow through a lock-free SPSC ring buffer as engine-agnostic `InputEvent`s, with high-frequency pointer-move coalescing, exposed to the caller via a non-blocking drain -- decoupled from the graphics pipeline in code structure (DESIGN.md Section 2.2), even though genuine thread separation is deferred.

## Tasks

1. **Consolidate `tre-platform` to one connection per backend.** Replace the `PlatformWindow` enum (one connection per window) with a `PlatformConnection` (one `wayland_client::Connection` or one `x11rb::xcb_ffi::XCBConnection`) that creates/owns multiple windows, each referenced by an opaque `WindowId`. `raw-window-handle` access becomes per-window (`display_handle()` shared, `window_handle(id)` per-window) rather than per-`PlatformWindow`.

2. **Generic SPSC ring buffer in `tre-memory`.** `tre-memory`'s stated purpose (TECHNICAL.md's crate list) already includes zero-allocation ring buffers; this is its first real implementation. Fixed-capacity, genuinely atomic/lock-free (not just "single-threaded and pretending"), so it's correct today and needs no redesign whenever a second thread is actually introduced.

3. **`InputEvent` and `WindowId` types in `tre-engine`.** Engine-agnostic event structures (DESIGN.md Section 5.1's `OnClick`/`OnHover`-style translation target, one level below that): `PointerMoved`, `PointerButtonDown/Up`, `KeyDown/Up`, `Resized`, `CloseRequested`, each carrying a `WindowId`. `tre-platform` depends on `tre-engine` to produce these (a new dependency edge, architecturally sound since `tre-engine` has no platform-specific content).

4. **Pointer + keyboard binding, both backends.** Wayland: bind `wl_seat` -> `wl_pointer`/`wl_keyboard`, translate motion/button/key events. X11: extend the existing window's event mask to include button/motion/key events, translate the existing `poll_for_event` loop's additional event variants.

5. **Pointer-move coalescing.** A new `PointerMoved` event for a window overwrites the ring buffer's existing pending `PointerMoved` entry for that same window instead of appending a new one, so a frame's worth of high-frequency motion collapses to the latest position by drain time (IMPLEMENTATION.md Step 1.2 task 3).

6. **Update all three Step 1.1 examples** (`walking_skeleton`, `multi_window`, `headless`) to the new `PlatformConnection` API, and **add a new demo** that visibly proves input works -- printing pointer position and button/key events to the terminal while the scene renders, since Step 1.1's demos couldn't exercise input at all and this step's entire point is that they now can.

## Verification plan

Same discipline as Phase 0 and Step 1.1:
- `cargo fmt`/`clippy -D warnings`/`build`/`test` clean across the workspace, including new unit tests for the SPSC ring buffer itself (capacity limits, coalescing behavior) independent of any windowing.
- All examples run against real hardware with `VK_LAYER_KHRONOS_validation` enabled, zero errors.
- The new input demo manually exercised (move the mouse, click, type) and its printed output checked against actual physical input performed during the run.
- Multi-window input specifically checked: events routed to the correct `WindowId` when two windows are open simultaneously.

## Explicitly out of scope for this step

- Windows/macOS input (still deferred with the platform bridges themselves).
- Touch input (unverifiable on this machine).
- A genuinely separate OS thread for the event pump (ring buffer is ready for it; not introduced yet).
- Signal/slot dispatch, hit-testing, and scene-tree focus management (DESIGN.md Section 5.1's "Decoupled Processing" step) -- that's UI-framework-side logic (Python), not this engine-side step's job; this step only gets raw, translated `InputEvent`s to the point where a UI framework *could* consume them.
