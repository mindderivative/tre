#!/usr/bin/env bash
# Demo: Phase 5, Step 5.1.3 -- Real Sort Key, Overlay Routing, Real Batch
# Flattening (the capstone of Step 5.1)
#
# Reproduces DESIGN.md Section 8's own worked example: Rect1(P1,Tex0) ->
# Text(P2,AtlasA) -> Rect2(P1,Tex0) -> OverlayRect(P1,Tex0) collapses
# into exactly 3 real dispatched batches (Rect1+Rect2 merge; Text and
# OverlayRect each stay alone), rendered through the existing
# sdf_rounded_rect and bindless_textured.vert/msdf.frag pipelines -- the
# first demo in this codebase to record more than one draw_indexed call
# in a single frame.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../.."

OUT_PATH="${TRE_CANVAS_BATCH_FLATTENING_OUTPUT:-demo/phase5_step5_1_3/canvas_batch_flattening_output.png}"
export TRE_CANVAS_BATCH_FLATTENING_OUTPUT="$OUT_PATH"

echo "Running canvas_batch_flattening_demo (validation loads automatically in debug builds)..."
cargo run -p tre-rhi-vulkan --example canvas_batch_flattening_demo

echo
echo "Wrote $OUT_PATH -- a 200x140 image: three white SDF rects (two of"
echo "them merged into a single real draw call) and one real MSDF text"
echo "glyph ('A'), proving real batch flattening still renders every"
echo "logical shape at its own correct, distinct position."
