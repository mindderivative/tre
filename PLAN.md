# Plan: M14 Phase 2 — Real Slider (§5, §7.3)

Corresponds to `BUILD_TRACKER.md` M14 Phase 2's own scoping:
`NodeKind::Slider(SliderState)` with real paint (track + thumb) and
real drag-to-set interaction, mirroring `Tree::set_splitter_position`/
`splitter_geometry`'s own established drag math.

## Investigation before writing code

- ARCHITECTURE.md §5's own sketch: `SliderState { thumb_position:
  Animated<f64> }` -- "0.0..=1.0 along the track," the identical shape
  `SplitterState.position` already has. No separate "value" field is
  shown or needed; `thumb_position` *is* the real value, the same way
  `SplitterState.position` already is.
- **Splitter dragging is entirely internal to `Tree::dispatch`,
  confirmed by direct read** — `PointerPressed` on a `Splitter` sets
  `self.dragging = Some(node)`; every subsequent `PointerMoved` while
  `self.dragging.is_some()` calls `update_drag`, which resolves real
  geometry and calls `set_splitter_position`; `PointerReleased` clears
  `self.dragging` unconditionally on any primary release. This is a
  *better* precedent to mirror than docking's own synthetic Python-
  level drag API — a slider drag is exactly the same "press it, follow
  the pointer, release ends it" shape a splitter already has, with no
  meaning-dependent decision engine-core can't make itself (unlike
  docking's "which zone" question).
- `self.dragging: Option<NodeId>` (`tree.rs:87`) is currently
  documented as Splitter-only, but its own real *type* already fits
  either kind — broadening its documented meaning (not its type) to
  "the node currently being pointer-dragged" is the natural, minimal
  extension, not a new field.
- `update_drag` (`tree.rs:729-742`) calls `splitter_geometry` directly
  and has no kind-dispatch today, confirmed by direct read — needs a
  real match on `self.dragging`'s own node kind to branch between
  splitter geometry (two flanking siblings) and slider geometry (the
  node's own width alone, no siblings involved).
- Scoped horizontal-only, mirroring `set_virtual_list_window`'s own
  stated "vertical-list only... not built since nothing here needs it
  yet" precedent for an analogous axis restriction.

## Design

`crates/engine-core/src/node.rs`: `NodeKind` gains `Slider
(SliderState)`; `SliderState { thumb_position: Animated<f64> }` with a
`new(value: f64) -> Self` constructor (clamped to `0.0..=1.0`).

`crates/engine-core/src/tree.rs`:
- New `Tree::set_slider_position(&mut self, id: NodeId, position: f64,
  now: Instant)` — mirrors `set_splitter_position`'s own instant
  (`Duration::ZERO`) `animate_to` + immediate `tick` shape exactly, but
  with no sibling-resize step (a slider doesn't resize anything else).
- `PointerPressed`'s own real arm: the `matches!(..., Some(NodeKind::
  Splitter(_)))` check widens to also match `Some(NodeKind::Slider
  (_))`, setting `self.dragging` the same way.
- `update_drag` gains a real match on `self.nodes[dragging].kind`:
  the existing `Splitter` branch (unchanged logic, just moved under
  the match), plus a new `Slider` branch computing `fraction = ((point
  .x - absolute_x) / width).clamp(0.0, 1.0)` from the node's own real
  absolute position/width (no flanking siblings), then calling `set_
  slider_position`.
- `tick_all`/`build_access_update` do **not** need a `Slider` arm:
  `thumb_position` is driven directly during a drag (`Duration::ZERO`
  + immediate tick, the same as `SplitterState.position`), never eased
  toward a target the central tick would need to advance; `checked`-
  style automatic accessibility derivation doesn't apply to a slider's
  own continuous value the same way (no MD3-standard boolean state to
  derive) — confirmed by re-reading ARCHITECTURE.md §7.3/§10, neither
  names an automatic accessibility flag for `Slider`.

`crates/engine-render/src/lib.rs`: `paint_node` gains a `NodeKind::
Slider(state)` arm — a real track (a thin, fixed-gray horizontal bar
spanning the node's own width, vertically centered) plus a real thumb
(a filled circle at `thumb_position.current * w`, using the node's own
real `background` color — the same universal field every other
`NodeKind`'s primary fill already uses).

`crates/engine-py/src/window.rs`: new `Window.add_slider(background=
.., width=.., height=.., value=0.0, x=None, y=None) -> Node`, mirroring
`add_checkbox`'s own real shape.

`crates/engine-py/src/node.rs`: `Node.animate("thumb_position", ...)`/
`Node.get("thumb_position")` reach `SliderState.thumb_position` when
`node.kind` is `Slider` — the second real arm of the two-level
dispatch M14 Phase 1 started. No `set_value`-style plain setter is
needed the way `Checkbox.set_checked` was: `thumb_position` *is* the
real value (continuous, not boolean), already reachable via `animate`/
`get`, and the real drag path sets it directly inside `engine-core`
(`set_slider_position`), not through Python at all.

## Verification plan

- `cargo test --workspace --release`/clippy/fmt. New `engine-core`
  tests: a real dispatched drag (press inside the slider, `PointerMoved`
  to a real point, `PointerReleased`) moves `thumb_position` to the
  real, correct fraction of the slider's own width; releasing ends the
  drag (a further `PointerMoved` no longer moves it); `set_slider_
  position` clamps to `0.0..=1.0` the same way `set_splitter_position`
  already does for its own position. Existing splitter-drag tests must
  keep passing completely unmodified — the real regression check that
  broadening `self.dragging`/`update_drag` didn't change splitter
  behavior at all.
- New `engine-render` pixel test (mirroring `checkbox_paint.rs`'s own
  split): a real thumb paints at its own real, correct position for two
  different `thumb_position` values, not a fixed spot.
- `maturin develop --release` + `pytest tests/`. New `test_slider.py`:
  `add_slider` returns a real `Node`; a real dispatched drag (via
  `Window`'s own existing `click`-style synthetic dispatch primitives,
  extended if needed) moves `thumb_position` to a real, distinct value;
  `"thumb_position"` is unknown on a non-`Slider` node, matching `"check
  _progress"`'s own established error contract.
- Run all examples; a new `examples/slider.py` demonstrating a real,
  live, drag-to-set slider.
