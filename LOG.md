# Log: M11 Phase 1 — True Bezier Curve Authoring (§11.10, §11.11)

Corresponds to `BUILD_TRACKER.md` M11 Phase 1. `stroke_path`/`set_hit_
test_path` gain real curved-segment authoring, closing the single
most-repeatedly-passed-over candidate across M8 and M10's own scoping.

## Investigation before writing code

`crates/engine-py/src/canvas.rs`'s own `polyline` helper (both real
callers, `stroke_path` and `set_hit_test_path`, went through it)
built a `BezPath` using only `move_to`/`line_to` — confirmed via
direct read and grep, no `quad_to`/`curve_to` call anywhere in the
file. `kurbo::BezPath::quad_to`/`curve_to` already exist in the pinned
kurbo version and already accept `(f64, f64)` tuples via `Into<Point>`
— the same mechanism `line_to` already relied on. `CustomHitTest::
Path`'s own distance math (`ParamCurveNearest`, already imported in
`tree.rs`) already generalizes to real curve segments — confirmed this
was purely a Python-facing authoring gap, not an engine-level one.

Considered and rejected adding parallel `stroke_curve`/`set_hit_test_
curve` methods (a second mechanism, against this project's own
repeated "one mechanism, not two/three" convention). Considered and
rejected a custom `FromPyObject` enum for a tagged point type — pyo3
0.29's `FromPyObject` trait needs `type Error: Into<PyErr>` plus a
`Borrowed<'a, 'py, PyAny>`-based `extract` method, real avoidable
complexity (confirmed via direct read of the pinned pyo3 source; zero
precedent for a custom `FromPyObject` impl exists anywhere in this
codebase) for what a plain arity-dispatch on `Vec<f64>` already solves.

## What happened

`polyline` renamed to `build_path`, widened from `points: Vec<(f64,
f64)>` to `points: Vec<Vec<f64>>` — a 2-element inner sequence is a
line-to (the only shape ever accepted before this phase, so every
existing call site keeps working unmodified with zero Python-side
changes), 4 elements is a quadratic (`quad_to`, 2 control + 2 end
coordinates), 6 is a cubic (`curve_to`, 4 control + 2 end). The first
point must be 2 elements (a `move_to` has no control points of its
own); any other length anywhere is a real `PyValueError` naming the
offending length. `stroke_path`/`set_hit_test_path` keep their exact
signatures and parameter name (`points`), both still calling the one
shared `build_path` helper. Two stale doc comments corrected (both
methods' own, which previously stated "a straight-line polyline... not
a general curve-authoring API").

New `engine-core` unit test (`tree.rs`):
`canvas_custom_path_hit_test_uses_the_real_curve_not_the_straight_
chord_between_its_endpoints` — the non-degenerate case the pre-existing
straight-line test's own doc comment named but never exercised. A real
`quad_to` curve (`move_to(0,0)`, control `(50,100)`, end `(100,0)`)
bulges to a true midpoint of `(50,50)`, far from the naive chord's own
midpoint `(50,0)`. Two adversarial points prove this is genuinely
curve-aware, not a chord approximation: `(50,50)` (on the real curve,
50px from the chord) hits; `(50,2)` (2px from the chord, ~48px from the
real curve) misses — the exact pair that would come out wrong under
chord-only distance.

New pytest coverage (`test_canvas.py`): `stroke_path`/`set_hit_test_
path` accept real quadratic and cubic segments without raising (the
FFI-authoring-surface proof; the Rust test above is the definitive
curve-aware-math proof, matching this file's own established split);
an invalid point length (3 numbers) raises a clear `ValueError`; a
curve segment as the first point is rejected with a clear message.

No `engine-core` production change this phase (only a new test) — full
`cargo test --workspace --release` (`engine-core` 82, up from 81),
`cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt
--check` all clean. `maturin develop --release` + full `pytest tests/`
(96 passed, up from 93, 1 skipped) and all sixteen examples confirmed
clean — including the three pre-existing `stroke_path` call sites
(`node_graph.py`, `positioned_graph.py`, `canvas.py`), unmodified and
still using plain 2-tuples, proving the widened parameter type is
fully backward-compatible.
