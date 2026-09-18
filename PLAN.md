# Plan: M30 Phase 1 Step 2 — Icon Button

Corresponds to `BUILD_TRACKER.md` M30 Phase 1 Step 2. Written
retroactively alongside implementation — see `LOG.md` and
`BUILD_TRACKER.md`'s own M30 Phase 1 entry for the complete real
investigation, findings, and verification record. Rewritten (not
accumulated) as each further M30 phase/step lands — `BUILD_TRACKER.md`
is the durable accumulated record.

## What changed

- `crates/engine-core/src/tree.rs`: `Tree::hit_test_at` — `NodeKind::
  Icon(_) => false` added alongside the earlier `NodeKind::Text` arm,
  proactively closing the identical hit-test gap before it could bite
  Icon Button.
- `crates/engine-py/src/window_factory.rs`: `Window.add_icon_button(
  icon, size=40.0, variant="standard", x=None, y=None)`, MD3's four
  real variants (filled/filled_tonal/outlined/standard). Reuses
  `resolve_button_colors` (with `"standard"` translated to its own
  `"text"` at the call site, not aliased inside it).
- `python/tre/_core.pyi`: `add_icon_button` stub added.
- New tests: `tests/test_icon_button.py`. New example:
  `examples/icon_button.py`.

## Why

Scoped explicitly by `BUILD_TRACKER.md`'s own M30 text as `Button`'s
anatomy with an `Icon` child instead of `Text` — built directly on
Step 1's own container/color machinery rather than a second,
near-duplicate implementation. The hit-test fix was applied
proactively this time (not found by a second failing test): Step 1's
own real bug (a centered child silently eating clicks meant for its
container) generalizes to any composite interactive node with a
same-shaped child, and `Icon` was the next real instance, confirmed
via grep before writing the fix that nothing relies on a standalone
icon being independently clickable today.

## Verification

Full chain, all green: `cargo check --workspace --all-targets`,
`cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt
--check`, `cargo test --workspace --release` (39 binaries), `maturin
develop --release`, `pytest tests/` (207 passed, 1 skipped, zero
regressions), all 32 examples, the showcase demo, `mypy --strict`
against `examples/icon_button.py`.
