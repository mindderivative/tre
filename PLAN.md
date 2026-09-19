# PLAN — M32 Phase 3: Real Scroll/Clip for Oversized Content

## Goal
Close the real, repeatedly-stated gap this catalog named across M30/
M31: "no `NodeKind` besides `VirtualList` clips its own children
today." Any node should be able to opt into clipping its own children
to its own box, the same real mechanism `VirtualList`/`Carousel`
already have, generalized.

## Steps
1. Confirmed the exact literal gap via re-grep of its own prior
   wording: "no `NodeKind` besides `VirtualList` clips **its own
   children** today" -- children-clipping specifically, not clipping a
   leaf node's own internal painted overflow (e.g. a `TextField`'s own
   glyphs, which have no node-tree children at all). Scoped
   accordingly: this closes the general children-clipping gap, letting
   an app wrap an oversized `Code Editor` (or anything else) in a
   clipping `Container` -- matching this project's own "app composes,
   engine provides the primitive" split every other composed capability
   here already follows (the gutter, the fold toggle).
2. Added `PaintProperties.clip_children: bool` (default `false`, a
   true no-op for every existing node, not `Animated` -- the identical
   deliberate choice `corner_radii_override` already made).
3. Generalized `engine-render::paint_node`'s existing `VirtualList`/
   `Carousel`-specific branching: `Carousel` still always clips (its
   own real MD3 anatomy, unconditional); any other `NodeKind` now
   clips too when `clip_children` is genuinely set, reusing the
   identical real clip-path/narrowed-visibility logic `Carousel`
   already established, with no scroll-offset translation (a real,
   stated v1 limit: clipping only).
4. Added `Node.set_clip_children(bool)` (`engine-py`) -- universal,
   not `NodeKind`-specific, unlike `set_syntax_spans`/`set_folded_
   ranges`.
5. Real, dedicated pixel-diff Rust integration test
   (`clip_children.rs`, modeled on `virtual_list_scroll.rs`): a 50x50
   parent with a 50x200 (4x too tall) child, proving `clip_children:
   true` genuinely hides the overflow and `clip_children: false`
   (the default) is a true no-op -- both passed on the first run.
6. Real, direct empirical script before pytest: `set_clip_children`
   doesn't raise on a plain `Rect` with a real child attached.
7. Added `tests/test_clip_children.py` (3 tests, including a real
   contrast proving this is universal, unlike the `TextField`-only
   `set_syntax_spans`/`set_folded_ranges`) and `examples/clip_
   children.py` (a "read more" card whose real content is taller than
   its own preview box) -- both checked for filename collisions first.
8. Full verification chain: cargo check/clippy/fmt/test, maturin
   develop, pytest (full suite), all 71 examples, showcase demo,
   mypy --strict.
9. Update `BUILD_TRACKER.md`, regenerate + republish the artifact,
   update memory, commit.

## Status
Complete. All steps done; full verification chain green (`engine-render`
gains 2 new pixel-diff tests, `pytest tests/` 514 passed/1 skipped, up
from 511, all 71 examples, showcase demo, mypy --strict clean).
