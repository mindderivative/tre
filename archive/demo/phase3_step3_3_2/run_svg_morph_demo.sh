#!/usr/bin/env bash
# Demo: Phase 3, Step 3.3.2 -- SIMD Path-Morphing Interpolation
#
# Morphs between two independently-parsed SVG keyframe shapes (a diamond
# and a square, same vertex count) via tre-math's real SIMD batch lerp
# (wide::f32x8) at t = 0.0, 0.5, and 1.0, re-triangulating fresh at each
# frame. The example itself reads back real rendered pixels using two
# probe points chosen so all three renders are pairwise distinguished --
# proving t=0.5 is a genuine, distinct interpolated shape, not a snap to
# either endpoint. Writes the t=0.5 frame as a PNG for visual inspection.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../.."

OUT_PATH="${TRE_SVG_MORPH_OUTPUT:-demo/phase3_step3_3_2/svg_morph_output.png}"
export TRE_SVG_MORPH_OUTPUT="$OUT_PATH"

echo "Running svg_morph_demo (validation loads automatically in debug builds)..."
cargo run -p tre-rhi-vulkan --example svg_morph_demo

echo
echo "Wrote $OUT_PATH -- a 300x300 image: the t=0.5 midpoint shape between"
echo "a diamond and a square, a tilted quadrilateral produced by a real"
echo "SIMD per-vertex interpolation, not a hand-authored shape."
