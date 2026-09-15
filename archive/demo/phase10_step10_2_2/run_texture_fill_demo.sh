#!/usr/bin/env bash
# Demo: Phase 10 Step 10.2.2 -- Texture Fill
#
# Proves real FillStyle::Texture rendering for all four shape kinds via
# ShapeRegistry::flatten_into (never hand-written RHI calls): Rectangle/
# Circle sample a bindless texture directly inside their own existing
# SDF shaders (a new texture branch mapping frag_uv onto the shape's own
# bounding box); Polygon/Path have no per-vertex style record at all, so
# they reuse the EXISTING TexturedQuad/bindless_textured.frag pipeline
# directly, with real bounding-box-normalized UVs computed at flatten
# time. Probes each shape's own upper-left and lower-right quadrant of a
# real four-quadrant flag texture to prove the UV mapping is correct.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../.."

OUT_PATH="${TRE_TEXTURE_FILL_OUTPUT:-demo/phase10_step10_2_2/texture_fill_output.png}"
export TRE_TEXTURE_FILL_OUTPUT="$OUT_PATH"

echo "Running texture_fill_demo (validation loads automatically in debug builds)..."
cargo run -p tre-rhi-vulkan --example texture_fill_demo

echo
echo "Wrote $OUT_PATH -- a 430x230 image: a rectangle, a circle, a"
echo "hexagon, and a square path, all filled with the same real"
echo "four-quadrant flag texture (red/green/blue/yellow), each one's"
echo "own bounding box correctly mapped to the texture's own UV space."
