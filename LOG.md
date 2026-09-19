# LOG — M38 Phase 1: Tessellated-Path Caching for Remaining Shapes

- User's own explicit instruction: "Let's knock out the known gaps" --
  scoped as M38 with 7 phases, ordered cheapest/most-grounded first.
  Phase 1 is pure mechanical extension of M34 Phase 1's own already-
  proven `GeometryCache` pattern to the remaining `NodeKind`s that
  still tessellate a curve fresh every frame -- zero new design
  questions, per the milestone's own scoping note.
- Widened `GeometryCache` (`crates/engine-render/src/geometry_cache.rs`):
  new `CircleParams`/`ArcParams` (`#[derive(Clone, Copy, PartialEq)]`,
  matching the existing `RectPathParams`'s own shape) and three new
  cache maps/methods -- `circle_primary`/`circle_secondary` (two
  independent per-node slots, since `RadioButton`'s ring+dot and
  `Switch`'s handle each need one, and no single node ever needs both
  at once, confirmed by direct read of every real call site before
  sharing the slot rather than assuming) and `arc` (for
  `CircularProgress`'s sweep). The private `get_or_build` helper
  generalized from a `RectPathParams`-only signature to
  `get_or_build<P: PartialEq>` so all four param types share one real
  implementation instead of four near-duplicates. `evict_stale`
  widened to retain-filter all three new maps alongside the two
  already there. Four new unit tests added (8/8 passing in the
  module, confirmed via `cargo test --release -p engine-render --lib
  geometry_cache`).
- `engine-render::paint_node`'s arms rewired: `Checkbox`'s box fill
  and `Switch`'s track fill/outline stroke reuse the *existing*
  `rounded_rect_fill`/`rounded_rect_border` methods directly (their
  geometry is byte-for-byte identical to what those already build for
  `Rect`/`Splitter`, confirmed by direct comparison before reusing);
  `RadioButton`'s ring/dot and `Switch`'s handle now call
  `circle_primary`/`circle_secondary`; `CircularProgress`'s arc
  stroke now calls `arc`. Removed the now-unused `Arc` import from
  `peniko::kurbo`.
- **Real correction made mid-phase, the identical discipline M35
  Phase 2's own rotation-field correction already established in this
  project:** the phase's own scoping note (`BUILD_TRACKER.md`) named
  `Terminal` as one of the five target `NodeKind`s, but the first
  implementation pass skipped it on an unverified assumption ("plain
  rects, nothing worth caching there"). Direct read of
  `NodeKind::Terminal`'s own paint arm (`lib.rs:574-597`), done before
  writing the BUILD_TRACKER.md completion note, found this assumption
  was wrong: its background is a real `RoundedRect::new(0.0, 0.0, w,
  h, radius).to_path(0.1)` fill, tessellated fresh every frame -- the
  exact same shape `rounded_rect_fill` already caches for `Rect`/
  `Splitter`/`Checkbox`. Fixed by routing it through
  `geometry.rounded_rect_fill(id, w, h, radius)` like the others (its
  own per-cell glyph/cursor painting, `TextRenderer::draw_terminal`,
  is unrelated -- that's `TextRenderer`'s own shaped-layout cache, not
  `GeometryCache`'s concern).
- **Two more real, previously-uncached tessellation sites found by
  the same direct-read discipline while verifying `Terminal`, both
  fixed the same way (pure reuse of the already-tested
  `rounded_rect_fill`, zero new cache code):**
  1. `TextField`'s own box fill (`lib.rs`, the arm directly above
     `Terminal`'s) had the identical fresh-`RoundedRect`-per-frame
     pattern -- routed through `rounded_rect_fill` too.
  2. The universal interaction state-layer/ripple bounds (`lib.rs`,
     the `if let Some(interaction) = &node.interaction` block that
     runs once per frame for *every* interactive node regardless of
     kind -- buttons, list items, icon buttons, anything with
     hover/ripple) built its own fresh `RoundedRect` every frame
     under the *same* `(id, w, h, radius)` as that node's own box
     fill -- now shares the identical cache slot via
     `rounded_rect_fill(id, w, h, radius)`, both for the hover fill
     and for each ripple's own fill inside its `push_layer`. In
     practice this was the single hottest redundant-tessellation
     site in the whole render loop (exercised once per frame per
     interactive node, unconditionally, far more often than any one
     `NodeKind`'s own shape arm) -- the discovery came only from
     checking real call sites while fixing `Terminal`, not from
     trusting the original five-`NodeKind` scoping list.
  3. The `VirtualList`/`Carousel`/`ScrollView`/general-`clip_children`
     scroll-clip path (`lib.rs`, the
     `if matches!(node.kind, NodeKind::VirtualList(_) |
     NodeKind::Carousel(_) | NodeKind::ScrollView(_)) ||
     node.paint.clip_children` block) built its own fresh
     `RoundedRect` clip every frame too -- routed through
     `rounded_rect_fill(id, w, h, clip_radius)`, safe to share the
     same slot since a node has either a box fill or a clip at a
     given `(w, h, radius)`, never conflicting params for the same
     `id` in the same frame.
- Full verification chain run twice: once right after the original
  five-`NodeKind` pass, again after the three additional discoveries
  above. Both runs: `cargo check --workspace --all-targets` clean;
  `cargo clippy --workspace --all-targets -D warnings` clean; `cargo
  fmt` + `cargo fmt --check` clean; `cargo test --workspace --release`
  clean (zero regressions -- `engine-core` unchanged at 183,
  `engine-render`'s own `geometry_cache` module 8/8 including the 4
  new tests, every other crate's suite unchanged); `maturin develop
  --release` rebuilt; `pytest tests/` 556 passed/1 skipped both times
  (unchanged -- pure internal Rust optimization, no Python-facing API
  change, so no new example/stub needed); all 75 examples and the
  showcase demo re-run clean both times.
- Updated `BUILD_TRACKER.md`: Phase 1 flipped `⬜` -> `✅` with a
  terse, single-line step-bullet note (this file carries the full
  writeup, per the established "BUILD_TRACKER.md is the durable
  index, PLAN.md/LOG.md carry the per-phase essay" convention); the
  milestone's own status line updated to "🚧 In progress -- Phase 1 of
  7 done"; the Top Metrics table row updated from 0%/Not started to
  14%/"Phase 1 of 7 done". Verified the parser's own reported item
  count was unchanged before/after (38/122/212 both times -- a pure
  status-flip with no new step bullets, exactly as expected).
  Regenerated and republished the Build Tracker artifact.
  **This closes M38 Phase 1. M38 itself remains open -- 6 phases
  remain (goal-column memory, fold-aware cursor navigation, Split
  Button inner-corner shape-tightening, Button Group per-child shape
  change on press/select, ScrollView scrollbar thumb, real scroll+clip
  for Code Editor with caret-follow).**
