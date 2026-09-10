# Plan: Phase 11 Step 11.1 — Migrate `tre-platform` to a `winit`-Backed Windowing Implementation

**Status: Complete (2026-09-10).** See `documentation/IMPLEMENTATION.md`'s
own Phase 11 write-up for the full technical account of what shipped,
`documentation/REVIEW.md`'s Phase 11 Step 11.1 section (findings #177-179:
one incidental fix, two disclosed-not-fixed decisions), and
`demo/phase11_step11_1/README.md` for the verification summary. This file
is the original plan (as approved via `EnterPlanMode`/`ExitPlanMode`),
archived unchanged below now that this step's real work is done; the
full-detail engineering plan itself (including the exact source-verified
winit API research) is preserved at
`/home/phil/.claude/plans/warm-painting-squid.md`.

## User request (verbatim, across the conversation that led to this plan)

> Ok, I believe we should look at window creation management crates...
> app_window looks promising with async runtimes, and supports all the
> desktop platforms.

> I understand wayland handles the position and movement of the window and
> is not something we can control. I am more concerned about efficiency and
> performance. I do not believe a hand-rolled approach for windowing is the
> right path as there are fully developed and tested options available.
> Winit seems like the best and most robust choice for us.

## Context

A prior investigation (same session) into whether `tre-platform` exposed a
full window-chrome/lifecycle interface (resize, position/movement, title
bar with close/minimize/maximize, icon) found real gaps in the hand-rolled
Wayland (`wayland-client`)/X11 (`x11rb`) backends: no post-creation title
change, no minimize/maximize, no icon support, plus a concrete leftover bug
(Wayland's `app_id` hardcoded to `"tre-walking-skeleton"`). Position/
movement control was confirmed to be a genuine Wayland-protocol-level
restriction no library can lift. The project owner's conclusion: replace
the hand-rolled protocol integrations with `winit`, primarily for
robustness/correctness, not to chase new API surface.

**Explicit non-goal:** no new public API surface (`set_title`,
`set_minimized`, `set_maximized`, `set_window_icon`, etc.) — `tre_platform::
PlatformConnection`'s public method signatures are preserved exactly, so
none of the 40 pre-existing demo files in `crates/tre-rhi-vulkan/examples/`
needed to change.

## Design (verified against winit 0.30.13's actual source before writing
## any code)

- `ActiveEventLoop` (required to create a `Window`) is only reachable
  inside an `ApplicationHandler` callback. Traced through winit's own
  `platform_impl` for both X11 and Wayland: passing `timeout: Some(Duration
  ::ZERO)` to `pump_app_events` guarantees `ApplicationHandler::new_events`
  runs on every single pump call (not just `resumed`, which fires exactly
  once). `create_window()` stages a request and immediately pumps once
  itself, draining it inside `new_events` before returning — fully
  synchronous from the caller's perspective.
- One `winit_backend::WinitConnection` (new) replaces both `wayland.rs` and
  `x11.rs` (deleted) — winit unifies both backends behind one set of types.
  `PlatformConnection::Wayland`/`X11` both wrap it, forced via
  `EventLoopBuilderExtWayland::with_wayland`/`EventLoopBuilderExtX11::
  with_x11`.
- `WindowId` allocation keeps the previous internal counter scheme exactly
  (grep-confirmed nothing outside `tre-platform` ever constructs a
  `WindowId` directly).
- `WindowEvent` → `tre_engine::InputEvent` translates 1:1, still routed
  through the existing `InputEventQueue` (pointer-move coalescing
  unchanged). `key_code` sourced from `winit::platform::scancode::
  PhysicalKeyExtScancode::to_scancode()`, confirmed to produce the same
  Linux evdev numbering the engine's `InputEvent::KeyboardKey` already
  contracts.
- `winit = { version = "0.30", default-features = false, features =
  ["rwh_06", "x11", "wayland", "wayland-dlopen"] }` — explicitly without
  `wayland-csd-adwaita` (client-side-decoration title-bar rendering this
  project doesn't need, since it talks to real compositors directly).

## Real risks disclosed before implementation (see REVIEW.md #177-179 for
## final disposition)

1. `scale_factor`'s preserved `i32` signature rounds away the real
   per-window `f64` precision winit now supplies.
2. `EventLoop` may only be constructed once per OS process, ever — a real,
   permanent restriction, confirmed harmless for every current call site.
3. Dependency footprint increases even with the trimmed feature set.

## Tasks

1. Update `crates/tre-platform/Cargo.toml` deps.
2. Add `crates/tre-platform/src/winit_backend.rs`; delete `wayland.rs`/
   `x11.rs`.
3. Update `lib.rs`: both `PlatformConnection` variants wrap
   `WinitConnection`; all five public methods delegate unchanged.
4. Update `ARCHITECTURE.md`/`IMPLEMENTATION.md`/`TECHNICAL.md`/`REVIEW.md`.
5. `demo/phase11_step11_1/README.md`.

**All five tasks completed as written**, plus one real, additional
hardening found and taken during implementation, not originally planned:
`tre-platform` no longer contains any `unsafe` code at all (winit's
`Window`/`EventLoop` implement `raw-window-handle` 0.6's traits directly),
so the crate now carries `#![forbid(unsafe_code)]` and is removed from
TECHNICAL.md Section 9.1's closed set of crates permitted to contain
`unsafe`.

## Verification plan — executed exactly as planned

`cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D
warnings`, `cargo build --workspace --all-targets`, `cargo test --workspace`
all clean (zero failures). `smoke_test.rs` run against a real Wayland
session and, forced via `TRE_FORCE_BACKEND=x11`, against XWayland — both
created a real window and received real `Resized`/`PointerMoved` events.
All 41 `tre_platform`-dependent demos re-run on real GPU hardware
(`VK_LAYER_KHRONOS_validation` enabled), zero failures: `main_loop_demo.rs`
(the project's own reference imperative main loop) completed 90 real
frames with its animation and Step 9.2 zero-allocation guard both verified;
`multi_window.rs` created two real windows on one `PlatformConnection` and
rendered both for 120 frames. `cargo tree -p tre-platform` inspected to
honestly report the real dependency-footprint increase (REVIEW.md #179)
rather than assume it acceptable without looking.
