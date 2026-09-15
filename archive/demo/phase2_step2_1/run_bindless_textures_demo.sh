#!/usr/bin/env bash
# Demo: Phase 2, Step 2.1 -- Vulkan Bindless Texture Arrays
#
# Uploads three distinct real textures via RhiDevice::create_texture and
# draws each with its own draw call through the SAME bound pipeline and
# SAME bound bindless descriptor set (VK_EXT_descriptor_indexing) -- only
# the push-constant texture index changes between draws, never a
# descriptor-set rebind. A fourth draw explicitly rebinds the "no texture"
# sentinel to prove Phase 0's flat vertex-color path still works unchanged.
#
# The example itself asserts the actual output pixel colors match each
# uploaded texture's known content (not just that the draw calls didn't
# crash) and writes a PNG for visual inspection.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../.."

OUT_PATH="${TRE_BINDLESS_TEXTURES_OUTPUT:-demo/phase2_step2_1/bindless_textures_output.png}"
export TRE_BINDLESS_TEXTURES_OUTPUT="$OUT_PATH"

echo "Running bindless_textures_demo (validation loads automatically in debug builds -- Phase 2 Step 2)..."
cargo run -p tre-rhi-vulkan --example bindless_textures_demo

echo
echo "Wrote $OUT_PATH -- four 160x160 columns: red, green, blue (three"
echo "separate real textures, same bound descriptor set) and yellow (no"
echo "texture bound, vertex-color fallback)."
