# Log: Phase 3, Step 3.3.1 -- SVG Ingestion & Ear-Clipping Tessellation

## Two real ear-clipping bugs found and fixed, both invisible to
area/count-only unit tests

The unit tests written first (square, L-shape) passed on the very first
implementation attempt. It was only building `svg_tessellation_demo`'s
non-convex five-pointed star and checking specific pixel positions (not
just total area) that surfaced two real, distinct bugs -- both later
locked in as `tre-svg` unit test regressions once understood.

### Bug 1: triangle indices valid against the wrong array

`triangulate`'s internal working copy of the polygon's points gets
deduped and, when the input's original winding needs correcting,
reversed. The first implementation emitted triangle indices straight
from that (possibly-reversed) working copy, but the function's contract
-- and every caller, including `to_ui_vertices` -- expects indices valid
against the CALLER's own `polygon.points` array. Whenever reversal
happened (any polygon whose original point order came out "clockwise"
under the shoelace formula, which this star's did), the returned indices
silently pointed at the wrong physical points.

The square/L-shape tests never caught this because reversal never
triggered for their point orderings (both already "positive" under the
formula), and even the star's own `triangulates_a_five_pointed_star`
test (before this step) only checked triangle *count*, not which
vertices each triangle actually names.

**Fix:** track `original_index: Vec<u32>` alongside the working `points`
copy, built and reversed/deduped in lockstep, and translate every
emitted triangle through it before returning.

### Bug 2: ear-validity check needed BOTH conditions, not either alone

After fixing Bug 1, the star still rendered wrong -- specifically, one of
its concave notches was incorrectly filled. Tracing the actual ear-cut
sequence (added temporary `eprintln!`s, removed once understood) and
cross-checking against an independent Python ray-casting
point-in-polygon reference isolated it to one specific accepted "ear"
whose triangle covered a real, remaining vertex (a concave-notch point)
without that vertex's own edges ever registering as a "proper" crossing
of the diagonal -- because both of that vertex's edges terminated
exactly at two of the triangle's own corners, which
`segments_properly_intersect`'s strict shared-endpoint handling
correctly (and unavoidably) treats as "not a proper crossing."

This is the mirror image of the original vertex-in-triangle-only
approach's own failure mode (documented in the L-shape test): that
approach missed a *different* case, where an edge crosses the triangle
without either of its own endpoints ever landing strictly inside it. Each
check alone has a real blind spot; only running both catches everything
a correct ear-clipping validity test needs to.

**Fix:** restored the vertex-in-triangle check alongside the edge-crossing
check (removed too aggressively when the edge-crossing check was first
added), requiring both to pass.

## What worked without needing a fix

- Curve flattening (recursive de Casteljau subdivision) matched its
  quarter-circle reference approximation on the first run, including the
  hard recursion-depth safety cap terminating correctly on a
  deliberately pathological, widely-separated control-point input.
- `usvg` integration (parsing, `<g>` transform resolution via the
  `to_affine2` bridge to `tre-math`'s `Affine2`, the byte-size and
  point-count hardening caps) all worked correctly on the first attempt.

## Verification performed

- `cargo test --workspace`: all tests pass, including `tre-svg`'s 15
  unit tests -- the five-pointed-star test now checks total area against
  the true shoelace-formula area (not just triangle count) AND that a
  known concave-notch point is covered by no emitted triangle, a
  regression test for exactly the class of bug found above.
- `cargo fmt --check` / `cargo clippy --workspace --all-targets -- -D
  warnings`: clean.
- `svg_tessellation_demo` run manually against the real GPU (AMD/Radeon,
  Wayland session) under the Vulkan validation layer: both pixel
  assertions (interior, concave notch) pass; output PNG visually
  inspected and shows a correctly-shaped five-pointed star.
- All 7 pre-existing Vulkan examples re-run manually, zero validation
  errors (this step adds a new dependency and a new example, but touches
  no RHI/vertex-format code).
