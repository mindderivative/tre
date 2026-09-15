#!/usr/bin/env bash
# Demo: Phase 10 Step 10.2.4 -- SDF Fidelity: Exact Ellipse Distance Field
#
# Proves the real, exact ellipse signed-distance field (sdf_ellipse.frag's
# new sd_ellipse, Inigo Quilez's Newton-Raphson refinement,
# iquilezles.org/articles/ellipsedist) on real GPU hardware, replacing
# the OLD "scaled circle" approximation that shipped in Step 10.2 (exact
# only when radius.x == radius.y). No prior demo ever drew a genuinely
# non-circular ellipse -- this draws a real, eccentric, bordered ellipse
# and confirms the real border/fill transition pixel matches an
# independent CPU reference at several angles, not just near the
# boundary by eye.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../.."

OUT_PATH="${TRE_ELLIPSE_SDF_FIDELITY_OUTPUT:-demo/phase10_step10_2_4/ellipse_sdf_fidelity_output.png}"
export TRE_ELLIPSE_SDF_FIDELITY_OUTPUT="$OUT_PATH"

echo "Running ellipse_sdf_fidelity_demo (validation loads automatically in debug builds)..."
cargo run -p tre-rhi-vulkan --example ellipse_sdf_fidelity_demo

echo
echo "Wrote $OUT_PATH -- a 420x200 image: a single, eccentric, bordered"
echo "ellipse (radius 140x40), the border verified uniform all the way"
echo "around against an independent Newton-iteration CPU reference."
