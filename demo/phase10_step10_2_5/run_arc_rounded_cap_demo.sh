#!/usr/bin/env bash
# Demo: Phase 10 Step 10.2.5 -- Rounded Stroke Caps on Partial-Arc Circles
#
# Proves real analytic rounded stroke caps at a partial arc's own two
# cut angles (sdf_ellipse.frag's new cap_sdf, unioned into the
# sector-clipped ellipse SDF via min()) on real GPU hardware. A
# bordered, partial-arc Circle used to have a hard, flat cutoff at each
# sweep boundary; real pixel probes a few degrees past each cut (within
# the cap's own bounded radius) confirm border color persists there
# (the rounding), while probes further past the cut confirm the wedge
# is still correctly excluded (the rounding is bounded, not a removed
# cutoff).
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../.."

OUT_PATH="${TRE_ARC_ROUNDED_CAP_OUTPUT:-demo/phase10_step10_2_5/arc_rounded_cap_output.png}"
export TRE_ARC_ROUNDED_CAP_OUTPUT="$OUT_PATH"

echo "Running arc_rounded_cap_demo (validation loads automatically in debug builds)..."
cargo run -p tre-rhi-vulkan --example arc_rounded_cap_demo

echo
echo "Wrote $OUT_PATH -- a 300x260 image: a bordered quarter-circle arc"
echo "(12 o'clock to 3 o'clock), both cut ends now rounded like a real"
echo "pen stroke cap instead of ending in a flat, hard edge."
