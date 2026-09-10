#!/usr/bin/env bash
# Demo: Phase 10 Step 10.2.1 -- Gradient Fill (Linear + Radial)
#
# Proves real FillStyle::Gradient rendering for all four shape kinds via
# ShapeRegistry::create_gradient/flatten_into (never hand-written RHI
# calls): a linear-gradient Rectangle with a real border, a
# radial-gradient Circle with a real border, and a linear-gradient
# hexagon Polygon routed through the entirely separate GradientFill
# pipeline (Polygon/Path have no per-vertex style record the way
# Rectangle/Circle do). Every probed pixel is compared against an
# independent Rust reference implementation of the exact same
# premultiplied, linear-space gradient evaluation the real shaders
# perform.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../.."

OUT_PATH="${TRE_GRADIENT_FILL_OUTPUT:-demo/phase10_step10_2_1/gradient_fill_output.png}"
export TRE_GRADIENT_FILL_OUTPUT="$OUT_PATH"

echo "Running gradient_fill_demo (validation loads automatically in debug builds)..."
cargo run -p tre-rhi-vulkan --example gradient_fill_demo

echo
echo "Wrote $OUT_PATH -- a 440x200 image: a bordered rectangle with a"
echo "real red-to-blue linear gradient fill, a bordered circle with a"
echo "real white-to-green radial gradient fill, and a hexagon with a"
echo "real red-to-green linear gradient fill rendered through the"
echo "separate GradientFill pipeline."
