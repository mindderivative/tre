#!/usr/bin/env bash
# Demo: Phase 4, Step 4.3.3 -- Atlas LRU Eviction Policy & Wiring
#
# The capstone of the whole Step 4.3 arc. A real 64x64 atlas exactly
# holds four 32x32 MSDF glyphs (zero leftover space, 100% capacity, past
# DESIGN.md Section 10.2's 85% eviction trigger). One glyph is kept
# fresh via a real lookup at a later frame; the other three are left
# untouched. Requesting a fifth glyph 700 frames later forces a real
# eviction pass inside AtlasOwner: the three stale glyphs are reclaimed,
# the fresh one survives unchanged, and the new one lands in real, reused
# atlas space -- then the finished atlas is uploaded as one real GPU
# texture and the two survivors rendered through the existing, unmodified
# msdf.frag pipeline (Step 4.2.3).
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../.."

OUT_PATH="${TRE_ATLAS_EVICTION_OUTPUT:-demo/phase4_step4_3_3/atlas_eviction_output.png}"
export TRE_ATLAS_EVICTION_OUTPUT="$OUT_PATH"

echo "Running atlas_eviction_demo (validation loads automatically in debug builds)..."
cargo run -p tre-rhi-vulkan --example atlas_eviction_demo

echo
echo "Wrote $OUT_PATH -- a 200x100 image: 'G' (the entry kept fresh, survived"
echo "eviction unchanged) and 'H' (the new glyph, placed into real, reclaimed"
echo "atlas space after 'L'/'Y'/'P' were evicted for being idle past 600 frames)."
