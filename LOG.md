# LOG — M37: Fix VirtualList Hit-Test-After-Scroll

- User's own explicit instruction: "Start the virtual list bug" --
  fixing the real, confirmed gap M36's own investigation found and
  deliberately left open (`BUILD_TRACKER.md`'s own M36 "Not scoped"
  note, which already named the two real fix options).
- Chose to migrate `VirtualList` to the same "bake position into
  `layout_style`" pattern `Carousel`/`ScrollView` already use, rather
  than a narrower, `VirtualList`-specific correction inside `Tree::
  hit_test_at` alone. Real reasoning: a second, independent parallel
  implementation in `hit_test_at` would have to stay in sync with
  `paint_node`'s own transform by hand forever -- the exact kind of
  implicit, unstated coupling between two separate code paths that
  caused this bug in the first place. The baked-in-`layout_style`
  pattern makes paint and hit-testing agree by construction, with
  nothing left to keep in sync.
- New `Tree::sync_virtual_list_layouts`, mirroring `sync_carousel_
  layouts`/`sync_scroll_view_layouts`'s own exact shape, wired into
  `compute_layout` right after the scroll-view sync: for every real
  materialized `(idx, child)` pair, writes `child`'s own `layout_
  style.inset.top = offset_of(idx) - scroll_offset.current` every
  frame -- the real position both paint and hit-testing now read
  identically.
- `engine-render::paint_node`'s own formerly-`VirtualList`-only
  branch (real clip + a separate paint-time-only `Affine::translate
  ((0.0, -scroll_offset))`) merged directly into the existing
  `Carousel`/`ScrollView` unconditional-clip branch -- `VirtualList`
  now takes the exact same code path as those two; the separate
  scroll-offset translate deleted entirely (no longer needed --
  `composed` alone, from the now-correct baked-in `layout_style`, is
  already right).
- Real, decisive regression test added directly at the Rust level
  (`crates/engine-core/src/tree.rs`), mirroring the exact scratch
  investigation M36's own scoping ran (that scratch test was removed
  after use there; this one is real, permanent regression coverage):
  a real 20-item list, `0..5` materialized, scrolled by 40px -- a
  real point at item 2's own genuine post-scroll screen position
  (y=10) now resolves to item 2 itself. Passed on the first run.
- Compiled clean on the first `cargo check`/`cargo clippy` attempt.
- Ran the full workspace test suite -- one real, expected failure
  surfaced: `virtual_list_scroll.rs`'s own `a_real_scroll_offset_
  shifts_materialized_children_up_by_that_many_pixels` broke. Real,
  careful investigation, not a reflexive "make it pass": the test
  mutated `VirtualListState.scroll_offset.current` directly, then
  rendered *without* calling `compute_layout` again -- valid under
  the old (buggy) paint-time-only-translate design, since paint read
  the live field fresh every call with no layout dependency; no
  longer valid under the new, correct design, where the scroll offset
  is baked into `layout_style` only during `compute_layout`. This is
  the identical real requirement `Carousel`/`ScrollView` already have
  (a direct field mutation needs a fresh layout pass to take visual
  effect), invisible in real usage since a real app's own per-frame
  loop always calls `compute_layout` before painting regardless.
  Fixed by adding the real, now-required `compute_layout` call.
- While fixing the second `virtual_list_scroll.rs` test (which
  happened to still pass), noticed its own real assertion had been
  passing for the *wrong* reason -- a stale, unshifted item position
  that never covered the tested pixel anyway, not a genuine proof of
  the real 1000px scroll clip. Added the same real `compute_layout`
  call there too and strengthened the test's own doc comment to state
  why, rather than leaving a real, accidentally-weak test in place
  now that it was noticed.
- Real, honest verification-surface limit found while looking for a
  Python-level empirical reproduction: `Window.add_virtual_list`'s
  own real Python API returns only a flat per-row background color
  from `materialize` and exposes no way to get a materialized row's
  own `Node` handle back to Python -- so there is no way to construct
  an equivalent real per-item-click-after-scroll reproduction at the
  Python/pytest level the way the Rust-level test above does
  directly. Verified at the Rust level instead, the same "some real
  claims are better suited to a lower level than an FFI test"
  precedent M31 Phase 6/M32 Phase 2 already established.
- Full verification: `cargo check --all-targets`/`cargo clippy
  --all-targets -D warnings`/`cargo fmt --check` clean, `cargo test
  --workspace --release` clean (`engine-core` +1 new decisive
  regression test, up from 182 to 183; the 2 pre-existing `virtual_
  list_scroll.rs` tests fixed and passing, zero other regressions),
  `maturin develop --release` rebuilt, `pytest tests/` 556 passed/1
  skipped (unchanged -- pure internal Rust fix, confirmed via the
  existing `test_virtual_list.py` suite's own 11/11 unchanged pass),
  all 75 examples and the showcase demo re-run clean.
- Updated `BUILD_TRACKER.md` (new M37, 1 phase, documenting the real
  fix, the real design choice made between the two options M36's own
  trailer named, and the real, honest verification-surface limit) --
  verified the parser's own reported item count before/after (36/114/
  204 -> 37/115/205, exactly +1/+1/+1 matching the single new phase),
  regenerated and republished the Build Tracker artifact. **This
  closes M37, its 1 phase.**
