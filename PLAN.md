# Plan: M30 Phase 1 Step 4 — Segmented Button (closes Phase 1)

Corresponds to `BUILD_TRACKER.md` M30 Phase 1 Step 4, closing Phase 1
(Actions) entirely — Steps 1-4 plus the incrementally-satisfied Step 5
(`.pyi` stubs). Written retroactively alongside implementation — see
`LOG.md` and `BUILD_TRACKER.md`'s own M30 Phase 1 entry for the
complete real investigation, findings, and verification record.

## What changed

- `crates/engine-core/src/node.rs`: `PaintProperties.
  corner_radii_override: Option<[f64; 4]>` (true no-op default,
  exposes kurbo's own already-real per-corner `RoundedRect` API).
- `crates/engine-render/src/lib.rs`: `paint_node`'s fill arm uses the
  override when set, falls back to the uniform scalar otherwise.
- `crates/engine-py/src/window_factory.rs`: `Window.
  add_segmented_button(labels, width, selected=None, height=40.0,
  x=None, y=None) -> list[Node]`.
- `python/tre/_core.pyi`: `add_segmented_button` stub added.
- New tests: `crates/engine-render/tests/corner_radii_paint.rs`,
  `tests/test_segmented_button.py`. New example:
  `examples/segmented_button.py`.

## Why

Scoped by `BUILD_TRACKER.md`'s own M30 text as Phase 1's fourth and
final Actions component. The scoped "docking tab anatomy" precedent
was checked directly and found not to transfer (docking's own "active
tab" is logical panel-visibility switching, no shared visual anatomy)
— genuine independent design was needed. The real per-corner-radius
gap was found by actually working through MD3's real anatomy (first/
last segments rounded only on their outer edge), not predicted in
advance; kurbo already had the primitive, so exposing it was a small,
well-scoped addition. Group-exclusivity was deliberately left to the
app, matching `BUILD_TRACKER.md`'s own already-stated Phase 2 design
principle for the future `Radio Button`, reused here rather than
building competing machinery.

## Verification

Full chain, all green: `cargo check --workspace --all-targets`,
`cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt
--check`, `cargo test --workspace --release` (40 binaries), `maturin
develop --release`, `pytest tests/` (234 passed, 1 skipped, zero
regressions), all 34 examples, the showcase demo, `mypy --strict`
against `examples/segmented_button.py`.
