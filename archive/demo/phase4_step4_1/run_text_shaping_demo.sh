#!/usr/bin/env bash
# Demo: Phase 4, Step 4.1 -- HarfBuzz & FreeType Integration
#
# Shapes a real mixed Latin/Hebrew string via rustybuzz (bidi + script run
# segmentation, verifying the RTL run's glyphs come back in visually
# reversed order), resolves a real fontconfig-driven fallback cascade and
# proves a codepoint absent from the primary font actually falls through
# to the emoji font, then extracts a real glyph's ('L') outline via skrifa
# and renders it through the pre-existing ear-clipping + flat-color
# Vulkan pipeline. Reads back real pixels: a point inside the glyph's
# vertical stroke is the fill color, the bounding box's own center (empty
# space for an 'L') is the background. Also shapes and renders the real
# word "TEXT" positioned entirely by rustybuzz's own advances, verified
# against an independently-computed point-in-polygon check per letter and
# per inter-letter gap. Writes both renders as PNGs.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../.."

OUT_PATH="${TRE_TEXT_SHAPING_OUTPUT:-demo/phase4_step4_1/text_shaping_output.png}"
WORD_OUT_PATH="${TRE_TEXT_SHAPING_WORD_OUTPUT:-demo/phase4_step4_1/text_shaping_word_output.png}"
export TRE_TEXT_SHAPING_OUTPUT="$OUT_PATH"
export TRE_TEXT_SHAPING_WORD_OUTPUT="$WORD_OUT_PATH"

echo "Running text_shaping_demo (validation loads automatically in debug builds)..."
cargo run -p tre-rhi-vulkan --example text_shaping_demo

echo
echo "Wrote $OUT_PATH -- a 300x300 image: a single white 'L' glyph, its"
echo "outline extracted from a real installed font file, on a dark"
echo "background."
echo
echo "Wrote $WORD_OUT_PATH -- a 300x300 image: the real word \"TEXT\","
echo "shaped and laid out entirely from rustybuzz's own glyph advances."
