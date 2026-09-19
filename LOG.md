# LOG — M38 Phase 4: Split Button Inner-Corner Shape-Tightening

- User's own explicit instruction: "Let's knock out the known gaps"
  -- M38's own fourth phase, per the milestone's own scoping order.
- Real spec-value research before writing any code: the M3 site's own
  split-button spec page (`m3.material.io/components/split-button/
  specs`) returned no fetchable static content (JS-rendered SPA, a
  `WebFetch` confirmed only the bare heading). Found the real number
  instead via `material-components-android`'s own real docs
  (`docs/components/ButtonGroup.md`): a connected group's own inner
  corners tighten to a real 8dp, outer corners stay fully round --
  the closest real, documented anatomy to Split Button's own stated
  "the inner corners change shape for hovered, focused, and pressed
  states" (`COMPONENT_SPLIT_BUTTONS.md`).
- Paused via `AskUserQuestion` before writing code, per this session's
  own standing discipline for genuinely novel-capability steps: no
  existing internal precedent for auto-driving a `shape` animation
  from interaction state (`hover_opacity`/`focus_ring`/ripple opacity
  are all engine-core-auto-driven; `shape: Animated<ShapeKey>`, M7
  Phase 4, has only ever been app-driven via `Node.animate("shape",
  ...)`). Presented two real options -- a smooth `Animated<ShapeKey>`
  morph (matching the real MD3 visual and `BUILD_TRACKER.md`'s own
  Phase 4 scoping note) vs. an instant, unanimated `corner_radii_
  override` swap. User chose the smooth morph.
- New `PaintProperties.interactive_shape: Option<(ShapeKey,
  ShapeKey)>` (`node.rs`) -- `(relaxed, tightened)`, `None` (every
  existing node) a true no-op. Wired directly into `Tree::update_
  hover` (`tree.rs`), right alongside its own existing `hover_opacity`
  retarget block: the node losing hover animates `shape` back to
  `relaxed`, the node gaining it animates toward `tightened`, using
  the identical `duration`/`now`/`MotionCurve::Linear` already passed
  to that method.
