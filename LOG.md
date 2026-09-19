# LOG — M32 Phase 3: Real Scroll/Clip for Oversized Content

- Re-read the exact prior wording of this gap before scoping anything:
  "no `NodeKind` besides `VirtualList` clips **its own children**
  today" -- confirmed via grep this is specifically about a node
  clipping its real node-tree children, not a leaf's own internal
  painted overflow (a `TextField`'s glyphs have no children at all, so
  a children-clip mechanism alone doesn't touch that case). Scoped
  Phase 3 to exactly what the gap actually says: general children
  clipping, letting an app *compose* a clipping wrapper around
  anything oversized (the same "app composes, engine provides the
  primitive" split the gutter/fold toggle already established), not a
  new TextField-internal scroll mechanism.
- Added `PaintProperties.clip_children: bool` (`engine-core::node.rs`),
  default `false`, a true no-op for every one of this codebase's
  existing nodes -- not `Animated`, the identical deliberate choice
  `corner_radii_override` already made (a static per-node choice, not
  something anything here needs to smoothly transition into/out of).
  No call-site breakage anywhere: every real `PaintProperties`
  construction already goes through `::new()`, confirmed via grep
  before adding the field (zero direct struct-literal sites exist
  outside the definition itself).
- Generalized `engine-render::paint_node`'s existing `VirtualList`/
  `Carousel`-specific clip branching rather than adding a third,
  parallel branch: `Carousel` still always clips (unconditional, its
  own real MD3 anatomy); the same branch now also fires for any other
  `NodeKind` when `node.paint.clip_children` is genuinely set, reusing
  the identical real clip-path-plus-narrowed-visibility logic already
  there. No scroll-offset translation for the general case -- the
  real, stated v1 limit: clipping only, not a new scroll mechanism.
- Added `Node.set_clip_children(bool)` (`engine-py::node.rs`) --
  deliberately universal (mutates `PaintProperties` directly, no
  `NodeKind` match/rejection), the real, deliberate contrast with
  `set_syntax_spans`/`set_folded_ranges` just above it in the same
  file, which reject any node that isn't a `TextField`.
- Full Rust verification chain green on the first pass: `cargo check`/
  `clippy -D warnings`/`fmt --check` clean.
- Wrote a real, dedicated pixel-diff integration test
  (`crates/engine-render/tests/clip_children.rs`, modeled directly on
  `virtual_list_scroll.rs`'s own render-to-texture-then-readback
  pattern): a 50x50 parent with a single 50x200 child (four times its
  own parent's real height). Two tests -- `clip_children: true`
  genuinely hides the overflow past y=50 (checked at y=90, well inside
  the child's own real height); `clip_children: false` (the default)
  is a true no-op, the oversized child still paints past the parent
  exactly as it always did, a real regression guard for every existing
  node's own unchanged behavior. Both passed on the very first run --
  no bug this time (unlike M32 Phase 2's own `Tree::set_layout_style`
  catch), the generalization of already-proven `Carousel`/`VirtualList`
  logic held up directly.
- Rebuilt the Python extension. Ran a real, direct empirical script
  before writing any pytest: `set_clip_children(True)` on a plain
  `Rect` with a real oversized child attached doesn't raise.
- Added `tests/test_clip_children.py` (3 new tests, checked for a
  filename collision first: none -- `test_clipboard.py` is unrelated)
  and `examples/clip_children.py` (checked for a collision first: none
  -- `clipboard.py` is unrelated) -- a real "read more" card whose real
  content is taller than its own 80px preview box, clipped cleanly
  instead of spilling out.
- Full verification: `cargo check`/`clippy -D warnings`/`fmt --check`
  clean, `cargo test --release` (`engine-render` +2 pixel-diff tests),
  `maturin develop --release`, `pytest tests/` 514 passed/1 skipped (3
  new, up from 511, zero regressions), all 71 examples (including the
  new `examples/clip_children.py`) and the showcase demo re-run clean,
  `mypy --strict` clean against `examples/clip_children.py`.
- Updated `BUILD_TRACKER.md` (Top Metrics row, Phase 3 heading and Step
  1) -- verified the parser's own reported item count before/after,
  regenerated and republished the Build Tracker artifact.
