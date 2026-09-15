#!/usr/bin/env bash
# Demo: Phase 1, Step 1 -- Headless (Zero-Window) Rendering
#
# Renders the same one-rect scene with NO window or display server
# involvement at all, reads the GPU output back to CPU memory, and writes
# it to a PNG -- a deterministic, automatable proof (no screenshot needed).
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../.."

OUT="${TRE_HEADLESS_OUTPUT:-demo/phase1_step1/headless_output.png}"
echo "Building the headless example..."
cargo build -p tre-rhi-vulkan --example headless

echo "Rendering to $OUT ..."
TRE_HEADLESS_OUTPUT="$OUT" cargo run -p tre-rhi-vulkan --example headless
echo "Open $OUT to see the result: a green rounded rect on a dark background."
