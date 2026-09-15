#!/usr/bin/env bash
# Demo: Phase 10 Step 10.2.6 -- Zero-Allocation Live Verification
#
# Proves ShapeRegistry/RenderingCanvas-driven shape rendering is
# genuinely zero-allocation in steady state (tre_memory::RenderTickGuard/
# DebugAllocGuard), closing ARCHITECTURE.md Section 7.5's own disclosed
# gap. 120 frames of a real, mutating, mixed scene (Rectangle,
# gradient-filled non-circular Circle, texture-filled Polygon,
# blend-mode Polygon, bordered partial-arc Circle) -- covering every new
# fill/blend feature Steps 10.2.1-10.2.5 added -- with position, color,
# and gradient stops mutated every frame. Found and fixed two real,
# previously-undetected per-frame allocations along the way
# (generate_polygon_points/fan_from_center, bounding_box_uvs); discloses
# one real, deeper gap it does NOT fix (lyon-backed Path/bordered-Polygon
# tessellation).
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../.."

OUT_PATH="${TRE_SHAPE_REGISTRY_ZERO_ALLOC_OUTPUT:-demo/phase10_step10_2_6/shape_registry_zero_alloc_output.png}"
export TRE_SHAPE_REGISTRY_ZERO_ALLOC_OUTPUT="$OUT_PATH"

echo "Running shape_registry_zero_alloc_demo (validation loads automatically in debug builds)..."
cargo run -p tre-rhi-vulkan --example shape_registry_zero_alloc_demo

echo
echo "Wrote $OUT_PATH -- the final frame (420x320) of a real, mutating,"
echo "mixed ShapeRegistry scene rendered with zero heap allocations after"
echo "warm-up."
