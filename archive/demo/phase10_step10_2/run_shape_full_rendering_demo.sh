#!/usr/bin/env bash
# Demo: Phase 10, Step 10.2 -- Full Shape Rendering Support
#
# Real GPU render, through ShapeRegistry (never hand-written RHI calls):
# a Rectangle with a sharp top-left/bottom-right corner pair, a rounded
# top-right/bottom-left pair, and a real border; plus a bordered,
# 270-degree (three-quarter) Circle. Reads back real pixels and asserts
# 8 real correctness checks -- the sharp corner staying sharp, the
# rounded corner genuinely excised by its own radius, border vs. fill
# color on both shapes, and the circle's own excluded arc wedge.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../.."

OUT_PATH="${TRE_SHAPE_FULL_RENDERING_OUTPUT:-demo/phase10_step10_2/shape_full_rendering_output.png}"
export TRE_SHAPE_FULL_RENDERING_OUTPUT="$OUT_PATH"

echo "Running shape_full_rendering_demo (validation loads automatically in debug builds)..."
cargo run -p tre-rhi-vulkan --example shape_full_rendering_demo

echo
echo "Wrote $OUT_PATH -- a 320x180 image: a non-uniform-corner bordered"
echo "rectangle and a bordered partial-arc circle, both rendered through"
echo "ShapeRegistry's real flattening pass."
