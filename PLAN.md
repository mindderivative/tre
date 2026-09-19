# PLAN — M39 Phase 1: Code Editor Horizontal Scroll

## Goal
Give Code Editor real horizontal scroll+clip for a real line wider
than its own box, with real horizontal caret-follow -- the real,
separate gap M38 Phase 7 explicitly left open (that phase built only
the vertical half).

## Steps
1. Confirmed the real gap directly: `engine-render::text::field_max_
   width` returns `f32::MAX` for every `multiline` field -- no real
   line ever wraps, it simply extends right, clipped since M38 Phase
   7 but not scrollable.
2. New `TextFieldState.horizontal_scroll_offset: Animated<f64>`
   (`node.rs`) -- parallel to `scroll_offset`'s own vertical field,
   identical real contract (driven directly, never eased).
3. Extended `Tree::scroll_text_field_caret_into_view` with the
   identical real "scroll just enough to reveal the caret" logic
   along the horizontal axis, using a new `Tree::real_column`-based
   caret column and a real, cited character-width ratio: `font_size *
   0.6`. Not an external citation this time -- this codebase's own
   real historical precedent (found via `git log -p` on `engine-
   render/src/text.rs`, predating M32 Phase 1's switch to real
   measured `monospace_cell_size`) already used exactly this ratio
   for `Terminal`'s own pre-real-font-metrics cell-width estimate.
4. Wired `horizontal_scroll_offset` into `engine-render`'s own
   `NodeKind::TextField` paint arm (`TextPlacement.x = -state.
   horizontal_scroll_offset.current`) -- the existing multiline clip
   layer from M38 Phase 7 already bounds both axes, no clip changes
   needed.
5. **Real, latent test-infrastructure bug found and fixed along the
   way:** while debugging a failing new horizontal test, found the
   existing vertical `caret_follow_scene()` test helper (and my own
   new horizontal one, copied from its pattern) wraps the field in a
   `Container` root built via `leaf(0.0, 0.0)` -- an explicit
   zero-width `Style`. Since taffy's own default `Display` is `Flex`
   with `flex_direction: Row` and `flex_shrink: 1.0`, this genuinely
   shrinks the field's own reported layout *width* down to 0 (height
   survives only because it's the cross axis, where an explicit size
   is honored directly, not stretched) -- real, silently wrong
   `layout(field).size.width`, invisible until now because no
   pre-existing test read it. Fixed both scene helpers to use the
   field as its own real `compute_layout` root instead, mirroring
   `scrollable_view`'s own already-correct pattern (M38 Phase 6).
6. Real tests: two new `tree.rs` unit tests for the horizontal caret-
   follow math (scroll-right, scroll-back-left), hand-verified against
   the real 0.6 ratio before running -- both passed on the first run
   after the scene-helper fix. One new pixel-level test in `crates/
   engine-render/tests/text_field_paint.rs` (a real whole-buffer diff
   between `horizontal_scroll_offset = 0.0` and `100.0`). One new
   pytest test exercising the real, full FFI surface with a genuinely
   overflowing single line -- no Python getter exists for `horizontal_
   scroll_offset` itself, the same verification-surface limit already
   established repeatedly.
7. Corrected the stale doc comments claiming horizontal scroll was
   still a real, separate open v1 limit: `add_code_editor`'s own Rust
   doc comment (`window_factory.rs`) and its `python/tre/_core.pyi`
   stub.
8. Full verification chain: `cargo check`/`clippy -D warnings`/`fmt`/
   `test --workspace --release`, `maturin develop --release`, full
   `pytest tests/`, all 75 examples, showcase demo.
9. `BUILD_TRACKER.md` Phase 1 flipped to done, Top Metrics updated to
   1-of-5, artifact regenerated (39/127/218, unchanged) and republished.

## Status
Complete. Full verification chain green (`cargo test --workspace
--release`: `engine-core` 198 passed (+2); `engine-render`'s own
`text_field_paint` suite 16 passed (+1); `pytest tests/`: 564 passed/1
skipped, up from 563, +1 new test; all 75 examples + showcase demo
clean). **M39 Phase 1 -- Code Editor Horizontal Scroll is now
complete. M39 itself remains open: 4 phases remain (Loading Indicator
+ Time Picker Dial, shape-morphed border inset fix, Terminal cell text
attributes, `Tree::tick_all` active-set optimization).**
