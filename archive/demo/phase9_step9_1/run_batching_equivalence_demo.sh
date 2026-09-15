#!/usr/bin/env bash
# Demo: Phase 9, Step 9.1 -- Batching-Equivalence Correctness Proof
#
# Records the identical real scene (four non-overlapping SDF rects
# sharing the same Layer/Pipeline/Texture/clip state) twice -- once
# through `RenderingCanvas::flatten()` (the real, production batched
# path: merges into one draw call) and once through the new
# `RenderingCanvas::flatten_unbatched()` (the same real radix sort,
# merge step skipped: four draw calls) -- renders both through the
# real, unmodified `execute_frame`, and asserts the two resulting
# images are byte-for-byte identical. A mismatch would mean a real
# batching or sort-key bug, never a performance regression.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../.."

OUT_PATH="${TRE_BATCHING_EQUIVALENCE_OUTPUT:-demo/phase9_step9_1/batching_equivalence_output.png}"
export TRE_BATCHING_EQUIVALENCE_OUTPUT="$OUT_PATH"

echo "Running batching_equivalence_demo (validation loads automatically in debug builds)..."
cargo run -p tre-rhi-vulkan --example batching_equivalence_demo

echo
echo "Wrote $OUT_PATH -- a 160x120 image: four rects, rendered from the"
echo "real batched (1 draw call) path. Byte-for-byte identical to the"
echo "same scene rendered unbatched (4 draw calls) -- proving flatten()'s"
echo "merge step changes draw-call count only, never visual output."
