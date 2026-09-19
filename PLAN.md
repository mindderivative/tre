# PLAN — M36 Phase 1: ScrollView Core Mechanism, closing M36

## Goal
Build a real, general scrollable container -- today only VirtualList/
Carousel/Terminal scroll, each bespoke; `clip_children`'s own stated
v1 limit explicitly names this as the real, separate gap.

## Steps
1. Real precedent read directly from the sibling pyCopper project's
   own `ScrollViewElement` (`widgets/scroll.py`) before designing
   anything: single child, measured unbounded on the scroll axis,
   scrolling as a pure paint-time translation, real clipping, real
   wheel handling that only stops propagating if the viewport moved.
2. **Real, load-bearing investigation before copying VirtualList's own
   scroll pattern:** a dedicated scratch Rust test confirmed a real,
   previously undiscovered bug -- hit-testing after a real VirtualList
   scroll resolves the WRONG item, since the scroll offset is applied
   only as an extra paint-time transform, never reflected back into
   layout_style that hit_test_at reads. Confirmed Carousel's own
   sync_carousel_layouts does NOT have this flaw (bakes real position
   into layout_style every frame). Decided: ScrollView follows
   Carousel's bug-free pattern, not VirtualList's flawed one.
3. New `NodeKind::ScrollView(ScrollViewState { scroll: Animated<f64>,
   horizontal: bool })`. New `Tree::sync_scroll_view_layouts` (mirrors
   sync_carousel_layouts), wired into compute_layout. New `Tree::
   scroll_scroll_view_by` (mirrors scroll_virtual_list_by). ScrollView
   joins Tree::dispatch's existing wheel-bubbling loop and paint_node's
   existing no-op/clip branches.
4. `Window.add_scroll_view(width, height, horizontal, x, y) -> Node`
   -- caller composes real content via the existing, generic Node.
   add_child, matching add_toolbar's own established "engine provides
   the primitive" split.
5. Widened `Window.scroll` with an optional `delta_x: f64 = 0.0` for
   real horizontal ScrollView testability -- backward-compatible.
6. Real, decisive tests at three levels: Rust unit tests (scroll-clamp
   math, and a genuine grandchild marker proving hit-test-after-scroll
   resolves the RIGHT node at its real post-scroll position, not its
   stale one); a real pixel-diff integration test proving genuine
   clip + scroll-shift; a real empirical script before any pytest.
7. Full verification chain: cargo check/clippy/fmt/test, maturin
   develop, `tests/test_scroll_view.py` (7 tests), `examples/
   scroll_view.py`, full pytest suite, all examples, showcase demo,
   mypy --strict (one real fix: a loop-capturing lambda needed a named
   handler factory, matching top_app_bar.py's own established pattern).
8. `BUILD_TRACKER.md` (Phase 1 closed, M36 itself closed, its 1 phase;
   the real VirtualList bug documented as a real, separate,
   not-fixed-here finding), artifact republish, memory update, commit,
   push (the full milestone now closes).

## Status
Complete. Full verification chain green (`pytest tests/` 556 passed/1
skipped, 7 new, zero regressions; all 75 examples + showcase demo
clean; mypy --strict clean). **M36 -- General Scrollable Container is
now fully complete, its 1 phase.**