- **Real, deliberate v1 scope choice made and stated directly, not
  glossed over:** tied to `hovered` only, not `focused`/`pressed`
  separately. Real reasoning: a mouse press can only ever land on an
  already-hovered node (`Tree::hit_test`'s own contract), so `hovered`
  already covers the entire press gesture for this purely cosmetic
  corner effect; keyboard-only focus already has its own dedicated
  `focus_ring` signal and doesn't need a second, redundant visual cue
  riding along with it. This also kept the whole new mechanism
  confined to one real call site instead of the ~6 separate places
  `Tree.pressed` gets mutated across `dispatch` (most without a
  `duration`/`config` cleanly available at that point) -- a real,
  deliberate complexity-bounding choice, not an oversight.
- `add_split_button` (`engine-py/src/window_factory.rs`): built the
  two real static `ShapeKey`s per button using kurbo's own real
  4-tuple per-corner `RoundedRect` constructor -- the identical real
  technique `GeometryCache::rounded_rect_fill_per_corner` already
  established (confirmed via direct read before reusing). Leading's
  own two *right* corners tighten (facing the trailing button across
  `SPLIT_BUTTON_GAP`); trailing's own two *left* corners tighten
  (facing leading). New `SPLIT_BUTTON_INNER_CORNER_RADIUS: f64 = 8.0`
  constant, both buttons' own `shape` initialized to the *relaxed*
  `ShapeKey` directly at construction (not left at `PaintProperties::
  new`'s own `ShapeKey::empty()` default) -- otherwise the first real
  hover would animate *from* an empty shape, a real, visible flash
  bug this avoided by construction rather than needing a special case.
- **Real, previously-dormant gap found and fixed along the way, the
  same "verify before writing the completion note" discipline M38
  Phase 1's own Terminal correction established:** `paint_node`'s own
  border-stroke path (`engine-render/src/lib.rs`) always used the
  plain uniform `corner_radius.current` for its own inset-radius
  computation, completely ignoring both `corner_radii_override` (M30
  Phase 1 Step 4) and the new `shape` morph -- confirmed by direct
  grep (`corner_radii_override` appeared exactly once in the whole
  file, only in the fill-path branch). This gap was invisible until
  now: `Segmented Button`, the only other real `corner_radii_override`
  consumer, never combines it with a nonzero border on the same node
  (checked directly). Split Button's own `"outlined"` variant is the
  first real caller to combine a genuine border with per-corner
  geometry, making the mismatch a real, visible bug.
  Fixed with a real three-way branch in the border block, matching
  the fill block's own existing precedence: shape-morph active ->
  stroke the shape's own silhouette directly (centered, no inset --
  a raw vertex path has no per-corner radius to shrink, a real, stated
  v1 simplification, documented directly rather than inventing a
  general path-offset operation nothing else in this codebase needs);
  `corner_radii_override` set -> new `GeometryCache::rounded_rect_
  border_per_corner` (mirrors `rounded_rect_border`'s own exact
  inset-then-clamp shape, added to the same `RectPathParams` enum and
  `border_paths` map `Border` already uses, since a node only ever
  needs one border shape at a time); neither -> the original uniform
  path, byte-for-byte unchanged.
- New `geometry_cache.rs` tests for `rounded_rect_border_per_corner`:
  cache-hit/cache-miss (mirroring the existing fill-path tests' own
  shape), a real asymmetric-geometry proof against a uniform border,
  and an inset-larger-than-radius clamp-to-zero proof. **Real test-
  quality bug found and fixed along the way while writing these:**
  direct inspection proved `BezPath::bounding_box()` reports the
  *identical* box for any valid corner radius on a given `w`/`h` (a
  rounded rect's own tight curve bounds always touch all four nominal
  edges regardless of radius) -- meaning the pre-existing `a_changed_
  radius_invalidates_the_cached_fill_path` test (M34 Phase 1) was only
  ever passing by incidental floating-point tessellation noise between
  the two radii it compared, not by genuinely proving the radius
  reached the geometry. Fixed it too, alongside the two new per-corner
  tests, all switched to comparing the path's own real starting
  `MoveTo` point (kurbo's `RoundedRect::to_path()` starts at `(x0, y0
  + top_left_radius)`, a real, deterministic, radius-dependent value)
  instead of `bounding_box()`.
- New `tree.rs` test, `update_hover_retargets_a_real_interactive_
  shape_toward_tightened_then_relaxed`: mirrors `update_hover_fades_
  the_old_node_out_and_the_new_one_in_on_a_real_change`'s own exact
  shape, proving a real hover-in retargets `shape.active.to` toward
  `tightened` and a real hover-out retargets it back to `relaxed`.
  Passed on the first run.
- Real, honest verification-surface check done properly (the same
  discipline M38 Phase 2/3 already established): found `Window.hover
  (node)`, a real, already-existing Python method dispatching a real
  `PointerMoved` event -- exactly the mechanism `interactive_shape`'s
  own retarget is wired into. No Python getter exists for the raw
  `shape` animation target itself, so two new pytest tests in `tests/
  test_split_button.py` instead prove the real, full dispatch-through-
  paint path runs clean end to end: hovering either button (then
  moving away) for the default variant, and hovering both buttons of
  an `"outlined"` Split Button specifically -- the real regression
  case for the border-fix gap above, since that's the one real variant
  with a visible border. Both passed on the first run.
- Corrected four now-stale doc comments that had explicitly stated
  the inner-corner shape-tightening was *not* implemented: `add_
  split_button`'s own Rust doc comment (`window_factory.rs`), its
  `python/tre/_core.pyi` stub (added a one-line real-behavior note),
  and `PaintProperties.interactive_shape`'s own new doc comment states
  the real design choice directly rather than needing a separate note
  elsewhere.
- Full verification: `cargo check --workspace --all-targets`/`cargo
  clippy --workspace --all-targets -D warnings`/`cargo fmt --check`
  clean; `cargo test --workspace --release` clean (`engine-core` 189
  passed, up from 188, +1; `engine-render` 28 passed, up from 24, +4
  the new geometry_cache tests; zero regressions); `maturin develop
  --release` rebuilt; `pytest tests/` 561 passed/1 skipped, up from
  559, +2 new tests; all 75 examples and the showcase demo re-run
  clean.
- Updated `BUILD_TRACKER.md`: Phase 4 flipped `⬜` -> `✅` with a
  terse step-bullet note; milestone status line and Top Metrics row
  updated to "Phase 4 of 7 done" / 57%. Verified the parser's own
  reported item count unchanged before/after (38/122/212 both times).
  Regenerated and republished the Build Tracker artifact.
  **This closes M38 Phase 4. M38 itself remains open -- 3 phases
  remain (Button Group per-child shape change on press/select,
  ScrollView scrollbar thumb, real scroll+clip for Code Editor with
  caret-follow).**
