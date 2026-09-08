#!/usr/bin/env bash
# Demo: Phase 6, Step 6.4.2 -- Wiring push_layer/pop_layer to Real
# Render-to-Texture
#
# Reproduces Step 6.4.1's own render-to-texture-demo scene and pixel
# coordinates, but recorded entirely through Canvas/execute_frame --
# push_layer, one draw_rounded_rect in the layer's own local space,
# pop_layer, flatten(), one execute_frame call. No hand-written RHI
# calls anywhere in this demo.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../.."

OUT_PATH="${TRE_LAYER_COMPOSITE_OUTPUT:-demo/phase6_step6_4_2/canvas_layer_composite_output.png}"
export TRE_LAYER_COMPOSITE_OUTPUT="$OUT_PATH"

echo "Running canvas_layer_composite_demo (validation loads automatically in debug builds)..."
cargo run -p tre-rhi-vulkan --example canvas_layer_composite_demo

echo
echo "Wrote $OUT_PATH -- a 200x150 image: a 100x80 rounded rect, recorded"
echo "via push_layer/draw_rounded_rect/pop_layer, rendered into an"
echo "offscreen transient target by execute_frame's own PushLayer"
echo "handling, and composited back onto the swapchain at (50,40) by its"
echo "own PopLayer handling -- real foreground where the rect's own"
echo "geometry is, real background showing through everywhere the layer"
echo "was genuinely cleared to transparent."
