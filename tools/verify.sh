#!/usr/bin/env bash
# The standing verification chain for tre, in one place. Run from anywhere:
#     tools/verify.sh            # everything except the cross-platform checks
#     tools/verify.sh --cross    # also clippy for Windows and macOS targets, and the MSRV check
#
# It stops at the first failure (set -e, and no check is piped through `tail`,
# which would hide its exit status). Needs: a venv with requirements-dev.txt installed
# (the script uses .venv/bin if there is one), rustup, and for `--cross` the targets
# `rustup target add x86_64-pc-windows-msvc aarch64-apple-darwin` and `rustup toolchain
# install 1.90 --profile minimal`. The live-window tests need a display and skip without
# one; CI has none.
set -euo pipefail

cd "$(dirname "$0")/.."
PY=python
if [ -x .venv/bin/python ]; then
    PY=.venv/bin/python
fi
step() { printf '\n== %s\n' "$*"; }

step "rustfmt"
cargo fmt --check
step "clippy (warnings are errors)"
cargo clippy --workspace --all-targets -- -D warnings
step "Rust tests"
cargo test --workspace --release
step "Rust damage tracker, verified against a full walk"
TRE_DAMAGE_VERIFY=1 cargo test -p engine-render --release

step "build the extension"
"$PY" -m maturin develop --release
step "generate docs/api/python.md (commit it if it changed)"
"$PY" tools/gen_python_api.py
step "stubtest"
"$PY" -m mypy.stubtest tre._core --allowlist tools/stubtest_allowlist.txt
step "mypy --strict"
"$PY" -m mypy --strict python
step "pytest"
"$PY" -m pytest tests -q
step "pytest with the damage tracker verified"
TRE_DAMAGE_VERIFY=1 "$PY" -m pytest tests -q
step "docs"
"$PY" -m mkdocs build --strict

if [ "${1:-}" = "--cross" ]; then
    step "clippy for Windows"
    cargo clippy --target x86_64-pc-windows-msvc -p engine-platform -p engine-core -- -D warnings
    step "clippy for macOS"
    cargo clippy --target aarch64-apple-darwin -p engine-platform -p engine-core -- -D warnings
    step "engine-py compiles for Windows"
    PYO3_CROSS=1 PYO3_CROSS_PYTHON_VERSION=3.12 cargo check --target x86_64-pc-windows-msvc -p engine-py
    step "MSRV (Rust 1.90)"
    cargo +1.90 check --workspace --all-targets
fi

printf '\nAll checks passed.\n'
if git status --short | grep -q 'docs/api/python.md'; then
    echo "note: docs/api/python.md changed -- commit it."
fi
