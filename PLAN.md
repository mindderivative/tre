# Plan: Phase 20 — Rendering Engine Performance & Optimization Test Suite

**Status: Complete (2026-09-13).** See `documentation/IMPLEMENTATION.md`'s
own "Phase 20: Rendering Engine Performance & Optimization Test Suite"
section for the full technical account and
`documentation/REVIEW.md`'s finding #229 for the one real bug found and
fixed while building it. This file records the plan as designed via
`EnterPlanMode`/`ExitPlanMode` and approved by the project owner,
archived here now that the work is done, per this project's own "one
active plan, archived once real work begins" convention (see this
file's own git history for prior phases' plans, each superseded in
place the same way).

## User request (verbatim)

> # Test Suite Specification: Rendering Engine Performance & Optimization
>
> ## Objective
>
> To design and implement a manually triggered suite of performance and
> optimization tests. These tests will profile the rendering engine
> under simulated production environments to identify performance
> limits, resource bottlenecks, and architectural thresholds.
>
> ## Execution Environment & Requirements
>
> - **Production Fidelity**: All tests must run using the engine's
>   production configuration, optimizations, and compilation flags.
> - **Native Integration**: Each test must initialize and render within
>   a standard, native OS window, mirroring real-world application
>   deployment.
> - **Execution Trigger**: Tests will be initiated manually via a
>   command-line interface or test runner.
>
> ## Telemetry & Telemetry Output
>
> The testing suite must collect performance metrics — including
> Frames Per Second (FPS), CPU utilization, GPU utilization, and memory
> thresholds — and output them to two targets: live console output, and
> a persistent structured log file (CSV or JSON).
>
> ## Test Profiles
>
> 1. **Time-Ramp Stress Tests**: 30 seconds per test, geometric scaling
>    of rendered primitives (`1 -> 2 -> 4 -> 8 -> ... N`) at fixed
>    intervals, to identify breaking points/degradation curves for
>    discrete rendering features (shape rendering, textures, shaders).
> 2. **Interaction-Driven Tests**: event-driven, variable length —
>    primary scenario is native window resizing, tracking swapchain
>    recreation/buffer reallocation/viewport scaling under manual
>    stress.

Three architecture decisions were confirmed directly with the project
owner via `AskUserQuestion` before finalizing this plan:
- New dedicated crate (`crates/tre-perf-suite`), not another
  `tre-rhi-vulkan` example — real CLI argument parsing is needed, which
  no existing example does.
- GPU utilization via Linux/AMDGPU sysfs `gpu_busy_percent`, not Vulkan
  timestamp queries or skipping it — matches this project's real dev
  GPU and its own established precedent for disclosed, platform-
  specific implementations.
- All three workload kinds (shapes/textures/shaders) in this first
  pass, not a narrower slice.

## Design (as executed)

New binary-only crate `crates/tre-perf-suite`
(`main.rs`/`telemetry.rs`/`workload.rs`/`ramp_test.rs`/`resize_test.rs`),
depending only on `tre-engine`/`tre-platform`/`tre-rhi-vulkan`/
`raw-window-handle`/`bytemuck` (all already used identically elsewhere
in the workspace). Hand-rolled CLI parsing (no `clap`), hand-formatted
JSON-Lines telemetry (no `serde_json`). Window/device/swapchain/pipeline
construction mirrors `main_loop_demo.rs`/`windowed_renderer.rs`'s own
proven sequence. `shapes`/`textures` workloads mirror
`shape_registry_zero_alloc_demo.rs`/`texture_fill_demo.rs`; `shaders`
was re-scoped during implementation from the originally-planned MSDF
`Text` rendering (no Rust-side `Text`-via-`ShapeRegistry` precedent
exists anywhere in the workspace) to `FillStyle::Gradient`
(`gradient_fill_demo.rs`'s own proven pipeline) — a disclosed
substitution, not a silent one. Full design rationale, the real
ring-buffer-sizing bug found and fixed via the first actual GPU run,
and the complete verification account all live in
`documentation/IMPLEMENTATION.md`'s Phase 20 section.

## Real, disclosed remaining scope

This sandbox has no `xdotool`/`wmctrl` (confirmed absent, the identical
gap Phase 19 Step 19.4 already disclosed) to synthesize a real window
resize unattended, so `resize_test.rs`'s own `InputEvent::Resized` ->
swapchain-rebuild -> `resize_event` logging path was verified only up
to what an unattended environment can exercise (window opens, renders
continuously, telemetry flows without error) — the actual resize-and-
rebuild path itself needs a human physically resizing the window during
a real run, matching this project's own established "a human still
needs to look at some real UI results" disclosure discipline.
