# Log: Phase 3, Step 3.3.2 -- SIMD Path-Morphing Interpolation

## A genuinely smooth implementation -- one real bug, caught by the unit
tests before the demo ever ran

Unlike Step 3.3.1 (two real ear-clipping bugs, both needing a non-convex
demo to surface), this step's SIMD batch-lerp math worked correctly on
the first real test run. The one bug found was in a hand-written unit
test's own assumption, not in the implementation.

## Bug: assumed bit-exact round-tripping at `t=1.0`

`lerp_points_batch_at_t_zero_and_one_returns_the_endpoints_exactly`
originally asserted `assert_eq!` (exact equality) between the SIMD
output at `t=1.0` and the raw `to` keyframe values. This failed
immediately on real sample data: `-1.4` round-tripped through
`(to - from).mul_add(1.0, from)` as `-1.4000001`, and `0.0` came back as
`-0.0` (equal under IEEE `==` semantics for most operations, but not
under `assert_eq!`'s exact array/slice comparison).

`(b - a).mul_add(1.0, a)` is mathematically `b`, but the fused multiply-add
only fuses the multiply-and-add into a single rounding -- the `b - a`
subtraction has already rounded once, separately, before the FMA ever
runs. Two roundings composed don't always cancel back to the original
value bit-for-bit, the same category of issue `compose_batch`'s own test
suite already documented for FMA vs. separate-operation composition.

**Fix:** rewrote the test to compare within `EPSILON`, matching every
other float-comparison test in this crate, instead of asserting exact
equality on a computed (not literal) value.

## What worked without needing a fix

- `lerp_points_batch` itself, including the generalized `gather<T>` helper
  shared with `compose_batch` -- matched its scalar reference across
  every SIMD remainder length (`0, 1, 7, 8, 9, 16, 17`) on the first run.
- `tre-svg::morph`'s topology validation and the `SvgError::TopologyMismatch`
  variant worked correctly immediately -- `t=0`/`t=1`/`t=0.5` unit tests
  and the mismatched-count rejection test all passed without iteration.
- `svg_morph_demo`'s two-probe-point design (one point inside `from` but
  not `to`, one point outside both keyframes but inside their exact
  midpoint shape) was verified independently in Python against a
  ray-casting point-in-polygon reference *before* writing any Rust code,
  and the actual GPU render matched that prediction exactly on the first
  run -- no debugging needed for the demo itself.

## Verification performed

- `cargo test --workspace`: all tests pass, including 3 new `tre-math`
  tests (`lerp_points_batch` SIMD-remainder comparison, exact-endpoint
  epsilon check, panics-on-mismatch) and 3 new `tre-svg` tests (`morph`
  at t=0/1/0.5, mismatched-count rejection).
- `cargo fmt --check` / `cargo clippy --workspace --all-targets -- -D
  warnings`: clean.
- `svg_morph_demo` run manually against the real GPU (AMD/Radeon, Wayland
  session) under the Vulkan validation layer: all three per-`t` pixel
  assertions pass; output PNG (the `t=0.5` frame) visually inspected and
  shows the expected tilted-quadrilateral midpoint shape.
- All 8 pre-existing Vulkan examples (including Step 3.3.1's
  `svg_tessellation_demo`) re-run manually, zero validation errors.
