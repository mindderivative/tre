# Plan: M8 Phase 3 — Real Scroll Input Wired to `VirtualList` (§11.7, closing M4 Phase 8's own gap and M8 entirely)

Corresponds to `BUILD_TRACKER.md` M8 Phase 3's own scoping: `Tree::
dispatch`'s `InputEvent::Scroll` arm, when it hits a real `NodeKind::
VirtualList`, updates that list's own real scroll offset instead of
remaining a no-op.

## Investigation before writing code

- `Tree::dispatch`'s `InputEvent::Scroll { .. } => DispatchOutcome::
  None` (confirmed via direct read, `tree.rs`) is the exact, real,
  deliberate no-op M4 Phase 8's own gap note has stated since it
  landed — this phase closes it for real.
- `Tree::hit_test(&self, root: NodeId, point: Point) -> Option<NodeId>`
  (M5 Phase 2, transform-aware) is the same real mechanism `Pointer
  Pressed`/`PointerReleased`'s own dispatch arms already use — scroll
  reuses it identically, hit-testing at the event's own `position`.
- A scroll gesture can land on any descendant of a `VirtualList` (its
  own materialized children, or a child's own child), not just the
  list's own root pixel — matching real browser/OS scroll-bubbling
  behavior, `dispatch` walks up the hit node's own `parent` chain
  (already a plain field on `Node`) until it finds the nearest
  `NodeKind::VirtualList` ancestor (inclusive of the hit node itself),
  or reaches the root with none found (a true no-op, matching every
  other dispatch arm's own "no real target, do nothing" shape).
- `ScrollDelta` (`engine-core/src/input.rs`) has two real variants,
  `Lines`/`Pixels` — confirmed via direct read, `engine-platform`'s own
  `translate_scroll_delta` is a straight passthrough from `winit::
  MouseScrollDelta`, no sign flip. **Checked `winit`'s own doc comment
  for `MouseScrollDelta` directly and found no explicit statement of
  which sign means "scroll down"** — rather than guess and present it
  as verified, this phase states its own chosen, real, tested
  convention plainly: a positive `y` component increases the list's
  own `scroll_offset` (content moves up, later items come into view).
  `Lines` needs a real pixels-per-line conversion — no existing
  constant for this anywhere in the codebase; a plain, stated `20.0px`
  per line (a common, ordinary default many toolkits use) is this
  phase's own real v1 choice, not pulled from any spec this project
  names.
- Clamping needs the list's own real content extent (`item_extent *
  item_count`) and its own real, computed viewport height (`Tree::
  layout(id).size.height`, already real since before this milestone)
  — `max(0, content_extent - viewport_height)` is the real upper bound;
  `0.0` the real lower bound. A list whose content is shorter than its
  own viewport clamps to `max_offset = 0.0`, meaning it can't scroll at
  all — correct, not a bug (there's nothing to reveal).
- A public `Tree::scroll_virtual_list_by(id, delta_y)` (clamped,
  directly settable) is the natural, testable, dispatch-reusable shape
  — the same "expose a direct method, `dispatch` reuses it internally"
  precedent `set_splitter_position`/`spawn_ripple` already establish,
  not a private-only helper `dispatch`'s own arm would otherwise
  duplicate if a future non-dispatch caller (a Python-facing "scroll by
  N" test entry point, matching `Window.click`/`.hover`'s own no-live-
  window-needed pattern) ever needs it.

## Design

- New `Tree::scroll_virtual_list_by(&mut self, id: NodeId, delta_y:
  f64)`: reads the list's own real `item_extent`/`item_count`/
  `layout(id).size.height`, computes `max_offset`, and clamps `state.
  scroll_offset.current + delta_y` into `[0.0, max_offset]`. Panics if
  `id` isn't a real `NodeKind::VirtualList` in this `Tree` — the same
  "internal bug, not a runtime condition" contract every other kind-
  specific `Tree` method (`set_splitter_position`) already uses.
- `Tree::dispatch`'s `InputEvent::Scroll { delta, position }` arm:
  hit-tests at `position`, walks up the hit node's own parent chain for
  the nearest `NodeKind::VirtualList`, and — if found — converts
  `delta` to a real pixel `delta_y` (`Lines(_, y) => y * 20.0`,
  `Pixels(_, y) => y`) and calls `scroll_virtual_list_by`. Still
  returns `DispatchOutcome::None` — this is a mechanical consequence
  handled entirely inside `dispatch` itself, the same shape ripple-
  spawn-on-press and hover-update already use, not something the app
  layer needs to be told happened.

## Verification plan

- `cargo test --workspace --release`/clippy/fmt. New tests: a real
  dispatched `Scroll` event over a `VirtualList`'s own materialized
  child updates that list's own `scroll_offset`, clamped correctly at
  both ends (scrolling past the real content extent doesn't overshoot;
  scrolling before `0.0` doesn't go negative); a `Scroll` event that
  hits nothing (or hits content with no `VirtualList` ancestor at all)
  is a true no-op, touching no node's state.
- `maturin develop --release` + `pytest tests/` + all examples. New
  `examples/scrollable_list.py`: a real `VirtualList` plus a real
  dispatched scroll (via a new, no-live-window-needed `Window.scroll`
  test entry point, mirroring `Window.click`/`.hover`'s own precedent
  exactly) proving the whole call chain compiles and runs end to end.
