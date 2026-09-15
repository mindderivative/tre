#!/usr/bin/env bash
# Demo: Phase 8, Step 8.1.2 -- The Full 8-Stage Continuous Main Loop
#
# Opens one native window and runs a real, continuous loop that
# executes all 8 named pipeline stages every frame: drains OS events,
# records the scene across 2 real worker threads + the root canvas
# every frame, stitches them into a shared arena, sorts/batches, packs
# the flattened vertex/index bytes into a real per-frame ring buffer,
# and submits/presents -- while a rect animates toward a target via
# Step 8.1.1's own FrameClock + spring_decay.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../.."

echo "Building the main loop demo example..."
cargo build -p tre-rhi-vulkan --example main_loop_demo

echo "One window will open with an amber rect that eases in from the left"
echo "toward the right edge, two white worker-thread rects near the top,"
echo "and one glyph drawn by the same worker that draws the last rect."
echo "Runs for ${TRE_MAIN_LOOP_FRAMES:-90} frames (env var to change), or"
echo "close the window to exit early."
cargo run -p tre-rhi-vulkan --example main_loop_demo
