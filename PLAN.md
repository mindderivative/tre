# Plan: M30 Phase 3 Step 1 — Badge

Corresponds to `BUILD_TRACKER.md` M30 Phase 3 Step 1. Written
retroactively alongside implementation — see `LOG.md` and
`BUILD_TRACKER.md`'s own M30 Phase 3 entry for the complete real
investigation, findings, and verification record.

## What changed

- `crates/engine-py/src/window_factory.rs`: `Md3Baseline::ERROR`/
  `ON_ERROR` added; `Window.add_badge(label=None, width=None, x=None,
  y=None)`.
- `python/tre/_core.pyi`: stub added.
- New tests: `tests/test_badge.py`. New example: `examples/badge.py`.

## Why

Scoped by `BUILD_TRACKER.md`'s own M30 text as the first Phase 3
component. Real MD3 tokens verified directly against Material Web's
source before writing code. Deliberately given no anchoring machinery
of its own — a badge is always overlaid on another component's corner
via plain caller-chosen `x`/`y`, matching the existing `add_rect`
contract rather than inventing a new positioning primitive for
something this simple.

## Verification

Full chain, all green: `cargo check --workspace --all-targets`,
`cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt
--check`, `cargo test --workspace --release` (42 binaries, unchanged
— no new engine-render capability needed), `maturin develop
--release`, `pytest tests/` (278 passed, 1 skipped, zero
regressions), all 39 examples, the showcase demo, `mypy --strict`
against `examples/badge.py`.
