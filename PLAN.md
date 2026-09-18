# Plan: M30 Phase 2 Step 4 — Menu (closes Phase 2)

Corresponds to `BUILD_TRACKER.md` M30 Phase 2 Step 4, closing Phase 2
(Selection) entirely — Steps 1-4 plus the incrementally-satisfied Step
5. Written retroactively alongside implementation — see `LOG.md` and
`BUILD_TRACKER.md`'s own M30 Phase 2 entry for the complete real
investigation, findings, and verification record.

## What changed

- `crates/engine-py/src/window_factory.rs`: `Md3Baseline::
  SURFACE_CONTAINER` added; `Window.add_menu_item(label, icon=None,
  width=200.0, x=None, y=None)`, `Window.build_menu(items,
  width=200.0)`, `Window.open_menu(anchor, menu)`, `Window.close_menu(
  menu)`.
- `python/tre/_core.pyi`: stubs added.
- New tests: `tests/test_menu.py`. New example: `examples/menu.py`.

## Why

Scoped by `BUILD_TRACKER.md`'s own M30 text as Phase 2's fourth and
final Selection component, explicitly distinct from the existing
right-click context-menu overlay mechanism. Reused `Tree::
open_overlay`/`close_overlay` directly rather than building a second
overlay primitive — `overlay.rs`'s own module doc comment already
named dropdown menus as a real intended consumer of the same
mechanism context menus use. The context-menu mechanism itself is
completely untouched.

## Verification

Full chain, all green: `cargo check --workspace --all-targets`,
`cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt
--check`, `cargo test --workspace --release` (42 binaries, unchanged
— pure composition plus existing overlay primitives), `maturin
develop --release`, `pytest tests/` (273 passed, 1 skipped, zero
regressions, `test_context_menu.py` itself unmodified and still
green), all 38 examples, the showcase demo, `mypy --strict` against
`examples/menu.py`.
