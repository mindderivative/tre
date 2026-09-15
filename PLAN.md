# Plan: M3 Phase 3 — `engine-spec` Minimal (§14 step 5)

Corresponds to `BUILD_TRACKER.md` M3 Phase 3, the sole step of that phase.

## Goal

Per §14 step 5: "**`engine-spec`, minimal:** parse one static `view.yaml`
(no `bindings:`/`handlers:` yet) — `WidgetSpec` → `NodeKind` mapping,
`deny_unknown_fields` validation — and build a `Tree` from it, rendered
through the pipeline steps 1–4 already proved. Zero `pyo3` involvement at
this point; this de-risks parsing/validation/mapping in isolation, per
Design Principle 5, before anything depends on it working."

## Scope

In scope:
- `engine-spec::spec`: `WidgetSpec`/`NodeKindSpec`/`TextSpec`/
  `FlexDirectionSpec`/`StyleSpec`, matching §16.1's `WidgetSpec` sketch
  minus `bindings`/`handlers` (deferred to step 12, needs
  `BindingResolver`/`engine-py`). `StyleSpec` is literal-value-only
  (`background: "#6750A4"`), not MD3 token names (needs step 11's
  `material-colors`). `deny_unknown_fields` throughout.
- `engine-spec::build`: `WidgetSpec` → `engine_core::Tree`, via
  `taffy::Style`/`NodeKind`/`PaintProperties` construction.
  `background` required (a clear `SpecError`, not a silent default) for
  `Rect`/`Text`; optional (defaults transparent) for `Container`.
- Unit tests: parsing correctness, `deny_unknown_fields` rejecting a
  typo'd key, a deterministic-layout build test (exact taffy positions,
  same discipline as step 3's own), `MissingField`/`InvalidColor` error
  paths.
- A real, standalone `view.yaml` fixture
  (`crates/engine-spec/examples/view.yaml`) and an `engine-render`
  integration test proving it renders through the real Phase 2 pipeline
  (headless pixel-readback: the swatch's exact declared color, real ink
  in the label's box).

Out of scope: `bindings:`/`handlers:`, `BindingResolver`, MD3 token
resolution, the stylesheet cascade (§16.3), reconciliation/hot-reload
(§16.4), `include:` composition (§16.6), any `pyo3` involvement.

## Verification

`cargo test --workspace` (all green), `cargo clippy --workspace
--all-targets -- -D warnings` and `cargo fmt --check` clean (matching
CI's own exact commands, now that CI is real and verified).
