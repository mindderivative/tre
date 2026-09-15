#!/usr/bin/env bash
# Demo: Phase 1, Step 2 -- Decoupled Event & Signal Pipeline
#
# Opens TWO native windows on one shared PlatformConnection and prints
# every translated pointer/keyboard/window-lifecycle InputEvent to the
# terminal, tagged with which window it came from -- proving both that
# real OS input is being translated correctly and that it's routed to the
# right window when more than one is open.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../.."

echo "Building the input demo example..."
cargo build -p tre-rhi-vulkan --example input_demo

echo "Two windows will open -- A (amber) and B (blue)."
echo "Move the mouse, click, and press keys in each window; watch this"
echo "terminal for events tagged [A] or [B]. Close both windows to exit"
echo "(or it exits automatically after ${TRE_INPUT_DEMO_FRAMES:-600} frames)."
cargo run -p tre-rhi-vulkan --example input_demo
