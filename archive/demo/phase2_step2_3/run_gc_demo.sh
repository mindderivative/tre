#!/usr/bin/env bash
# Demo: Phase 2, Step 2.3 -- Generational Garbage Collection
#
# Checks 25 distinct sizes into the transient render-target pool (~240 MB,
# comfortably past the 85%-of-128MB GC trigger threshold), then runs real
# begin_frame/submit_and_present cycles -- no shortened stand-in constants
# -- until the background GC thread has evicted the now-stale entries
# (the real 600-frame age threshold) and the main thread has physically
# destroyed them (the real 3-frame grace period). Validation loads
# automatically in debug builds (Phase 2 Step 2), so this also proves the
# new background thread introduces no cross-thread Vulkan misuse.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../.."

echo "Running gc_demo..."
cargo run -p tre-rhi-vulkan --example gc_demo
