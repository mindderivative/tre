#!/usr/bin/env bash
# Demo: Phase 3, Step 3.3.3 -- Stencil-and-Cover Fallback Rendering
#
# Renders a genuinely self-intersecting pentagram (which tre_svg::triangulate
# provably cannot handle -- confirmed by the example itself) via a real
# two-pass stencil-and-cover GPU technique, under both NonZero and EvenOdd
# fill rules. Reads back real pixels proving the textbook case: the
# pentagram's central pentagon is filled under NonZero (winding number 2)
# but empty under EvenOdd (crossed an even number of times) -- the two
# fill rules genuinely disagree on this exact shape. Writes the NonZero
# render as a PNG for visual inspection.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../.."

OUT_PATH="${TRE_STENCIL_AND_COVER_OUTPUT:-demo/phase3_step3_3_3/stencil_and_cover_output.png}"
export TRE_STENCIL_AND_COVER_OUTPUT="$OUT_PATH"

echo "Running stencil_and_cover_demo (validation loads automatically in debug builds)..."
cargo run -p tre-rhi-vulkan --example stencil_and_cover_demo

echo
echo "Wrote $OUT_PATH -- a 300x300 image: a solid white five-pointed star"
echo "(the pentagram filled under the NonZero rule, including its central"
echo "pentagon) on a dark background."
