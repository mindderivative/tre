# Log: M3 Phase 2, Step 2 — Animated&lt;T&gt; + Central Tick (§14 step 2)

Corresponds to `PLAN.md` / `BUILD_TRACKER.md` M3 Phase 2, step 2 of 4.

## What happened

**Implemented §5's animation core in `engine-core/src/animation.rs`, validated standalone first:** `Interpolate` (linear `f64`; `peniko::Color` via `AlphaColor::lerp_rect` — checked the real `color` crate source rather than hand-rolling a per-channel lerp, since `lerp_rect` is specifically the correct method for a rectangular color space like `Srgb`, as opposed to `lerp`'s hue-based interpolation for polar spaces), `MotionCurve` (deliberately just `Linear` for now — real MD3 cubic-bezier curves are §7.5's own scope, not manufactured ahead of a step that actually needs one), `ActiveAnimation<T>`, and `Animated<T>` with `animate_to`/`tick`. Six unit tests, all passing on the first real attempt: linear interpolation at t=0/0.5/1, color channel lerping verified against exact expected float values, a no-op tick with nothing active, mid-flight-then-snap-on-completion, a zero-duration animation (verified it doesn't divide by zero), and interrupting a running animation (verified `from` captures the *current* value, not the old target).

**Then proved it actually drives real rendering, not just that the math is correct in isolation:** `engine-render/tests/animated_rect.rs` creates an `Animated<Color>` and a separate `Animated<f64>` (opacity), starts both animating, ticks them to the halfway point, renders through the real `vello_hybrid` pipeline, reads back the pixel, and asserts the rendered color is strictly *between* the start and end values (not equal to either) and the alpha reflects a partial, non-zero, non-full opacity. This is the actual claim this step exists to prove -- that `engine-core`'s `tick()` output is what `engine-render` painted, not two independently-correct halves that happen to compile together.

**Extended the real windowed demo** (`tests/rect_window.rs`, from step 1) to animate the rect's color (purple to teal) and opacity (100% to 40%) across its 60 presented frames, using the same `Animated<T>` mechanism, ticked each frame against real `Instant::now()`.

**Found something worth knowing, not worth fixing here:** the windowed demo's own diagnostic (`animation_start.elapsed()` at the final frame) showed the 60 frames completed in ~0.19s, not the ~1s the animation duration assumed -- this environment's surface isn't vsync-throttling in `ControlFlow::Poll` mode (frames present as fast as the GPU can produce them, ~300+ fps for a single small rect). The animation math itself is still exercised correctly every frame regardless of pacing (proven separately, rigorously, by the headless test above); this is a frame-pacing observation relevant to §6's later frame-budget work, not a defect in what step 2 needed to prove.

## Verification

```
$ cargo test --workspace
    ...
     Running unittests src/lib.rs (engine_core-...)
running 6 tests
test animation::tests::color_interpolate_lerps_each_channel ... ok
test animation::tests::f64_interpolate_is_linear ... ok
test animation::tests::interrupting_a_running_animation_starts_from_current_not_old_target ... ok
test animation::tests::tick_advances_partway_then_snaps_on_completion ... ok
test animation::tests::tick_with_no_active_animation_is_a_no_op ... ok
test animation::tests::zero_duration_animation_snaps_immediately_no_panic ... ok

     Running unittests src/lib.rs (engine_render-...)
running 1 test
test tests::rect_scene_renders_expected_pixels ... ok

     Running tests/animated_rect.rs (engine_render)
running 1 test
test animated_color_and_opacity_render_the_interpolated_value_mid_flight ... ok

     Running tests/rect_window.rs (harness = false)
engine-render §14 step 1/2: first frame presented, 400x400, animating 100%->40% opacity
engine-render §14 step 2: animation ran for 0.19s across 60 frames
engine-render §14 step 1/2: exited cleanly after 60 frames
    ...
test result: ok. (all crates, 0 failures)

$ cargo clippy --workspace --all-targets   # clean
$ cargo fmt --check                        # clean
```

## Next

`BUILD_TRACKER.md` M3 Phase 2 updated (step 2 of 4 done). Next: step 3, wiring `taffy` for layout of multiple static nodes, plus the frame-time CI benchmark (§6 Locked Decisions target: 16.6ms/8.3ms) -- the first point a real render+layout+tick pipeline exists to measure against that stated target. This is also where `Node`/`Tree` first appear, so `Animated<T>` moves from "ticked by hand" (this step) to actually being a field inside `PaintProperties`.
