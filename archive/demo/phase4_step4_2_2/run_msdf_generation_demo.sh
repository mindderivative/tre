#!/usr/bin/env bash
# Demo: Phase 4, Step 4.2.2 -- MSDF Glyph Generation
#
# Generates a real 32x32 Multi-channel Signed Distance Field via `fdsm`
# from a real glyph's real outline -- deliberately 'O', a glyph with a
# true hole, the exact case Step 4.1's ear-clipping-based rendering
# explicitly couldn't handle. No GPU, no shader, no Vulkan at all this
# sub-step (that's Step 4.2.3) -- verified entirely on the CPU via an
# independently computed median-of-3 scan (proving two separate
# inside-regions exist along one scanline, impossible for a solid shape)
# and a human-viewable preview PNG rendered by fdsm's own CPU-side
# renderer.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../.."

OUT_PATH="${TRE_MSDF_GENERATION_OUTPUT:-demo/phase4_step4_2_2/msdf_generation_output.png}"
RAW_OUT_PATH="${TRE_MSDF_RAW_OUTPUT:-demo/phase4_step4_2_2/msdf_raw_output.png}"
export TRE_MSDF_GENERATION_OUTPUT="$OUT_PATH"
export TRE_MSDF_RAW_OUTPUT="$RAW_OUT_PATH"

echo "Running msdf_generation_demo (no GPU/Vulkan needed this sub-step)..."
cargo run -p tre-text --example msdf_generation_demo

echo
echo "Wrote $OUT_PATH -- a 256x256 preview: a correctly-hollow white 'O'"
echo "ring on black, rendered from a real 32x32 MSDF via fdsm's own"
echo "CPU-side median-of-channels evaluation."
echo
echo "Wrote $RAW_OUT_PATH -- the same 32x32 MSDF's raw channel values,"
echo "nearest-neighbor upscaled, showing the actual stored distance-field"
echo "bytes rather than their rendered coverage."
