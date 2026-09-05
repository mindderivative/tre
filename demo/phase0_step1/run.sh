#!/usr/bin/env bash
# Demo: Phase 0, Step 1 -- Walking Skeleton
#
# Opens a window, clears it to a dark background, and draws one
# Canvas::draw_rounded_rect call through the real Canvas -> IR ->
# RhiCommandBuffer::draw_indexed -> swapchain pipeline, then exits after
# a fixed number of frames.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../.."

echo "Building the walking skeleton example..."
cargo build -p tre-rhi-vulkan --example walking_skeleton

echo "Running for ${TRE_WALKING_SKELETON_FRAMES:-120} frames (set that env var to change it, e.g. a large number to keep the window open for a screenshot)..."
cargo run -p tre-rhi-vulkan --example walking_skeleton
