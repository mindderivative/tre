# LOG — M38 Phase 5: Button Group Per-Child Shape Change on Press

- User's own explicit instruction: "Let's knock out the known gaps"
  -- M38's own fifth phase. The milestone's own scoping note already
  named the real mechanism to reuse: "the existing shape-morph
  machinery (`PaintProperties.shape`, M7 Phase 4) and the already-
  tracked `Tree.hovered`/`Tree.pressed` live interaction state" --
  and, following M38 Phase 4, `interactive_shape`'s own real pattern
  to mirror was now already built and proven.
- Real spec-value research first: the M3 site's own buttons spec page
  (`m3.material.io/components/buttons/specs`) is JS-rendered, the
  identical "no fetchable static content" finding M38 Phase 4's own
  research already made for Split Button. Found the real per-size
  pressed-corner-radius table instead via a real, cited open-source
  MD3 Expressive implementation (`callstack/react-native-paper` PR
  #5097): 8dp for XS/S, 12dp for M, 16dp for L/XL -- corroborated by
  a separate real search confirming the general MD3 Expressive
  concept directly ("buttons can morph to become more square" on
  press, "the corner radius value differs for each button size").
- **Real scope narrowing, stated directly, not silently dropped:**
  `BUILD_TRACKER.md`'s own Phase 5 title said "Per-Child Shape Change
  on Press/Select" -- but Standard Button Group has no real selection
  concept anywhere in its own MD3 anatomy (it's a row of independent
  action buttons, not a segmented/choice control; `add_button_group`'s
  own existing doc comment already states this exact real distinction
  -- Segmented Button, M30 Phase 6, is the real component with
  selection). Scoped to press only, matching what's genuinely real
  rather than inventing a selection mechanism nothing in this
  component's own spec calls for.
- Mapped the real 8/12/16dp table onto this codebase's own literal
  `height: f32` parameter (no discrete size-class enum anywhere in
  this catalog, `add_button`/`add_split_button`'s own established
  convention, confirmed by direct read before reusing) using MD3's
  own real published button-size scale (XS 32dp/S 36dp/M 40dp/L 48dp/
  XL 56dp, found via a separate real search) -- new `button_group_
  pressed_corner_radius(height)` places the real tier boundaries at
  the honest midpoints (38dp between S/M, 44dp between M/L) rather
  than guessing arbitrary cutoffs.
- New `PaintProperties.press_interactive_shape: Option<(ShapeKey,
  ShapeKey)>` (`node.rs`) -- deliberately a *separate* field from M38
  Phase 4's own `interactive_shape`, not one field reacting to both
  `hovered`/`pressed`: a real Button Group child must not visually
  tighten on a mere hover (only a genuine press, the opposite real
  trigger from Split Button's own inner corners), so sharing one field
  between the two real components would have made Button Group
  children incorrectly react to hover too.
- New `Tree::set_pressed` (`tree.rs`) -- the single real chokepoint
  every one of the 5 real `self.pressed` mutation sites across
  `dispatch` now goes through (`dismiss_overlays_outside`'s own
  early-return, `press_blocked_by_modal_overlay`'s own early-return,
  the real hit/no-hit press assignment, and `PointerReleased`'s own
  clear), refactored in place rather than duplicated -- mirrors
  `update_hover`'s own exact shape-retarget shape, keyed on `pressed`/
  `press_interactive_shape` instead of `hovered`/`interactive_shape`.
  Compares by `NodeId` alone (not the full `(button, node)` pair): a
  same-node press with a genuinely different button must not needlessly
  restart the shape animation. Reused `config.hover_duration` for the
  transition (no dedicated press-shape duration added) -- the same
  real, minimal-new-surface choice Phase 4 already made for its own
  hover-driven case, not overengineering a separate knob nothing else
  needs yet.
  Ran the full `engine-core` test suite immediately after this
  refactor alone, before adding any new real feature, specifically to
  confirm zero behavioral regression from touching a fairly central
  dispatch mechanism used by drag/slider/splitter/carousel/context-
  menu/ripple-spawn logic -- all 189 pre-existing tests passed
  unmodified on the first run.
- `add_button_group` (`engine-py/src/window_factory.rs`): built the
  two real uniform `ShapeKey`s (relaxed full pill, tightened to
  `button_group_pressed_corner_radius(height)`) once outside the
  child-construction loop (every real child shares the identical
  `width`/`height`, real MD3 anatomy already established, M35 Phase
  3's own doc comment), then set on each child right after `add_
  button` constructs it -- `shape` initialized directly to the
  *relaxed* `ShapeKey` (not left at `PaintProperties::new`'s own
  `ShapeKey::empty()` default), the identical "avoid a first-press
  flash-from-empty bug" discipline Phase 4 already established for
  Split Button.
- New `tree.rs` test, `set_pressed_retargets_a_real_press_interactive_
  shape_toward_tightened_then_relaxed`: mirrors Phase 4's own `update_
  hover_retargets_a_real_interactive_shape_toward_tightened_then_
  relaxed` exactly, calling the private `set_pressed` method directly
  (valid since `mod tests` is a child module of the one that declares
  it, the same real access this file's other direct-state tests
  already use) rather than routing through a full `dispatch` event.
  Passed on the first run.
- Real, honest verification-surface check, the same discipline M38
  Phase 2/3/4 already established: `window_factory.rs` has no Rust
  unit-test module at all anywhere in this codebase (confirmed by
  direct grep before assuming one should exist) -- `engine-py`'s own
  factory functions are tested exclusively at the Python/pytest FFI
  level, its own established convention, so `button_group_pressed_
  corner_radius` gets no dedicated Rust test (a trivial, deterministic
  pure function, consistent with that convention). Two new pytest
  tests in `tests/test_button_group.py` instead, reusing `Window.click`
  (a real primary press+release, exercising `set_pressed`'s own new
  shape retarget on both the press and release transitions in one
  call) for the default variant and, separately, the `"outlined"`
  variant -- the same real border-plus-shape-morph combination that
  surfaced a genuine bug for Split Button in Phase 4, re-checked here
  since Button Group reuses `add_button` (and therefore the identical
  border-stroke code path) directly. Both passed on the first run.
- Corrected `python/tre/_core.pyi`'s own `add_button_group` docstring
  to mention the real new press-driven reshape behavior alongside the
  existing width-reflow description.
- Full verification: `cargo check --workspace --all-targets`/`cargo
  clippy --workspace --all-targets -D warnings`/`cargo fmt --check`
  clean; `cargo test --workspace --release` clean (`engine-core` 190
  passed, up from 189, +1 new test, zero regressions from the `set_
  pressed` refactor); `maturin develop --release` rebuilt; `pytest
  tests/` 562 passed/1 skipped, up from 561, +1 new test; all 75
  examples and the showcase demo re-run clean.
- Updated `BUILD_TRACKER.md`: Phase 5 flipped `⬜` -> `✅` with a
  terse step-bullet note; milestone status line and Top Metrics row
  updated to "Phase 5 of 7 done" / 71%. Verified the parser's own
  reported item count unchanged before/after (38/122/212 both times).
  Regenerated and republished the Build Tracker artifact.
  **This closes M38 Phase 5. M38 itself remains open -- 2 phases
  remain (a real `ScrollView` scrollbar thumb, real scroll+clip for
  Code Editor with caret-follow).**
