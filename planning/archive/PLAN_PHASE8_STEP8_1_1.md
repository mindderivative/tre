# Plan: Phase 8, Step 8.1.1 -- Real Frame Clock & Spring-Decay Primitives

## Goal

Build and prove the two real, independent math/timing primitives
IMPLEMENTATION.md Step 8.1's own tasks 1-2 name -- a real, hardware-
backed monotonic frame clock, and the real exponential spring-decay
formula -- as pure, unit-tested building blocks with no consumer yet.
Split from "Step 8.1: Loop Orchestration & Frame Timing" after real
investigation found task 3 (wiring the entire engine into one real,
continuously-running 8-stage loop) needs a genuinely separate, larger
effort of its own (including a real, previously-undiscovered gap:
`execute_frame` hardcodes a zero vertex/index buffer offset, so it
cannot yet accept a real per-frame ring-buffer-backed buffer at all) --
confirmed with the project owner via AskUserQuestion: split into
8.1.1/8.1.2, this plan covers 8.1.1 only.

## Scope decisions

1. **Neither primitive exists anywhere in this codebase today --
   confirmed by a real, repo-wide investigation, not assumed.** No code
   anywhere uses `std::time::Instant` (or any other monotonic clock) to
   compute a frame delta-time; the only existing `Instant` usages are
   unrelated timeout-polling loops in demo/test code (`canvas_
   accessibility_verify.rs`, `tre-a11y`'s own tests). A repo-wide grep
   for `spring|exp(-|.exp()|.powf` returns zero hits -- `tre-math`
   (`crates/tre-math/src/lib.rs`, read in full) has only `Affine2`
   transform ops, `compose_batch`, `lerp_points_batch` (a fixed-`t` SIMD
   lerp, not time-based), and `tone_map` (Step 7.1's own tone-mapping
   curve, unrelated).

2. **`std::time::Instant` is the correct, portable primitive for
   TECHNICAL.md Section 7.1's own requirement, not a platform-specific
   hand-roll.** Section 7.1 names `QueryPerformanceCounter` (Windows)/
   `clock_gettime(CLOCK_MONOTONIC)` (Linux) explicitly -- `std::time::
   Instant` wraps exactly these platform APIs internally (per Rust's own
   standard library implementation) and already exceeds the spec's own
   $1\mu s$ precision floor (backed by nanosecond-resolution `Duration`
   values). Matches this project's own established "prefer a portable,
   safe abstraction over hand-rolled per-platform code" precedent
   (`wide` for SIMD, TECHNICAL.md Section 2.2) -- no per-platform
   `#[cfg]` code needed.

3. **The frame clock lives in `tre-engine`, not `tre-math`.** It wraps a
   real OS-level timer and owns real, mutable frame-to-frame state (the
   previous tick's own timestamp) -- an engine-lifecycle concern, not a
   pure numeric function. Matches where `InputEventQueue`/`WindowId`/
   other engine-lifecycle types already live in `tre-engine`, as
   distinct from `tre-math`'s purely numeric, state-free functions.

4. **The spring-decay formula lives in `tre-math`, matching `tone_map`'s
   own exact precedent** (`crates/tre-math/src/lib.rs`, Step 7.1): a
   pure function taking every input as an explicit parameter (`current`,
   `target`, `lambda`, `dt`), no hidden state, real formula from
   IMPLEMENTATION.md Step 8.1 task 2 exactly:
   $x(t+\Delta t) = x_{\text{target}} + (x(t) - x_{\text{target}}) \cdot
   e^{-\lambda \Delta t}$. "Spring physics and lerp decay" (the task's
   own phrasing) names one formula, not two -- this is pure exponential
   smoothing toward a target (monotonic, never overshoots), not a
   mass-spring-damper ODE with oscillation; implementing anything beyond
   the one stated formula would be inventing scope not in the outline.

5. **No wiring to any real consumer -- deliberately, matching this
   project's own established precedent.** `Affine2::compose_batch` was
   built and proven against synthetic data with "no scene-graph tree...
   to call it with real parent-child data" (TECHNICAL.md Section 7.2's
   own write-up); `tone_map` (Step 7.1) was built and proven with no
   real HDR swapchain ever selecting it. Both primitives here are the
   same: real, tested, and correct on their own terms, with their real
   consumer (Step 8.1.2's own continuous main loop, and eventually a
   real animation/UI layer beyond Phase 8) deferred until it exists.

## Tasks

1. `tre-engine`: add a real `FrameClock` (name TBD at implementation
   time) wrapping `std::time::Instant`, with a `tick(&mut self) -> f32`
   (or similar) method returning the real elapsed seconds since the
   previous call as an `f32`, first call returning `0.0` (no prior tick
   to measure from). Real unit tests: two real `tick()` calls separated
   by a real `std::thread::sleep` report a delta reasonably close to the
   real sleep duration (a real, if coarse, hardware-timing check, not a
   mocked clock); the first call returns exactly `0.0`.
2. `tre-math`: add `spring_decay(current: f32, target: f32, lambda: f32,
   dt: f32) -> f32` per the exact formula in scope decision 4. Real unit
   tests: `dt == 0.0` returns `current` unchanged; a large `dt` converges
   to within a small epsilon of `target`; monotonic approach (samples at
   increasing `dt` never cross `target`, matching pure exponential
   decay's own real mathematical property); `lambda == 0.0` returns
   `current` unchanged regardless of `dt` (no decay at all).
3. Update `documentation/IMPLEMENTATION.md` -- split "Step 8.1: Loop
   Orchestration & Frame Timing" the same way Step 6.4/7.2 were split:
   the existing heading stays as the original outline, with a new "Step
   8.1.1" write-up below it.

## Verification plan

- `cargo fmt`/`clippy -D warnings`/`build`/`test` clean across the
  workspace.
- New unit tests (both crates) pass, including the real
  `std::thread::sleep`-based clock test.
- No existing demo or test touches either new item (both are net-new,
  zero-consumer additions) -- confirmed via a real grep before
  finalizing, not assumed.
- Commit; push only on explicit "push it"; `gh run watch` after any push
  (`accessibility-validation`'s pre-existing, documented failure
  expected and unrelated).

## Explicitly out of scope

- Wiring either primitive into a real consumer -- Step 8.1.2's own job
  (the frame clock) or later, real UI/animation work beyond Phase 8 (the
  spring-decay formula).
- The full 8-stage continuous main loop, and fixing `execute_frame`'s
  own hardcoded zero buffer offset to support real per-frame ring-buffer
  packing -- both Step 8.1.2's own job.
- Any oscillating/damped mass-spring simulation (position + velocity
  state, critical/under/over-damping) -- not what IMPLEMENTATION.md's
  own Step 8.1 task 2 specifies; the one stated formula is pure
  exponential decay toward a target.
