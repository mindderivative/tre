# Log: M20 Phase 2 — Real TextField Component Theming (§7.1, §7.3), closing M20

Corresponds to `BUILD_TRACKER.md` M20 Phase 2, closing M20 entirely
(both phases). `TextFieldState` gains `text_tint: Color`, threaded
through `TextRenderer::draw_field`'s own `at.color` and pushed the
same real way Phase 1's two fields already are.

## Investigation before writing code

`paint_node`'s `TextField` arm computed `text_color` from a hardcoded
`Color::from_rgba8(0x1C, 0x1B, 0x1F, 0xFF)` literal — confirmed via
direct read. **Real, confirmed continuation of Phase 1's own finding:**
`0x1C1B1F` is not equal to `ThemeState::on_surface()`'s own
no-theme-set default (real black) — the identical real gap Phase 1
found and fixed for `Checkbox`/`Slider` applies here too.

## What happened

`TextFieldState` gains `text_tint: Color`, defaulting to the exact
historical literal. `Tree::set_all_component_tints` gained a
`NodeKind::TextField(state) => state.text_tint = tint` arm, closing
the milestone's own real mechanism (already called from both `Window.
set_theme` and the real live OS `ThemeChanged` path, unchanged since
Phase 1). `paint_node`'s `TextField` arm reads `state.text_tint`
instead of the literal. `Window.add_text_field` seeds `text_tint` from
the current theme, gated on `ThemeState::is_set()` — the identical
pattern Phase 1 already established for `add_checkbox`/`add_slider`,
avoiding the exact same real "default color doesn't coincide with
`on_surface()`'s no-theme black" bug that phase caught before shipping.

Widened the existing `set_all_component_tints` `engine-core` test to
also cover `TextField`. New `engine-render` pixel test (`text_field_
paint.rs`, passed first run): the same real diff-based proof M17
Phase 2's own preedit-underline test established for a similarly
hard-to-pin-down-exact-pixel claim — two otherwise-identical fields,
default vs. a real, different `text_tint`, must paint genuinely
different pixels. Updated `examples/theme.py`: a `TextField` created
*after* `set_theme`, alongside the existing checkbox-before/
slider-after cases from Phase 1.

Full `cargo test --workspace --release` (`engine-render` 8 in
`text_field_paint.rs`, up from 7)/`cargo clippy --workspace
--all-targets -- -D warnings`/`cargo fmt --check` all clean — every
prior test passed unmodified. `maturin develop --release` + full
`pytest tests/` (168 passed, unchanged, 1 skipped — confirming no new
Python-facing FFI surface was needed) and all twenty-seven examples
confirmed clean.

M20 — MD3 Component Color Theming is now fully complete.
