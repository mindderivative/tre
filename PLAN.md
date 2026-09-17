# Plan: M15 Phase 1 — Real `TextField` State & Paint (§5, §16.7)

Corresponds to `BUILD_TRACKER.md` M15 Phase 1's own scoping:
`NodeKind::TextField(TextFieldState)` with real, plain byte-offset
state and real paint (text, plus a real caret and selection-highlight
computed via `parley`'s own editing module purely for painting), and
real keyboard-focus integration making a `TextField` focusable. No
editing yet — that's Phase 2.

## Investigation before writing code

- `NodeKind` (`engine-core/src/node.rs`) has `Rect`/`Container`/
  `Text(TextState)`/`Splitter`/`VirtualList`/`Canvas`/`Checkbox`/
  `Slider` — no `TextField`, confirmed by direct read. `TextState`'s
  own shape (`content`/`font_family`/`font_weight`/`font_size`) is the
  real precedent for `TextFieldState`'s own font fields.
- **Real finding: `NodeKind::Text` has zero imperative Python
  constructor anywhere** — confirmed via grep, no `add_text`/`Text`
  reference exists in `engine-py::window.rs` at all; `Text` is
  YAML-only (`engine-spec`) today. `Checkbox`/`Slider` both *do* have
  real `Window.add_checkbox`/`add_slider` constructors (M14 Phases
  1/2), matching every other *interactive* component. Since M15's own
  point is a real, live, runnable example proving keyboard text entry
  (the same "example script proves it" discipline this whole project
  uses), `TextField` follows the `Checkbox`/`Slider` precedent, not
  `Text`'s — a real `Window.add_text_field` belongs in this phase, not
  deferred to Phase 2 the way the original scoping paragraph's own
  wording suggested (a minor, honest scope-boundary refinement made on
  contact with implementation, not a change to the milestone's overall
  shape).
- **Real finding: `Tree::set_access` has zero real callers from
  `engine-py` today** — confirmed via grep across `window.rs`/
  `node.rs`, every `Window.add_*` method leaves every node at the
  default `Role::Unknown`, no label, no actions. `AccessNodeData`/
  `Action`/`Role` are exercised only by direct `engine-core`-level
  tests. `accesskit::Role::TextInput` is real (confirmed via direct
  source read of the pinned `accesskit = "0.25.0"`), as is a real
  `Value` property (`Node::set_value`/`.value()`/`.clear_value()`, the
  macro-generated accessor at `accesskit/src/lib.rs:2118`). For
  `TextField` to be genuinely Tab-reachable (`Tree::collect_
  interactive`'s own real rule: `!access.actions.is_empty()`),
  `Window.add_text_field` needs to be the *first* real `engine-py`
  caller of `Tree::set_access`, setting `Role::TextInput` +
  `Action::Focus` — small, real, additive, and honestly the first of
  its kind, not silently glossed over.
- **Real finding, changing the caret-visibility design:** `Interaction
  State.focus_ring: Animated<f64>` is ticked but genuinely never
  painted anywhere in `engine-render` — confirmed via grep, no
  `focus_ring` reference exists in `lib.rs` at all. Rather than invent
  a second, TextField-specific "is this visually focused" concept,
  `paint_node` already receives `tree: &Tree` directly (confirmed by
  direct read of its own signature) — so the caret is gated on the
  real, live `tree.focused() == Some(id)`, genuinely reusing §10's
  already-real focus state, not a new field.
- **Real, scope-simplifying finding (already recorded in the
  milestone's own scoping):** `parley::{Cursor, Selection}` (the crate
  root re-exports `editing::*`, confirmed via direct read of
  `parley/src/lib.rs`) already does the hard cursor-geometry math:
  `Cursor::from_byte_index(&layout, index, affinity) -> Cursor`,
  `Cursor::geometry(&layout, width) -> BoundingBox`, `Selection::new
  (anchor, focus)`, `Selection::geometry(&layout) -> Vec<(BoundingBox,
  usize)>`. `BoundingBox { x0, y0, x1, y1 }` (`f64`) is the real shape
  (`parley/src/util.rs`). These need the *same* shaped `parley::Layout`
  the glyphs themselves were painted against, or the caret/highlight
  geometry won't line up — so both must be built from one shared
  layout inside one function, not reconstructed twice.
- `TextRenderer::draw` (`engine-render/src/text.rs`) is the real,
  existing precedent for building that `Layout` from a `TextState`-
  shaped struct and painting its glyphs — `TextFieldState` mirrors
  `TextState`'s own four font/content fields exactly, so a sibling
  `draw_field` method can reuse the identical layout-building code,
  adding only the caret/selection painting on top.
- `crates/engine-render/tests/checkbox_paint.rs` is the real, established
  headless render-to-texture-then-readback pattern for a genuine pixel-
  level proof — reused verbatim for `text_field_paint.rs`.

## Design

- `engine-core::node::TextFieldState { content: String, font_family:
  String, font_weight: f32, font_size: f32, cursor: usize,
  selection_anchor: Option<usize> }`, `new(content, font_family,
  font_weight, font_size)` seeding `cursor` at `content.len()` (a real
  text field's own real initial-cursor-at-end convention).
  `NodeKind::TextField(TextFieldState)`, exported from `lib.rs`.
- `Tree::collect_access_nodes` gains a `NodeKind::TextField` arm:
  `access_node.set_value(state.content.clone())` — the same real,
  automatic "one property, no duplicated copy" derivation `Checkbox`'s
  own `set_toggled` already established.
- `TextRenderer::draw_field(scene, resources, state: &TextFieldState,
  at: TextPlacement, show_caret: bool)`: builds the same `Layout` as
  `draw`, paints a real selection-highlight rect first (only when
  `selection_anchor` is `Some` and differs from `cursor`, via
  `Selection::geometry`), then glyphs, then a real caret line last
  (only when `show_caret`, via `Cursor::geometry`) — highlight behind
  text, caret on top, real MD3/every-text-editor stacking order.
- `paint_node` gains a `NodeKind::TextField(state)` arm calling
  `draw_field` with `show_caret: tree.focused() == Some(id)`.
- `Window.add_text_field(background, width, height, content="",
  font_family="Roboto", font_weight=400.0, font_size=16.0, x=None,
  y=None) -> Node`, mirroring `add_checkbox`'s own real shape, plus the
  new `tree.set_access(id, AccessNodeData::new(Role::TextInput)
  .with_action(Action::Focus))` call.
- `Node.get_text() -> PyResult<String>`, mirroring `get_checked`'s own
  shape exactly (rejects a non-`TextField` node the same way).

## Verification plan

`cargo test --workspace --release`/`clippy -D warnings`/`fmt --check`;
new `engine-core` tests (`TextFieldState::new` seeds `cursor` at
`content.len()`; `build_access_update` reports the real `Role::
TextInput`/`value`/`Action::Focus` for a `TextField`); new
`engine-render/tests/text_field_paint.rs` (glyphs paint; a caret
paints only when the node is the tree's own focused node, proven both
ways); `maturin develop --release`; new `tests/test_text_field.py`
(`add_text_field` returns a usable `Node`; `get_text` reads back
content; `get_text` rejects a non-`TextField` node; a real `Window.
press_key("tab")` reaches it, proven via `Node.get("opacity")`-style
focus-adjacent check or a direct `Window`-level focus query if one
exists — investigate at implementation time); new `examples/
text_field.py`; every existing example re-run for regressions; `LOG.md`
/`BUILD_TRACKER.md`/tracker artifact/commit/memory.
