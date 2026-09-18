# Plan: M30 Phase 3 Step 2 — Progress Indicator (Linear and Circular)

Corresponds to `BUILD_TRACKER.md` M30 Phase 3 Step 2. Written
retroactively alongside implementation — see `LOG.md` and
`BUILD_TRACKER.md`'s own M30 Phase 3 entry for the complete real
investigation, findings, and verification record.

## What changed

- `crates/engine-core/src/node.rs`: new `LinearProgressState { value:
  Animated<f64>, track_tint, indicator_tint }` and `CircularProgressState
  { value: Animated<f64>, indicator_tint }`; `NodeKind::LinearProgress`/
  `NodeKind::CircularProgress`.
- `crates/engine-core/src/tree.rs`: `tick_all` gained ticking arms for
  both.
- `crates/engine-render/src/lib.rs`: paint arms — linear bar (flat,
  `corner-none`, real `value * w` indicator width) and circular arc
  (kurbo `Arc`, 12 o'clock start, clockwise sweep).
- `crates/engine-py/src/node.rs`: the `"value"` arm of `animate`/`get`
  for both, `kind_name` entries.
- `crates/engine-py/src/window_factory.rs`: `Window.add_linear_progress(
  width, height=4.0, value=0.0, x=None, y=None)`, `Window.
  add_circular_progress(size=48.0, value=0.0, x=None, y=None)`.
- `python/tre/_core.pyi`: stubs added.
- New tests: `crates/engine-render/tests/progress_paint.rs`,
  `tests/test_progress.py`. New example: `examples/progress.py`.

## Why

Scoped by `BUILD_TRACKER.md`'s own M30 text as Phase 3's second
component, both real shapes. Mirrors `Slider`'s own real `thumb_
position` shape (the closest existing precedent: a continuous 0..1
value painted as track+indicator), with the one genuine difference
that a progress indicator is never draggable. Real MD3 tokens verified
directly against Material Web's source before writing code — found
that circular indicators have no background track ring at all in real
MD3, and that both indicators use a flat, unrounded shape rather than
this catalog's usual rounded default.

## Verification

Full chain, all green: `cargo check --workspace --all-targets`,
`cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt
--check`, `cargo test --workspace --release` (43 binaries), `maturin
develop --release`, `pytest tests/` (286 passed, 1 skipped, zero
regressions), all 40 examples, the showcase demo, `mypy --strict`
against `examples/progress.py`.
