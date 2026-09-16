# Plan: M4 Phase 8 — Scroll-Wheel Input Plumbing (§11.7/§11.8 groundwork)

## Context

No `InputEvent::Scroll` variant exists anywhere (confirmed via grep).
This phase adds the real translation from `winit`'s `WindowEvent::
MouseWheel` into a new `engine_core::InputEvent::Scroll`, deliberately
stopping at input plumbing — wiring it to `VirtualList`'s window
movement needs the still-open real scrollable-viewport gap (clipping +
scroll offset, §11.8/§11.9), which isn't built yet and isn't this
phase's job to build.

## Investigation before writing code

Re-read §11.7 (Virtualization) and §11.8 (Culling) — neither section
actually specifies a scroll *input* mechanism; §11.7 only says
"scrolling recycles `NodeId` slots" as a fact about `VirtualList`'s own
behavior once scrolling happens, by whatever means. Like M4 itself,
this phase isn't fulfilling an explicit architecture requirement so
much as building the one remaining named prerequisite for a real gap
already on record (`VirtualList` has no real scrollable viewport yet,
`BUILD_TRACKER.md`'s own Known Gaps).

Verified `winit = "0.30.13"`'s real API directly in its vendored
`event.rs` before writing anything (this project's own standing
discipline, after `PointerButton`'s own past correction):

- `WindowEvent::MouseWheel { device_id, delta: MouseScrollDelta, phase }`
  is real.
- `MouseScrollDelta` has exactly two variants: `LineDelta(f32, f32)`
  (a touchpad/wheel notch count) and `PixelDelta(PhysicalPosition<f64>)`
  (raw pixels, when the platform/device supports it) — genuinely
  different units, not two names for the same thing. Collapsing both
  into one plain `(f64, f64)` would misrepresent real magnitude
  differences (a `LineDelta` of 1.0 is not 1.0 pixel) for no real
  reason yet, since nothing consumes the value's magnitude at all this
  phase — the honest choice is to preserve the real distinction,
  mirroring how `PointerButton`'s own past correction found value in
  matching the real API exactly rather than assuming a simplification.

## Approach

1. **`engine-core`**: new `ScrollDelta { Lines(f64, f64), Pixels(f64, f64) }`
   and `InputEvent::Scroll { delta: ScrollDelta, position: Point }` —
   `position` included since every other pointer-originated
   `InputEvent` carries one (a future scroll-to-node wiring will need
   to know which node the cursor is over, the same way `MouseInput`
   already reuses `last_cursor_position`), reusing `win.
   last_cursor_position` in `engine-platform` the identical way
   `MouseInput` already does. `Tree::dispatch` gains the required match
   arm — a true no-op, `DispatchOutcome::None` unconditionally,
   matching this phase's own explicit "plumbing only" scope; adding
   real hit-testing-driven behavior here would be building ahead of
   the still-open scrollable-viewport gap this phase deliberately
   doesn't touch.
2. **`engine-platform`**: new `translate_scroll_delta(delta:
   MouseScrollDelta) -> ScrollDelta` pure function, mirroring
   `translate_pointer_button`/`translate_key`'s own shape exactly (unit
   -testable directly, since `winit` has no public API to inject a
   synthetic `WindowEvent` into a live loop, verified in M4 Phase 1
   step 2 and unchanged since). `WindowEvent::MouseWheel` handled in
   the main match, calling `on_input` with the translated event.
3. **Tests**: `engine-core` unit test that a real `Tree::dispatch` call
   with `InputEvent::Scroll` returns `DispatchOutcome::None` and
   touches no other state (`hovered`/`pressed`/`dragging` all
   unchanged) — proving it's a genuine no-op, not silently wrong.
   `engine-platform` unit tests for `translate_scroll_delta`, mirroring
   the existing `translate_pointer_button`/`translate_key` test shape:
   both real variants translate correctly, sign/magnitude preserved.

## Files to touch

- `crates/engine-core/src/input.rs` — `ScrollDelta`, `InputEvent::
  Scroll`.
- `crates/engine-core/src/tree.rs` — `dispatch`'s new match arm, new
  unit test.
- `crates/engine-platform/src/lib.rs` — `translate_scroll_delta`,
  `WindowEvent::MouseWheel` handling, new unit tests.

## Verification

- `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D
  warnings`, `cargo fmt --check`.
- `maturin develop && python -m pytest tests/ -v` plus all examples run
  (no Python-facing API changes expected — this phase is `engine-core`/
  `engine-platform` only, nothing for `engine-py` to expose yet since
  there's no real consumer).
