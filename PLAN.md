# PLAN — M38 Phase 5: Button Group Per-Child Shape Change on Press

## Goal
Add real MD3 Expressive "buttons reshape as you press them" to
Standard Button Group's own children -- each of a group's real
buttons should morph from a fully-round pill down to a real, per-size
"square" corner radius while genuinely pressed, alongside the existing
width-reflow mechanic (M35 Phase 3), then relax back on release.

## Steps
1. Real spec-value research (the M3 buttons spec page is JS-rendered,
   no fetchable static content, the identical real finding M38 Phase
   4's own Split Button research already made): found the real per-
   size pressed-corner-radius table via a real, cited open-source MD3
   Expressive implementation instead (`callstack/react-native-paper`
   PR #5097): 8dp for XS/S, 12dp for M, 16dp for L/XL. Mapped onto
   this codebase's own literal `height` parameter (no size-class enum,
   matching `add_button`/`add_split_button`'s own established
   convention) using MD3's real published button-size scale (XS 32dp/
   S 36dp/M 40dp/L 48dp/XL 56dp), placing tier boundaries at the
   honest midpoints (38dp, 44dp) between adjacent tiers.
2. **Real scope narrowing, stated directly:** `BUILD_TRACKER.md`'s own
   Phase 5 title said "press/select" -- Standard Button Group has no
   real selection concept in MD3 anatomy at all (it's a row of
   independent action buttons, not a segmented/choice control; that's
   `Segmented Button`'s own real anatomy, already built, M30 Phase 6).
   Scoped to press only, matching what's actually real.
3. New `PaintProperties.press_interactive_shape: Option<(ShapeKey,
   ShapeKey)>` (`node.rs`) -- deliberately a *separate* field from M38
   Phase 4's own `interactive_shape`, not a shared one reacting to
   both `hovered`/`pressed`: a Button Group child must not tighten on
   a mere hover, only a genuine press, the opposite real trigger Split
   Button's own inner corners need.
4. New `Tree::set_pressed` (`tree.rs`) -- the single real chokepoint
   every one of the 5 real `self.pressed` mutation sites across
   `dispatch` now goes through (refactored, not duplicated), mirroring
   `update_hover`'s own exact shape-retarget pattern for `shape`/
   `interactive_shape`. Ran the full `engine-core` test suite
   immediately after this refactor alone (before adding any new real
   feature) to confirm zero behavioral regression from touching a
   fairly central dispatch mechanism -- all 189 pre-existing tests
   passed unmodified.
5. `add_button_group` (`engine-py::window_factory.rs`): new
   `button_group_pressed_corner_radius(height)` pure function (the
   real tier table above); every child's own `shape`/`press_
   interactive_shape` set right after `add_button` constructs it (the
   same "initialize shape non-empty from construction, not left at
   `ShapeKey::empty()`" discipline Phase 4 already established, to
   avoid a first-press flash-from-empty bug).
6. Real tests: `tree.rs` gained a direct `set_pressed` retarget test
   (mirrors Phase 4's own `update_hover` test exactly, calling the
   private method directly since `mod tests` is a child module).
   `window_factory.rs` has no Rust unit-test module (this codebase's
   own established convention: `engine-py` factory functions are
   tested at the Python/pytest FFI level, not with Rust unit tests) --
   two new pytest tests instead, reusing `Window.click` (a real
   primary press+release) to exercise the full dispatch-through-paint
   path for both the default and `"outlined"` variants.
7. Full verification chain: `cargo check`/`clippy -D warnings`/`fmt`/
   `test --workspace --release`, `maturin develop --release`, full
   `pytest tests/`, all 75 examples, showcase demo.
8. `BUILD_TRACKER.md` Phase 5 flipped to done, Top Metrics updated to
   5-of-7, artifact regenerated (38/122/212, unchanged) and republished.

## Status
Complete. Full verification chain green (`cargo test --workspace
--release`: `engine-core` 190 passed (+1); `pytest tests/`: 562
passed/1 skipped, up from 561, +1 new test; all 75 examples + showcase
demo clean). **M38 Phase 5 -- Button Group Per-Child Shape Change is
now complete. M38 itself remains open: 2 phases remain (ScrollView
scrollbar thumb, real scroll+clip for Code Editor).**
