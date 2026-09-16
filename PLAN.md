# Plan: M7 Phase 1 — Real MD3 Motion Tokens (§7.5)

## Context

`MotionCurve` has exactly one real variant, `Linear` — confirmed via
grep, its own doc comment has said "deliberately not built yet" since
M3 step 2. §7.5's own text: MD3 named easing curves are cubic-bezier
control points, evaluated with `kurbo`'s curve math or a direct
implementation, living as a static data table in `engine-md3`.

## Investigation before writing code

- **The exact MD3 curve values had to be verified against a real,
  authoritative source, not recalled from memory.** Fetched multiple
  sources; two independent secondary sources agreed on the single-
  segment curves but *disagreed* on "Emphasized" — one flattened it to
  the same value as "Standard" (a common web-dev CSS approximation,
  since CSS `transition-timing-function` can't express a compound
  curve), the other correctly stated Emphasized has no single cubic-
  bezier value at all. Cross-checked against Android's own
  `MotionTokens.kt` (generated directly from the official Material
  Design spec, per Material Components for Android's documentation) —
  the authoritative source: `Emphasized` is a genuine two-segment SVG
  path, `M 0,0 C 0.05,0 0.133333,0.06 0.166666,0.4 C 0.208333,0.82
  0.25,1 1,1` — two cubic Bézier segments joined at `(0.166666, 0.4)`,
  not a single 4-parameter curve. The single-segment curves (`Standard`/
  `StandardDecelerate`/`StandardAccelerate`/`EmphasizedDecelerate`/
  `EmphasizedAccelerate`) are real, standard CSS-style
  `cubic-bezier(x1, y1, x2, y2)` values (curve fixed at `(0,0)`→`(1,1)`,
  only the two control points vary), confirmed consistently across
  every source checked.
- **Evaluating a cubic-bezier timing function needs solving "given x,
  find t such that `X(t) = x`, then return `Y(t)`"** — the same problem
  every browser engine solves for CSS `cubic-bezier()`. `kurbo`'s own
  `ParamCurveNearest` (already used for M5 Phase 3's custom hit-testing)
  solves a *different* problem (nearest point to an arbitrary point, not
  "the point whose X coordinate is exactly x") and doesn't fit here
  directly. **Real implementation, per §7.5's own "or a direct
  implementation" text:** bisection on `t ∈ [0, 1]` against the cubic's
  own `X(t)` — valid because every real easing curve here has `x1, x2 ∈
  [0, 1]`, which keeps `X(t)` monotonic (the same precondition CSS's own
  spec requires of a valid `cubic-bezier()`). Simple, robust, and
  correct without needing Newton-Raphson's own derivative-near-zero
  edge case.
- **"Emphasized" generalizes the same per-segment solve** — represented
  as two independent 4-point Bézier segments (not the `(0,0)`→`(1,1)`-
  fixed 2-parameter shorthand the single-segment curves use), selecting
  which segment to solve within based on whether the input `x` falls
  before or after the real, documented split point `x = 0.166666`.

## Approach

1. **`engine-core/src/animation.rs`**: a private `CubicSegment { p0,
   p1, p2, p3: (f64, f64) }` with `eval(t) -> (f64, f64)` (De Casteljau/
   direct cubic formula) and `solve_y_for_x(x) -> f64` (bisection on
   `X(t)`, ~40 iterations for full `f64` precision). `MotionCurve`
   gains `Standard`/`StandardDecelerate`/`StandardAccelerate`/
   `Emphasized`/`EmphasizedDecelerate`/`EmphasizedAccelerate`, each
   `ease()`-ing through the real verified control points above (the
   single-segment curves via one `CubicSegment` fixed at `(0,0)`→
   `(1,1)`; `Emphasized` via two, split at `x = 0.166666`).
2. **New tests**: boundary conditions (`ease(0.0) == 0.0`, `ease(1.0)
   == 1.0`) for every curve; monotonicity (sampled); a round-trip
   consistency check per curve (forward-evaluate `(x, y)` at a chosen
   `t` via `eval`, then confirm `solve_y_for_x(x)` recovers `y`) —
   validates the bisection solver against the curve's own forward
   math, the same rigor `Affine::inverse()`'s own round-trip tests use.
   `Emphasized` additionally: continuity across the segment join (no
   jump at `x = 0.166666`) and the real, externally-verified landmark
   value (`y ≈ 0.4` at the join) — a genuine match-the-real-spec check,
   not just internal self-consistency.
3. **No consumers wired to a specific new curve yet** — this phase is
   the data table itself, per §7.5's own scope; M7 Phase 3
   (ripple/hover/focus-ring) and Phase 5 (container transform's own
   "Emphasized" choreography) are what actually *use* a non-`Linear`
   curve for the first time.

## Files to touch

- `crates/engine-core/src/animation.rs` — `CubicSegment`, new
  `MotionCurve` variants, tests.

## Verification

- `cargo test --workspace`, `cargo clippy --workspace --all-targets --
  -D warnings`, `cargo fmt --check`.
- Full pre-existing suite must stay green unmodified (purely additive:
  new enum variants, no existing variant's behavior changes).
