# Log: M14 Phase 2 — Real Slider (§5, §7.3)

Corresponds to `BUILD_TRACKER.md` M14 Phase 2. `NodeKind::Slider
(SliderState)` with real paint (track + thumb) and real drag-to-set
interaction, mirroring `Tree::set_splitter_position`/`splitter_
geometry`'s own established drag math.

## Investigation before writing code

ARCHITECTURE.md §5's own sketch: `SliderState { thumb_position:
Animated<f64> }` -- no separate "value" field, `thumb_position` *is*
the real value, the identical shape `SplitterState.position` already
has. Confirmed by direct read: splitter dragging is entirely internal
to `Tree::dispatch` -- `PointerPressed` on a `Splitter` sets `self.
dragging`; every subsequent `PointerMoved` calls `update_drag`, which
resolves real geometry and calls `set_splitter_position`; `PointerRel
eased` clears `self.dragging` unconditionally. This is a better
precedent to mirror than docking's own synthetic Python-level drag API
-- a slider drag is exactly the same "press it, follow the pointer,
release ends it" shape, with no meaning-dependent decision engine-core
can't make itself.

## What happened

`NodeKind::Slider(SliderState)`; `SliderState::new(value)` clamps to
`0.0..=1.0`. New `Tree::set_slider_position` mirrors `set_splitter_
position`'s own instant (`Duration::ZERO`) `animate_to` + immediate
`tick` shape, with no sibling-resize step. `self.dragging: Option
<NodeId>` broadened (documentation, not type) to "the node currently
being pointer-dragged" -- `PointerPressed`'s own condition now also
matches `Slider`; `update_drag` gains a real match on the dragged
node's own kind, splitting into `update_splitter_drag` (unchanged
logic) and new `update_slider_drag` (much simpler: the node's own real
absolute position/width *is* the whole track, no flanking siblings).
Horizontal-only for now, the same "not built since nothing here needs
it yet" scope limit `set_virtual_list_window`'s own vertical-only
restriction already established.

**Real finding while designing `tick_all`'s own scope:** unlike
`SplitterState.position` (never exposed to `Node.animate()` at all),
`thumb_position` *is* exposed (`"thumb_position"`, this phase's own
second real kind-payload `animate()` arm) for a real, app-triggered
eased move distinct from a drag. That path needs `Tree::tick_all` to
actually tick it centrally, or a nonzero-duration `animate()` call
would set an active animation that never progresses -- caught while
writing the design, not after. `tick_all` gains a `Slider` arm ticking
`thumb_position`, alongside the existing `Checkbox` one; the drag path
itself already ticks manually, so this is a true no-op for that path.

`paint_node` gains a `Slider` arm: a real track (thin, fixed-gray bar,
vertically centered, spanning the node's own width) plus a real thumb
(filled circle at `thumb_position * w`, the node's own real
`background` color). `Node.animate`/`.get` gain `"thumb_position"`,
the second real kind-payload dispatch arm. New `Window.add_slider
(value=0.0, ...)` mirrors `add_checkbox`'s own shape. No `set_value`-
style plain setter was added -- `thumb_position` *is* the value,
already reachable via `animate`/`get`, and the real drag path sets it
directly inside `engine-core`, never through Python.

New `engine-core` tests (mirroring the existing splitter-drag tests
exactly, via a new `slider_scene()` helper): a real dispatched drag
moves `thumb_position` live as the pointer moves, in two steps, not a
one-shot snap; clamps to `0.0..=1.0` past either edge; release ends
the drag so further moves don't affect it; `tick_all` genuinely
animates `thumb_position` toward a real target. Existing splitter-drag
tests kept passing completely unmodified -- the real regression check
that broadening `self.dragging`/`update_drag` didn't change splitter
behavior at all. New `engine-render/tests/slider_paint.rs`: a thumb at
`0.0` paints at the real left edge (and nowhere near the right, off
the track's own vertical band too); a thumb at `1.0` paints at the
real right edge, the mirror case.

New `tests/test_slider.py` (6 tests) + new `examples/slider.py`: a
real, live slider seeded at a non-zero value, moved via a real
programmatic `animate("thumb_position", ...)` call.

Full `cargo test --workspace --release` (`engine-core` 92, up from 88,
plus 2 new `engine-render` pixel tests)/`cargo clippy --workspace
--all-targets -- -D warnings`/`cargo fmt --check` all clean -- every
prior splitter test passed unmodified. `maturin develop --release` +
full `pytest tests/` (119 passed, up from 113, 1 skipped) and all
twenty-one examples (twenty existing + new `slider.py`) confirmed
clean.
