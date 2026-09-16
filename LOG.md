# Log: M4 Phase 8 — Scroll-Wheel Input Plumbing (§11.7/§11.8 groundwork)

Corresponds to `BUILD_TRACKER.md` M4 Phase 8. No `InputEvent::Scroll`
variant existed anywhere (confirmed via grep). This phase adds the real
translation from `winit`'s `WindowEvent::MouseWheel`, deliberately
stopping at input plumbing.

## Investigation before writing code

Re-read §11.7 (Virtualization) and §11.8 (Culling) — neither actually
specifies a scroll *input* mechanism; §11.7 only states "scrolling
recycles `NodeId` slots" as a fact about `VirtualList`'s own behavior
once scrolling happens, by whatever means. Like M4 itself, this phase
builds the next real, named prerequisite toward an already-recorded gap
(`VirtualList` has no real scrollable viewport yet), not an explicit
architecture line item.

Verified `winit = "0.30.13"`'s real API directly in its vendored
`event.rs` before writing anything, this project's own standing
discipline after `PointerButton`'s own past correction:

- `WindowEvent::MouseWheel { device_id, delta: MouseScrollDelta, phase }`
  is real.
- `MouseScrollDelta` has exactly two variants: `LineDelta(f32, f32)`
  (a wheel/touchpad notch count) and `PixelDelta(PhysicalPosition<f64>)`
  (raw pixels). Genuinely different units — collapsing both into one
  plain `(f64, f64)` would misrepresent real magnitude for no reason,
  since nothing consumes the value's magnitude yet at all. Chose to
  preserve the real distinction as `engine_core::ScrollDelta`.

## What happened

`engine-core`: new `ScrollDelta { Lines(f64, f64), Pixels(f64, f64) }`
and `InputEvent::Scroll { delta, position }` — `position` included
since every other pointer-originated event carries one, reusing
`last_cursor_position` the identical way `MouseInput` already does
(`winit`'s own `MouseWheel` carries no position either). `Tree::
dispatch` gained the required match arm, a true, deliberate no-op —
`DispatchOutcome::None` unconditionally, matching this phase's own
"plumbing only" scope; wiring real behavior would mean building ahead
of the still-open scrollable-viewport gap (§11.8/§11.9) this phase
doesn't touch.

`engine-platform`: new `translate_scroll_delta`, mirroring
`translate_pointer_button`/`translate_key`'s own pure-function shape
exactly (unit-testable directly, since `winit` still has no public API
to inject a synthetic `WindowEvent` into a live loop — verified at M4
Phase 1 step 2, unchanged). `WindowEvent::MouseWheel` now translates
and dispatches through the same `on_input` closure every other real
input event already uses.

## Verification

New `engine-core` unit test: a real `Tree::dispatch` call with
`InputEvent::Scroll` (both a `Lines` and a `Pixels` delta) returns
`DispatchOutcome::None` and leaves `hovered` untouched — proving it's a
genuine no-op, not silently wrong. New `engine-platform` unit test:
`translate_scroll_delta` preserves both real variants and their
sign/magnitude. Both passed on the first run.

```
$ cargo build --workspace                                    # clean
$ cargo clippy --workspace --all-targets -- -D warnings       # clean
$ cargo fmt --check                                           # clean (after one real fmt fix)
$ cargo test --workspace                                      # all green: engine-core 43 (+1), engine-platform 5 (+1)

$ maturin develop
$ python -m pytest tests/ -v
53 passed, 1 skipped (the TRE_RUN_BENCHMARK-gated benchmark) -- unaffected, no engine-py surface touched this phase

$ python examples/*.py    # all five exit cleanly, unaffected
```

## Next

M4 Phase 9 (the last M4 phase): docking drag-to-rearrange (§11.4) —
moving a whole panel between zones, not resizing (which M4 Phase 3
already covered). Real, stated-not-silent gaps unchanged: scroll input
reaches `Tree::dispatch` for real but does nothing yet; `VirtualList`
still has no real scrollable viewport (clipping, an actual scroll
offset/transform, §11.8/§11.9) to wire it to.
