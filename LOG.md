# Log: M7 Phase 1 — Real MD3 Motion Tokens (§7.5)

Corresponds to `BUILD_TRACKER.md` M7 Phase 1. `MotionCurve` had exactly
one real variant, `Linear` — its own doc comment said "deliberately not
built yet" since M3 step 2.

## Investigation before writing code

- **The exact MD3 curve values had to be verified against a real,
  authoritative source, not recalled from memory.** Fetched multiple
  sources; two independent secondary sources agreed on the single-
  segment curves but *disagreed* on "Emphasized" — one flattened it to
  `Standard`'s own value (a common CSS approximation, since
  `transition-timing-function` can't express a compound curve), the
  other correctly stated it has no single cubic-bezier value at all.
  Cross-checked against Android's own `MotionTokens.kt` (generated
  directly from the official Material Design spec) — the authoritative
  source: `Emphasized` is a genuine two-segment SVG path, `M 0,0 C
  0.05,0 0.133333,0.06 0.166666,0.4 C 0.208333,0.82 0.25,1 1,1`, joined
  at `(0.166666, 0.4)` — not a single 4-parameter curve. The single-
  segment curves (`Standard`/`StandardDecelerate`/`StandardAccelerate`/
  `EmphasizedDecelerate`/`EmphasizedAccelerate`) are real, standard
  CSS-style `cubic-bezier(x1, y1, x2, y2)` values, confirmed
  consistently across every source checked.
- **Evaluating a cubic-bezier timing function needs solving "given x,
  find t such that `X(t) = x`, then return `Y(t)`"** — the same problem
  every browser engine solves for CSS `cubic-bezier()`. `kurbo`'s own
  `ParamCurveNearest` (already used for M5 Phase 3's custom hit-testing)
  solves a different problem and doesn't fit here. Real implementation,
  per §7.5's own "or a direct implementation" text: bisection on `t ∈
  [0, 1]` against the cubic's own `X(t)` — valid because every real
  curve here keeps `x1, x2 ∈ [0, 1]`, the same monotonicity precondition
  CSS's own spec requires.
- **"Emphasized" generalizes the same per-segment solve** — represented
  as two independent 4-point Bézier segments, selecting which to solve
  within based on whether the input falls before or after the real
  documented split point `x = 0.166666`.

## What happened

`engine-core/src/animation.rs`: new `CubicSegment { p0, p1, p2, p3 }`
(arbitrary-space cubic Bézier, not the `(0,0)`→`(1,1)`-fixed CSS
shorthand, so it can also represent one segment of `Emphasized`'s
compound curve) with `eval(t)` and `solve_y_for_x(x)` (bisection, ~40
iterations for full `f64` precision). `MotionCurve` gains `Standard`/
`StandardDecelerate`/`StandardAccelerate`/`Emphasized`/
`EmphasizedDecelerate`/`EmphasizedAccelerate`, each `ease()`-ing through
the real verified control points.

Four new tests: boundary conditions (`ease(0.0) == 0.0`/`ease(1.0) ==
1.0`) for every curve; monotonicity (sampled at 200 points per curve);
a round-trip consistency check (forward-evaluate via `eval`, confirm
`solve_y_for_x` recovers the same `y`) — the same rigor `kurbo::
Affine::inverse()`'s own tests use; and `Emphasized`'s own real,
externally-verified landmark (`y ≈ 0.4` at the segment join) plus
continuity across it. All four passed on the first run.

No consumers wired to a non-`Linear` curve yet — this phase is the data
table itself, per §7.5's own scope; M7 Phase 3 (ripple/hover/focus-ring)
and Phase 5 (container transform) are what actually use one for the
first time.

Full `cargo test --workspace --release` clean (`engine-core` 55 tests,
up from 51), `cargo clippy --workspace --all-targets -- -D warnings`,
`cargo fmt --check` all clean. `maturin develop --release` + full
`pytest tests/` (78 passed, 1 skipped, unchanged — no `engine-py` code
touched this phase) confirmed unaffected.
