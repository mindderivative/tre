# Log: M8 Phase 2 — `VirtualList` Real Scroll Offset & Clipping (§11.7)

Corresponds to `BUILD_TRACKER.md` M8 Phase 2. A new `Animated<f64>`
scroll offset on `VirtualListState`, composed into materialized
children's own effective position; real clipping via `Scene::
push_layer`'s own `clip_path` support.

## Investigation before writing code

- `VirtualListState` had no scroll concept at all — `Tree::
  set_virtual_list_window` requires the caller to supply an explicit
  `visible: Range<usize>`; this phase doesn't change that contract, it
  only makes the paint-time position/clip of whatever's already
  materialized real.
- Real finding: `Tree::tick_all` only ever ticks `node.paint`/`node.
  interaction` — kind-specific `Animated<T>` fields (confirmed via
  `SplitterState.position`'s own precedent) are driven by their own
  dedicated mechanism, ticked manually inline where they're set, not
  centrally. `scroll_offset` follows the identical shape — a plain
  `Animated<f64>` set directly, not eased by default.
- Real finding: `vello_hybrid::Scene::push_layer`'s own `clip_path` is
  baked into absolute/device-space strips at the moment `push_layer` is
  called (confirmed by reading its own source: `layer_transform =
  self.effective_path_transform()`, captured once, before any content
  is drawn). A clip pushed under the `VirtualList` node's own `composed`
  transform stays correctly anchored even though each child painted
  inside the layer goes on to set its own transform — the same real
  mechanism ripple's own clip (M4 Phase 5) already proves works.
- `paint_node`'s own child-recursion applied zero clipping anywhere for
  any `NodeKind` before this phase — this phase introduces a real clip
  only for `VirtualList`, not implicit `overflow: hidden` everywhere.
- M8 Phase 1's own `visible: Rect` threading is directly reusable: the
  `bounds` value already computed for a node's own culling check is
  exactly the rect a `VirtualList`'s real clip also uses — intersecting
  `visible` with `bounds` before recursing into a `VirtualList`'s own
  children means an off-screen-within-the-clip materialized child also
  gets engine-culled, not just visually hidden. Scoped to `VirtualList`
  specifically, since no other `NodeKind` introduces a real visual clip.

## What happened

`VirtualListState` gains `pub scroll_offset: Animated<f64>`, defaulting
to `Animated::new(0.0)` in `::new`. `paint_node`'s final child-recursion
becomes `NodeKind`-aware: for `NodeKind::VirtualList(state)`, pushes a
real clip layer (a local `RoundedRect(0,0,w,h,corner_radius)` path,
matching the list's own real corner radius), composes `Affine::
translate((0.0, -state.scroll_offset.current))` into the transform
children recurse with, narrows `visible` to `visible.intersect(bounds)`
before recursing, then pops the layer. Every other `NodeKind` keeps the
exact plain recursion from before this phase.

New `crates/engine-render/tests/virtual_list_scroll.rs`: a real scroll
offset genuinely shifts materialized children's own painted position
(a partially-scrolled item shows exactly the real overlap remaining);
scrolled-out content is genuinely clipped, not just repositioned (an
item scrolled 1000px up shows nothing anywhere in the list, not moved
off to a still-visible spot). Both passed on the first run.

Full `cargo test --workspace --release` clean (`engine-render` gains 2
new tests — the full pre-existing `virtual_list.rs` suite passed
unmodified, confirming the default `scroll_offset: 0.0` is a true
no-op), `cargo clippy --workspace --all-targets -- -D warnings`, `cargo
fmt --check` all clean. This phase adds no new Python-facing API —
`maturin develop --release` + full `pytest tests/` (78 passed, 1
skipped, unaffected) and all fourteen examples confirmed clean, a pure
regression check.
