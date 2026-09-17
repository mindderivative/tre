# Plan: M20 Phase 1 — Real Checkbox/Slider Component Theming (§7.1, §7.3)

Corresponds to `BUILD_TRACKER.md` M20 Phase 1: `CheckboxState` gains
`mark_tint: Color`, `SliderState` gains `track_tint: Color` — both
threaded through `paint_node`'s existing hardcoded-literal paint
sites, pushed a real resolved color by `Window.set_theme`.

## Investigation before writing code

- Confirmed via direct source read: `CheckboxState`/`SliderState`
  (`crates/engine-core/src/node.rs`) have no `#[derive(...)]` at all
  today, and `peniko::Color` is already imported in that file — adding
  a plain `Color` field to each needs no new derive/import.
- `paint_node`'s `NodeKind::Checkbox` arm (`crates/engine-render/src/
  lib.rs`) paints the checkmark with a hardcoded `Color::from_rgba8
  (0xFF, 0xFF, 0xFF, 0xFF)`; the `NodeKind::Slider` arm paints the
  track with a hardcoded `Color::from_rgba8(0x79, 0x74, 0x7A, 0xFF)`
  — both confirmed via direct read, both replaced with a read from the
  new state fields.
- `Tree::set_all_interaction_tints` (`crates/engine-core/src/tree.rs`)
  is the real, established M7 Phase 3 precedent — but it's specifically
  scoped to `InteractionState::tint` (an `Option<InteractionState>`
  field every node may or may not opt into via `enable_interaction()`).
  `mark_tint`/`track_tint` are different in kind: plain fields every
  real `CheckboxState`/`SliderState` *always* has (not an optional
  capability), living on `NodeKind`-specific state, not
  `InteractionState`. Widening `set_all_interaction_tints` itself to
  also match on `NodeKind` would conflate two distinct real mechanisms
  and change an already-tested method's own documented behavior. A
  new, separate `Tree::set_all_component_tints(tint: Color)` matches
  this session's own repeated "distinct real behaviors, distinct
  methods" precedent (`set_text_field_cursor`/`extend_text_field_
  selection`, `hit_test`/`hit_test_local`).
- `Window.set_theme` (`crates/engine-py/src/window.rs`) already
  resolves one real `tint: Color` (`state.on_surface()`) and calls
  `tree.borrow_mut().set_all_interaction_tints(tint)` — this phase
  adds one more call, `set_all_component_tints(tint)`, using the exact
  same already-resolved value (the real, deliberate M20 scope decision
  to reuse the existing "on-surface" role rather than resolve a second,
  more specific one per component).

## Design

- `CheckboxState` gains `pub mark_tint: Color`, defaulted in `::new`
  to the exact historical literal (`0xFF, 0xFF, 0xFF, 0xFF`) — zero
  visual change for a node whose app never calls `set_theme`.
- `SliderState` gains `pub track_tint: Color`, defaulted in `::new`
  to the exact historical literal (`0x79, 0x74, 0x7A, 0xFF`).
- `paint_node`'s `Checkbox`/`Slider` arms read `state.mark_tint`/
  `state.track_tint` instead of the hardcoded literals.
- New `Tree::set_all_component_tints(&mut self, tint: Color)`: walks
  every node, setting `mark_tint`/`track_tint` on a `Checkbox`/
  `Slider` respectively; any other `NodeKind` untouched. Unconditional
  per matching node (no opt-in gate — unlike `InteractionState`, every
  real `CheckboxState`/`SliderState` always has these fields).
- `Window.set_theme` calls the new method alongside the existing one.

## Verification plan

`cargo test --workspace --release`/`clippy -D warnings`/`fmt --check`;
new `engine-core` test proving `set_all_component_tints` updates
`Checkbox`/`Slider` nodes and leaves others untouched; new
`engine-render` pixel tests proving a themed checkmark/track paint the
real resolved color, not the old literal, while an un-themed one still
paints the exact historical default; `maturin develop --release`; full
`pytest tests/` (a new hermetic test via `Window.set_theme` +
`Node.get`-style readback if the color is Python-observable, otherwise
the pixel tests are the definitive proof, matching this project's own
established "pixel test for paint claims" split); every example
re-run; `LOG.md`/`BUILD_TRACKER.md`/tracker artifact/commit/push/
memory.
