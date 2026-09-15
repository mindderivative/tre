#!/usr/bin/env bash
# Demo: Phase 5, Step 5.3.3 -- The Capstone: A Real Rendered Scene,
# Verified Live (closes Step 5.3 in full)
#
# Two real, separate processes (REVIEW.md finding #126 explains why a
# single self-verifying process never reliably worked in CI):
# canvas_accessibility_demo draws and tags three rects -- a plain
# Generic rect, a plain Button rect, and a rotated Image rect -- from
# the exact same local coordinates, reads them back from the real GPU
# framebuffer, and hands the tagged nodes to canvas_accessibility_verify
# (a separate, genuinely Vulkan/X11-free binary) via a small handoff
# file. The verify binary publishes them via a real tre_a11y::A11yBridge
# and independently re-queries a real, live AT-SPI2 registry as its own
# second D-Bus client -- matching how a real screen reader (always a
# separate process) actually works. Needs a real accessibility bus
# reachable for the verify step (any normal Linux desktop session
# already provides one).
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../.."

OUT_PATH="${TRE_CANVAS_ACCESSIBILITY_OUTPUT:-demo/phase5_step5_3_3/canvas_accessibility_output.png}"
export TRE_CANVAS_ACCESSIBILITY_OUTPUT="$OUT_PATH"
NODES_PATH="${TRE_CANVAS_ACCESSIBILITY_NODES_PATH:-demo/phase5_step5_3_3/canvas_accessibility_nodes.txt}"
export TRE_CANVAS_ACCESSIBILITY_NODES_PATH="$NODES_PATH"

echo "Running canvas_accessibility_demo (validation loads automatically in debug builds)..."
cargo run -p tre-rhi-vulkan --example canvas_accessibility_demo

echo
echo "Running canvas_accessibility_verify (a separate, Vulkan/X11-free process)..."
cargo run -p tre-rhi-vulkan --example canvas_accessibility_verify

echo
echo "Wrote $OUT_PATH -- a 300x200 image: two plain white rects and one"
echo "rotated white rect, each independently confirmed to match its own"
echo "real, live AT-SPI2 Component.GetExtents exactly."
