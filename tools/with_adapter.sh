#!/usr/bin/env bash
# Run a command on a chosen GPU adapter, with the right environment for it.
#
#     tools/with_adapter.sh radv     -- python -m pytest tests/test_shader.py -q   # AMD RADV (real GPU)
#     tools/with_adapter.sh lvp      -- python -m pytest tests/test_shader.py -q   # llvmpipe (software Vulkan)
#     tools/with_adapter.sh fallback -- cargo test -p engine-render --release      # wgpu's own fallback adapter
#     tools/with_adapter.sh list                                                   # what is installed
#
# radv and lvp pin the Vulkan loader to one ICD (VK_ICD_FILENAMES), which is what the Python tests
# need. fallback sets TRE_TEST_FORCE_FALLBACK=1, which is what the Rust tests read. The ICD files
# are looked for in the usual Vulkan directories; override with TRE_ICD_DIR. Only Linux; the
# adapter names are the ones this machine has (AMD Radeon 890M on RADV, and llvmpipe).
set -euo pipefail

dirs=("${TRE_ICD_DIR:-}" /usr/share/vulkan/icd.d /etc/vulkan/icd.d /usr/local/share/vulkan/icd.d)
find_icd() {
    for d in "${dirs[@]}"; do
        [ -n "$d" ] && [ -f "$d/$1" ] && { echo "$d/$1"; return 0; }
    done
    return 1
}

usage() { sed -n '2,12p' "$0" | sed 's/^# \{0,1\}//'; exit "${1:-2}"; }

adapter="${1:-}"
[ -n "$adapter" ] || usage

if [ "$adapter" = list ]; then
    for pair in radv:radeon_icd.json lvp:lvp_icd.json; do
        if icd=$(find_icd "${pair#*:}"); then echo "${pair%%:*}: $icd"; else echo "${pair%%:*}: not installed"; fi
    done
    echo "fallback: TRE_TEST_FORCE_FALLBACK=1 (no ICD file)"
    exit 0
fi
shift
[ "${1:-}" = "--" ] && shift
[ "$#" -gt 0 ] || { echo "with_adapter.sh: no command after the adapter name" >&2; usage; }

case "$adapter" in
    radv) icd=$(find_icd radeon_icd.json) || { echo "with_adapter.sh: radeon_icd.json not found (see: list)" >&2; exit 1; }
          export VK_ICD_FILENAMES="$icd" ;;
    lvp)  icd=$(find_icd lvp_icd.json) || { echo "with_adapter.sh: lvp_icd.json not found (see: list)" >&2; exit 1; }
          export VK_ICD_FILENAMES="$icd" ;;
    fallback) export TRE_TEST_FORCE_FALLBACK=1 ;;
    -h|--help|help) usage 0 ;;
    *) echo "with_adapter.sh: unknown adapter '$adapter' (radv, lvp, fallback, list)" >&2; exit 2 ;;
esac
echo "adapter: $adapter${VK_ICD_FILENAMES:+ ($VK_ICD_FILENAMES)}${TRE_TEST_FORCE_FALLBACK:+ (TRE_TEST_FORCE_FALLBACK=1)}" >&2
exec "$@"
