# Log: M3 Phase 5, Step 10 — Shape Morph Module (§14 step 10)

Corresponds to `PLAN.md` / `BUILD_TRACKER.md` M3 Phase 5, step 10 of 4 (steps 8-11).

## What happened

**`engine-md3`'s first real content.** Confirmed directly first that
this is the correct crate: `engine-md3` has been an empty skeleton
since M3 Phase 1, and steps 8/9 (shadow, ripple) both landed in
`engine-render` instead, because those needed `vello_hybrid`, which
§15's Risk Register confines to `engine-render` by name. Shape morphing
only needs `kurbo::BezPath` geometry -- no GPU calls -- so this is the
first step whose mechanism genuinely belongs in `engine-md3`. Added
`peniko` as a direct dependency (matching every other crate's "go
through `peniko::kurbo`, not `kurbo` directly" convention; already
resolved at 0.6.1 elsewhere, zero new dependency-graph cost) and
corrected `engine-md3/src/lib.rs`'s stale doc comment, which still said
real content started "at step 8" from before that scope decision was
made.

**Read §7.4's review note as a literal spec, not commentary**: "equalize
count, then lerp" alone was explicitly flagged as producing
self-intersecting or wildly-rotating morphs, because naive per-index
pairing assumes point *N* on one shape corresponds to point *N* on the
other. The fix it names -- try every rotational offset and both winding
directions, keep whichever minimizes total point-travel distance, then
lerp -- is exactly what `engine_md3::shape_morph::best_aligned`
implements, not a simplified approximation of it.

**`ShapeKey` implements `engine_core::Interpolate` directly** -- §5's
own type sketch names this integration point (`pub shape:
Animated<ShapeKey>`, "`ShapeKey` (whose impl is the correspondence-
then-lerp technique in §7.4)"), meaning the *existing* `Animated<T>`
machinery from step 2 needed zero changes: `ShapeKey` slots in exactly
like `f64`/`peniko::Color` already do. The whole equalize-then-align-
then-lerp pipeline runs fresh inside `interpolate()` on every call
(a real, explicit choice, not an oversight) rather than being cached
once per animation -- the same "naive until profiling says otherwise"
call this project has made consistently since `Tree::tick_all`'s own
whole-tree walk, and cheap in absolute terms at the vertex counts an
MD3 shape actually has.

**Deliberately scoped to vertex positions, not full curve-type
correspondence**: `ShapeKey::from_path` extracts each `PathEl` segment's
endpoint (dropping bezier control handles), and `to_path` rebuilds a
straight-edged polygon from the interpolated points. True curve-to-
curve morphing (matching which segments are curves vs. lines between
two arbitrary shapes) is a substantially larger problem than "budget
real implementation time" (§7.4's own phrasing) asks for, and nothing
in §14's build order calls for it -- MD3's own shapes are close enough
to polygons for this to be the correct v1 scope. Also assumes one
closed subpath per shape, matching every real MD3 shape.

**The actual proof, not just "doesn't panic"**: two tests construct the
literal failure case §7.4's review note describes -- a square's four
corners listed starting from a different corner (`[C,D,A,B]` vs.
`[A,B,C,D]`), and the same square wound the opposite direction. Naive
index-0 pairing between either pair would lerp opposite corners at
`t=0.5` (e.g. `midpoint(A,C)` and `midpoint(C,A)` both landing on the
square's own center), collapsing all four points onto one spot -- an
unambiguous, easy-to-detect failure signature. Both tests instead assert
the midpoint output is the *unmoved* square to within `1e-9`, proving
the alignment search actually found the zero-cost rotation/reversal
rather than defaulting to naive pairing. A third test proves
`equalize_point_counts`/`subdivide_longest_edge` pick the genuinely
longest edge of an asymmetric shape, not an arbitrary one; a fourth
proves `t=0.0`/`t=1.0` snap exactly to the equalized/aligned endpoints
with no drift.

## Verification

```
$ cargo test -p engine-md3 -- --nocapture
running 5 tests
test shape_morph::tests::same_shape_with_opposite_winding_morphs_to_itself ... ok
test shape_morph::tests::subdivide_longest_edge_splits_the_actual_longest_edge ... ok
test shape_morph::tests::from_path_and_to_path_round_trip_a_triangle ... ok
test shape_morph::tests::same_shape_started_at_a_different_corner_morphs_to_itself_not_a_collapsed_point ... ok
test shape_morph::tests::interpolate_at_t_zero_and_one_returns_the_equalized_endpoints_exactly ... ok

$ cargo test --workspace             # all green
$ cargo clippy --workspace --all-targets -- -D warnings   # clean
$ cargo fmt --check                  # clean
```

## Next

`BUILD_TRACKER.md` updated: Phase 5 step 10 done (3 of 4 steps in this
phase). Next: step 11 -- wire `material-colors` for a full dynamic
color theme (§7.1), the step that finally gives `engine-md3` a real
color source: an HCT-based scheme generated from a seed color, verified
against Material Color Utilities' own published reference test vectors
(§7.1's own "acceptance gate, not just verify maintenance status")
before it's pinned.
