# Plan: M30 Phase 2 Step 3 — Chip

Corresponds to `BUILD_TRACKER.md` M30 Phase 2 Step 3. Written
retroactively alongside implementation — see `LOG.md` and
`BUILD_TRACKER.md`'s own M30 Phase 2 entry for the complete real
investigation, findings, and verification record.

## What changed

- `crates/engine-py/src/window_factory.rs`: `Md3Baseline::
  ON_SURFACE_VARIANT` added; new `resolve_chip_colors` helper;
  `Window.add_chip(label, width, variant="assist", icon=None,
  selected=False, removable=False, x=None, y=None)`.
- `python/tre/_core.pyi`: stub added.
- New tests: `tests/test_chip.py`. New example: `examples/chip.py`.

## Why

Scoped by `BUILD_TRACKER.md`'s own M30 text as Phase 2's third
Selection component, all four real variants. Deliberately built as a
plain composition, not a new `NodeKind` — confirmed the real
architectural dividing line this catalog already draws (`Segmented
Button`'s own app-owned selection vs. `Checkbox`/`RadioButton`/
`Switch`'s engine-owned state) before choosing which side Chip's own
Filter variant belongs on, rather than defaulting to "give it a new
NodeKind because it's stateful." Real MD3 tokens verified directly
against Material Web's source before writing code — found a genuine,
easy-to-miss shape distinction (Chips use `corner-small`, not the
"Full" shape every other component in this catalog so far uses).

## Verification

Full chain, all green: `cargo check --workspace --all-targets`,
`cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt
--check`, `cargo test --workspace --release` (42 binaries, unchanged
— no new engine-render capability needed), `maturin develop
--release`, `pytest tests/` (263 passed, 1 skipped, zero
regressions), all 37 examples, the showcase demo, `mypy --strict`
against `examples/chip.py`.
