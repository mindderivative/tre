#!/usr/bin/env bash
# Demo: Phase 4, Step 4.2.4 -- Multi-Window Atlas Concurrency
#
# The capstone of the whole Step 4.2 arc. Three real producer threads
# concurrently request MSDF atlas space for the six letters of "GLYPHS"
# from a real cascade font, through the real bounded MPSC ring buffer; a
# single real background AtlasOwner thread drains requests, performs real
# Guillotine packing (Step 4.2.1) and real MSDF generation (Step 4.2.2)
# for each, and publishes results into the real SWMR slot table. After
# every producer rejoins, every letter's placement is verified
# non-overlapping and byte-identical to an independently-regenerated MSDF
# for that same glyph -- then the whole shared atlas is uploaded as one
# real GPU texture and every letter rendered in a single draw call via
# the existing, unmodified msdf.frag pipeline (Step 4.2.3).
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../.."

OUT_PATH="${TRE_ATLAS_CONCURRENCY_OUTPUT:-demo/phase4_step4_2_4/atlas_concurrency_output.png}"
export TRE_ATLAS_CONCURRENCY_OUTPUT="$OUT_PATH"

echo "Running atlas_concurrency_demo (validation loads automatically in debug builds)..."
cargo run -p tre-rhi-vulkan --example atlas_concurrency_demo

echo
echo "Wrote $OUT_PATH -- a 460x100 image: the word \"GLYPHS\", rendered"
echo "entirely from concurrent atlas insertion into one shared texture."
