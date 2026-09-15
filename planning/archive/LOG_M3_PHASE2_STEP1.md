# Log: M3 Phase 2, Step 1 — Render Core Spike (§14 step 1)

Corresponds to `PLAN.md` / `BUILD_TRACKER.md` M3 Phase 2. First contact with real external dependencies in the v2 rebuild — everything in M3 Phase 1 was local-path-only.

## What happened

**Verified `vello_hybrid` is real before writing anything against it.** `cargo add --dry-run` confirmed `vello_hybrid 0.2.0` is a real, published crate with `wgpu`/`wgpu_default`/`text` features (not default-on) — the core architectural bet `ARCHITECTURE.md` §1 makes actually holds. Read its own `examples/render_to_file.rs` reference example directly rather than guessing the `Scene`/`Renderer` API shape from documentation memory.

**Found two real, concrete instances of the Linebender-family version-coupling risk §3 already flagged in the abstract — both found by trying to actually build, not by review:**

1. `vello_hybrid 0.2.0` requires `wgpu = "29.0.3"`. Adding the latest `wgpu` (`30.0.1`) independently resolved **two different major versions of wgpu into the same dependency graph** — Cargo allowed it silently; it would have failed to compile the moment a `wgpu::Device` created in `engine-render`'s own code was passed into a `vello_hybrid` function expecting its own, incompatible `wgpu` 29.x type. Fixed by pinning `engine-render`'s own `wgpu` to exactly `29.0.3`, confirmed by `grep -c 'name = "wgpu"' Cargo.lock` dropping from 2 to 1.
2. Four different `kurbo` versions are in the dependency graph from unrelated transitive deps. `peniko 0.6.1` (already a dependency) happens to pin the same `kurbo = "0.13.1"` I'd have added directly — no active conflict *right now* — but this is coincidence, not a guarantee: nothing stops a future independent `cargo update` of either from drifting them apart silently, since Cargo treats compatible-semver bumps as fine and won't warn. Documented this explicitly in `Cargo.toml` with instructions to re-verify against `peniko`'s own `Cargo.toml` before bumping either, rather than leaving the coincidence unexamined.

**Implemented and verified the actual step 1 scope:**
- `engine-render::build_rect_scene` / `FrameRenderer` — a static rounded rect, `peniko::Color::from_rgba8`, `kurbo::RoundedRect::to_path`, wrapped `vello_hybrid::Renderer`.
- `engine-platform::run_windowed` — minimal `winit` `ApplicationHandler`, hands back `Arc<Window>` (not `&Window`) specifically because a caller building a `wgpu::Surface` needs to keep it alive across every subsequent frame callback, which a borrow scoped to one call cannot support.
- `crates/engine-render/tests/rect_window.rs` — a real, working windowed run: opens an actual OS window (verified against this environment's real Wayland display and AMD GPU, not just Lavapipe), creates a real `wgpu::Surface`, presents 60 real frames, exits 0. Lives in `tests/`, not `examples/`, matching `archive/crates/tre-rhi-vulkan`'s own precedent exactly (checked directly, not assumed) — `[[test]] harness = false` gives it a real, single main thread, which `winit::EventLoop::new()` requires and a plain `#[test]` fn (run on a worker thread) cannot provide.
- A headless pixel-readback unit test in `engine-render/src/lib.rs`: renders to an offscreen texture, reads it back, asserts the fill color landed exactly at the rect's center and the background is untouched at a corner outside it. Real correctness verification, not just "it didn't panic."
- Graceful exit-0 on "no display"/"no GPU" (`run_windowed` returns `Result` instead of panicking on `EventLoop::new()` failure; the adapter-request failure path in the test calls `std::process::exit(0)` directly) — applied from this step's first commit, per TRE v1's own established convention (LESSONS_LEARNED.md, finding #261), not retrofitted after the fact.

**Structural fix along the way:** the same-file-serves-two-target-kinds approach (`[[example]]` + `[[test]]` both pointing at `examples/rect.rs`) produced a real Cargo warning ("found to be present in multiple build targets"). Checked `archive/crates/tre-rhi-vulkan/Cargo.toml` directly rather than guessing around the warning — TRE v1 never dual-registered a single file; demos meant for `cargo test` lived in `tests/`, separate from `examples/`. Moved `rect.rs` to `tests/rect_window.rs` to match exactly; the warning is gone.

## Verification

```
$ cargo test --workspace
    ...
     Running unittests src/lib.rs (engine_render-...)
running 1 test
test tests::rect_scene_renders_expected_pixels ... ok

     Running tests/rect_window.rs (rect_window-...)
engine-render §14 step 1: first frame presented, 400x400
engine-render §14 step 1: exited cleanly after 60 frames
    ...
test result: ok. (all crates, 0 failures)

$ cargo clippy --workspace --all-targets   # clean, one collapsible-if fixed along the way
$ cargo fmt --check                        # clean
```

## Next

`BUILD_TRACKER.md` M3 Phase 2 partially updated (step 1 of 4 done — the phase itself stays 🚧 until steps 2–4 land). Next: step 2, `Animated<T>` + the central tick, animating this same rect's color/elevation.
