# Plan: M30 Phase 2 Step 1 — Radio Button

Corresponds to `BUILD_TRACKER.md` M30 Phase 2 Step 1. Written
retroactively alongside implementation — see `LOG.md` and
`BUILD_TRACKER.md`'s own M30 Phase 2 entry for the complete real
investigation, findings, and verification record.

## What changed

- `crates/engine-core/src/node.rs`: new `RadioButtonState { selected,
  select_progress: Animated<f64>, unselected_tint, selected_tint }`
  and `NodeKind::RadioButton(RadioButtonState)`.
- `crates/engine-core/src/tree.rs`: `tick_all` gained an explicit new
  `select_progress` ticking arm; `set_toggled` accessibility mirror
  added.
- `crates/engine-render/src/lib.rs`: paint arm — a stroked ring
  (color interpolated between the two tints via `select_progress`)
  plus a scaling inner dot.
- `crates/engine-py/src/node.rs`: `set_selected`/`get_selected`, the
  `"select_progress"` arm of `animate`/`get`, `kind_name` entry.
- `crates/engine-py/src/window_factory.rs`: `Window.add_radio_button(
  size=20.0, selected=False, x=None, y=None)`.
- `python/tre/_core.pyi`: stub added.
- New tests: `crates/engine-render/tests/radio_button_paint.rs`,
  `tests/test_radio_button.py`. New example: `examples/radio_button.py`.

## Why

Scoped by `BUILD_TRACKER.md`'s own M30 text as Phase 2's first
Selection component, explicitly mirroring `Checkbox`'s own real shape.
The one real anatomy difference (a color-interpolating ring, not a
static box) was designed by actually working through MD3's real radio
button appearance, not predicted in advance. Group-exclusivity was
deliberately left to the app, matching the scope text's own explicit
framing and Design Principle 6.

## Verification

Full chain, all green: `cargo check --workspace --all-targets`,
`cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt
--check`, `cargo test --workspace --release` (41 binaries), `maturin
develop --release`, `pytest tests/` (243 passed, 1 skipped, zero
regressions), all 35 examples, the showcase demo, `mypy --strict`
against `examples/radio_button.py`.
