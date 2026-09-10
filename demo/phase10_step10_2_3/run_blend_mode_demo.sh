#!/usr/bin/env bash
# Demo: Phase 10 Step 10.2.3 -- Non-Normal Blend Modes
#
# Proves real BlendMode rendering (Multiply/Screen/Overlay/SoftLight/
# ColorDodge) for Polygon/Path solid fill, via VK_KHR_dynamic_rendering_
# local_read -- a real framebuffer read (subpassLoad against a real
# VK_DESCRIPTOR_TYPE_INPUT_ATTACHMENT descriptor), not a VkBlendOp
# selection. VK_EXT_blend_operation_advanced, this step's originally-
# planned primary path, is not implemented by RADV (this project's own
# real dev GPU/driver -- see documentation/REVIEW.md for the full
# account), so this demo exists to prove the real, portable alternative
# actually works on real hardware. Six polygon swatches (one per
# BlendMode, including Normal as a routing-correctness control) are
# drawn over a solid background rectangle; every swatch's own center
# pixel is compared against an independent Rust reference implementation
# of the exact W3C blend formula.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../.."

OUT_PATH="${TRE_BLEND_MODE_OUTPUT:-demo/phase10_step10_2_3/blend_mode_output.png}"
export TRE_BLEND_MODE_OUTPUT="$OUT_PATH"

echo "Running blend_mode_demo (validation loads automatically in debug builds)..."
cargo run -p tre-rhi-vulkan --example blend_mode_demo

echo
echo "Wrote $OUT_PATH -- a 600x200 image: a background rectangle with six"
echo "polygon swatches drawn on top, one per BlendMode (Normal, Multiply,"
echo "Screen, Overlay, SoftLight, ColorDodge), each one's own center pixel"
echo "verified against an independent Rust reference of the real W3C"
echo "blend formula."
