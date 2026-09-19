# LOG — M39 Phase 3: Shape-Morphed Border Inset Fix

- User's own explicit instruction: "Start" continued into M39 Phase 3
  (item 3 from the gap-sweep answer, third in the user's own chosen
  order): the shape-morphed border inset gap M38 Phase 4's own stated
  v1 simplification left open.
- Real investigation first, per this phase's own scoping note in
  `BUILD_TRACKER.md`: does `kurbo` already expose a polygon-inset/
  offset operation, before writing anything by hand? Direct source
  read of the vendored `kurbo = "0.13.1"` crate found a real `offset.
  rs` module -- but its one public function, `offset_cubic(c: CubicBez,
  d: f64, tolerance: f64, result: &mut BezPath)`, offsets a single
  cubic Bézier curve. `ShapeKey`'s own shapes (`shape_morph.rs`'s own
  module doc comment) are always straight-line segments between
  vertices, never curves -- `offset_cubic` is genuinely the wrong
  tool for this shape, not merely an unused one. Confirmed via grep:
  zero hits for any polygon-offset/inset operation anywhere in this
  codebase's own kurbo usage, so this phase writes real, new,
  self-contained geometry rather than reusing something that already
  existed.
- New `ShapeKey::inset_path(amount) -> BezPath` (`shape_morph.rs`): the
  classic real "offset each edge inward along its own normal, then
  re-intersect adjacent offset edges" polygon-shrink algorithm (a real
  miter join at each vertex). Which of an edge's two perpendicular
  normals is "inward" is resolved per-edge against the real polygon
  centroid (whichever normal points toward it) rather than assuming a
  fixed CW/CCW winding order -- `ShapeKey::from_path` extracts
  vertices from whatever `BezPath` a caller supplied with no
  guaranteed winding, and `interpolate`'s own real alignment search
  (this module's own existing doc comment) can reorder them further,
  so a fixed-winding assumption would have been a real, silent
  correctness bug for at least one real caller eventually. New private
  `line_intersection` helper solves `p1 + t*d1 == p2 + s*d2`, returning
  `None` (falls back to the offset edge's own start point) for two
  near-parallel adjacent edges rather than propagating a near-infinite
  value.
- **Real, stated v1 scope limit, written directly into the doc
  comment rather than glossed over:** correct for the real border
  widths this codebase actually uses (MD3's own 1-4dp outline range)
  against MD3-scale shapes -- not proven robust for an inset large
  enough to invert a polygon's own edges or force two non-adjacent
  offset edges to cross, a real, harder self-intersection-avoidance
  problem no real caller here needs solved.
- `engine-render`'s border block (`lib.rs`) now strokes `node.paint.
  shape.current.inset_path(inset)` for the real "active shape morph"
  branch, replacing the old `.to_path()` (raw silhouette, centered,
  the exact real bug this phase closes) -- a one-line real change once
  the actual geometry primitive existed; the fill path itself was
  already correct and untouched.
- **Hand-verification before trusting the algorithm, the same "verify,
  don't assume" discipline this whole project already applies:**
  traced `inset_path`'s own real math by hand against the existing
  `square(0, false)` test helper's own 10x10 corners before writing a
  single assertion -- worked through each of the four edges' own real
  inward normal, each offset line, and each of the four real vertex
  intersections by hand, landing on exactly `(2,2)-(8,2)-(8,8)-(2,8)`
  for a `2.0` inset. Wrote the test to assert that exact, independently
  -derived result, not simply run the code once and copy whatever it
  produced.
- Real tests: 4 new `engine-core` unit tests (the hand-derived square
  case above; a reversed-winding twin proving the centroid-based
  normal resolution is winding-independent, landing every corner
  strictly inside `2..8` either way; a `0.0`-amount true no-op; a
  degenerate 2-point shape true no-op). New real pixel-readback
  integration test, `engine-render/tests/shape_morph_paint.rs`'s own
  `a_border_on_a_real_active_shape_morph_stays_inside_the_fills_own_
  edge`: a 60x60 shape (`20..80`) centered in a 100x100 box with a
  real 20px border (`inset=10`) -- before this phase, the border would
  have stroked the raw `20..80` edge centered, covering `10..30`, a
  real 10px bleed past the shape's own edge into plain background;
  after, the border strokes the real inset `30..70` edge centered,
  covering `20..40`, entirely inside. The test asserts all three real
  zones directly: the pre-fix bleed zone is now plain background, the
  real inset border band genuinely is the border color, and the
  shape's own interior is unaffected.
- Full verification chain, all green: `cargo check --workspace --all-
  targets`; `cargo clippy --workspace --all-targets -- -D warnings`;
  `cargo fmt` + `cargo fmt --check`; `cargo test --workspace --
  release` (`engine-core`: 212 passed, +4; `engine-render`'s
  `shape_morph_paint` suite: 3 passed, +1); `maturin develop --release`
  (no Python-facing API changed this phase -- rebuilt anyway per the
  standing verification chain); `pytest tests/` (581 passed, 1
  skipped, unchanged from the prior step, as expected); all 77
  examples + showcase demo clean.
- `BUILD_TRACKER.md` Phase 3's own heading, step bullet, milestone
  status line ("Phase 3 of 5 done", up from "Phase 2"), and Top
  Metrics row (60%, up from 40%) all updated together, mirroring
  Phase 2's own precedent for a single-step phase (unlike Phase 2's
  own two-step shape, which updated the whole-phase lines only once
  both steps closed). Parser re-confirmed balanced (39 milestones, 127
  phases, 218 items, unchanged); artifact regenerated and republished.
  **M39 Phase 3 is now complete. Phases 4-5 remain: Terminal Cell Text
  Attributes, `Tree::tick_all` Active-Set Optimization.**
