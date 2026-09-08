#!/usr/bin/env bash
# Demo: Phase 5, Step 5.3.3 -- The Capstone: A Real Rendered Scene,
# Verified Live (closes Step 5.3 in full)
#
# Three rects are drawn and tagged from the exact same local
# coordinates -- a plain Generic rect, a plain Button rect, and a
# rotated Image rect -- then read back from the real GPU framebuffer,
# published via a real tre_a11y::A11yBridge, and independently
# re-queried from a real, live AT-SPI2 registry as this same process's
# own second D-Bus client. Needs a real accessibility bus reachable
# (any normal Linux desktop session already provides one; headless CI
# wraps this demo in `dbus-run-session`, see .github/workflows/ci.yml).
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../.."

OUT_PATH="${TRE_CANVAS_ACCESSIBILITY_OUTPUT:-demo/phase5_step5_3_3/canvas_accessibility_output.png}"
export TRE_CANVAS_ACCESSIBILITY_OUTPUT="$OUT_PATH"

echo "Running canvas_accessibility_demo (validation loads automatically in debug builds)..."
cargo run -p tre-rhi-vulkan --example canvas_accessibility_demo

echo
echo "Wrote $OUT_PATH -- a 300x200 image: two plain white rects and one"
echo "rotated white rect, each independently confirmed to match its own"
echo "real, live AT-SPI2 Component.GetExtents exactly."
