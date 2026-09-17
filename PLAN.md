# Plan: M18 Phase 1 — Real Click-to-Position (§8, §10, §11.9, §11.10)

Corresponds to `BUILD_TRACKER.md` M18 Phase 1's own scoping: a real
pointer click landing inside a `TextField` computes the nearest real
character boundary at that point and moves the field's own cursor
there, clearing any active selection.

## Investigation before writing code

- **Real, bigger-than-scoped finding:** `Tree::dispatch`'s own
  `PointerPressed` arm never sets `self.focused` at all today —
  confirmed via direct read. Focus movement is Tab-driven only
  (`move_focus`) or explicit (`set_focus_to`, used by AT-SPI action
  dispatch and tests) — there is no click-to-focus mechanism for *any*
  node kind yet. A real click-to-position feature that only works on
  an *already*-focused field would be wrong UX (every real text field
  focuses itself on click) — so this phase adds click-to-focus
  specifically for `NodeKind::TextField` inside `Tree::dispatch`'s
  existing `PointerPressed` arm, reusing `Tree::set_focus_to` (already
  `pub fn`, already handles the real focus-ring transition) verbatim.
  Not a generic click-to-focus for every node kind — that's real,
  separate scope creep beyond what this phase needs.
- `parley::editing::Cursor::from_point<B: Brush>(layout: &Layout<B>,
  x: f32, y: f32) -> Self` is real (confirmed via direct source read
  of the vendored crate), and `Cursor::index(&self) -> usize` is a
  public accessor for the resulting byte offset — exactly the
  primitive M15's own scoping already named as available.
- `TextRenderer::draw_field` already builds the identical real
  `Layout` this needs, from `state.content` at `state.font_family`/
  `font_weight`/`font_size`, broken to `at.max_width` — reused, not
  duplicated, by extracting the shared layout-building logic into a
  small private helper both `draw_field` and a new hit-test method
  call.
- **Real, deliberate scope simplification:** hit-testing operates on
  `state.content` alone, ignoring an active `preedit` (M17 Phase 2).
  A real click landing inside an in-progress IME composition is a
  genuine corner case — `winit`'s own real behavior already keeps
  composing and plain input mutually exclusive, and mixing click-
  positioning with a live composition is out of this phase's stated
  scope, not silently mishandled.
- **Real, confirmed architectural finding:** `engine-py::app.rs`'s
  `on_input` closure's own `runtimes_for_input.borrow()` is an
  immutable `RefCell::borrow()`, but a real hit-test call into
  `TextRenderer` needs `&mut self.font_cx`/`&mut self.layout_cx`, the
  same as `draw_field` already requires (`GpuState.text_renderer` is a
  plain field, not independently wrapped in its own `RefCell`) — this
  phase changes that one borrow to `borrow_mut()`/`get_mut`, confirmed
  safe: nothing else inside the closure body re-borrows the same
  `RefCell<HashMap<WindowId, WindowRuntime>>` re-entrantly.
- **Real, additional finding:** `Tree::hit_test_at`'s own real
  `local_point` (the click position already transformed into the hit
  node's own local space, composing `parent_transform * translate(
  layout.location) * node.paint.transform.current` the identical way
  `paint_node` does, M6 Phase 4's own transform-awareness fix) is
  computed internally but never exposed — needed here since `draw_
  field` always paints at local `(0.0, 0.0)` (`paint_node`'s own call
  site), so the hit node's own local point *is* exactly the coordinate
  `Cursor::from_point` needs. A new `Tree::hit_test_local` exposes
  both the hit `NodeId` and that local `Point`, reusing `hit_test_at`'s
  own composition math rather than recomputing it a second way.
- `dock::start_drag` (`engine-py::dock.rs`) is already a confirmed,
  real, safe no-op for any node that isn't a registered dock handle —
  the existing `PointerPressed` raw-match arm's own `dock::start_drag`
  call can coexist with new `TextField`-click handling in the same
  arm without interference.

## Design

- `Tree::dispatch`'s `PointerPressed` arm: when the hit node is a
  `NodeKind::TextField`, call `self.set_focus_to(node, config.
  focus_ring_opacity, config.focus_ring_duration, now)` — real click-
  to-focus, reusing entirely existing infrastructure.
- New `Tree::hit_test_local(&self, root, point) -> Option<(NodeId,
  Point)>` — `hit_test`'s own sibling, additionally returning the
  local-space point.
- New `Tree::set_text_field_cursor(&mut self, field, offset) -> bool`
  — pure `engine-core` mutation: confirms `field` is a real
  `NodeKind::TextField`, clamps `offset` to a real UTF-8 char boundary
  within `0..=content.len()` (defensive; `Cursor::from_point`'s own
  result should already be a valid boundary, but this method must be
  correct even if called from anywhere else later), sets `cursor =
  offset`, clears `selection_anchor` (a plain click always collapses
  any existing selection, matching `dispatch_text_field_key`'s own
  established non-shift-movement behavior).
- New `TextRenderer::hit_test_position(&mut self, state, at, point) ->
  usize` (`engine-render`) — builds the real `Layout` (shared helper
  with `draw_field`), calls `Cursor::from_point(&layout, (point.x -
  at.x) as f32, (point.y - at.y) as f32).index()`.
- `engine-py::app.rs`'s `on_input` closure: `runtimes_for_input.
  borrow()` → `borrow_mut()`/`get_mut`. The existing `PointerPressed`
  raw-match arm gains real `TextField` handling alongside its existing
  `dock::start_drag` call: hit-test via `Tree::hit_test_local`, and
  when the hit node is a `TextField`, compute the byte offset via
  `TextRenderer::hit_test_position` and apply it via `Tree::
  set_text_field_cursor`.

## Verification plan

`cargo test --workspace --release`/`clippy -D warnings`/`fmt --check`;
new `engine-core` tests for `set_focus_to`-on-click, `hit_test_local`'s
own local-point correctness under an ancestor transform, and `set_
text_field_cursor`'s clamping/selection-clearing; new `engine-render`
test proving `hit_test_position` returns the expected byte offset for
a real click point; `maturin develop --release`; full `pytest tests/`;
new hermetic FFI-level test coverage (a real `Window.click_at(x, y)`-
style synthetic entry point, mirroring `press_key`/`type_text`'s own
"dispatch exactly like a real winit event would" precedent — the real
window-driven path itself needs a live window, same category of gap
M17 Phase 1 already established for Ctrl+C/X/V, so this needs its own
hermetic test surface); every example re-run; `LOG.md`/`BUILD_
TRACKER.md`/tracker artifact/commit/memory.
