# Plan: M30 Phase 3 Step 3 — Card

Corresponds to `BUILD_TRACKER.md` M30 Phase 3 Step 3. Written
retroactively alongside implementation — see `LOG.md` and
`BUILD_TRACKER.md`'s own M30 Phase 3 entry for the complete real
investigation, findings, and verification record.

## What changed

- `crates/engine-py/src/window_factory.rs`: `Md3Baseline::
  OUTLINE_VARIANT`/`SURFACE` added; new `resolve_card_colors` helper;
  `Window.add_card(width, height, variant="elevated", x=None,
  y=None)`.
- `python/tre/_core.pyi`: stub added.
- New tests: `tests/test_card.py`. New example: `examples/card.py`.

## Why

Scoped by `BUILD_TRACKER.md`'s own M30 text as Phase 3's third
component, all three real variants. Reuses `Button`'s own real
Elevated/Filled/Outlined color pattern at the container level. Built
as a plain `Rect` with no fixed anatomy — a card's real content is
always arbitrary, so the app composes it via the already-generic
`Node.add_child` rather than a new card-specific composition API.

## Verification

Full chain, all green: `cargo check --workspace --all-targets`,
`cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt
--check`, `cargo test --workspace --release` (43 binaries, unchanged
— no new engine-render capability needed), `maturin develop
--release`, `pytest tests/` (294 passed, 1 skipped, zero
regressions), all 41 examples, the showcase demo, `mypy --strict`
against `examples/card.py`.
