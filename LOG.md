# Log: M8 Phase 3 — Real Scroll Input Wired to `VirtualList` (§11.7), closing M8 entirely

Corresponds to `BUILD_TRACKER.md` M8 Phase 3, the milestone's own final
phase. `Tree::dispatch`'s `InputEvent::Scroll` arm, when it hits a real
`NodeKind::VirtualList`, updates that list's own real scroll offset
instead of remaining a no-op — closing M4 Phase 8's own long-stated gap.

## Investigation before writing code

- `Tree::dispatch`'s `InputEvent::Scroll { .. } => DispatchOutcome::
  None` (confirmed via direct read) was the exact, deliberate no-op M4
  Phase 8's own gap note has stated since it landed.
- `Tree::hit_test` (M5 Phase 2, transform-aware) is the same real
  mechanism `PointerPressed`/`PointerReleased` already use — scroll
  reuses it identically at the event's own `position`.
- A scroll gesture can land on any descendant of a `VirtualList`, not
  just its own root pixel — `dispatch` walks up the hit node's own
  `parent` chain until it finds the nearest `NodeKind::VirtualList`
  ancestor (inclusive of the hit node itself), matching real browser/OS
  scroll-bubbling behavior.
- Checked `winit`'s own doc comment for `MouseScrollDelta` directly and
  found no explicit statement of which sign means "scroll down" —
  rather than present a guess as verified, this phase states its own
  chosen, tested convention plainly: a positive `y` increases
  `scroll_offset`. `Lines` uses a plain, stated `20.0px`-per-line
  conversion (no existing constant anywhere in the codebase for this).
- **Real, in-scope finding, not silently worked around:**
  `Window.add_virtual_list`'s own doc comment said a real scrollable
  viewport "isn't built yet," so its own `Style` left `height: auto()`.
  Since every materialized item is `Position::Absolute` (resolved from
  its own `inset`, never counted toward the parent's own intrinsic
  size), `auto()` never actually gave a real viewport height at all —
  meaning `scroll_virtual_list_by`'s own real clamping would have been
  degenerate for every real Python-created list. Fixed as part of this
  phase: `add_virtual_list` gains an optional `height` parameter
  (defaulting to the `Window`'s own real height, the same fallback
  shape `width` already uses) — fully backward compatible (confirmed
  via grep: no existing caller anywhere passes `width` either).

## What happened

New `Tree::scroll_virtual_list_by(&mut self, id: NodeId, delta_y: f64)`:
reads the list's own real `item_extent`/`item_count`/`layout(id).size.
height`, computes `max_offset = max(0, content_extent - viewport_
height)`, and clamps `scroll_offset.current + delta_y` into `[0.0,
max_offset]`. `Tree::dispatch`'s `InputEvent::Scroll` arm now
hit-tests, walks up to the nearest `VirtualList`, converts the delta to
real pixels, and calls it — still returning `DispatchOutcome::None`
(the same "mechanical consequence handled entirely inside dispatch"
shape ripple-spawn/hover-update already use).

`engine-py::PyWindow` gains `scroll(node, delta_y)` — the same no-live-
window-needed proof pattern `click()`/`hover()` already established,
dispatching a real `InputEvent::Scroll` at `node`'s own real center
point. `add_virtual_list` gains the real `height` fix above.

New `engine-core` tests (in `tree.rs`): `scroll_virtual_list_by` moves
and clamps correctly at both ends; a list shorter than its own viewport
can't scroll at all; a real dispatched scroll over a `VirtualList`'s own
child (not the list's own root) updates its real scroll offset; a
scroll that hits nothing with no `VirtualList` ancestor is a true
no-op. New pytest test proves the real FFI call chain runs without
error (Python has no way to read `scroll_offset` back — no consumer
needs one yet, matching this project's own "pixel-level/value proof
stays in Rust" split throughout). New `examples/scrollable_list.py`: a
real 1,000-row list, a real viewport, a real scroll gesture, end to end.

Full `cargo test --workspace --release` clean (`engine-core` gains 4
tests: 65 → 69 — every prior test passed unmodified), `cargo clippy
--workspace --all-targets -- -D warnings`, `cargo fmt --check` all
clean. `maturin develop --release` + full `pytest tests/` (79 passed,
up from 78, 1 skipped) and all fifteen examples (fourteen existing + new
`scrollable_list.py`) confirmed clean.

M8 — Virtualization & Culling is now complete: all 3 phases done.
