# PLAN — M37: Fix VirtualList Hit-Test-After-Scroll

## Goal
Fix the real, confirmed bug M36's own investigation found and left
open: a real point-based hit-test at a `VirtualList` materialized
item's own genuine post-scroll screen position resolves to the wrong
item, since the scroll offset is applied only as a paint-time
translate, never reflected back into `layout_style`.

## Steps
1. Investigated the two real fix options M36's own trailer named:
   (a) a narrower, VirtualList-specific correction inside `Tree::
   hit_test_at` itself, or (b) migrate VirtualList to the same "bake
   position into layout_style" pattern Carousel/ScrollView already
   use. Chose (b): a second, independent implementation in hit_test_at
   would have to stay in sync with paint_node's own transform by hand
   forever -- the exact class of bug that caused this in the first
   place. Baking into layout_style makes paint and hit-test agree by
   construction.
2. New `Tree::sync_virtual_list_layouts` (mirrors sync_carousel_
   layouts/sync_scroll_view_layouts exactly), wired into compute_
   layout right after the scroll-view sync: writes each materialized
   item's own `layout_style.inset.top = offset_of(idx) - scroll_
   offset.current` every frame.
3. `engine-render::paint_node`'s own VirtualList-only branch (clip +
   a separate paint-time Affine::translate) merged directly into the
   existing Carousel/ScrollView unconditional-clip branch -- the
   separate scroll-offset translate deleted entirely.
4. Real, decisive regression test added at the Rust level: a real
   20-item list, scrolled by 40px, a real point at item 2's own
   genuine post-scroll screen position now resolves to item 2 itself
   -- mirrors the exact scratch investigation M36's own scoping ran.
5. Two pre-existing pixel-diff tests in virtual_list_scroll.rs broke
   as a real, correct consequence (they mutated scroll_offset.current
   directly without a following compute_layout call, which the new
   design correctly requires -- the identical requirement Carousel/
   ScrollView already have). Fixed both; strengthened one whose own
   assertion had been coincidentally passing for the wrong reason.
6. Real, honest verification-surface limit confirmed: add_virtual_
   list's own Python API returns only a flat per-row color and never
   exposes a materialized row's own Node handle, so no equivalent
   Python-level reproduction of this specific bug is constructible --
   verified at the Rust level instead, the same precedent M31P6/M32P2
   already established for other cases.
7. Full verification chain: cargo check/clippy/fmt/test, maturin
   develop, full pytest suite (unchanged, pure internal fix), all
   examples, showcase demo.
8. BUILD_TRACKER.md (new M37, 1 phase, documenting the real fix and
   the real design choice made), artifact republish, memory update,
   commit, push (a full milestone closes).

## Status
Complete. Full verification chain green (`cargo test --workspace
--release` 183 passed up from 182, 2 pre-existing tests fixed; `pytest
tests/` 556 passed/1 skipped unchanged; all 75 examples + showcase
demo clean). **M37 -- Fix: VirtualList Hit-Test-After-Scroll is now
fully complete, its 1 phase.**
