# Plan: M30 Phase 3 Step 4 — Divider

Corresponds to `BUILD_TRACKER.md` M30 Phase 3 Step 4. Written
retroactively alongside implementation — see `LOG.md` and
`BUILD_TRACKER.md`'s own M30 Phase 3 entry for the complete real
investigation, findings, and verification record.

## What changed

- `crates/engine-py/src/window_factory.rs`: `DIVIDER_THICKNESS`
  constant; `Window.add_divider(length, vertical=False, x=None,
  y=None)`.
- `python/tre/_core.pyi`: stub added.
- New tests: `tests/test_divider.py`. New example:
  `examples/divider.py`.

## Why

Scoped by `BUILD_TRACKER.md`'s own M30 text as Phase 3's fourth
component — the simplest so far. Real MD3 token verified directly
(1dp, `outline_variant`, the same role `Card`'s Outlined variant
already resolves). One method covers both orientations rather than
two, matching the real single-dimension-plus-orientation shape a line
naturally has.

## Verification

Full chain, all green: `cargo check --workspace --all-targets`,
`cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt
--check`, `cargo test --workspace --release` (43 binaries, unchanged
— no new engine-render capability needed), `maturin develop
--release`, `pytest tests/` (297 passed, 1 skipped, zero
regressions), all 42 examples, the showcase demo, `mypy --strict`
against `examples/divider.py`.
