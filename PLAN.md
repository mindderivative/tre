# Plan: M29 — Render Loop Dirty-Tracking

Corresponds to `BUILD_TRACKER.md` M29 (both phases). Written
retroactively alongside implementation — see `LOG.md` and
`BUILD_TRACKER.md`'s own M29 entry for the complete real investigation,
findings, and verification record.

## What changed

- `crates/engine-core/src/tree.rs`: `Tree` gained a private `dirty:
  bool` field (starting `true`) and `pub fn take_dirty(&mut self) ->
  bool`. All 29 real mutating methods set it as their first statement
  (mechanically inserted via a script against exact signature
  boundaries); `tick_all` sets it whenever its own `any_active` is
  `true`. New test `take_dirty_reports_true_after_each_real_mutation_
  category_and_false_between`.
- `crates/engine-py/src/app.rs`: `App::run`'s per-frame closure reads
  `tree.take_dirty()` after `tick_all`/before `compute_layout`, skipping
  layout/GPU work entirely when nothing changed. The closure now
  returns `bool` (`tick_all`'s own `any_active`) at every exit point,
  for `engine-platform`'s own polling decision.
- `crates/engine-platform/src/lib.rs`: `run_windowed_multi`'s `on_frame`
  bound widened to `FnMut(WindowId, u32) -> bool`. `PerWindow` gained
  `animating: bool`. `RedrawRequested` sets `ControlFlow::Poll` while
  any open window is animating, `ControlFlow::Wait` once all have
  settled. Every real input-handling arm in `window_event`
  (`CursorMoved`/`MouseInput`/`KeyboardInput`/`MouseWheel`/
  `ThemeChanged`/`Ime`) now calls `request_redraw()` explicitly, as
  does the AccessKit `ActionRequested` path in `user_event`.
  `run_windowed`'s own public signature stayed unchanged — its internal
  wrapper always reports `true`, preserving `rect_window.rs`/
  `access_button.rs`'s pre-M29 behavior byte-for-byte.
- `crates/engine-platform/tests/multi_window.rs`: updated its
  `on_frame` closure to return `true` (same reasoning as
  `run_windowed`'s wrapper).

## Real bug found and fixed during implementation

A window opened with `max_frames: Some(_)` — this codebase's own
dominant example/test pattern — hung indefinitely under the first real
Phase 2 implementation: once `on_frame` reported "not animating,"
nothing kept requesting its next redraw, so it never reached its own
frame count. Caught by actually running every example against the
change (several timed out), not assumed. Fixed by treating any window
with `max_frames: Some(_)` as always-animating for `ControlFlow`
purposes, regardless of what `on_frame` itself reports — only a
genuinely unbounded window (`max_frames: None`) gets Phase 2's
idle-CPU benefit. Documented in `run_windowed_multi`'s own doc comment.

## What was deliberately not done

- Window resize (`WindowEvent::Resized`/`ScaleFactorChanged`) handling
  — confirmed via grep this codebase has no resize support anywhere
  yet, a real, pre-existing, separate gap this milestone's own
  investigation surfaced but didn't need to touch.
- Partial/incremental repaint (redrawing only the changed screen
  region) — a materially larger change to `engine-render`'s whole-tree
  paint walk; this milestone's dirty flag is coarse (whole-frame
  yes/no), matching the existing architecture.

## Verification

Full `cargo check`/clippy `-D warnings`/fmt clean. `cargo test
--workspace --release` clean (`engine-core` 146, up from 145).
`maturin develop --release` + pytest (187 passed, 1 pre-existing skip),
all 29 examples (re-run twice — once before, once after the
`max_frames` fix, to confirm the hang was genuinely resolved), and the
showcase demo (real click/keyboard/drag interaction, not just a static
scene) all clean with the real display. Empirical idle-CPU check: an
unbounded static window settled to ~0.8–1.3% CPU over several real
seconds (`ps -o pcpu`), consistent with `ControlFlow::Wait` genuinely
taking effect.

See `LOG.md` for the full narrative.
