# Plan: M3 Phase 5, Step 10 — Shape Morph Module (§14 step 10)

Corresponds to `BUILD_TRACKER.md` M3 Phase 5, step 10 of 4 (steps 8-11).

## Goal

Per §14 step 10: "Shape morph module (§7.4) — the one component with no
library to lean on." §7.4's own text: equalize point/segment counts
between two `kurbo::BezPath`s, then linearly interpolate corresponding
point positions. Its own review note is explicit that this is only half
the real technique: naive per-index pairing assumes point *N* on one
shape visually corresponds to point *N* on the other, which is usually
false and produces self-intersecting or wildly-rotating mid-morph
geometry. The missing, harder half is a correspondence/alignment search
— try every rotational offset (and winding-direction flip) between the
two point sets, keep whichever minimizes total point-travel distance —
*before* lerping.

§5's own type sketch names the integration point directly: `ShapeKey`
is a value type implementing `engine_core::Interpolate` (§5: "`ShapeKey`
(whose impl is the correspondence-then-lerp technique in §7.4)"),
meaning it plugs into the *existing* `Animated<T>` machinery from step 2
with zero changes to that machinery — the same pattern `f64`/
`peniko::Color` already use, not a parallel animation mechanism.

## Scope

This is `engine-md3`'s first real content (an empty skeleton crate
since M3 Phase 1). In scope:

- `engine-md3::shape_morph::ShapeKey`: extracts a closed `BezPath`'s own
  vertex sequence (`from_path`), rebuilds a straight-edged closed
  `BezPath` from it (`to_path`), and implements `engine_core::
  Interpolate` — the equalize-then-align-then-lerp pipeline runs fresh
  inside `interpolate()` itself, so a caller drives it exactly like any
  other `Animated<T>` (`Animated::new(ShapeKey::from_path(a))`,
  `.animate_to(ShapeKey::from_path(b), duration, curve, now)`) with no
  separate setup step to remember.
- Real unit tests proving the two mechanisms independently: the
  correspondence search actually finds a zero-cost alignment when one
  exists (two point lists describing the *same* physical square, one
  rotated to start at a different corner, one wound the opposite
  direction) rather than defaulting to naive index pairing -- and edge
  subdivision (`equalize_point_counts`) genuinely splits the longest
  edge, not an arbitrary or zero-length one.

Deliberately scoped to a path's own vertices (each `PathEl`'s endpoint),
not full curve-type-aware control-point morphing — a curved segment's
control handles are dropped, and morph output is always straight edges
between interpolated vertices. MD3's own shapes are close enough to
polygons (rounded corners aside) for this to be the correct v1 scope;
true bezier-segment-type correspondence is a substantially larger
problem than "budget real implementation time" asks for, and nothing in
§14's build order calls for it. Also assumes one closed subpath per
shape — every MD3 shape is a single contour, and no planned component
needs a shape with a hole.

Out of scope (deferred, not a gap): wiring `ShapeKey`/`Animated<
ShapeKey>` into `PaintProperties`/`NodeKind` — `engine-core::node.rs`'s
own comment has deferred `shape: Animated<ShapeKey>` since step 3
("needs a non-trivial `Interpolate` impl... land[s] whenever a later
step first animates a transform or a shape morph"); that step is real
component work (a FAB morphing into an extended FAB, say) which doesn't
exist yet. This step builds and proves the `ShapeKey`/`Interpolate`
machinery `engine-core` can already consume unchanged, the same
"prove the primitive, defer `Tree`-level wiring" shape steps 8/9 both
used.

## Verification

`cargo test -p engine-md3` passes with real geometric proofs (a
same-shape-different-start-vertex pair morphs to itself, not a
collapsed degenerate shape), not just "doesn't panic". `cargo test
--workspace`, `cargo clippy --workspace --all-targets -- -D warnings`,
`cargo fmt --check` all clean.
