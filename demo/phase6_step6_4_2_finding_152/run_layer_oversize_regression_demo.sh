#!/usr/bin/env bash
# Demo: REVIEW.md Finding #152 -- PushLayer/PopLayer Oversized-Borrow Fix
#
# Proves the real, shipped Step 6.4.2 PushLayer/PopLayer compositing no
# longer silently drops a layer's own content whenever RhiDevice::
# acquire_transient_target's documented "oversized borrow" fallback
# hands back a texture larger than requested: push/pop a first, larger
# layer (a fresh pool allocation, then released), then push/pop a
# second, smaller, never-before-requested layer -- the pool hands back
# the first layer's own freed, larger texture, and the second layer's
# content must still composite correctly.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../.."

OUT_PATH="${TRE_LAYER_OVERSIZE_REGRESSION_OUTPUT:-demo/phase6_step6_4_2_finding_152/layer_oversize_regression_output.png}"
export TRE_LAYER_OVERSIZE_REGRESSION_OUTPUT="$OUT_PATH"

echo "Running layer_oversize_regression_demo (validation loads automatically in debug builds)..."
cargo run -p tre-rhi-vulkan --example layer_oversize_regression_demo

echo
echo "Wrote $OUT_PATH -- a 300x200 image: a first, larger (200x150) layer,"
echo "released back to the transient pool, followed by a second, smaller"
echo "(50x40) layer that borrows the first one's freed texture. The"
echo "second layer's own content composites correctly at its own real,"
echo "requested on-screen position and size -- confirmed against real GPU"
echo "pixel readback, not just a visual check."
