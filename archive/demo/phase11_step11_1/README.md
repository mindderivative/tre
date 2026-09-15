# Demo: Phase 11 Step 11.1 -- winit-Backed Windowing Migration

```bash
./demo/phase11_step11_1/run_regression_sweep.sh
```

**What this closes.** `tre-platform` (Phase 1 Steps 1.1/1.2) previously
hand-rolled two separate, protocol-level window-creation backends: raw
Wayland (`wayland-client`/`wayland-protocols`, `xdg_toplevel`) and raw X11
(`x11rb` XCB FFI), ~786 lines of manual protocol/unsafe-pointer code
combined. At the project owner's own explicit direction -- "I do not
believe a hand-rolled approach for windowing is the right path" -- both
were replaced by a single implementation backed by `winit`, the de facto
standard Rust windowing crate.

**No new demo binary.** Unlike every other per-step folder in this
project, this step is a bounded backend swap, not new functionality:
`PlatformConnection`'s public API (`new`/`new_wayland`/`new_x11`,
`create_window`, `poll_events`, `scale_factor`, `window_handle`,
`HasDisplayHandle`) is preserved exactly, so all 40 pre-existing demo files
in `crates/tre-rhi-vulkan/examples/` needed zero changes. The correctness
oracle for this step is therefore that every one of those pre-existing
demos still behaves identically under the new backend -- not a new demo
proving new behavior.

**What's real now.** `crates/tre-platform/src/winit_backend.rs` (new) --
one `WinitConnection` type backed by `winit` 0.30.13, replacing the
deleted `wayland.rs`/`x11.rs`. The hardest design question (whether
`create_window()` could stay synchronous despite winit's callback-driven
`ApplicationHandler` model) was resolved by reading winit's actual source,
not assumed -- see `documentation/IMPLEMENTATION.md`'s Step 11.1 write-up
for the full technical account.

**Full verification performed during implementation:**

- `cargo fmt --all -- --check` / `cargo clippy --workspace --all-targets --
  -D warnings` / `cargo build --workspace --all-targets` / `cargo test
  --workspace` -- all clean, zero failures across the whole workspace.
- `crates/tre-platform/examples/smoke_test.rs` run against a real Wayland
  session (auto-detected), then explicitly forced to each backend
  (`TRE_FORCE_BACKEND=wayland` / `TRE_FORCE_BACKEND=x11`, the latter via
  XWayland) -- all three created a real window and received real
  `Resized`/`PointerMoved` events.
- **All 41 `tre_platform`-dependent demos** in
  `crates/tre-rhi-vulkan/examples/` re-run individually against real GPU
  hardware (`VK_LAYER_KHRONOS_validation` enabled): zero panics, zero
  non-zero exit codes, zero error/failure text in output.
- `main_loop_demo.rs` (this project's own reference imperative main loop)
  specifically verified end to end: 90 real frames presented, its
  `spring_decay` animation re-verified, and its Step 9.2
  `tre_memory::RenderTickGuard` zero-allocation guard still passed
  (honest scope note: `poll_events()` runs *before* that guard's span
  begins, so this confirms the render pipeline is unaffected, not that
  winit's own `pump_app_events` call is itself allocation-free).
- `multi_window.rs` specifically verified: two real windows created on one
  `PlatformConnection`, both rendered for 120 frames, confirming the
  multi-window creation/event-routing pattern this whole design was built
  to preserve.
- `cargo tree -p tre-platform` inspected to honestly report the real
  dependency-footprint increase (REVIEW.md finding #179) rather than
  assume the trimmed `winit` feature set was acceptable without checking.

**Three real findings disclosed, not fixed silently or hidden** (REVIEW.md
findings #177-179):

- Wayland's `app_id` (hardcoded to `"tre-walking-skeleton"`, a Phase-0
  leftover) disappeared as a side effect of deleting the backend it lived
  in -- an incidental fix, not a targeted one.
- `scale_factor`'s `i32` return type initially rounded away the real
  per-window `f64` precision winit now supplies (Wayland
  `wp-fractional-scale`, X11 `Xft.dpi`/RandR) -- **fixed same-day**, at the
  project owner's explicit follow-up direction: both `PlatformConnection::
  scale_factor` and `WinitConnection::scale_factor` were widened to `f64`.
  A workspace-wide grep before making the change confirmed the only real
  caller anywhere was `smoke_test.rs`'s own diagnostic print -- none of the
  40 `tre-rhi-vulkan` demos call this method at all.
- `winit`'s dependency tree is materially larger than the four crates it
  replaced, even with `default-features = false` and a trimmed feature
  list excluding `wayland-csd-adwaita` -- the accepted, expected cost of
  moving off a hand-rolled protocol integration.

**One real, additional hardening found and taken, not originally
planned:** `tre-platform` no longer contains any `unsafe` code at all
(winit's `Window`/`EventLoop` implement `raw-window-handle` 0.6's traits
directly, so no `RawWindowHandle`/`RawDisplayHandle` is ever hand-
constructed here anymore) -- the crate now carries
`#![forbid(unsafe_code)]` and is removed from TECHNICAL.md Section 9.1's
closed set of crates permitted to contain `unsafe`.
