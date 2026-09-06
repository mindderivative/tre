# Demo: Phase 3, Step 3.1 -- SIMD Affine Matrix Math

```bash
./demo/phase3_step3_1/run_affine_tests.sh
```

Every prior demo in this project has proven something by driving real
Vulkan hardware -- a screenshot, a pixel readback, a validation-layer run.
This step has none of that, deliberately: `tre-math` has no `unsafe`, no
FFI, no GPU, no display server. It's pure functions over `f32`s, and the
unit test suite itself is the verification artifact.

**What's actually being proven:** `tre-math` gained a real `Affine2` type
(TECHNICAL.md Section 7.2's exact translation/rotation/scale formula,
stored as six `f32`s rather than a dense 3x3 -- the bottom row of a genuine
affine transform is always `[0, 0, 1]`), plus `compose_batch`, which
processes 8 parent-child transform pairs at a time via
`wide::f32x8::mul_add`. Running the suite exercises:

- Hand-computed correctness for translation, rotation, scale (including a
  negative-scale flip), and the combined formula.
- That composition isn't commutative, and applies the child's transform
  before the parent's, in the documented order.
- **The real proof point:** `compose_batch`'s SIMD path checked against a
  plain scalar reference implementation of the identical math, across
  slice lengths `0, 1, 7, 8, 9, 16, 17` -- every remainder case relative to
  the 8-wide SIMD chunk, so the "leftover" scalar fallback path is
  genuinely exercised, not just the common in-multiples-of-8 case.

**No scene graph exists yet to call this with real parent-child data** --
`compose_batch` operates on plain slices, proven against synthetic test
data. This matches the same "build the tested primitive before its exact
consumer exists" pattern already used for `tre_memory::SpscRingBuffer`
(Phase 1) and the dynamic ring buffer/transient pool (Phase 2 Step 1).

**Why the SIMD-vs-scalar comparison uses an epsilon, not exact equality:**
`wide::f32x8::mul_add` is genuine hardware FMA (a single rounding) on a
target that supports it, or a separate multiply-then-add (two roundings)
otherwise -- so the two code paths can legitimately differ in the last bit
or two of an `f32`. A test asserting bit-exact equality would really only
be testing today's specific CPU, not the math.
