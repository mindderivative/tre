#!/usr/bin/env bash
# Demo: Phase 3, Step 3.2 -- Analytical SDF Rounded Rectangles
#
# Renders one real rounded rectangle (300x200, corner radius 40) through a
# genuine analytical SDF fragment shader with fwidth-based anti-aliasing --
# not the flat-color placeholder every prior example uses. The example
# itself reads back the actual rendered pixels and asserts: the interior
# is exactly the foreground color, a point in the bounding box's corner
# well outside the rounding arc is exactly the background clear color, and
# a real partial-alpha blend exists near the rounded corner -- not just
# "it compiles and doesn't crash." Writes a PNG for visual inspection too.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../.."

OUT_PATH="${TRE_SDF_ROUNDED_RECT_OUTPUT:-demo/phase3_step3_2/sdf_rounded_rect_output.png}"
export TRE_SDF_ROUNDED_RECT_OUTPUT="$OUT_PATH"

echo "Running sdf_rounded_rect_demo (validation loads automatically in debug builds)..."
cargo run -p tre-rhi-vulkan --example sdf_rounded_rect_demo

echo
echo "Wrote $OUT_PATH -- a 340x240 image: a 300x200 white rounded rectangle"
echo "(corner radius 40) on a dark background, with real anti-aliased"
echo "corners rendered by the sdf_rounded_rect shader pair."
