# PLAN — M30 Phase 9 Step 5: Carousel

## Goal
Add `Window.add_carousel` — a real MD3 carousel (`COMPONENT_CAROUSEL.md`).
Checked with the user before starting, given the real scope (continuous
layout-invalidating animation, real wheel/drag input this codebase
doesn't have anywhere else); the user chose "full real MD3 carousel" —
hero/multi_browse layouts with items that resize as they scroll, real
wheel + drag-to-scroll input, real snap animation, matching pyCopper's
own real scope. Closes Phase 9 and, with it, M30 itself (all 10
phases, 0-9).

## Steps
1. Read pyCopper's own real `Carousel` widget in full — three layouts
   (`uncontained`/`hero`/`multi_browse`), the real `_item_width`
   interpolation formula, real dimension constants, real wheel
   (`dx`/`dy`) and drag (`DRAG_INDEX_THRESHOLD=60px`) handling.
2. Investigated TRE's own real architecture for the hardest open
   question (an animated value that also invalidates layout): found
   this codebase already gets it for free — `Tree::tick_all` sets
   `dirty` whenever any `Animated<T>` is active, and `app.rs`'s own
   per-frame loop already calls `compute_layout` unconditionally on a
   dirty frame. No new central-ticking infrastructure needed.
3. Investigated real wheel/drag precedent — both already real and
   reusable: `InputEvent::Scroll`'s own dispatch arm already hit-tests
   and walks the parent chain to the nearest `VirtualList` (M8 Phase
   3); `Tree::dispatch`'s own `self.dragging` mechanism (`Splitter`/
   `Slider`) is a real, generic press/move/release drag pattern.
   Widened both with a `NodeKind::Carousel` branch.
4. Designed real item positioning: taffy has no "measure my children
   after my own size is known" hook, so chose real `Position::Absolute`
   insets computed by hand every layout pass (`Tree::
   sync_carousel_layouts`), mirroring pyCopper's own manual
   `positions`/`shift` math — giving hit-testing and paint the
   identical real position, unlike `VirtualList`'s own paint-time-only
   scroll offset. One extra `compute_layout` pass per frame while a
   carousel exists is a real, stated, unavoidable v1 cost.
5. Implemented `engine-core`: `NodeKind::Carousel(CarouselState)`,
   `CarouselLayout`, shared `pub const` dimension constants,
   `Tree::sync_carousel_layouts`/`set_carousel_index`/
   `set_carousel_scroll`/`carousel_on_wheel`/`update_carousel_drag`,
   wired into `tick_all`/`dispatch`'s `PointerPressed`/`PointerMoved`/
   `PointerReleased`/`Scroll` arms, 5 new Rust unit tests.
6. Implemented `engine-render`: a `Carousel` paint arm (background via
   the existing universal path, a real clip matching MD3's own
   `CLIPS_CHILDREN` anatomy) — no paint-time translation needed, per
   step 4's own design.
7. Implemented `engine-py`: `Window.add_carousel`, `Node.
   set_carousel_index`/`get_carousel_index`/`get_carousel_position`/
   `set_carousel_scroll`/`get_carousel_scroll` (the established
   dedicated-typed-getter precedent `get_checked`/`get_selected`
   already use, not the generic `Node.animate`/`get`).
8. Ran a real, direct empirical end-to-end script before writing any
   pytest suite: real wheel notch synchronously moves the destination
   index; a real `App.run()` genuinely ticks `position` toward it; real
   Uncontained scroll clamps to its own real content extent.
9. Wrote `tests/test_carousel.py` and `examples/carousel.py` — checked
   for filename collisions first. **A real, confirmed pytest-suite
   hazard found live:** a first draft's own `App().run()` call made
   `test_terminal.py`'s real shell test fail whenever it ran afterward
   in the same process — a second real event-loop invocation within
   one process, the identical fragility `test_terminal.py`'s own doc
   comment already flags for calling `App.run()` twice on the same
   `App`. Removed; the real tick-moves-it claim is already proven at
   the Rust level (step 5's own halfway-ticked test).
10. Full verification chain: cargo check/clippy/fmt/test, maturin
    develop, pytest (full suite), all 67 examples, showcase demo, mypy
    --strict.
11. Update `BUILD_TRACKER.md` — verified the parser's own reported item
    count before/after (191, unchanged, since only existing stub
    bullets were filled in, none added), regenerate + republish the
    Build Tracker artifact. Closes Phase 9 (all 6 steps) and M30 itself
    (all 10 phases).
12. Update memory, commit, push.

## Status
Complete. All steps done; full verification chain green (`engine-core`
163 up from 158, `pytest tests/` 494 passed/1 skipped up from 482, all
67 examples, showcase demo). A real MD3 carousel genuinely resizes its
own items as a real wheel/drag gesture snaps between them, confirmed
by direct observation, not assumed. **M30 is now fully complete, all
10 phases (0-9).**
