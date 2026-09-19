# PLAN — M38 Phase 4: Split Button Inner-Corner Shape-Tightening

## Goal
Close the real, previously-stated v1 scope limit: Split Button's two
buttons always painted fully rounded, never tightening their own
facing inner corners on hover/press the way real MD3 anatomy
("the inner corners change shape for hovered, focused, and pressed
states," `COMPONENT_SPLIT_BUTTONS.md`) calls for.

## Steps
1. Real spec value research (the M3 site's own split-button spec page
   is JS-rendered, no static content to fetch): confirmed via
   `material-components-android`'s own real `docs/components/
   ButtonGroup.md` -- connected groups tighten their own inner corners
   to a real 8dp, outer corners stay fully round.
2. Paused via `AskUserQuestion` -- no existing internal precedent for
   auto-driving a shape animation from interaction state (only
   `hover_opacity`/`focus_ring`/`ripple` opacity are engine-core-auto-
   driven today; `shape: Animated<ShapeKey>`, M7 Phase 4, has only
   ever been app-driven via `Node.animate("shape", ...)`). User chose
   the real "Animated smooth morph" approach over an instant, non-
   animated `corner_radii_override` swap.
3. New `PaintProperties.interactive_shape: Option<(ShapeKey,
   ShapeKey)>` (`relaxed`, `tightened`) -- `None` (every existing
   node) is a true no-op. Wired into `Tree::update_hover` alongside
   its own existing `hover_opacity` retarget: the node losing hover
   animates `shape` back to `relaxed`, the node gaining it animates
   toward `tightened`.
4. **Real, deliberate v1 scope choice, stated directly:** tied to
   `hovered` only, not `focused`/`pressed` separately -- a real mouse
   press can only ever land on an already-hovered node (`hit_test`'s
   own contract), so `hovered` already covers the whole press gesture
   for this purely cosmetic corner effect; keyboard focus already has
   its own dedicated `focus_ring` signal and doesn't need a second,
   redundant visual cue. Kept the whole new mechanism confined to one
   call site (`update_hover`) rather than touching the ~6 separate
   real `pressed`-mutation sites across `dispatch`, most of which
   don't have a `duration`/`now` cleanly available.
5. `add_split_button` (`engine-py::window_factory.rs`): built the two
   real static `ShapeKey`s per button (leading's own two *right*
   corners tighten, facing the trailing button; trailing's own two
   *left* corners tighten, facing leading), using kurbo's real 4-tuple
   per-corner `RoundedRect` constructor -- the identical real technique
   `GeometryCache::rounded_rect_fill_per_corner` already established.
   New `SPLIT_BUTTON_INNER_CORNER_RADIUS: f64 = 8.0` constant.
6. **Real, previously-dormant gap found and fixed along the way:**
   `paint_node`'s own border-stroke path (`engine-render/src/lib.rs`)
   always used the plain uniform `corner_radius`, completely ignoring
   both `corner_radii_override` (M30 Phase 1 Step 4) and the new
   `shape` morph -- invisible until Split Button's own `"outlined"`
   variant (the only real caller combining a nonzero border with
   per-corner geometry) made it a real, visible bug. Fixed: the border
   now matches whichever real fill geometry the node used (shape-morph
   silhouette stroked directly/centered when active, new `Geometry
   Cache::rounded_rect_border_per_corner` when `corner_radii_override`
   is set, the original uniform path otherwise).
7. Real tests: `geometry_cache.rs` gained per-corner-border cache-hit/
   miss/asymmetry/clamp tests, plus fixed a discovered fragile
   pre-existing test (`bounding_box()` can't distinguish different
   corner radii on the same box -- switched to the path's own real
   starting point). `tree.rs` gained a direct `update_hover` retarget
   test. Two new Python-level tests reusing `Window.hover` (real
   pointer-moved dispatch) -- no Python getter exists for the raw
   `shape` animation target, so these prove the real end-to-end
   dispatch-through-paint path runs clean, including the outlined-
   variant border-fix regression case.
8. Full verification chain: `cargo check`/`clippy -D warnings`/`fmt`/
   `test --workspace --release`, `maturin develop --release`, full
   `pytest tests/`, all 75 examples, showcase demo.
9. `BUILD_TRACKER.md` Phase 4 flipped to done, Top Metrics updated to
   4-of-7, artifact regenerated (38/122/212, unchanged) and republished.

## Status
Complete. Full verification chain green (`cargo test --workspace
--release`: `engine-core` 189 passed (+1), `engine-render` 28 passed
(+4, the new geometry_cache tests); `pytest tests/`: 561 passed/1
skipped, up from 559, +2 new tests; all 75 examples + showcase demo
clean). **M38 Phase 4 -- Split Button Inner-Corner Shape-Tightening
is now complete. M38 itself remains open: 3 phases remain (Button
Group per-child shape change, ScrollView scrollbar thumb, real
scroll+clip for Code Editor).**
