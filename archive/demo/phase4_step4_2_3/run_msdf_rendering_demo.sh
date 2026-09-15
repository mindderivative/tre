#!/usr/bin/env bash
# Demo: Phase 4, Step 4.2.3 -- MSDF Evaluation Shader & Real Anti-Aliased
# Glyph Render
#
# The payoff moment for the whole Step 4.2 arc: a real glyph ('O', the
# same hole-having case from Step 4.2.2), its real MSDF, uploaded as a
# real GPU texture and rendered through a real evaluation shader
# (TECHNICAL.md Section 5.3's exact canonical formula) at a generous
# ~7x on-screen magnification. Reads back real pixels: a scan across the
# glyph's own center proves both a real hollow ring (white ring material,
# background in the hole) and real sub-pixel anti-aliasing (genuinely
# intermediate, blended pixels at the ring's own edges -- not a hard
# binary transition, the concrete fix for the jagged 'X' observed in
# Step 4.1's demo).
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../.."

OUT_PATH="${TRE_MSDF_RENDERING_OUTPUT:-demo/phase4_step4_2_3/msdf_rendering_output.png}"
export TRE_MSDF_RENDERING_OUTPUT="$OUT_PATH"

echo "Running msdf_rendering_demo (validation loads automatically in debug builds)..."
cargo run -p tre-rhi-vulkan --example msdf_rendering_demo

echo
echo "Wrote $OUT_PATH -- a 300x300 image: a real 'O' glyph, correctly"
echo "hollow and smoothly anti-aliased, rendered from a real MSDF texture"
echo "through a real GPU shader at roughly 7x magnification."
