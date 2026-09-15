#!/usr/bin/env bash
# Demo: Phase 7, Step 7.2.2 -- Wire Dual-Kawase Blur to push_layer/pop_layer
#
# Proves a real, GPU-accelerated Dual-Kawase blur, requested entirely
# through Canvas::push_layer(&LayerDesc { blur: true, .. }), works end
# to end: push_layer/draw/pop_layer are the only calls that build the
# scene, and execute_frame alone drives every RHI call (including the
# real blur chain, RhiCommandBuffer::apply_layer_blur) from the
# resulting IR -- no hand-written RHI calls anywhere in the caller.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../.."

OUT_PATH="${TRE_LAYER_BLUR_OUTPUT:-demo/phase7_step7_2_2/layer_blur_output.png}"
export TRE_LAYER_BLUR_OUTPUT="$OUT_PATH"

echo "Running layer_blur_demo (validation loads automatically in debug builds)..."
cargo run -p tre-rhi-vulkan --example layer_blur_demo

echo
echo "Wrote $OUT_PATH -- a 256x128 image: a small white square, drawn"
echo "inside a push_layer(&LayerDesc { blur: true, .. }) layer, blurred"
echo "by the real Dual-Kawase chain, and composited onto the swapchain."
echo "The square's own deep interior stays foreground and its original"
echo "hard edge now shows a genuine partial blur blend outward -- both"
echo "confirmed against real GPU pixel readback, recorded and driven"
echo "entirely through Canvas/execute_frame."
