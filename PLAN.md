# Plan: M20 Phase 2 — Real TextField Component Theming (§7.1, §7.3), closing M20

Corresponds to `BUILD_TRACKER.md` M20 Phase 2: `TextFieldState` gains
`text_tint: Color`, threaded through `TextRenderer::draw_field`'s own
`at.color` and pushed the same real way Phase 1's two fields already
are.

## Investigation before writing code

`paint_node`'s `NodeKind::TextField` arm (`crates/engine-render/src/
lib.rs`) computes `text_color` from a hardcoded `Color::from_rgba8
(0x1C, 0x1B, 0x1F, 0xFF)` literal, passed into `TextPlacement.color` —
confirmed via direct read. `Window.add_text_field` (`crates/engine-py/
src/window.rs`) constructs a fresh `TextFieldState` with no theme
awareness at all today.

**Real, confirmed continuation of Phase 1's own finding:** `0x1C1B1F`
(28, 27, 31) is *not* equal to `ThemeState::on_surface()`'s own
no-theme-set default (real black, `0,0,0,255`) — the identical real
gap Phase 1 found and fixed for `Checkbox`/`Slider`. The same
`is_set()`-gated construction-time read applies here too.

## Design

- `TextFieldState` gains `pub text_tint: Color`, defaulted in `::new`
  to the exact historical literal (`0x1C, 0x1B, 0x1F, 0xFF`).
- `paint_node`'s `TextField` arm reads `state.text_tint` (still passed
  through `with_opacity` exactly as before) instead of the literal.
- `Tree::set_all_component_tints` gains a `NodeKind::TextField(state)
  => state.text_tint = tint` arm, closing the milestone's own real
  mechanism.
- `Window.add_text_field` seeds `text_tint` from `self.theme.borrow()
  .on_surface()`, gated on `theme.is_set()` — the identical pattern
  `add_checkbox`/`add_slider` already established in Phase 1.

## Verification plan

`cargo test --workspace --release`/`clippy -D warnings`/`fmt --check`;
widen the existing `set_all_component_tints` engine-core test to also
cover `TextField`; new `engine-render` pixel tests in `text_field_
paint.rs` proving an un-themed field still paints the real historical
dark text/caret color and a themed field paints the real resolved
tint; `maturin develop --release`; full `pytest tests/` (no new FFI
surface expected, matching Phase 1's own finding — colors aren't
Python-observable); update `examples/theme.py` with a themed
`TextField` case; `LOG.md`/`BUILD_TRACKER.md`/tracker artifact/commit/
push/memory — closing M20 entirely (both phases).
