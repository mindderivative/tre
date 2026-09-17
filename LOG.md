# Log: M18 Phase 2 — Real Drag-to-Select (§8, §10), closing M18

Corresponds to `BUILD_TRACKER.md` M18 Phase 2, closing M18 entirely (both
phases): a real press-and-drag starting inside a focused `TextField`
extends a real selection as the pointer moves.

## Investigation before writing code

**The genuine open design question Phase 1 left unresolved, now
answered:** `Tree`'s existing `self.dragging`/`update_drag` mechanism
(Splitter/Slider) lives entirely inside `engine-core`, reached
automatically via `Tree::dispatch`'s own `PointerMoved` handling — but
`update_splitter_drag`/`update_slider_drag` are pure geometry
(`absolute_position`/`layout` only). Text drag-selection needs the
same real per-glyph hit-test Phase 1 established lives in
`engine-render` — `engine-core`'s existing drag mechanism structurally
can't extend to it without violating §4. The drag-tracking state
machine had to live in `engine-py` instead.

**Real, confirmed finding:** `InputEvent::PointerMoved` wasn't matched
anywhere in `app.rs`'s own raw-event match before this phase — falling
to the trailing `_ => {}`. A related, pre-existing, *separate* gap
noticed but explicitly not touched: `dock::drag_over` (M10 Phase 3's
own drop-zone highlight) is likewise never called from the real
winit-driven path, only from `Window.drag_panel_over`'s own synthetic
test entry point — real, but out of this phase's own scope.

**Real, deliberate, stated scope boundary:** a real drag-select in
most desktop text editors keeps extending even once the pointer
leaves the field's own bounds (clamped to the nearest edge). This
phase re-hit-tests on every `PointerMoved` and only extends the
selection while the pointer is still genuinely inside the *same*
dragged field's own bounds — leaving mid-drag simply pauses the update
(not silently wrong) until the pointer re-enters. A further, real,
un-scoped refinement beyond this phase.

## What happened

New private `Tree::char_boundary` factors out the clamping logic
`set_text_field_cursor` already had. New `Tree::extend_text_field_
selection(field, offset) -> bool` — a genuinely different operation
from `set_text_field_cursor`, not the same method with a flag: seeds
`selection_anchor` at the *current* `cursor` only if `None` (the same
`get_or_insert`-at-first-move pattern shift-arrow selection already
uses, M15 Phase 3), then moves `cursor`, growing the selection instead
of collapsing it.

`WindowRuntime` (`engine-py::app.rs`) gained `text_drag: Option<
NodeId>` (plain field, mutated only within `on_input`). A new shared
`text_field_hit_offset` helper factors out the "is `hit` a `TextField`,
what byte offset does `local_point` land on" logic Phase 1's own
`PointerPressed` arm had inline — now reused by both `PointerPressed`
(arms `text_drag` on a successful click-to-position) and the new
`PointerMoved` arm (extends the selection only while the pointer is
still on the *same* dragged field). `PointerReleased` clears `text_
drag` unconditionally, mirroring `self.dragging = None`'s own existing
on-release precedent.

**Confirmed, not assumed:** `Window` (the no-live-window-needed
synthetic test surface) owns no `TextRenderer` at all — only a live
`App`/window does — so this phase genuinely has no hermetic
Python-facing test surface, the same real scope boundary click-to-
position (Phase 1), clipboard (M17 Phase 1), and IME (M17 Phase 2) all
independently established. `pytest` count stayed exactly 163, unchanged
from Phase 1, confirming this.

New `engine-core` tests (4, all passed first run): the anchor seeds at
the current cursor on the first move; repeated extend calls keep
growing without moving the anchor; clamps beyond content length; a
true no-op on a non-`TextField`.

Full `cargo test --workspace --release` (`engine-core` 137, up from
133)/`cargo clippy --workspace --all-targets -- -D warnings`/`cargo
fmt --check` all clean — every prior test passed unmodified. `maturin
develop --release` + full `pytest tests/` (163 passed, unchanged, 1
skipped) and all twenty-four examples confirmed clean.

M18 — TextField Mouse Interaction is now fully complete.
