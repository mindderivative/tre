# Log: M15 Phase 1 — Real `TextField` State & Paint (§5, §16.7)

Corresponds to `BUILD_TRACKER.md` M15 Phase 1. `NodeKind::TextField
(TextFieldState)` with real, plain byte-offset state and real paint
(text, plus a real caret and selection-highlight computed via
`parley`'s own editing module purely for painting), and real
keyboard-focus integration making a `TextField` focusable. No editing
yet — that's Phase 2.

## Investigation before writing code

`NodeKind` had no `TextField` variant, confirmed by direct read.
**Real finding:** `NodeKind::Text` has zero imperative Python
constructor anywhere (confirmed via grep) — YAML-only today; unlike
`Checkbox`/`Slider`, both of which *do* have real `Window.add_*`
constructors. Since this milestone's own point is a real, live example
proving keyboard text entry, `TextField` follows the `Checkbox`/
`Slider` precedent — `Window.add_text_field` moved into this phase
rather than Phase 2, a small, honest refinement of the original
scoping paragraph's own wording. **Real finding:** `Tree::set_access`
has zero real callers from `engine-py` (confirmed via grep) — every
`add_*` method leaves a node at `Role::Unknown`, no actions.
`accesskit::Role::TextInput` and a real `Value` property (`set_value`/
`.value()`) are both real, confirmed via direct source read of the
pinned `accesskit = "0.25.0"`. `add_text_field` is the first real
`engine-py` caller of `set_access`, setting `Role::TextInput` +
`Action::Focus` at construction (a `TextField` is inherently
interactive, not opt-in the way `Rect`'s Tab-reachability is a side
effect of `set_on_click`). **Real finding:** `InteractionState.
focus_ring` is ticked but never painted anywhere in `engine-render`
(confirmed via grep) — rather than invent a second focus concept, the
caret is gated on the real, live `tree.focused() == Some(id)`,
reusing §10's already-real state; `paint_node` already receives
`tree: &Tree` directly, confirmed by direct read of its own signature.
**Real, scope-simplifying finding (recorded at scoping time, confirmed
again here):** `parley::{Cursor, Selection}` (crate-root re-exported,
confirmed via direct source read) already does the hard cursor-
geometry math — `Cursor::from_byte_index`/`.geometry(width)`,
`Selection::new`/`.geometry()` — both need the *same* `Layout` the
glyphs were shaped against, so `draw_field` builds one shared layout
and derives all three (highlight, glyphs, caret) from it, not
reconstructed separately.

## What happened

`TextFieldState { content, font_family, font_weight, font_size,
cursor, selection_anchor }` mirrors `TextState`'s own four font fields
exactly (so `engine-render` can share layout-building code) plus real
`cursor: usize`/`selection_anchor: Option<usize>` byte offsets.
`TextFieldState::new` seeds `cursor` at `content.len()` — a real text
field's own expected initial-cursor-at-end convention. `NodeKind::
TextField(TextFieldState)` added, exported from `lib.rs`.

`Tree::collect_access_nodes` gains a `TextField` arm: `access_node.
set_value(state.content.clone())` — the same real, automatic,
never-duplicated derivation `Checkbox`'s own `set_toggled` already
established.

New `TextRenderer::draw_field` (`engine-render/src/text.rs`): builds
the identical kind of `Layout` `draw` does, then paints in real MD3
stacking order — a real selection-highlight rect first (only when
`selection_anchor` is `Some` and differs from `cursor`, via `Selection
::geometry`), glyphs next (identical to `draw`), a real caret last
(only when `show_caret`, via `Cursor::geometry`). Neither color is yet
theme-resolved (`engine-render` has no `engine-md3` dependency, §4) —
both derive from `at.color`, the same "real but not yet theme-aware"
scope `Checkbox`'s own hardcoded white checkmark already established.
`paint_node` gains a `NodeKind::TextField` arm: paints the node's own
real `RoundedRect` fill first (unlike `NodeKind::Text`, which
repurposes `background` as the glyph color with no separate box —
`TextField` is a genuinely boxed input), then calls `draw_field` with
a fixed real text color and `show_caret: tree.focused() == Some(id)`.

New `Window.add_text_field(background, width, height, content="",
font_family="Roboto", font_weight=400.0, font_size=16.0, x=None,
y=None) -> Node` mirrors `add_checkbox`'s own real shape, plus the new
`tree.set_access` call described above. New `Node.get_text()` mirrors
`get_checked`'s own exact shape. New `Node.is_focused()` (§10) — a
second real, small, additive finding: no Python-facing way to read
`Tree::focused()` existed at all before this, needed both to prove Tab
actually reaches a `TextField` and as a genuinely useful capability on
its own for any interactive `NodeKind`, not `TextField`-specific.

New `engine-core` tests: `TextFieldState::new` seeds `cursor` at
`content.len()`; `build_access_update` reports the real `Role::
TextInput`/`value`/`Action::Focus` for a `TextField`. New `engine-
render/tests/text_field_paint.rs` (2 pixel tests, both passed on the
first run): an unfocused field paints only its own plain fill, no
caret; the identical field, once it's the `Tree`'s own real focused
node, paints a real, visible caret distinct from that fill — proving
`show_caret`'s gate is genuinely live, not hardcoded either way. New
`tests/test_text_field.py` (7 tests, all passed on the first run,
including a real `Window.press_key("tab")` reaching the field and a
second Tab genuinely moving focus away to a real `Checkbox`). New
`examples/text_field.py`: a real, live field seeded with initial
content, Tab-focused, its own `is_focused()`/`get_text()` printed
before and after, run through a real 60-frame render loop.

Full `cargo test --workspace --release` (`engine-core` 96, up from 94,
plus 2 new `engine-render` pixel tests)/`cargo clippy --workspace
--all-targets -- -D warnings`/`cargo fmt --check` all clean — every
prior test passed unmodified. `maturin develop --release` + full
`pytest tests/` (138 passed, up from 131, 1 skipped) and all
twenty-three examples (twenty-two existing + new `text_field.py`)
confirmed clean.
