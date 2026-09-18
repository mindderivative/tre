# Plan: M30 Phase 2 Step 2 — Switch

Corresponds to `BUILD_TRACKER.md` M30 Phase 2 Step 2. Written
retroactively alongside implementation — see `LOG.md` and
`BUILD_TRACKER.md`'s own M30 Phase 2 entry for the complete real
investigation, findings, and verification record.

## What changed

- `crates/engine-core/src/node.rs`: new `SwitchState { on,
  toggle_progress: Animated<f64>, track_off_tint, track_on_tint,
  track_outline_tint, handle_off_tint, handle_on_tint }` and
  `NodeKind::Switch(SwitchState)`.
- `crates/engine-core/src/tree.rs`: `tick_all` gained a `toggle_
  progress` ticking arm; `set_toggled` accessibility mirror added.
- `crates/engine-render/src/lib.rs`: paint arm — track fill
  (color-interpolated), a fading outline stroke, a handle that both
  slides and grows.
- `crates/engine-py/src/node.rs`: `set_on`/`get_on`, the `"toggle_
  progress"` arm of `animate`/`get`, `kind_name` entry.
- `crates/engine-py/src/window_factory.rs`: `Window.add_switch(
  width=52.0, height=32.0, on=False, x=None, y=None)`;
  `Md3Baseline::SURFACE_CONTAINER_HIGHEST` added.
- `python/tre/_core.pyi`: stub added.
- New tests: `crates/engine-render/tests/switch_paint.rs`,
  `tests/test_switch.py`. New example: `examples/switch.py`.

## Why

Scoped by `BUILD_TRACKER.md`'s own M30 text as Phase 2's second
Selection component. Real MD3 tokens verified directly against
Material Web's source before any code was written, matching the
discipline `FAB`/`Segmented Button` already established for this
milestone. The handle's slide-and-grow formula was derived
algebraically from those verified ratios, not fitted by trial and
error — checked that both endpoints reduce to the same symmetric
travel range before committing to the formula.

## Verification

Full chain, all green: `cargo check --workspace --all-targets`,
`cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt
--check`, `cargo test --workspace --release` (42 binaries), `maturin
develop --release`, `pytest tests/` (251 passed, 1 skipped, zero
regressions), all 36 examples, the showcase demo, `mypy --strict`
against `examples/switch.py`.
