#!/usr/bin/env bash
# Demo: Phase 2, Step 1 -- Zero-Allocation Ring Buffers & Transient Pools
#
# Proves the real RhiDevice::create_dynamic_ring_buffer and
# acquire_transient_target/release_transient_target implementations,
# replacing the unimplemented!() stubs every earlier phase left in place.
# No window is shown -- like the headless demo, this drives real frames
# through a HeadlessSwapchain purely to exercise memory management.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../.."

echo "Building the memory pools demo..."
cargo build -p tre-rhi-vulkan --example memory_pools_demo

echo "Running (assertions in the demo itself fail loudly on any regression)..."
cargo run -p tre-rhi-vulkan --example memory_pools_demo
