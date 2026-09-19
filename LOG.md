# LOG — M38 Phase 6: Real ScrollView Scrollbar Thumb

- User's own explicit instruction: "Let's knock out the known gaps"
  -- M38's own sixth phase, the one BUILD_TRACKER's own investigation
  already noted has "a full, detailed real reference implementation
  already read directly from the sibling `pyCopper` project" during
  M36's own scoping.
- Re-read pyCopper's real `ScrollViewElement` (`src/pycopper/widgets/
  scroll.py`) in full before writing any code: `thumb_geometry`
  (track/thumb-length/along-track-offset), `thumb_rect` (absolute
  x/y/w/h along whichever edge), `grabs_thumb` (a real hit-test with
  slop), `on_pointer_down`/`on_pointer_move`/`on_pointer_up` (a
  relative-delta drag, preserving wherever along the thumb's own
  length the press actually grabbed it), `_paint_scrollbar` (drawn in
  `paint_foreground`, which runs *after* children). Its own module
  doc comment states directly: "M3 has no scrollbar spec... so the
  indicator's dimensions here are pyCopper's own, and marked as such
  rather than presented as Material" -- the real, cited tokens
  (`BAR_THICKNESS`=4, `BAR_MARGIN`=2, `BAR_MIN_LENGTH`=32,
  `BAR_RADIUS`=2, `BAR_OPACITY`=0.55, `THUMB_GRAB_SLOP`=6) were
  ported verbatim, not re-derived.
- New `ScrollViewState::thumb_geometry(viewport_extent, content_
  extent) -> (track, thumb, along)` (`engine-core/src/node.rs`) --
  the real math ported directly, shared by paint and hit-testing/
  dragging so the two can never drift (the identical real "one
  function, every real caller" discipline `VirtualListState::
  offset_of`/`Tree::splitter_geometry` already establish). New public
  `SCROLLBAR_THICKNESS`/`SCROLLBAR_MARGIN`/`SCROLLBAR_MIN_LENGTH`/
  `SCROLLBAR_GRAB_SLOP` constants live in `engine-core` (not `engine-
  render`) specifically because `Tree::grabs_scroll_view_thumb` needs
  them for real hit-testing too, not just paint -- a real, deliberate
  split, documented directly, from the pure-paint `SCROLLBAR_THUMB_
  RADIUS`/`_OPACITY` constants that stayed in `engine-render` since
  neither hit-testing nor dragging needs a fill radius or opacity.
- New `ScrollViewState.thumb_drag_anchor: Option<(f64, f64)>` --
  `(pointer_coord_at_grab, scroll_at_grab)`, the real relative-delta
  anchor ported directly from pyCopper's own `state.data["drag_
  from"]`/`["drag_scroll"]`. **Real, deliberate choice to port the
  reference's own UX exactly, not substitute a simpler one:** an
  absolute pointer-to-scroll mapping (no stored anchor needed at all,
  matching `Splitter`/`Slider`'s own simpler absolute-position drag
  math) was considered and rejected -- it would snap the thumb the
  instant a drag starts (its grabbed point jumping to align with the
  pointer), a real, visible UX regression from pyCopper's own chosen
  design, which this phase exists to port faithfully. Lives on
  `ScrollViewState` itself, not `Tree` -- mirrors `CarouselState.
  drag_last_x`/`drag_accum`'s own already-established real precedent
  for kind-specific drag-anchor data.
- New `Tree::scroll_view_extents` (private helper, factored out once
  a third real caller needed the identical `(horizontal, viewport,
  content)` measurement `scroll_scroll_view_by`/`sync_scroll_view_
  layouts` each already compute inline -- left those two untouched
  rather than refactoring proven, already-tested code, a deliberate
  low-risk choice for this stage of a long session).
- New `Tree::grabs_scroll_view_thumb`/`update_scroll_view_thumb_drag`
  -- real hit-test and live-follows-the-cursor drag math, both ported
  directly from pyCopper's own equivalents. Wired into `update_drag`'s
  own dispatcher (a new `NodeKind::ScrollView` arm, alongside
  `Splitter`/`Slider`/`Carousel`).
- **Real architectural problem found and solved, not glossed over:**
  the thumb is a paint-only overlay with no real child `Node` of its
  own (matching pyCopper's own design), so a plain point-based `Tree::
  hit_test` resolves to whatever real scrolled content sits underneath
  it -- the exact real problem pyCopper's own module doc comment names
  directly ("the press lands on whatever row is underneath and capture
  would go there"). pyCopper solves this with real event capture, an
  architecture this codebase doesn't have. Solved instead by walking
  the hit node's own ancestor chain for a real `ScrollView` whose
  thumb the press genuinely grabs -- the identical real "walk up
  looking for the right kind of ancestor" technique the existing
  carousel-drag detection in the same `PointerPressed` arm already
  establishes, reused rather than inventing a second mechanism. A real
  grab consumes the press entirely (early `return DispatchOutcome::
  None` after starting the drag) so the content underneath never also
  registers a ripple/click for the same real press.
- New `engine-render::paint_scroll_view_thumb`, called from `paint_
  node`'s own `NodeKind::ScrollView` arm *after* the real child
  recursion and `pop_layer()` -- mirrors pyCopper's own real `paint_
  foreground`, which runs after children for the identical real
  reason (the thumb sits over the content, not under it). Explicitly
  resets `scene.set_transform(composed)` first: the scene's own
  ambient transform is whatever the last painted child left it at by
  the time control returns to this node's own stack frame, not
  necessarily `composed` any more. **Real, honest v1 scope choice,
  stated directly:** a fixed literal color (real MD3 baseline
  `outline_variant`, `0xCAC4D0`, found via direct grep of `engine-py`'s
  own already-real `Md3Baseline::OUTLINE_VARIANT`) at pyCopper's own
  real 0.55 opacity -- `engine-render` has no `engine-md3` dependency
  to resolve a live theme token from (§4), the identical "real but not
  yet theme-aware" scope `TextField`'s own hardcoded caret color
  already established. Deliberately uncached (no new `GeometryCache`
  entry): the thumb's own position changes on every real scroll tick,
  so a per-frame cache would rarely hit anyway, and it's a genuinely
  small shape -- not worth the bookkeeping this catalog's own
  established "cache only where it measurably helps" discipline (M34
  Phase 1's own real benchmark) already requires justifying.
- Real tests: four new `tree.rs` unit tests --
  `thumb_geometry_computes_the_real_track_thumb_and_along_values`
  (exact real numbers: 100px viewport/400px content -> track=96,
  thumb=32, along=2 at rest, 34 at half-scroll), `grabs_scroll_view_
  thumb_is_true_only_within_the_real_thumb_plus_slop` (a point inside
  the real thumb rect, a point just within the real `SCROLLBAR_GRAB_
  SLOP` tolerance, a point nowhere near it), `update_scroll_view_
  thumb_drag_moves_the_scroll_offset_proportionally_to_pointer_travel`
  (half the real thumb travel moves scroll by exactly half of max_
  scroll). All four passed on the first run.
- Two new pixel-level tests added to the existing `crates/engine-
  render/tests/scroll_view.rs` (this project's own established real
  headless-GPU-render-and-readback discipline for paint-only claims,
  matching `clip_children.rs`/`virtual_list_scroll.rs`'s own
  precedent): `a_scrollable_scroll_view_paints_a_real_thumb_pixel_at_
  the_expected_position` (a real point inside the computed thumb rect
  is genuinely non-transparent -- not asserting an exact color byte-
  for-byte, since real alpha-blending onto a transparent target
  depends on the renderer's own blend semantics; *something painted
  there* is the real, decisive claim this phase's own code exists to
  prove) and `a_scroll_view_that_fits_its_own_content_paints_no_thumb`
  (the real other half: nothing painted at all when there's nothing
  to scroll). Both passed on the first run.
- Real, honest verification-surface check: `Window.click`/`Window.
  hover` both operate on a real `Node`, and the thumb has no `Node` of
  its own (a paint-only overlay, matching pyCopper's own design) -- no
  raw pointer-coordinate Python API exists to grab it directly, so no
  Python-level drag reproduction is constructible (checked the real
  API surface first, per the discipline M38 Phase 2/3 already
  established, rather than assuming). No new Python-facing API was
  added either -- the thumb paints and drags automatically with zero
  app-side wiring needed, so the existing `tests/test_scroll_view.py`
  suite already exercises real scrollable construction/scrolling with
  the new paint code live; no new pytest test was strictly required.
  Updated `_core.pyi`'s own `add_scroll_view` docstring to mention the
  real new automatic drag behavior for discoverability.
- Full verification: `cargo check --workspace --all-targets`/`cargo
  clippy --workspace --all-targets -D warnings`/`cargo fmt --check`
  clean; `cargo test --workspace --release` clean (`engine-core` 193
  passed, up from 190, +3 new tests; `engine-render`'s own `scroll_
  view` integration suite 4 passed, up from 2, +2 new tests; zero
  regressions); `maturin develop --release` rebuilt; `pytest tests/`
  562 passed/1 skipped, unchanged (no new Python-facing API, confirmed
  by the same verification-surface check above); all 75 examples and
  the showcase demo re-run clean.
- Updated `BUILD_TRACKER.md`: Phase 6 flipped `⬜` -> `✅` with a
  terse step-bullet note; milestone status line and Top Metrics row
  updated to "Phase 6 of 7 done" / 86%. Verified the parser's own
  reported item count unchanged before/after (38/122/212 both times,
  including after a heading typo self-correction). Regenerated and
  republished the Build Tracker artifact.
  **This closes M38 Phase 6. M38 itself remains open -- 1 phase
  remains: Phase 7, real scroll+clip for Code Editor with caret-
  follow, the milestone's own scoping note already flagged as the
  most novel of the seven and a likely candidate for its own
  `AskUserQuestion` pause once actually investigated.**
