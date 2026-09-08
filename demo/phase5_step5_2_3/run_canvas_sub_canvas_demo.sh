#!/usr/bin/env bash
# Demo: Phase 5, Step 5.2.3 -- The Capstone: Real Concurrent Recording,
# Rendered (closes Step 5.2 in full)
#
# min(available_parallelism() - 1, 4) real OS worker threads each get
# their own SubCanvas: every one draws its own rect, thread 0 draws
# inside a begin_overlay/end_overlay bracket, and the last thread also
# draws a real atlas-backed MSDF glyph. Every thread calls stitch_into
# on a shared FrameArena as its own last action before it exits -- the
# real "workers stitch themselves" design point Step 5.2.2 was built
# around, exercised here for real instead of only in a unit test. The
# root canvas draws one more rect directly and stitches into the same
# arena. Rendered through the existing sdf_rounded_rect and
# bindless_textured.vert/msdf.frag pipelines.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../.."

OUT_PATH="${TRE_CANVAS_SUB_CANVAS_OUTPUT:-demo/phase5_step5_2_3/canvas_sub_canvas_output.png}"
export TRE_CANVAS_SUB_CANVAS_OUTPUT="$OUT_PATH"

echo "Running canvas_sub_canvas_demo (validation loads automatically in debug builds)..."
cargo run -p tre-rhi-vulkan --example canvas_sub_canvas_demo

echo
echo "Wrote $OUT_PATH -- a 350x150 image: one white rect per real worker"
echo "thread plus the root canvas's own rect, and one real MSDF text"
echo "glyph ('A') drawn on a worker thread, all recorded concurrently"
echo "and stitched into one shared frame via a real lock-free arena."
