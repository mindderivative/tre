#!/usr/bin/env bash
# Demo: Phase 11 Step 11.1 -- winit-Backed Windowing Migration
#
# Unlike every other per-step demo folder in this project, this step adds
# no new demo binary of its own: it replaced tre-platform's internal
# windowing backend (hand-rolled Wayland/X11 -> winit) while preserving
# PlatformConnection's public API exactly, so the correctness oracle here
# is that every PRE-EXISTING demo still behaves identically. This script
# re-runs the representative subset: the platform smoke test under both
# explicitly forced backends, the reference imperative main loop
# (main_loop_demo), and the real multi-window regression case
# (multi_window). The full sweep run during implementation covered all 41
# tre_platform-dependent demos with zero failures -- see README.md in this
# directory for that full account.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../.."

echo "=== smoke_test (auto-detected backend) ==="
TRE_SMOKE_TEST_ITERS="${TRE_SMOKE_TEST_ITERS:-20}" cargo run -p tre-platform --example smoke_test

echo
echo "=== smoke_test (forced Wayland) ==="
TRE_FORCE_BACKEND=wayland TRE_SMOKE_TEST_ITERS="${TRE_SMOKE_TEST_ITERS:-20}" cargo run -p tre-platform --example smoke_test

echo
echo "=== smoke_test (forced X11 / XWayland) ==="
TRE_FORCE_BACKEND=x11 TRE_SMOKE_TEST_ITERS="${TRE_SMOKE_TEST_ITERS:-20}" cargo run -p tre-platform --example smoke_test

echo
echo "=== main_loop_demo (reference imperative main loop, real GPU) ==="
cargo run -p tre-rhi-vulkan --example main_loop_demo

echo
echo "=== multi_window (real two-window creation/routing, real GPU) ==="
cargo run -p tre-rhi-vulkan --example multi_window

echo
echo "All representative demos completed without error under the new"
echo "winit-backed tre-platform implementation."
