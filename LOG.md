# LOG — M36 Phase 1: ScrollView Core Mechanism, closing M36

- User instruction "Start the scrollable container" followed directly
  from a "what would you recommend" exploratory answer that offered
  two real options (this general scrollable container, or the smaller
  Loading Indicator); the user picked the infrastructure option.
- Real precedent read directly from the sibling pyCopper project's own
  `ScrollViewElement` (`widgets/scroll.py`) before designing anything:
  single child, measured unbounded on the scroll axis so it reports
  its own true content extent; scrolling is a pure paint-time
  translation (`child_origin`), not a relayout; real clipping; real
  wheel handling that only stops propagating if the viewport actually
  moved (so an exhausted inner scroll view lets the wheel keep
  travelling to an outer one); a real scrollbar thumb pyCopper's own
  comment states plainly is not from the M3 spec.
- **Real, load-bearing finding, made while grounding the design in
  existing precedent, not assumed:** before copying `VirtualList`'s
  own scroll pattern verbatim, checked directly whether real point-
  based hit-testing after a real scroll actually resolves correctly.
  It does not. A dedicated scratch Rust test (a real VirtualList,
  scrolled by 40px, then a real point-based hit_test call at the
  item's own genuine post-scroll screen position) confirmed it
  resolves to the WRONG materialized item; only the stale, pre-scroll
  position resolves correctly. Root cause: the scroll offset is
  applied only as an extra `engine-render::paint_node` translate,
  never reflected back into `layout_style`, which `Tree::hit_test_at`
  reads directly. Silently never caught because `Window.click(node)`'s
  own synthetic test helper computes its target point from the
  identical pre-scroll `self.layout(node)`, so the two coincidentally
  agree without either reflecting the real, live, post-scroll visual
  position -- a real mouse click at a genuine screen pixel after
  scrolling would be affected; this project's own synthetic click-
  dispatch tests never would be. Removed the scratch test after
  confirming the finding.
- Checked whether `Carousel`'s own `sync_carousel_layouts` (M30 Phase
  9 Step 5) has the same flaw -- it does not: it bakes each child's
  real absolute position directly into `layout_style` every frame via
  `Tree::set_layout_style`, which both `paint_node` and `hit_test_at`
  already read correctly, by construction, with zero extra work.
  Real, deliberate design decision: `ScrollView` follows `Carousel`'s
  bug-free pattern, not `VirtualList`'s flawed one -- the first real
  case this session's own "check existing precedent for correctness"
  discipline was turned inward on the codebase's own prior work.
- New `NodeKind::ScrollView(ScrollViewState)` -- `scroll: Animated<f64>`
  (driven directly, never eased, matching `VirtualListState::scroll_
  offset`'s own real precedent -- confirmed via reading `Tree::
  tick_all` directly that no kind-specific `Animated<T>` field is ever
  ticked centrally) plus `horizontal: bool` (single-axis only, matching
  pyCopper's own real design, never simultaneous 2D scroll).
  `#[derive(Clone, Debug, PartialEq)]` removed -- `Animated<T>`
  implements none of those, the same real precedent `NodeKind`/
  `IconState` already establish for `Splitter`/`Icon`.
- New `Tree::sync_scroll_view_layouts` (mirrors `sync_carousel_
  layouts` exactly), wired into `compute_layout` right after the
  button-group sync. Real, honest re-clamp on every real layout pass
  (content may have shrunk since last frame), matching pyCopper's own
  `_clamped_scroll` discipline. New `Tree::scroll_scroll_view_by`
  (mirrors `scroll_virtual_list_by`, real content-extent measurement
  from the child's own real layout instead of an item-count formula).
- `ScrollView` joins `Tree::dispatch`'s existing "walk up to the
  nearest scrollable ancestor" wheel-bubbling loop -- a third real
  widening after `VirtualList`/`Carousel`. `engine-render::paint_node`
  needed only two small additive changes: `ScrollView` paints nothing
  of its own (joins the existing no-op arm) and always clips (joins
  `Carousel`'s own unconditional-clip branch, no scroll-offset
  translation needed there either, the identical real reason
  `Carousel` doesn't need one).
- `Window.add_scroll_view(width, height, horizontal, x, y) -> Node`
  returns a plain container; the caller composes real content in via
  the existing, generic `Node.add_child`, the identical "engine
  provides the primitive, app composes" split `add_toolbar` (M35
  Phase 1) already established.
- Widened `Window.scroll` with an optional `delta_x: f64 = 0.0` for
  real horizontal `ScrollView` testability -- backward-compatible,
  proven by a dedicated regression test that every pre-existing real
  caller passing only `delta_y` still works unchanged.
- Real, decisive tests at three levels: Rust unit tests prove the
  exact scroll-clamp math at both ends, a real zero-max-scroll case,
  real wheel-dispatch bubbling, and -- the real, decisive one -- a
  genuine grandchild marker inside the scrolled content resolves
  correctly to a point at its own post-scroll screen position (a
  single-child-only test would have been structurally unable to prove
  this, since the one child's own full unclipped extent would
  trivially contain almost any in-viewport point regardless of
  whether scroll were ever subtracted at all -- caught this while
  writing the first draft of the test, fixed by adding a real,
  precisely-positioned grandchild marker instead). A real pixel-diff
  integration test (`crates/engine-render/tests/scroll_view.rs`,
  mirroring `clip_children.rs`'s own pattern) proves genuine clipping
  and a genuine scroll-shift into view -- both passed on the first
  run. A real empirical script before any pytest proved the full real
  end-to-end FFI path.
- Real, honest limit confirmed while writing the hit-test Rust test:
  a point past the real clipped viewport edge but still within the
  child's own full (unclipped) content rect still resolves to that
  child -- `hit_test_at` has no independent clip-bounds check anywhere
  in this codebase today, confirmed this is shared, pre-existing
  behavior with `VirtualList`/`Carousel`, not a regression or a gap
  unique to `ScrollView`; adjusted the test's own assertion to match
  the real, verified behavior rather than an incorrect assumption.
- Full verification: `cargo check --all-targets`/`cargo clippy
  --all-targets -D warnings`/`cargo fmt --check` clean, `cargo test
  --workspace --release` clean (`engine-core` +4, `engine-render` +2),
  `maturin develop --release` rebuilt, `pytest tests/` 556 passed/1
  skipped (7 new, up from 549, zero regressions), all 75 examples
  (including the new `examples/scroll_view.py`) and the showcase demo
  re-run clean, `mypy --strict` clean against `examples/scroll_view.py`
  (one real fix: a per-iteration closure needed a named handler
  factory instead of a loop-variable-capturing lambda, caught by
  `mypy --strict` itself, matching `examples/top_app_bar.py`'s own
  already-established pattern).
- Updated `BUILD_TRACKER.md` (Phase 1 closed; M36 itself closed, its
  1 phase; Top Metrics row at 100%; a real "Not scoped" trailer note
  documenting the real, previously-undiscovered `VirtualList` hit-
  test-after-scroll bug as a real, separate, pre-existing gap this
  investigation surfaced but did not fix, left for a future
  milestone) -- verified the parser's own reported item count
  unchanged (only an existing item's status flipped), regenerated and
  republished the Build Tracker artifact. **This closes M36 Phase 1
  and, with it, M36 itself, its 1 phase.**
