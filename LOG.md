# LOG — M38 Phase 7: Real Scroll+Clip for Code Editor (closes M38)

- User's own explicit instruction: "Let's knock out the known gaps"
  -- M38's own seventh and final phase, the one its own scoping note
  already flagged as the most novel and most likely to need its own
  design pause.
- Investigated the real current state before writing any code:
  `add_code_editor` (`engine-py/src/window_factory.rs`) is a plain
  `NodeKind::TextField`, fixed `width`/`height`, `multiline = true`.
  Direct read of `NodeKind::TextField`'s own paint arm found zero clip
  logic anywhere -- overflowing content simply painted past the node's
  own box, a real, previously-existing gap.
- Paused via `AskUserQuestion` before writing code: the milestone's
  own original Phase 7 scoping note preferred composing the already-
  built `ScrollView` around `TextField`/Code Editor. Investigated
  whether that's actually buildable: `ScrollView`'s own child
  measurement needs the wrapped child to report a real, unbounded
  intrinsic content height to `taffy` -- a real `taffy` measure-
  function integration. Grepped the whole codebase for any existing
  `MeasureFunc`/`with_measure` usage first: none exists anywhere. This
  is genuinely new, unproven capability, not a straightforward
  composition. Presented two real options -- build the `taffy`
  measure-function integration (matches the original scoping note
  literally, reuses Phase 6's own new scrollbar thumb for free once
  working, but unknown real scope) vs. a dedicated `TextFieldState`
  scroll mechanism (self-contained, no `taffy` changes, mirrors
  `VirtualList`/`Carousel`'s own already-established "per-`NodeKind`
  scroll, not `ScrollView` reuse" pattern). User chose the dedicated
  mechanism.
- New `TextFieldState.scroll_offset: Animated<f64>` (`node.rs`) -- a
  real, direct-write pixel scroll (not through `animate_field`),
  mirroring `ScrollViewState.scroll`/`VirtualListState.scroll_offset`'s
  own already-established real precedent for per-`NodeKind` scroll
  values. **Real downstream break found and fixed:** `TextFieldState`
  could no longer derive `Clone`/`Debug`/`PartialEq` once it gained a
  real `Animated<f64>` field (the identical real constraint `Scroll
  ViewState`/`Splitter`/`Icon` already carry) -- `cargo check` caught
  one real, live consumer: `engine-py::app.rs`'s own `text_field_hit_
  offset` (the real click-to-position helper) cloned `state` out of a
  `RefCell` borrow specifically to escape that borrow's own lifetime.
  Fixed by holding the borrow for the whole helper function instead --
  confirmed both real call sites never held a conflicting borrow
  across the call (each already scoped its own prior borrow to a
  single statement before calling this helper).
- New `Tree::scroll_text_field_caret_into_view` (`tree.rs`) -- if the
  caret's own real line would currently sit outside the visible
  viewport, scrolls just enough to reveal it (scrolls up to `caret_
  top` if the caret is above the viewport, down to `caret_bottom -
  viewport_height` if below), clamped to `[0, max_scroll]`. **Real,
  honest v1 approximation, stated directly:** the real per-line pixel
  height used for this decision (`font_size * 1.35`) is an estimate,
  not an exact measured value -- `engine-core` has no font-shaping
  access to measure one exactly (§4's crate-boundary rule: only
  `engine-render` loads fonts). The real, cited source for `1.35`:
  VS Code's own documented default `editor.lineHeight` behavior (`0`
  means "compute from `fontSize`", real default multiplier `1.35` on
  non-macOS, confirmed via a real web search, not guessed). This
  imprecision is a real, acceptable heuristic limitation, not a
  correctness bug: whatever `scroll_offset` this computes drives both
  the real clip layer and the real glyph y-shift at paint time
  identically, so the actually-*painted* result stays internally
  consistent regardless of how precisely the heuristic estimated the
  ideal scroll target.
  Wired into the same four real cursor-moving dispatch chokepoints
  Phase 2/3's own `goal_column` reset already established: the
  `dispatch_text_field_key` caller site in `dispatch` (guaranteed to
  run regardless of which of that method's own ~15 individual early-
  return arms actually fired, rather than auditing and touching each
  one individually), `set_text_field_cursor` (mouse click), `extend_
  text_field_selection` (drag-select), and the `InputEvent::TextInput`
  dispatch arm (typing/IME commit).
- Real clip + scroll wired into `engine-render`'s own `NodeKind::
  TextField` paint arm: a real `scene.push_layer`/`pop_layer` clip
  when `state.multiline`, `TextPlacement.y = -state.scroll_offset.
  current` (`0.0` for every existing single-line field, byte-for-byte
  unchanged). Clip scoped to `multiline` only -- a single-line field's
  own real horizontal-overflow behavior is untouched, a real,
  deliberate v1 scope match to this phase's own "Code Editor" title.
- **Real, previously-uncached tessellation found and fixed along the
  way, the same "verify before trusting a prior write-up" discipline
  M38 Phase 1's own Terminal correction already established:** direct
  source read (done specifically to find the right insertion point for
  the new clip layer) found `TextField`'s own box fill was *still* a
  fresh, uncached `RoundedRect::to_path` call every frame -- despite
  M38 Phase 1's own completion note in this very file explicitly
  claiming "TextField's own box fill... routed through rounded_rect_
  fill too." That claim was wrong; caught here by checking the actual
  current source before writing this phase's own note, not by
  trusting the earlier one. Fixed: routed through `GeometryCache::
  rounded_rect_fill(id, w, h, radius)`, the identical real cache
  `Checkbox`/`Switch`/`Terminal` already use, and reused directly for
  this same node's own new clip layer (the same params, so the second
  call is a guaranteed real cache hit, not a second tessellation).
- Real tests: three new `tree.rs` unit tests for `scroll_text_field_
  caret_into_view`, each hand-computed against the real 1.35 ratio
  before running (a 20-line, 200x100px, `font_size = 14.0` scene where
  each real line is exactly 6 bytes, so a target line's own real byte
  offset is `line_index * 6`, and `line_height = 18.9`):
  `scroll_text_field_caret_into_view_scrolls_down_to_reveal_a_caret_
  below_the_viewport` (jump to line 5, expect scroll = 13.4), `..._
  scrolls_back_up_to_reveal_a_caret_above_the_viewport` (jump to line
  15 then back to line 0, expect scroll returns to exactly 0.0), `..._
  is_a_true_no_op_for_a_single_line_field`. All three passed on the
  first run, confirming the hand-calculated math matched the real
  implementation exactly.
- Two new pixel-level tests added to `crates/engine-render/tests/
  text_field_paint.rs` (this project's own established real headless-
  GPU-render-and-readback discipline for paint-only claims):
  `a_genuinely_overflowing_multiline_field_clips_its_own_content_to_
  its_own_box` (20 real lines in a 40px box -- the very bottom row
  must show only the field's own plain fill color, `[0xEE, 0xEE,
  0xEE, 0xFF]`, proving a real clip, not just running out of box to
  paint in) and `a_nonzero_scroll_offset_paints_genuinely_different_
  pixels_than_unscrolled` (a real whole-buffer diff between `scroll_
  offset = 0.0` and `60.0`, mirroring the existing `a_folded_range_
  paints_genuinely_different_pixels_than_unfolded`'s own exact
  technique). Both passed on the first run.
- Real, honest verification-surface check: no Python getter exists
  for `scroll_offset` itself (checked `python/tre/_core.pyi` first,
  same discipline this whole milestone already established), so one
  new pytest test in `tests/test_code_editor.py`, `test_navigating_
  and_editing_still_works_correctly_in_a_genuinely_overflowing_
  editor`, proves the real, full FFI surface (construct a 30-line
  editor in an 80px box, navigate 40 real `ArrowUp`s well past the
  visible viewport back to line 0, type a marker, confirm it landed
  on the real correct line; then a real click after all that
  scrolling still resolves to a real, valid position -- confirmed by
  content length growing by exactly one character, not by predicting
  *which* line a center-click lands on, since that depends on exact
  real font metrics this test can't compute in advance). Passed on
  the first run.
- Corrected the now-stale doc comments that explicitly named this gap
  as still open: `add_code_editor`'s own Rust doc comment (`window_
  factory.rs`, the real "no real vertical/horizontal scroll+clip...
  content past the box's own edges simply isn't visible" v1-limit
  paragraph, replaced with the real M38 Phase 7 design note and an
  honest note that horizontal scroll specifically remains a real,
  separate, still-open v1 limit) and its `python/tre/_core.pyi` stub.
- Full verification: `cargo check --workspace --all-targets`/`cargo
  clippy --workspace --all-targets -D warnings`/`cargo fmt --check`
  clean; `cargo test --workspace --release` clean (`engine-core` 196
  passed, up from 193, +3 new tests; `engine-render`'s own `text_
  field_paint` integration suite 15 passed, up from 13, +2 new tests;
  zero regressions anywhere, including from the `TextFieldState`
  derive removal and its one real downstream fix); `maturin develop
  --release` rebuilt; `pytest tests/` 563 passed/1 skipped, up from
  562, +1 new test; all 75 examples and the showcase demo re-run
  clean.
- Updated `BUILD_TRACKER.md`: Phase 7 flipped `⬜` -> `✅`, **closing
  M38 entirely (all 7 phases)** -- the milestone's own status line
  changed to "✅ Complete", the Top Metrics table row updated to
  100%, and a full "Just closed" trailer added at the top of the file
  (this milestone's own established convention for a fully-closed
  milestone), summarizing the real, full seven-phase arc: Phase 1
  (tessellated-path caching extension, plus two real discoveries
  beyond the original scope), Phase 2 (goal-column memory), Phase 3
  (fold-aware cursor navigation), Phase 4 (Split Button inner-corner
  shape-tightening, paused via `AskUserQuestion`, plus a real border/
  per-corner-geometry bug fixed along the way), Phase 5 (Button Group
  per-child press shape morph, reusing Phase 4's own new mechanism),
  Phase 6 (a real `ScrollView` scrollbar thumb, ported directly from
  pyCopper), Phase 7 (this phase). Verified the parser's own reported
  item count unchanged before/after (38/122/212 both times).
  Regenerated and republished the Build Tracker artifact.
  **This closes M38 Phase 7 and M38 itself entirely -- all seven of
  the real, previously-open gaps this milestone's own investigation
  found are now closed.**
