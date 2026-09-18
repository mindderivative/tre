# Plan: M30 Phase 3 Step 5 — Tooltip (closes Phase 3)

Corresponds to `BUILD_TRACKER.md` M30 Phase 3 Step 5, closing Phase 3
(Communication & Containment, Part 1) entirely — Steps 1-5 plus the
incrementally-satisfied Step 6. Written retroactively alongside
implementation — see `LOG.md` and `BUILD_TRACKER.md`'s own M30 Phase
3 entry for the complete real investigation, findings, and
verification record.

## What changed

- `crates/engine-py/src/window_factory.rs`: `Md3Baseline::
  INVERSE_SURFACE`/`INVERSE_ON_SURFACE` added; `Window.add_tooltip(
  text, width, x=None, y=None)`.
- `python/tre/_core.pyi`: stub added.
- New tests: `tests/test_tooltip.py`. New example:
  `examples/tooltip.py`.

## Why

Scoped by `BUILD_TRACKER.md`'s own M30 text as Phase 3's fifth and
final component (Plain variant). Deliberately reused `Window.
open_menu`/`close_menu` (Step 4) rather than building a second overlay
mechanism — `overlay.rs`'s own module doc comment already names
tooltips as a real intended consumer of the same primitive. Triggered
from the app's own already-generic `Node.set_on_hover_enter`/
`set_on_hover_exit` rather than a dedicated open/close pair that would
only duplicate the existing ones.

## Verification

Full chain, all green: `cargo check --workspace --all-targets`,
`cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt
--check`, `cargo test --workspace --release` (43 binaries, unchanged
— pure composition plus existing overlay primitives), `maturin
develop --release`, `pytest tests/` (301 passed, 1 skipped, zero
regressions), all 43 examples, the showcase demo, `mypy --strict`
against `examples/tooltip.py`.
