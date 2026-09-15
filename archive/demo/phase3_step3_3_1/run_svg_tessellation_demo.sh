#!/usr/bin/env bash
# Demo: Phase 3, Step 3.3.1 -- SVG Ingestion & Ear-Clipping Tessellation
#
# Parses a real SVG document (a five-pointed star path) via the `usvg`
# crate, tessellates it with tre-svg's own hand-rolled ear-clipping
# triangulator, and renders the result through the pre-existing
# flat-color pipeline -- no new shader needed, since a plain triangle
# soup has no SDF to evaluate. The example itself reads back real
# rendered pixels and asserts the star's interior is filled and one of
# its concave notches is not, proving the triangulation is topologically
# correct, not just "some triangles got drawn somewhere." Writes a PNG
# for visual inspection too.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../.."

OUT_PATH="${TRE_SVG_TESSELLATION_OUTPUT:-demo/phase3_step3_3_1/svg_tessellation_output.png}"
export TRE_SVG_TESSELLATION_OUTPUT="$OUT_PATH"

echo "Running svg_tessellation_demo (validation loads automatically in debug builds)..."
cargo run -p tre-rhi-vulkan --example svg_tessellation_demo

echo
echo "Wrote $OUT_PATH -- a 300x300 image: a white five-pointed star"
echo "(tessellated from real SVG path data by tre-svg's ear-clipping"
echo "triangulator) on a dark background."
