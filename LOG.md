# Log: M6 Phase 2 — `transform` exposure from Python

Corresponds to `BUILD_TRACKER.md` M6 Phase 2. `Node.animate()`'s real
property list (`engine-py/src/node.rs`) was `opacity`/`corner_radius`/
`elevation`/`background` only, confirmed by direct reading — no
`transform`, despite M5 Phase 1 building the whole real
`PaintProperties.transform` composition mechanism. This is the exact
gap M5 Phase 4 hit while trying to write a live-animated Python pan/zoom
example.

## Investigation before writing code

- **The Python-facing representation has to match `Interpolate for
  Affine`'s own real, already-stated limitation, not expose more than
  it correctly supports.** M5 Phase 1's own `Interpolate` impl is a
  plain componentwise coefficient lerp — exact only for the convex
  subspace of affines with no rotation/shear. A raw 6-coefficient tuple
  would let a Python caller construct a rotated/sheared `Affine` that,
  when animated, would visibly "morph" rather than sweep through a
  correct arc — a real limitation that's existed since M5 Phase 1 but
  was never actually reachable, so never exercised. **Scope narrowed,
  correctly, to match the framework's own real interpolation
  guarantee:** `transform` is exposed as a `(translate_x, translate_y,
  scale)` 3-tuple — "pan offset × zoom scale," §11.9's own text,
  verbatim — composed as `Affine::translate((tx, ty)) *
  Affine::scale(scale)`, the exact product M5 Phase 1's own pixel test
  already used. Not a reduced convenience shape layered over a fuller
  one — the actual boundary of what this framework's transform
  animation is correct for today. A rotation-capable API is additive
  whenever `Interpolate` itself gets a real decomposition.

## A real scope correction found mid-implementation, before writing the wrong test

`PLAN.md` originally called for a new `engine-render` pixel test
proving a live Python `animate("transform", ...)` call moves a rendered
pixel. Investigation (grepping every `tests/*.py` file) found this
isn't actually this project's established pattern for *any*
`animate()`-settable property — no Python-level test anywhere reads
back a rendered pixel, or even a non-`f64` property's applied value
(`background`'s own `animate()` path has never been pixel- or
value-verified from Python either, the same "isn't a single f64, and
nothing yet needs to read it back" precedent `Node.get()`'s own doc
comment already states for excluding it). The underlying paint
mechanism for `PaintProperties.transform` is already exhaustively
pixel-proven at the Rust level (M5 Phase 1's `transform_composition.
rs`) — this phase adds a new *write path* to that same field, not new
paint/hit-test logic, so there was nothing new for a pixel test to
prove. Building embedded-Python-interpreter Rust tests to force a pixel
readback from the FFI layer would have been real, disproportionate new
test infrastructure this project has never needed for any other
property — corrected in `PLAN.md` before writing it, not after.

## What happened

`engine-py/src/node.rs`: new `extract_translate_scale(to, property) ->
Result<(f64, f64, f64), EngineError>`, mirroring `extract_f64`/
`extract_color`'s exact shape. New `"transform"` arm in `animate()`'s
match, building `Affine::translate((tx, ty)) * Affine::scale(scale)`
and calling `node.paint.transform.animate_to(...)`.

New pytest coverage (matching the same FFI-wiring-only scope every
other non-`f64` `animate()` property already has): `transform` added to
the "accepts each known paint property" test, plus a new
`TypeMismatch` test for a bad-shape argument. New
`examples/pan_zoom.py`: a real, live-animated pan+zoom on one node — the
concrete thing M5 Phase 4 found it couldn't build. A real, honest
constraint discovered while writing it: `Window` has no Python-facing
way to create a plain `Container` yet (only `add_rect`/`add_splitter`/
`add_virtual_list`/`add_canvas` exist), so the example animates one
visible rect's own transform directly rather than a "camera" wrapping
children — a real, currently-buildable shape, not a compromise (a
node's own transform moves *itself* too, per §11.9's "exactly like
nested `<g transform>`" semantics, so this is a complete, real
demonstration on its own).

Full `cargo test --workspace --release` clean (unchanged Rust test
count — this phase touched no paint/hit-test logic), `cargo clippy
--workspace --all-targets -- -D warnings`, `cargo fmt --check` all
clean. `maturin develop --release` + full `pytest tests/` (74 passed,
up from 73, 1 skipped) and all nine examples (eight existing + new
`pan_zoom.py`) confirmed clean.
