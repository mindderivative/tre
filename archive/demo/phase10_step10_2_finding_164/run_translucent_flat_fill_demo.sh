#!/usr/bin/env bash
# Demo: REVIEW.md Finding #164 -- walking_skeleton.frag Premultiplied-Alpha Fix
#
# Proves the real, shipped FlatColor pipeline (Polygon/Path fill and
# stroke's own shader) now premultiplies its output by alpha before the
# GPU's premultiplied-alpha blend equation runs, matching every other
# real fragment shader in this codebase. Draws a genuinely translucent
# flat-fill rectangle and compares the real GPU readback against an
# independent Rust reference of the correct blend -- and against what
# the old, unfixed shader would have produced -- so the demo would
# actually have failed before the fix, not passed either way.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../.."

OUT_PATH="${TRE_TRANSLUCENT_FLAT_FILL_OUTPUT:-demo/phase10_step10_2_finding_164/translucent_flat_fill_output.png}"
export TRE_TRANSLUCENT_FLAT_FILL_OUTPUT="$OUT_PATH"

echo "Running translucent_flat_fill_demo (validation loads automatically in debug builds)..."
cargo run -p tre-rhi-vulkan --example translucent_flat_fill_demo

echo
echo "Wrote $OUT_PATH -- a 200x150 image: a translucent (alpha 120/255)"
echo "flat-filled rectangle blended over the swapchain's own real"
echo "background. The blended color matches a real, independent"
echo "premultiplied-alpha reference calculation, and is measurably"
echo "different from what the old, unfixed non-premultiplied shader"
echo "would have produced."
