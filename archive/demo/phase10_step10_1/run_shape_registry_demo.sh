#!/usr/bin/env bash
# Demo: Phase 10, Step 10.1 -- Efficient Shape Primitives
#
# Real GPU pixel-diff: the exact same rounded rectangle, drawn once
# directly through today's real immediate-mode RenderingCanvas::
# draw_rounded_rect, and once through the new retained-mode
# ShapeRegistry (insert a Rectangle, then flatten_into). Reads back real
# pixels and asserts the two renders are byte-for-byte identical --
# proving the new shape-primitive layer is a faithful convenience
# wrapper over the existing IR/sort/batch/RHI pipeline, never a second,
# divergent rendering path.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../.."

OUT_PATH="${TRE_SHAPE_REGISTRY_OUTPUT:-demo/phase10_step10_1/shape_registry_output.png}"
export TRE_SHAPE_REGISTRY_OUTPUT="$OUT_PATH"

echo "Running shape_registry_demo (validation loads automatically in debug builds)..."
cargo run -p tre-rhi-vulkan --example shape_registry_demo

echo
echo "Wrote $OUT_PATH -- a 340x240 image: one rounded rectangle, rendered"
echo "via the new ShapeRegistry retained-mode layer. Byte-for-byte"
echo "identical to the same shape drawn directly via draw_rounded_rect."
