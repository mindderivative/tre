# LOG — M30 Phase 9 Step 5: Carousel

- Read pyCopper's own real `Carousel` widget in full: three layouts
  (`uncontained` — free pixel scroll, items keep their own width;
  `hero`/`multi_browse` — items automatically resize and snap into
  place), the real `_item_width` interpolation formula (position is a
  continuous animated float, so an item promoted from medium to large
  grows *as it travels*, not on arrival), and real dimension constants
  (some MD3-sourced, some pyCopper's own honest unsourced choices —
  `MEDIUM=112dp`, `HEIGHT=160dp`, `DRAG_INDEX_THRESHOLD=60px`).
- Confirmed with the user via `AskUserQuestion` before starting, given
  the real scope: "full real MD3 carousel" over a scoped-down v1.
- Investigated the step's own hardest open question directly, before
  writing any code: does TRE's `Animated<T>`/`Tree::tick_all` design
  support an animated value that also invalidates *layout*, not just
  paint? Found this already works for free — `tick_all` already sets
  `Tree.dirty` whenever any animation is active, and `app.rs`'s own
  per-frame loop already calls `compute_layout` unconditionally on a
  dirty frame. No new central-ticking mechanism needed at all, unlike
  pyCopper's own framework, which needed an explicit
  `invalidates="layout"` opt-in.
- Investigated real wheel/drag precedent, found both already reusable
  rather than needing to be built from scratch: `InputEvent::Scroll`'s
  own dispatch arm already hit-tests and walks the parent chain to the
  nearest scrollable ancestor (M8 Phase 3, for `VirtualList`); `Tree::
  dispatch`'s own real `self.dragging` mechanism (`Splitter`/`Slider`)
  is a real, generic press/move/release drag pattern. Both widened with
  a `NodeKind::Carousel` branch.
- Designed real item positioning: taffy has no "measure my children
  after my own size is known" hook the way pyCopper's own custom
  `perform_layout` does. Chose real, precedented `Position::Absolute`
  insets (`open_overlay`/`add_rect`'s own real shape), computed by hand
  every layout pass exactly like pyCopper's own manual `positions`/
  `shift` math — a real, deliberate design choice: both paint and
  hit-testing read the identical real `Layout::location`, unlike
  `VirtualList`'s own scroll offset (composed only at paint time, so
  its own hit-testing never actually accounts for it). One extra
  `compute_layout` pass per frame while a carousel exists is genuinely
  unavoidable with taffy's single-pass API — a real, stated v1 cost.
- Implemented `CarouselState`/`CarouselLayout` in `engine-core::
  node.rs`, plus shared `pub const` dimension constants (the same
  anti-drift precedent `terminal_cell_size` established).
- Implemented `Tree::sync_carousel_layouts` (called from `compute_
  layout`, real per-frame item-geometry sync), `set_carousel_index`/
  `set_carousel_scroll`, `carousel_on_wheel`/`update_carousel_drag`,
  wired into `tick_all` (position needs real central ticking, the
  identical `thumb_position` precedent) and `dispatch`'s
  `PointerPressed`/`PointerMoved`/`PointerReleased`/`Scroll` arms.
- Implemented the `engine-render` paint arm: background/corner radius
  via the existing universal path, a real clip (MD3's own real
  `CLIPS_CHILDREN` anatomy) — no paint-time translation needed, per the
  positioning design above.
- Implemented `Window.add_carousel` and `Node.set_carousel_index`/
  `get_carousel_index`/`get_carousel_position`/`set_carousel_scroll`/
  `get_carousel_scroll` in `engine-py` — dedicated typed accessors, the
  established `get_checked`/`get_selected` precedent, not the generic
  `Node.animate`/`get` (index/position aren't universal `f64` fields).
- Full Rust verification chain green on the first pass: `cargo check`/
  `clippy -D warnings`/`fmt --check`/`cargo test --release` all clean.
- Wrote 5 real Rust unit tests: hero items at rest match the real
  `_item_width` formula; a real halfway-ticked snap proves items resize
  continuously mid-travel, not on arrival; a wheel notch landing on an
  item (not the carousel's own body) snaps via the real parent-walk; a
  real drag crossing the threshold commits exactly one index and
  release clears its own per-gesture bookkeeping; Uncontained keeps
  each item's own real width and clamps scroll to its real content
  extent. All 5 passed on the first run (`engine-core` 163, up from
  158).
- Rebuilt the Python extension. **Ran a real, direct empirical
  end-to-end script before writing any pytest suite**: a real wheel
  notch synchronously moves the destination index (no tick needed); a
  real `App.run()` genuinely ticks `position` toward it across real
  frames; real Uncontained scroll clamps to its own real content
  extent. All passed.
- Wrote `tests/test_carousel.py` — checked for a filename collision
  first. **A real, confirmed pytest-suite hazard found live, not
  predicted in advance:** a first draft's own `App().run(max_frames=
  30)` call (added to prove `position` ticks across real frames) made
  `test_terminal.py`'s own real shell-response test fail every time it
  ran afterward in the same pytest process — reproduced down to just
  those two tests. Root cause: a second real event-loop invocation
  within one process, the identical "not a supported, tested pattern"
  fragility `test_terminal.py`'s own module doc comment already flags
  for calling `App.run()` twice on the *same* `App`. Removed rather
  than worked around — the real "does a tick actually move it" claim
  is already proven for real at the Rust level (the halfway-ticked
  test above), matching the identical division of labor
  `test_checkbox.py`'s own doc comment already establishes.
- Wrote `examples/carousel.py` — also checked for a filename collision
  first. Clean on the first run: a real wheel notch snaps the hero
  strip, real Uncontained scroll clamps correctly, a real `App.run()`
  ticks the snap across real frames.
- Full verification: `cargo check`/`clippy -D warnings`/`fmt --check`
  clean, `cargo test --release` (`engine-core` 163, up from 158),
  `maturin develop --release`, `pytest tests/` (494 passed, 1 skipped,
  up from 482 — 12 new, zero regressions after removing the real
  cross-test `App.run()` hazard above), all 67 examples (including the
  new `examples/carousel.py`) and the showcase demo re-run clean,
  `mypy --strict` clean against `examples/carousel.py`.
- Updated `BUILD_TRACKER.md` (Top Metrics row now 100%, Step 5 and
  Step 6 both marked done, Phase 9 heading ✅, closing M30 itself, all
  10 phases) — verified the parser's own reported item count
  before/after (191, unchanged, since no bullets were added or
  removed, only existing ones filled in), regenerated and republished
  the Build Tracker artifact at
  https://claude.ai/artifact/CaPkWjpd91oR7YFbcqC9ty.
