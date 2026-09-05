#!/usr/bin/env bash
# Demo: Phase 1, Step 1 -- Multi-Window RhiDevice Sharing
#
# Opens TWO native windows independently, sharing ONE VulkanDevice --
# proving the sharing is real (each window has its own swapchain,
# pipeline, and rect color) rather than a relabeled single-window path.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../.."

echo "Building the multi-window example..."
cargo build -p tre-rhi-vulkan --example multi_window

echo "Running for ${TRE_MULTI_WINDOW_FRAMES:-120} frames (set that env var to change it)..."
echo "Window A is amber, window B is blue. Note: Wayland gives clients no"
echo "control over window position, so the two windows may open stacked on"
echo "top of each other -- drag one aside to see both clearly."
cargo run -p tre-rhi-vulkan --example multi_window
