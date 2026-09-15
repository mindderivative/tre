#!/usr/bin/env bash
# Demo: Phase 10 Step 10.2 follow-up -- lyon-backed Path fill (with a
# real hole) and Path/Polygon border/stroke rendering.
#
# Real GPU render, through ShapeRegistry (never hand-written RHI calls):
# a "donut" Path (two subpaths, wound oppositely -- an outer square with
# a real subtractive hole) and a bordered hexagon Polygon. Reads back
# real pixels and asserts the ring fills, the hole stays background, and
# both shapes' borders render in their own border_color.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../.."

OUT_PATH="${TRE_PATH_AND_POLYGON_OUTPUT:-demo/phase10_step10_2_followup/path_and_polygon_output.png}"
export TRE_PATH_AND_POLYGON_OUTPUT="$OUT_PATH"

echo "Running path_and_polygon_demo (validation loads automatically in debug builds)..."
cargo run -p tre-rhi-vulkan --example path_and_polygon_demo

echo
echo "Wrote $OUT_PATH -- a 320x200 image: a donut-shaped Path (a real"
echo "compound shape with a hole) and a bordered hexagon Polygon, both"
echo "rendered through ShapeRegistry's real flattening pass via lyon."
