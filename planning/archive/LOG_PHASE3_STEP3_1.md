# Log: Phase 3, Step 3.1 -- SIMD Affine Matrix Math

## A genuinely different kind of step: nothing broke at runtime

Every prior step in this project (Phases 0-2) surfaced at least one real
bug caught by actually running the code -- the Vulkan validation layer, a
deliberate test, or a live run. This step found none. `tre-math` has no
`unsafe`, no FFI, no GPU, no display server -- just pure functions over
`f32`s, backed by real unit tests from the start. The compiler and the test
suite caught everything there was to catch, before "running" was ever a
separate step from "compiling."

## Compile-time issues found and fixed (not given REVIEW.md finding
numbers, matching this project's standing practice of reserving those for
issues a real run or review surfaces)

1. **`clippy::similar_names` on `out_tx`/`out_ty`.** These are exactly
   `Affine2`'s own field names (`tx`/`ty`), not an accidental near-typo of
   each other -- `#[allow(clippy::similar_names)]` added on `compose_batch`
   with a comment explaining why, rather than renaming to something less
   clear just to satisfy the lint.

2. **`clippy::doc_markdown` on a LaTeX-style doc comment.** The original
   `Affine2` doc comment wrote the matrix using LaTeX subscripts
   (`t_x`/`t_y`), which clippy's doc-markdown lint reads as unbackticked
   code-like identifiers. Rewritten in plain backticked notation
   (`` `[[a, b, tx], [c, d, ty], [0, 0, 1]]` ``) matching the actual field
   names directly -- clearer for a reader of the generated docs anyway,
   not just a lint workaround.

3. **`clippy::float_cmp` on four tests using exact `assert_eq!`.**
   Translation and scale by small integer values involve no floating-point
   rounding at all, so exact equality is the correct check there -- unlike
   the rotation/composition tests, which go through `sin_cos` and are only
   approximately exact, and use an epsilon-tolerance helper instead.
   `#[allow(clippy::float_cmp)]` added to the four exact-arithmetic tests
   specifically, with a comment distinguishing them from the epsilon-based
   ones, rather than blanket-loosening every test's precision.

## What worked without needing a fix

- The SIMD batch-composition formula (derived by hand from the standard
  2D affine composition rule, then implemented via `wide::f32x8::mul_add`)
  matched the scalar reference implementation on the very first test run,
  across every remainder length tested (`0, 1, 7, 8, 9, 16, 17`) -- no
  off-by-one in the chunking, no lane-ordering mistake in the
  structure-of-arrays gather/scatter.
- `wide` (added as a real dependency for the first time in this project)
  resolved and built cleanly against this workspace's pinned `rust-version
  = 1.75` on the first attempt, correctly selecting the 0.7.x line over
  the newer 1.x (which needs a newer rustc than this workspace targets).
