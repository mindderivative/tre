#!/usr/bin/env bash
# Demo: Phase 7, Step 7.2.1 -- Real Dual-Kawase Blur RHI Capability
#
# Proves the real, hand-written RHI-level Dual-Kawase blur chain works
# end to end on real GPU hardware (REVIEW.md finding #130, closed
# 2026-09-08): draws a small opaque square into a full-size offscreen
# target, downsamples it twice (L0 -> L1 -> L2), upsamples it back twice
# (L2 -> U1 -> U0), and composites the result onto the swapchain --
# reading back real pixels to confirm the square's own interior stays
# foreground while a point just outside its original hard edge shows a
# genuine partial blur blend, not pure background.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../.."

OUT_PATH="${TRE_DUAL_KAWASE_BLUR_OUTPUT:-demo/phase7_step7_2_1/dual_kawase_blur_output.png}"
export TRE_DUAL_KAWASE_BLUR_OUTPUT="$OUT_PATH"

echo "Running dual_kawase_blur_demo (validation loads automatically in debug builds)..."
cargo run -p tre-rhi-vulkan --example dual_kawase_blur_demo

echo
echo "Wrote $OUT_PATH -- a 256x128 image: a small white square, blurred"
echo "by a real 5-stage Dual-Kawase downsample/upsample chain, composited"
echo "onto its own background. The square's own deep interior stays"
echo "foreground and its original hard edge now shows a genuine partial"
echo "blur blend outward -- both confirmed against real GPU pixel"
echo "readback, not just a visual check."
