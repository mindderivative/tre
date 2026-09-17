# Plan: M18 Phase 2 — Real Drag-to-Select (§8, §10)

Corresponds to `BUILD_TRACKER.md` M18 Phase 2, closing M18 entirely: a
real press-and-drag starting inside a focused `TextField` extends a
real selection as the pointer moves.

## Investigation before writing code

- **The genuine open design question Phase 1 left unresolved, now
  answered:** `Tree`'s existing `self.dragging: Option<NodeId>` +
  `update_drag` mechanism (Splitter/Slider drags) lives entirely
  inside `engine-core`, reached automatically via `Tree::dispatch`'s
  own `PointerMoved` handling — but `update_splitter_drag`/`update_
  slider_drag` are pure geometry (`absolute_position`/`layout` only),
  needing no rendering knowledge at all. Text drag-selection
  fundamentally needs the same real per-glyph hit-test Phase 1 already
  established lives in `engine-render` (`TextRenderer::hit_test_
  position`) — `engine-core`'s existing drag mechanism structurally
  can't extend to it without violating §4. The drag-tracking state
  machine has to live in `engine-py` instead, a new plain field on
  `WindowRuntime` (`text_drag: Option<NodeId>`), mutated only within
  `on_input`'s own closure — not `RefCell`-wrapped, since nothing else
  needs to read it.
- `InputEvent::PointerMoved` is not currently matched anywhere in
  `app.rs`'s own raw-event match at all (falls to the trailing `_ =>
  {}`) — this phase adds the first real arm for it there.
- **Real, deliberate, stated scope boundary:** a real drag-select in
  every desktop text editor keeps extending even once the pointer
  leaves the field's own bounds (clamped to the nearest edge). Building
  that clamp is real, additional geometry work beyond what proves this
  phase's own core claim. This phase re-hit-tests via `Tree::
  hit_test_local` on every `PointerMoved` and only extends the
  selection while the pointer is still genuinely inside the *same*
  dragged field's own bounds — a pointer that leaves mid-drag simply
  stops updating the selection (not silently wrong, a stated boundary)
  until it re-enters. A further, real, un-scoped candidate beyond even
  this phase.
- Phase 1's own `set_text_field_cursor` always clears `selection_
  anchor` — correct for a plain click, wrong for drag-extension. A new
  sibling method is needed rather than widening `set_text_field_
  cursor` itself (a plain click and a drag-extend are genuinely
  different operations, the same "two distinct real behaviors, two
  methods" shape `text_field_selected_text`/`cut_text_field_selection`
  already established as separate rather than one flag-taking method).
  The char-boundary-clamping logic the two now share is factored into
  a small private helper.

## Design

- New private `Tree::char_boundary(content, offset) -> usize` — the
  clamping logic `set_text_field_cursor` already has, factored out so
  `extend_text_field_selection` doesn't duplicate it.
- New `Tree::extend_text_field_selection(&mut self, field, offset) ->
  bool` — seeds `selection_anchor` at the *current* `cursor` only if
  it's `None` (the same `get_or_insert`-at-first-move pattern shift-
  arrow selection already uses, M15 Phase 3), then moves `cursor` to
  the clamped `offset`. Never clears an existing anchor — that's what
  makes a drag a real, growing selection instead of repeatedly
  collapsing.
- `WindowRuntime` (`engine-py::app.rs`) gains `text_drag: Option<
  NodeId>`, initialized `None`. The existing `PointerPressed` arm
  (already home to Phase 1's click-to-position) additionally sets it
  to `Some(hit)` whenever the hit is a `TextField`. A new `PointerMoved`
  arm: if `text_drag` names a field and a fresh `hit_test_local` still
  lands on that same field, computes the byte offset via `TextRenderer::
  hit_test_position` and applies it via `Tree::
  extend_text_field_selection` — otherwise does nothing this frame (the
  stated boundary above). The existing `PointerReleased` arm clears
  `text_drag` unconditionally, matching `self.dragging = None`'s own
  existing on-release precedent.

## Verification plan

`cargo test --workspace --release`/`clippy -D warnings`/`fmt --check`;
new `engine-core` tests for `extend_text_field_selection` (seeds the
anchor once, keeps extending on repeated calls, clamps, no-op on a
non-`TextField`); `maturin develop --release`; full `pytest tests/` —
investigate whether `Window`'s existing synthetic surface can exercise
this at all (it likely can't for the same real reason click-to-
position couldn't, §4 — `Window` owns no `TextRenderer`); every
example re-run; `LOG.md`/`BUILD_TRACKER.md`/tracker artifact/commit/
memory — closing M18 entirely (both phases).
