# Plan: M30 Phase 1 Step 3 — FAB and Extended FAB

Corresponds to `BUILD_TRACKER.md` M30 Phase 1 Step 3. Written
retroactively alongside implementation — see `LOG.md` and
`BUILD_TRACKER.md`'s own M30 Phase 1 entry for the complete real
investigation, findings, and verification record. Rewritten (not
accumulated) as each further M30 phase/step lands.

## What changed

- `crates/engine-py/src/window_factory.rs`: `ButtonBaseline` renamed
  `Md3Baseline`, gained `PRIMARY_CONTAINER`/`ON_PRIMARY_CONTAINER`/
  `TERTIARY_CONTAINER`/`ON_TERTIARY_CONTAINER`/
  `SURFACE_CONTAINER_HIGH`. New `resolve_fab_colors`/`fab_shape`
  helpers (verified against Material Web's own token source). New
  `resolve_icon_path` helper, factored out of `add_icon` once it
  crossed 2+ real call sites. `Window.add_fab(icon, size="default",
  variant="surface", x=None, y=None)` and `Window.add_extended_fab(
  label, width, icon=None, variant="primary", x=None, y=None)`.
- `python/tre/_core.pyi`: `add_fab`/`add_extended_fab` stubs added.
- New tests: `tests/test_fab.py`. New example: `examples/fab.py`.

## Why

Scoped by `BUILD_TRACKER.md`'s own M30 text as the third real Actions
component. Investigation found `FAB`'s real MD3 color-variant system
(Surface/Primary/Secondary/Tertiary) and per-size shape tokens are
genuinely distinct from `Button`'s own — verified directly against
Material Web's real token source before writing any code, not assumed
transferable from Step 1/2's own machinery. `Extended FAB` reuses
`FAB`'s color/elevation system but needed its own real icon+label
anatomy, including the real (verified) padding difference between the
icon and no-icon cases.

## Verification

Full chain, all green: `cargo check --workspace --all-targets`,
`cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt
--check`, `cargo test --workspace --release` (39 binaries), `maturin
develop --release`, `pytest tests/` (227 passed, 1 skipped, zero
regressions), all 33 examples, the showcase demo, `mypy --strict`
against `examples/fab.py`.
