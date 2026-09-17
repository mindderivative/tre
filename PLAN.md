# Plan: M8 Phase 2 — `VirtualList` Real Scroll Offset & Clipping (§11.7)

Corresponds to `BUILD_TRACKER.md` M8 Phase 2's own scoping: a new
`Animated<f64>` scroll offset on `VirtualListState`, composed into
materialized children's own effective position; real clipping via
`Scene::push_layer`'s own `clip_path` support.

## Investigation before writing code

- `VirtualListState` (`engine-core/src/node.rs`) currently has
  `item_count`, `item_extent`, `materialized: BTreeMap<usize, NodeId>`
  — no scroll concept at all. `Tree::set_virtual_list_window` requires
  the *caller* to supply an explicit `visible: Range<usize>`; this
  phase doesn't change that contract (a real, stated scope boundary,
  not silently expanded) — it only makes the *paint-time* position/
  clip of whatever's already materialized real.
- **Real finding: kind-specific `Animated<T>` fields are never ticked
  by `Tree::tick_all`.** Confirmed by reading `tick_all` directly — it
  only ever ticks `node.paint`/`node.interaction`. `SplitterState.
  position` (also kind-specific) is never touched by `tick_all` either
  — it's ticked manually, inline, inside its own dedicated `Tree::
  set_splitter_position` method. `scroll_offset` follows the identical
  precedent: driven by its own dedicated mechanism, not the central
  tick. Given real scroll input (Phase 3) is a continuous, high-
  frequency, per-wheel-event *direct* value (like a slider being
  dragged, not a discrete eased transition), `Animated<f64>`'s plain
  `.current` field is what a caller sets directly — using `Animated<
  f64>` still keeps the door open for a future "scroll to item,
  animated" caller without requiring one now.
- **Real finding: `vello_hybrid::Scene::push_layer`'s own `clip_path`
  is baked into absolute/device-space strips at the moment `push_layer`
  is called** (confirmed by reading `push_layer`'s own source directly:
  `layer_transform = self.effective_path_transform()`, captured once,
  before any content is drawn into the layer). This means a clip
  pushed using the `VirtualList` node's own `composed` transform stays
  correctly anchored even though each child painted inside the layer
  goes on to call `scene.set_transform` again for its own composed
  transform — the same real mechanism ripple's own clip (M4 Phase 5)
  already proves works, just reused for a node's own bounds instead of
  a circle.
- `paint_node`'s own child-recursion currently applies zero clipping
  anywhere for any `NodeKind` (confirmed via direct read) — Container's
  children can already overflow its own bounds with nothing hiding
  them; this phase only introduces a real clip for `VirtualList`
  specifically, not implicit `overflow: hidden` everywhere (a much
  larger, unscoped change no other `NodeKind` asked for).
- M8 Phase 1's own `visible: Rect` threading is directly reusable here:
  the `bounds` value `paint_node` already computes for *this* node's
  own culling check (its real composed absolute bounding box) is
  exactly the rect a `VirtualList`'s own real visual clip also uses —
  intersecting `visible` with `bounds` before recursing into a
  `VirtualList`'s own children means an off-screen-*within-the-clip*
  materialized child also gets engine-culled for real, not just
  visually hidden behind the clip. This narrowing must be scoped to
  `VirtualList` specifically (the one `NodeKind` that introduces a real
  visual clip) — narrowing `visible` for every `NodeKind` would
  incorrectly cull legitimately-overflowing content under any other
  kind, since nothing else in this engine clips today.

## Design

- `VirtualListState` gains `pub scroll_offset: Animated<f64>`,
  initialized to `Animated::new(0.0)` in `VirtualListState::new`.
  Vertical scroll only, a real, stated v1 scope limit (§11.7's own
  text/every real scrolling list this framework's own examples need is
  vertical) — additive whenever a real horizontal-scroll consumer
  exists.
- `paint_node`'s child-recursion becomes `NodeKind`-aware: for
  `NodeKind::VirtualList(state)`, pushes a real clip layer (`Scene::
  push_layer` with a local `Rect::new(0.0, 0.0, w, h)` path, matching
  ripple's own clip precedent), composes `Affine::translate((0.0,
  -state.scroll_offset.current))` into the transform its children
  recurse with (so materialized content visually scrolls without
  moving the list's own frame), narrows `visible` to `visible.
  intersect(bounds)` (`bounds` already computed for this node's own
  Phase 1 culling check) before recursing, then pops the layer. Every
  other `NodeKind` keeps the exact plain recursion from before this
  phase — no shared code path changes for them.

## Verification plan

- `cargo test --workspace --release`/clippy/fmt — full pre-existing
  suite must pass unmodified (a `VirtualList` at the default `scroll_
  offset: 0.0` must render byte-for-byte the same as before this
  phase, the same "provably a no-op" standard every additive feature
  since M5 Phase 1 has been held to — the existing `virtual_list.rs`
  pixel test is the real proof). New tests: a non-zero scroll offset
  genuinely shifts materialized children's own painted position; a
  materialized child positioned outside the list's own clip bounds
  (post-scroll) is genuinely not visible (proves the real clip, not
  just an assumption that positioning it off-list would look right
  anyway); a child *within* the list's own bounds pre-scroll but
  pushed outside them by scrolling is clipped, not still fully painted.
- `maturin develop --release` + `pytest tests/` + all examples — this
  phase adds no new Python-facing API (scroll offset is set/read only
  at the Rust `Tree`/test level for now; Phase 3 is what wires a real,
  Python-reachable trigger) — a pure regression check.
