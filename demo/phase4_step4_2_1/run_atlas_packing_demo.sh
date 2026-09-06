#!/usr/bin/env bash
# Demo: Phase 4, Step 4.2.1 -- Guillotine Atlas Bin-Packing
#
# Packs a deliberately varied sequence of rectangle sizes (mimicking a mix
# of glyph- and icon-sized atlas entries) into a 256x256 atlas via a real
# Guillotine bin-packer, then renders every successfully-placed rectangle
# as its own distinct flat-colored quad through the existing, unmodified
# flat-color pipeline. Reads back real pixels: each rectangle's own center
# is exactly its own color, and a point the packer left unpacked is still
# the background -- proving the returned placements are real, correctly
# non-overlapping coordinates, not just "some `Option` came back `Some`."
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../.."

OUT_PATH="${TRE_ATLAS_PACKING_OUTPUT:-demo/phase4_step4_2_1/atlas_packing_output.png}"
export TRE_ATLAS_PACKING_OUTPUT="$OUT_PATH"

echo "Running atlas_packing_demo (validation loads automatically in debug builds)..."
cargo run -p tre-rhi-vulkan --example atlas_packing_demo

echo
echo "Wrote $OUT_PATH -- a 256x256 image: 12 distinctly-colored"
echo "rectangles, packed by a real Guillotine bin-packer with no overlaps."
