#!/usr/bin/env bash
# Demo: Phase 2, Step 2 -- GPU API Validation in Debug & CI Builds
#
# Proves VK_LAYER_KHRONOS_validation now loads automatically in debug
# builds -- no VK_LOADER_LAYERS_ENABLE/VK_INSTANCE_LAYERS env vars needed
# anymore, unlike every earlier phase's demos. Explicitly unsets those
# vars (in case your shell has them set from habit) and uses the Vulkan
# loader's own diagnostic logging to show the layer was enabled by THIS
# APPLICATION's code, not by an environment variable.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../.."

echo "Building the headless example..."
cargo build -p tre-rhi-vulkan --example headless

echo
echo "Running with VK_LOADER_LAYERS_ENABLE/VK_INSTANCE_LAYERS explicitly"
echo "unset -- watch for 'Enabled By: By the Application' below, proving"
echo "tre-rhi-vulkan's own code requested the validation layer, not an"
echo "env var:"
echo
env -u VK_LOADER_LAYERS_ENABLE -u VK_INSTANCE_LAYERS VK_LOADER_DEBUG=layer \
  cargo run -p tre-rhi-vulkan --example headless 2>&1 \
  | grep -A2 "VK_LAYER_KHRONOS_validation" | grep -B2 "Enabled By" || true
echo
echo "(If nothing printed above, the validation layer isn't installed on"
echo "this machine -- tre-rhi-vulkan degrades gracefully rather than"
echo "failing to run, but you won't see the confirmation. Install your"
echo "distro's Vulkan validation layers package to see it.)"
