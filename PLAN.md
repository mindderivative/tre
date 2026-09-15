# Plan: M3 Phase 2, Step 2 — Animated&lt;T&gt; + Central Tick (§14 step 2)

Corresponds to `BUILD_TRACKER.md` M3 Phase 2, step 2 of 4.

## Goal

Per §14 step 2: "Add `Animated<T>` + the central tick; animate that rect's color/elevation. Validates the animation core in isolation." Implement §5's core animation primitive (`Interpolate`, `Animated<T>`, `ActiveAnimation<T>`, `MotionCurve`) in `engine-core`, tested standalone (no `Node`/`Tree` yet — that's step 3), then prove it actually drives what step 1's rect renders.

## Scope

In scope:
- `engine-core::animation`: `Interpolate` (impls for `f64` and `peniko::Color`), `MotionCurve` (just `Linear` for now — real MD3 cubic-bezier curves are §7.5's own scope, added whenever a later step first needs one), `ActiveAnimation<T>`, `Animated<T>` with `animate_to`/`tick`.
- Pure-logic unit tests in `engine-core` covering: linear interpolation correctness, tick advancing/snapping/completing, zero-duration edge case, interrupting a running animation.
- A headless integration test in `engine-render` (`tests/animated_rect.rs`) proving `engine-core`'s `tick()` output is what actually gets painted — not just that each crate's own logic is correct in isolation, but that they compose.
- Extending the real windowed demo (`tests/rect_window.rs`) to animate color and a second, independent `f64` (opacity, standing in for "elevation" until real shadow rendering exists at step 8) across its 60 frames.

Out of scope: `Node`/`Tree`/`PaintProperties`/`AnimatedNodeState` (step 3 — this step is explicitly "in isolation"), real MD3 motion curves (§7.5), real elevation-driven shadow rendering (step 8/§7.2).

## Verification

`cargo test --workspace` (all green, including the new isolated unit tests and the composed integration test), `cargo clippy --workspace --all-targets` and `cargo fmt --check` clean, and the real windowed demo actually presenting a visibly animating rect.
