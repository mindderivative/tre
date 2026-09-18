# Log: M30 Phase 3 Step 2 — Progress Indicator (Linear and Circular)

## `Slider`'s own real shape, mirrored

`LinearProgressState`/`CircularProgressState` both carry `value:
Animated<f64>` directly — the identical "the animated field is the
value" precedent `SliderState.thumb_position` already establishes.
The one real, deliberate anatomy difference: a progress indicator is
never draggable, since it only ever displays a value the app computes
elsewhere (a download percentage, a loading state), never something a
user directly manipulates.

## Real MD3 data, verified before writing any code

Checked Material Web's own real token source for both shapes.
Linear (`_md-comp-linear-progress-indicator.scss`): a flat,
`corner-none` bar — genuinely not rounded, unlike almost every other
shape this catalog has used so far — 4dp track (`surface_container_
highest`) and a separate 4dp indicator (`primary`). Circular
(`_md-comp-circular-progress-indicator.scss`): 48dp size, 4dp stroke,
`primary`. **A real, confirmed finding:** no `track-color` token
exists for the circular indicator at all — real MD3 genuinely paints
no background ring behind the arc, unlike the linear indicator's own
separate track. Confirmed rather than assumed consistent between the
two shapes.

## Real geometry, verified by a passing pixel test on the first try

The circular indicator's arc uses kurbo's own real `Arc` type
(`center`, `radii`, `start_angle`, `sweep_angle`, `x_rotation`) —
confirmed it implements the `Shape` trait before using it, so the
existing `to_path`/`stroke_path` pipeline needed no new machinery.
Start angle `-PI/2` (12 o'clock) with a positive sweep for clockwise
motion in this engine's y-down screen coordinates — `progress_paint.
rs`'s own quarter-value test (12 o'clock start, exactly reaching 3
o'clock at a real 0.25 sweep, nothing at 6 o'clock) passed on the
first run, confirming the angle-direction convention was correct
without needing a second attempt.

## A real ticking gap, checked for proactively this time

`RadioButton` (Phase 2) found that `Tree::tick_all`'s per-`NodeKind`
ticking uses `if let` arms, not an exhaustive `match` — a missing arm
compiles clean and just silently never animates. Checked for this
directly before considering either new `NodeKind` done this time,
rather than finding it via a second failing test.

## No accessibility mirror — checked, not an oversight

`Checkbox`/`RadioButton`/`Switch` each got a `set_toggled` mirror.
Checked `Slider`'s own real precedent for `thumb_position` first and
found it has no accessibility mirror at all — so neither progress
indicator gets one either, honest parity with the closest real
analogue rather than a new, inconsistent gap.

## Verification

`cargo check --workspace --all-targets`, `cargo clippy --workspace
--all-targets -- -D warnings`, `cargo fmt --check` — all clean. `cargo
test --workspace --release`: 43 binaries, all green (`progress_paint.
rs`'s 4 tests included). `maturin develop --release` rebuilt. `pytest
tests/`: 286 passed, 1 skipped (8 new in `test_progress.py`, zero
regressions). All 40 examples and the showcase demo re-run clean.
`mypy --strict` clean against `examples/progress.py`.
