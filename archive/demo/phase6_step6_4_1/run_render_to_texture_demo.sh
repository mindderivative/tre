#!/usr/bin/env bash
# Demo: Phase 6, Step 6.4.1 -- Real RHI Render-to-Texture Capability
#
# The first time this project has ever rendered into an offscreen target
# and sampled the result back, on real Vulkan hardware. No Canvas/IR
# involvement -- every RHI call is hand-written: acquire a transient
# Rgba16Float target, redirect rendering into it via
# begin_render_to_texture/end_render_to_texture (a real rounded rect,
# via the existing sdf_rounded_rect pipeline), register it bindless,
# resume swapchain rendering, then composite it back as a textured quad
# via the existing bindless-textured pipeline and the default
# premultiplied-alpha blend state -- no special-casing needed for a
# correct "over" composite.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../.."

OUT_PATH="${TRE_RENDER_TO_TEXTURE_OUTPUT:-demo/phase6_step6_4_1/render_to_texture_output.png}"
export TRE_RENDER_TO_TEXTURE_OUTPUT="$OUT_PATH"

echo "Running render_to_texture_demo (validation loads automatically in debug builds)..."
cargo run -p tre-rhi-vulkan --example render_to_texture_demo

echo
echo "Wrote $OUT_PATH -- a 200x150 image: a 100x80 rounded rect, rendered"
echo "into an offscreen transient target, composited back onto the"
echo "swapchain at (50,40) -- real foreground where the rect's own"
echo "geometry is, real background showing through everywhere the layer"
echo "was genuinely cleared to transparent."
