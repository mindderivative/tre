#!/usr/bin/env bash
# Demo: Phase 6, Step 6.5 -- The Combining Capstone
#
# One real recorded Canvas scene, submitted as a single execute_frame
# call, exercising real clipping, real layer compositing, and three real
# pipelines together: a rounded rect clipped directly on the swapchain
# (SdfRoundedRect), real shaped text rendered into an offscreen layer
# (MsdfText) and composited back (TexturedQuad).
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../.."

OUT_PATH="${TRE_COMBINED_SCENE_OUTPUT:-demo/phase6_step6_5/canvas_combined_scene_output.png}"
export TRE_COMBINED_SCENE_OUTPUT="$OUT_PATH"

echo "Running canvas_combined_scene_demo (validation loads automatically in debug builds)..."
cargo run -p tre-rhi-vulkan --example canvas_combined_scene_demo

echo
echo "Wrote $OUT_PATH -- a 200x150 image: a rounded rect cropped by a"
echo "real GPU scissor test (top-left), and the word \"OK\" rendered into"
echo "an offscreen layer and composited back onto the swapchain"
echo "(bottom-right) -- all driven by one real Canvas scene through one"
echo "execute_frame call."
