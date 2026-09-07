#!/usr/bin/env bash
# Demo: Phase 5, Step 5.1.2 -- Canvas Text Rendering (draw_text)
#
# tre-engine's first real wiring into tre-text/tre-atlas: Canvas::draw_text
# shapes and renders a real word ("TEXT") through a real AtlasOwner
# background thread, proving both halves of its documented cache
# contract -- a first draw_text call against a brand-new word is a
# cache miss for every glyph (zero commands emitted, a real
# request_insert fired instead), and a second call after the atlas
# resolves is a cache hit for every glyph (one real textured
# DrawGeometry command per glyph), rendered through the existing,
# unmodified bindless_textured.vert/msdf.frag pipeline (Step 4.2.4).
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../.."

OUT_PATH="${TRE_CANVAS_DRAW_TEXT_OUTPUT:-demo/phase5_step5_1_2/canvas_draw_text_output.png}"
export TRE_CANVAS_DRAW_TEXT_OUTPUT="$OUT_PATH"

echo "Running canvas_draw_text_demo (validation loads automatically in debug builds)..."
cargo run -p tre-rhi-vulkan --example canvas_draw_text_demo

echo
echo "Wrote $OUT_PATH -- a 300x120 image: the word \"TEXT\" shaped via a"
echo "real cascade font and rendered as four real MSDF glyph quads, each"
echo "resolved from a real atlas after an initial cache-miss frame."
