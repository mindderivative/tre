#!/usr/bin/env bash
# Demo: Phase 3, Step 3.1 -- SIMD Affine Matrix Math
#
# Unlike every Phase 2 demo, there is no GPU-facing behavior here to
# screenshot or read back pixels from -- tre-math has no unsafe, no FFI,
# no GPU, no display server. The unit tests themselves are the
# verification artifact: real math checked against hand-computed values
# and a scalar reference implementation, not "it compiles."
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../.."

echo "Running tre-math's Affine2/compose_batch test suite..."
cargo test -p tre-math

echo
echo "Notably: compose_batch_matches_scalar_reference_across_every_simd_remainder"
echo "compares wide::f32x8::mul_add's SIMD-batched composition against a plain"
echo "scalar reference implementation of the identical formula, across slice"
echo "lengths 0, 1, 7, 8, 9, 16, and 17 -- every remainder case relative to the"
echo "8-wide SIMD chunk size -- with an epsilon tolerance, not exact equality,"
echo "since hardware FMA and a separate multiply-then-add can legitimately"
echo "differ in the last bit or two of an f32."
