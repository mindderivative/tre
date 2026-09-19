# LOG — M39 Phase 1: Code Editor Horizontal Scroll

- User's own explicit instruction: "Scope 2, 1, 3, 4, and 5" (after
  being asked "are there any gaps or fixes left?"), then "Start" --
  M39's own first phase, in the user's own explicitly-specified order
  (item 2 from the gap-sweep answer: Code Editor horizontal scroll).
- Confirmed the real gap directly before writing any code:
  `engine-render::text::field_max_width` returns `f32::MAX` for every
  `multiline` field -- no real line ever wraps, it simply extends
  right, clipped since M38 Phase 7 but not scrollable until now.
- New `TextFieldState.horizontal_scroll_offset: Animated<f64>`
  (`node.rs`) -- the identical real contract `scroll_offset`'s own
  vertical field already establishes (driven directly, never eased,
  `0.0` a true no-op for every existing field).
- Extended `Tree::scroll_text_field_caret_into_view` (`tree.rs`) with
  the identical real "scroll just enough to reveal the caret" logic
  along the horizontal axis: real caret column via the already-built
  `Self::real_column` helper (M38 Phase 2), a real character-width
  estimate (`font_size * 0.6`), the real longest-line character count
  (`content.split('\n').map(|l| l.chars().count()).max()`) for the
  real `max_h_scroll` clamp. New `Tree::CODE_EDITOR_CHAR_WIDTH_RATIO`
  constant -- **not an external citation this time:** this codebase's
  own real historical precedent, found via `git log -p` on `engine-
  render/src/text.rs` (predating M32 Phase 1's switch to real
  measured `monospace_cell_size`), already used exactly `0.6` for
  `Terminal`'s own pre-real-font-metrics analytic cell-width estimate
  -- reused verbatim rather than re-deriving or citing an external
  source, since this project's own prior, real, considered choice is
  the more directly applicable precedent.
- Wired `horizontal_scroll_offset` into `engine-render`'s own
  `NodeKind::TextField` paint arm: `TextPlacement.x = -state.
  horizontal_scroll_offset.current` (mirroring `scroll_offset`'s own
  `y` shift). The existing multiline clip layer from M38 Phase 7
  already bounds both axes as a plain rectangular clip -- no clip
  logic changes needed at all, only the new offset.
- **Real, latent test-infrastructure bug found and fixed along the
  way, the same "verify, don't assume" discipline this whole project
  already established repeatedly:** while debugging a genuinely
  failing new horizontal-scroll unit test (got `176.4` where `76.4`
  was expected), added a temporary debug `eprintln!` of the real
  computed `Layout` and found `size.width` was `0.0` despite the
  field's own `Style` explicitly requesting `100.0`. Root-caused via
  direct read of `taffy = "0.14.0"`'s own real source: `Style::
  default()`'s own `display` is `Display::DEFAULT` = `Display::Flex`,
  `flex_direction: FlexDirection::Row`, `flex_shrink: 1.0` -- the test
  scene wrapped the field inside a separate `Container` root built via
  `leaf(0.0, 0.0)`, an explicit *zero-width* `Style`. In a flex-row
  layout with zero available main-axis (width) space, a shrinkable
  child (the default) genuinely shrinks down to fit, even though its
  own preferred width was `100.0`. Height survived only because it's
  the *cross* axis in a row-direction flex container, where an
  explicit (non-`Auto`) child size is honored directly rather than
  stretched to the container's own cross size -- this asymmetry is
  exactly why the bug went unnoticed for two whole milestones (M38
  Phase 7's own vertical-only tests never read `.size.width`).
  Confirmed the identical bug existed in the pre-existing, already-
  passing vertical `caret_follow_scene()` too (same `leaf(0.0, 0.0)`
  wrapping-root pattern) -- fixed both scene helpers the same way:
  the field is now its own real `compute_layout` root, mirroring
  `scrollable_view`'s own already-correct pattern (M38 Phase 6),
  which sidesteps the whole issue since a top-level root's own size
  comes directly from the `available_space` argument, no flex
  algorithm involved. Re-ran the full pre-existing `engine-core` test
  suite immediately after this fix alone to confirm zero behavioral
  regression from touching two shared test helpers -- all pre-existing
  tests passed unmodified.
- New tests: `scroll_text_field_caret_into_view_scrolls_right_to_
  reveal_a_caret_past_the_viewport` and `..._scrolls_back_left_to_
  reveal_a_caret_before_the_viewport` (`tree.rs`), hand-computed
  against the real `0.6` ratio before running (a 100px-wide field, a
  30-character real line, `char_width = 8.4`: column 20 -> h-scroll
  `76.4`; back to column 0 -> h-scroll exactly `0.0`). Both passed on
  the first run once the scene-helper fix landed.
- New pixel-level test in `crates/engine-render/tests/text_field_
  paint.rs`, `a_nonzero_horizontal_scroll_offset_paints_genuinely_
  different_pixels_than_unscrolled` -- mirrors M38 Phase 7's own
  vertical whole-buffer-diff test exactly, just along the horizontal
  axis (a real 40-character line, far wider than a real 100px box).
  Passed on the first run.
- Real, honest verification-surface check: no Python getter exists
  for `horizontal_scroll_offset` itself (checked `_core.pyi` first,
  the same discipline already established repeatedly this whole
  project). One new pytest test, `test_navigating_horizontally_
  still_works_correctly_in_a_genuinely_overflowing_line` (`tests/
  test_code_editor.py`), mirroring M38 Phase 7's own vertical overflow
  test: a real 60-character single line in a 100px-wide box, 60 real
  `ArrowLeft`s walking well past the visible viewport back to column
  0, typing a marker, confirming it landed at the real correct column.
  Passed on the first run.
- Corrected the two stale doc comments claiming horizontal scroll was
  still a real, separate open v1 limit: `add_code_editor`'s own Rust
  doc comment (`window_factory.rs`, the M38 Phase 7 paragraph that
  explicitly named this exact gap) and its `python/tre/_core.pyi`
  stub.
- Full verification: `cargo check --workspace --all-targets`/`cargo
  clippy --workspace --all-targets -D warnings`/`cargo fmt --check`
  clean; `cargo test --workspace --release` clean (`engine-core` 198
  passed, up from 196, +2 new tests; `engine-render`'s own `text_
  field_paint` integration suite 16 passed, up from 15, +1 new test;
  zero regressions, including from the two scene-helper fixes);
  `maturin develop --release` rebuilt; `pytest tests/` 564 passed/1
  skipped, up from 563, +1 new test; all 75 examples and the showcase
  demo re-run clean.
- Updated `BUILD_TRACKER.md`: Phase 1 flipped `⬜` -> `✅` with a
  terse step-bullet note; milestone status line and Top Metrics row
  updated to "Phase 1 of 5 done" / 20%. Verified the parser's own
  reported item count unchanged before/after (39/127/218 both times).
  Regenerated and republished the Build Tracker artifact.
  **This closes M39 Phase 1. M39 itself remains open -- 4 phases
  remain (Loading Indicator + Time Picker Dial, shape-morphed border
  inset fix, Terminal cell text attributes, `Tree::tick_all` active-
  set optimization).**
