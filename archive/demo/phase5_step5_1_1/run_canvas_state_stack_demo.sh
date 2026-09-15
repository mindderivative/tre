#!/usr/bin/env bash
# Demo: Phase 5, Step 5.1.1 -- Canvas Drawing-Context State Stack
#
# RenderingCanvas gains a real hierarchical Drawing Context state stack --
# save/restore (transform + alpha) and a separate push_clip/pop_clip
# scissor stack -- wired into draw_rounded_rect (Step 3.2). Renders a
# transformed rect, an alpha-blended rect, and a clipped rect (checked at
# the IR level) through the existing, unmodified sdf_rounded_rect
# pipeline.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../.."

OUT_PATH="${TRE_CANVAS_STATE_STACK_OUTPUT:-demo/phase5_step5_1_1/canvas_state_stack_output.png}"
export TRE_CANVAS_STATE_STACK_OUTPUT="$OUT_PATH"

echo "Running canvas_state_stack_demo (validation loads automatically in debug builds)..."
cargo run -p tre-rhi-vulkan --example canvas_state_stack_demo

echo
echo "Wrote $OUT_PATH -- a 200x150 image: a rect translated by (70,60) via"
echo "save/transform/restore, a rect blended at 50% alpha via"
echo "save/set_alpha/restore, and a rect drawn inside a push_clip/pop_clip"
echo "bracket (checked at the IR level, not a GPU scissor test yet)."
