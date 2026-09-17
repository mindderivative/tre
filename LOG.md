# Log: M18 Phase 1 — Real Click-to-Position (§8, §10, §11.9, §11.10)

Corresponds to `BUILD_TRACKER.md` M18 Phase 1. A real pointer click
landing inside a `TextField` computes the nearest real character
boundary at that point and moves the field's own cursor there,
clearing any active selection.

## Investigation before writing code

**Real, bigger-than-scoped finding:** `Tree::dispatch`'s own
`PointerPressed` arm never touched `self.focused` at all before this
phase — focus was Tab-driven (`move_focus`) or explicit (`set_focus_
to`) only. A real click-to-position that only worked on an already-
focused field would be wrong UX, so this phase adds real click-to-
focus for `NodeKind::TextField` specifically (not a generic mechanism
for every node kind), reusing `set_focus_to` verbatim.

`parley::editing::Cursor::from_point`/`Cursor::index()` are real,
confirmed via direct source read, exactly the primitive M15's own
scoping named as available. `engine-core` has zero `engine-render`/
`parley` visibility by design (§4), so the real per-glyph hit-test has
to happen in `engine-render`, with `engine-core` only ever applying
the resulting byte offset.

**Real, confirmed architectural finding:** `engine-py::app.rs`'s
`on_input` closure's own `runtimes_for_input.borrow()` was an
immutable `RefCell::borrow()`, but a real hit-test call into
`TextRenderer` needs `&mut self.font_cx`/`&mut self.layout_cx` — fixed
by widening to `borrow_mut()`/`get_mut`, confirmed safe (nothing else
in the closure re-borrows the same `RefCell` re-entrantly).

**Real, additional finding:** `Tree::hit_test_at`'s own real
`local_point` (the click already transformed into the hit node's own
local space) was computed internally but never exposed — needed since
`draw_field` always paints at local `(0.0, 0.0)`, so the hit node's
own local point *is* exactly the coordinate `Cursor::from_point`
needs. New `Tree::hit_test_local` exposes it, reusing `hit_test_at`'s
own composition math.

## What happened

`Tree::dispatch`'s `PointerPressed` arm: a primary-button press on a
`NodeKind::TextField` now calls `self.set_focus_to(...)`, reusing the
existing focus-ring transition mechanism. `hit_test_at`'s return type
widened from `Option<NodeId>` to `Option<(NodeId, Point)>`; `hit_test`
keeps its old signature (discards the point), new `hit_test_local`
exposes both. New `Tree::set_text_field_cursor(field, offset) -> bool`
— clamps to a real UTF-8 char boundary, sets `cursor`, clears
`selection_anchor` (a plain click always collapses any selection).

`engine-render::TextRenderer` gained a private `build_field_layout`
helper (extracted from `draw_field`'s own layout-building, so a
hit-test can never silently use different content/font/width than
what was actually painted) and a new public `hit_test_position(state,
at, point) -> usize`, deliberately hit-testing against `state.content`
alone (ignoring an active IME `preedit` — a stated, deliberate scope
simplification, a real corner case `winit`'s own mutual-exclusivity
behavior already keeps rare).

`engine-py::app.rs`'s `on_input` closure: the existing `PointerPressed`
raw-match arm (already home to `dock::start_drag`) widened to use
`hit_test_local`, and when the hit node is a `TextField`, computes the
byte offset via `TextRenderer::hit_test_position` (mirroring
`paint_node`'s own real `TextPlacement`/`max_width` derivation
exactly) and applies it via `Tree::set_text_field_cursor`.

**Real, pleasant scope-simplifying finding, deviating from `PLAN.md`'s
own speculative plan:** `PLAN.md` proposed a new `Window.click_at(x,
y)`-style hermetic FFI entry point for testing. Investigation found
this unnecessary for the focus half: `Window.click(node)` (M4 Phase 1
step 3) already dispatches a real `PointerPressed`/`PointerReleased`
pair through `Tree::dispatch` at the node's own real center point —
it already reaches the new click-to-focus code path for free, no new
`engine-py` API needed. The byte-offset-positioning half genuinely has
no hermetic Python-facing test surface (`Window` owns no
`TextRenderer` at all — only a live `App`/window does), matching the
same real scope boundary M17 Phase 1/2 already established for
Ctrl+C/X/V and real IME composition.

New `engine-core` tests (7, all passed first run): click-to-focus on a
`TextField`; click on a non-`TextField` doesn't move focus;
`hit_test_local`'s own local-point correctness under a real ancestor
transform; `set_text_field_cursor` moves the cursor and clears a
selection, clamps beyond content length, snaps a mid-character offset
to a real char boundary, is a true no-op on a non-`TextField`. New
`engine-render` tests (3, pure unit tests — no GPU/pixel-readback
needed at all, since `hit_test_position` returns a plain `usize`):
byte offset 0 at the field's own left edge; the full content length
far past every real glyph; `at.x`/`at.y` genuinely subtracted before
reaching `Cursor::from_point`. New `tests/test_text_field.py` coverage
(2 tests): a real `Window.click(field)` moves focus there; clicking a
Checkbox does not (click-to-focus stays scoped to `TextField`).
Updated `examples/text_field.py`: a real `Window.click(field)`
demonstration, using a second `TextField` as the Tab-away target (a
`Checkbox` doesn't work for this — it never opts into `Tree::
set_access`'s own focus order, confirmed while writing this).

Full `cargo test --workspace --release` (`engine-core` 133, up from
126; `engine-render` 7 in `text_field_paint.rs`, up from 4)/`cargo
clippy --workspace --all-targets -- -D warnings`/`cargo fmt --check`
all clean — every prior test passed unmodified. `maturin develop
--release` + full `pytest tests/` (163 passed, up from 161, 1
skipped) and all twenty-four examples confirmed clean.
