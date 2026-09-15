# Plan: M3 Phase 2, Step 1 — Render Core Spike (§14 step 1)

Corresponds to `BUILD_TRACKER.md` M3 Phase 2. First of four steps in that phase; steps 2–4 (`Animated<T>`, `taffy` layout, `parley` text) are separate, later steps, not attempted here.

## Goal

Per §14 step 1: "`engine-render`: one static rounded rect through `vello_hybrid`, presented into a real window `engine-render`'s own example opens via `engine-platform`... No layout, no text, no Python." Prove the actual rendering pipeline exists and the pinned dependency versions really compile and run together — the specific unknown this step exists to de-risk (Design Principle 5), and the first point `ARCHITECTURE.md`'s "verify at implementation time" caveats start getting checked against reality instead of staying theoretical.

## Scope

In scope:
- `engine-render`: a function building one static `vello_hybrid::Scene` (a single rounded rect, solid fill) and a thin wrapper around `vello_hybrid::Renderer` for rendering it into a caller-supplied `wgpu` target.
- `engine-platform`: the minimal winit plumbing to open a real window and pump its event loop, calling back into a per-frame closure. No `AppHandler`/`InputEvent` yet — that's §4/§9's later work, not needed for a single static rect.
- A `[[test]] harness = false` target (matching TRE v1's own `tre-rhi-vulkan` precedent exactly: lives in `tests/`, not `examples/`) that opens a real window, creates a real `wgpu::Surface`, and presents 60 real frames before exiting — self-asserting, not requiring a human to watch it.
- A headless unit test (render-to-texture, read back pixels, assert the fill color landed exactly where expected) as the actual correctness check, independent of whether a display is available.
- Graceful exit-0 (not panic) on "no display" / "no GPU reachable" — TRE v1's own established convention (finding #261), applied from this step's first commit rather than retrofitted later.

Out of scope (later steps in this same phase, or later phases entirely):
- Layout (`taffy`, step 3), text (`parley`, step 4), Python bindings (step 6), multiple/dynamic nodes, animation.

## Steps

1. Verify `vello_hybrid` is a real, current crate (not just an assumption in `ARCHITECTURE.md`) and check its actual public API against its own reference example, rather than guessing signatures.
2. Add real dependencies to `engine-render` (`vello_hybrid`, `wgpu`, `kurbo`, `peniko`, `raw-window-handle`) and `engine-platform` (`winit`) — checking each against what the *other* Linebender-family crates actually require, not just adding the latest version of each independently.
3. Implement `build_rect_scene`/`FrameRenderer` in `engine-render`, verified by a headless pixel-readback test.
4. Implement `run_windowed` in `engine-platform`.
5. Wire both together in a `tests/rect_window.rs` integration test that opens a real window and presents real frames.
6. `cargo test --workspace`, `cargo clippy --workspace --all-targets`, `cargo fmt` all clean.

## Verification

`cargo test --workspace` passes, including both `engine-render` tests (a headless correctness assertion and a real windowed run). `cargo clippy`/`cargo fmt --check` clean.
