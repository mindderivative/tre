#!/usr/bin/env bash
# Demo: Phase 7, Step 7.1 -- Linear sRGB Conversions & HDR
#
# Proves the real shader-side sRGB-to-linear conversion fix (REVIEW.md
# finding #92, deferred here since Phase 4 Step 4.2.1): draws a fully
# opaque rect with a genuinely non-fixed-point color, reads back real
# GPU pixels, and confirms the round-tripped color matches its own
# authored value -- while also computing what the old, unfixed
# double-encoding would have produced, proving the fix has real,
# measurable effect.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../.."

OUT_PATH="${TRE_LINEAR_COLOR_OUTPUT:-demo/phase7_step7_1/linear_color_output.png}"
export TRE_LINEAR_COLOR_OUTPUT="$OUT_PATH"

echo "Running linear_color_demo (validation loads automatically in debug builds)..."
cargo run -p tre-rhi-vulkan --example linear_color_demo

echo
echo "Wrote $OUT_PATH -- a 200x150 image: a solid rgb(150,100,200) rect,"
echo "round-tripped through the fixed shader-side sRGB<->linear"
echo "conversion and the swapchain's own hardware encode-on-store,"
echo "reading back as its own exact authored color -- before this fix,"
echo "it would have read back as rgb(202,168,229) instead."
