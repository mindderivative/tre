# Plan: M30 Phase 1 Step 1 — `Button`

Corresponds to `BUILD_TRACKER.md` M30 Phase 1 Step 1. Written
retroactively alongside implementation — see `LOG.md` and
`BUILD_TRACKER.md`'s own M30 Phase 1 entry for the complete real
investigation, findings, and verification record. Rewritten (not
accumulated) as each further M30 phase/step lands, matching this
project's own established `PLAN.md`/`LOG.md` convention —
`BUILD_TRACKER.md` is the durable accumulated record.

## What changed

- `crates/engine-core/src/node.rs`: `PaintProperties` gained
  `border_color: Animated<Color>` / `border_width: Animated<f64>`
  (universal, true-no-op defaults). New `TextAlign` enum
  (Start/Center/End, `#[default] Start`) and `TextState.align:
  TextAlign`.
- `crates/engine-render/src/lib.rs`: `paint_node`'s `Rect`/`Splitter`
  arm now strokes a real border (inset by half its own width) when
  `border_width > 0.0`.
- `crates/engine-render/src/text.rs`: `shaped_layout`/`LayoutCacheKey`
  widened with `align: TextAlign`, resolved to real
  `parley::Alignment::{Start,Center,End}` instead of the old hardcoded
  `Start`.
- `crates/engine-core/src/tree.rs`: `Tree::hit_test_at` — a bare
  `NodeKind::Text` never independently claims a hit any more, always
  deferring to whatever's behind it. Real bug fix, not a design
  preference — see `BUILD_TRACKER.md`'s own writeup.
- `crates/engine-py/src/window.rs`: `ThemeState::role(&self, name:
  &str) -> Option<Color>`, a general MD3 role resolver alongside the
  existing `on_surface()`.
- `crates/engine-py/src/window_factory.rs`: `Window.add_button(label,
  width, height, variant="filled", x=None, y=None)`, MD3's five real
  variants (elevated/filled/filled_tonal/outlined/text). Returns the
  container `Node`; does not auto-`enable_interaction()`.
- `python/tre/_core.pyi`: `add_button` stub added.
- New tests: `engine-render/tests/border_paint.rs`,
  `engine-render/tests/text_align.rs`, `tests/test_button.py`. New
  example: `examples/button.py`.

## Why

`Button` was the one component this catalog's own M30 scoping
confirmed was hand-composed from `Rect`+`Text`+ripple in every
existing example — a real, first-class component was the whole point
of Phase 1 Step 1. Two genuine engine gaps surfaced only by actually
building it, not predicted up front: `engine-render` had no way to
paint a border at all (needed for the Outlined variant) or to center
text within its own box (needed for every variant's label) — both
fixed as universal capabilities, not `Button`-specific hacks, matching
how `elevation`/`shape`/`transform` were each added universally to
`PaintProperties` when a real consumer first needed them. The
hit-testing fix was a real, confirmed functional bug (a button's own
click handler was unreachable), caught by `tests/test_button.py`'s own
click-dispatch test, not designed in advance.

## Verification

Full chain, all green: `cargo check --workspace --all-targets`,
`cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt
--check`, `cargo test --workspace --release` (39 binaries), `maturin
develop --release`, `pytest tests/` (197 passed, 1 skipped, zero
regressions), all 31 examples, the showcase demo, `mypy --strict`
against `examples/button.py` plus a deliberate-error probe confirming
the `.pyi` stub carries real type information.
