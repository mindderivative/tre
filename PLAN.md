# Plan: M11 Phase 1 — True Bezier Curve Authoring (§11.10, §11.11)

Corresponds to `BUILD_TRACKER.md` M11 Phase 1's own scoping:
`stroke_path`/`set_hit_test_path` gain a real way to author curved
segments (quadratic/cubic bezier), not just straight polylines.

## Investigation before writing code

- `crates/engine-py/src/canvas.rs`'s own `polyline(points: Vec<(f64,
  f64)>) -> PyResult<BezPath>` (confirmed by direct read, lines 28-37):
  builds a `BezPath` via exactly `move_to(points[0])` then
  `line_to(p)` for every remaining point — no `quad_to`/`curve_to`
  call anywhere in this file (confirmed via grep). Both real callers,
  `stroke_path` and `set_hit_test_path`, go through this one helper —
  reused as the one place to extend, not duplicated.
- `kurbo::BezPath` (confirmed via direct read of the pinned `kurbo`
  0.11.3 source, `bezpath.rs`) already exposes `quad_to<P: Into<Point>>
  (p1: P, p2: P)` and `curve_to<P: Into<Point>>(p1: P, p2: P, p3: P)`
  alongside the already-used `move_to`/`line_to` — `(f64, f64)` tuples
  already satisfy `Into<Point>` (the exact mechanism `polyline`'s own
  existing `line_to(p)` call already relies on). Nothing new needed
  from `kurbo` itself.
- `CustomHitTest::Path { path: BezPath, tolerance: f64 }`
  (`engine-core/canvas.rs`) already stores a full `BezPath`, and
  `tree.rs` already imports `kurbo::ParamCurveNearest` for its
  distance-to-path hit-test math (confirmed via grep,
  `tree.rs:35`) — `ParamCurveNearest` is implemented generically for
  every `PathSeg` variant (`Line`/`Quad`/`Cubic`), so a `BezPath`
  containing real curve segments already hit-tests correctly with zero
  `engine-core` changes. This phase is purely a Python-facing
  *authoring* gap, confirmed by direct read, not an engine-level one.
- Real API-design question investigated before choosing: how does a
  Python caller specify a curve? Considered adding parallel
  `stroke_curve`/`set_hit_test_curve` methods, but that would be
  exactly the "second mechanism" this project's own conventions
  consistently avoid (ARCHITECTURE.md §11.3's own "one overlay
  mechanism, not three" reasoning; §11.5's "one mechanism, two call
  sites" for splitters). Instead: widen `polyline`'s own `points`
  parameter from `Vec<(f64, f64)>` to `Vec<Vec<f64>>` — pyo3 already
  extracts a nested `Vec<Vec<f64>>` from any Python sequence of
  sequences with zero custom code (the same "any sequence of numbers"
  mechanism the existing `Vec<(f64, f64)>` extraction already relies
  on), so every existing call site (`stroke_path(points=[(ax, ay),
  (bx, by)], ...)` in `node_graph.py`/`positioned_graph.py`/
  `canvas.py`) keeps working unmodified — a 2-element inner sequence
  still means a line-to point, exactly as before. A 4-element inner
  sequence means a quadratic control+end point; a 6-element sequence
  means a cubic. Considered and rejected a custom `FromPyObject` enum
  (would need pyo3 0.29's own more complex
  `type Error: Into<PyErr>` + `Borrowed<'a, 'py, PyAny>` trait shape,
  confirmed via direct read of the pinned pyo3 0.29.2 source — real,
  avoidable complexity for what a plain arity-dispatch on `Vec<f64>`
  already solves with zero new trait machinery, and zero precedent for
  a custom `FromPyObject` impl exists anywhere in this codebase,
  confirmed via grep).

## Design

`crates/engine-py/src/canvas.rs`:

- `polyline` renamed to `build_path` (no longer polyline-only) and
  widened from `points: Vec<(f64, f64)>` to `points: Vec<Vec<f64>>`.
  The first point must be a 2-element sequence (`move_to`); each
  remaining point may be 2 elements (`line_to`), 4 (`quad_to`, control
  + end), or 6 (`curve_to`, two controls + end) — any other length is
  a real `PyValueError` naming the offending length, not a silent
  misinterpretation.
- `stroke_path`/`set_hit_test_path` both keep their exact existing
  signatures (`points: Vec<Vec<f64>>` in place of `Vec<(f64, f64)>`,
  same parameter name so existing `points=[...]` keyword calls are
  unaffected) and both keep calling the one shared `build_path` helper
  — zero duplication between the stroke path and hit-test path.
- Doc comments on `stroke_path`/`set_hit_test_path` corrected — they
  currently state "a straight polyline via `move_to`/`line_to` — not a
  general curve-authoring API this phase" (`stroke_path`) and "a
  straight polyline here, same underlying real distance-to-path math a
  true curve would use" (`set_hit_test_path`), both now stale once
  this phase ships real curve authoring.

## Verification plan

- `cargo test --workspace --release`/clippy/fmt — this phase touches
  only `engine-py`, no `engine-core` change (confirmed by design: the
  hit-test math already generalizes, nothing there needs to change).
- `maturin develop --release` + `pytest tests/`. New coverage: a
  `stroke_path`/`set_hit_test_path` call using a real quadratic and a
  real cubic segment builds without error; an existing 2-tuple-only
  call keeps working unmodified (regression guard); an invalid
  point-length (e.g. 3 or 5 numbers) raises a real, clear
  `ValueError`. `set_hit_test_path`'s own real functional proof: a
  point genuinely on a real curve (not just near the straight
  chord between its endpoints) hits; a point near the chord but off
  the true curve misses — the one test that actually exercises
  `ParamCurveNearest`'s own curve-aware math, not just "it didn't
  crash."
- Run all examples — confirm the three existing `stroke_path` call
  sites (`node_graph.py`, `positioned_graph.py`, `canvas.py`) still
  exit cleanly, unmodified.
