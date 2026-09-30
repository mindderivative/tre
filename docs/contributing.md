# Contributing

## Development setup

```bash
git clone https://github.com/mindderivative/tre.git
cd tre
python -m venv .venv
source .venv/bin/activate
pip install maturin
maturin develop --release
```

See [Installation](installation.md) for the Rust toolchain requirement
and portable-wheel build notes.

## Running the checks

```bash
cargo test --workspace --release
cargo clippy --workspace --all-targets --release -- -D warnings
cargo fmt --all --check
cargo +1.90 check --workspace --all-targets   # MSRV
python -m pytest tests/
mypy --strict python/tre
python -m mypy.stubtest tre --allowlist tools/stubtest_allowlist.txt
mkdocs build --strict
```

`stubtest` compares the type stub with the built extension, so run it after
`maturin develop`; CI runs it and `mypy --strict` on Linux.

Platform-only code (`#[cfg(target_os = ...)]`, such as the macOS title bar
and the Windows double-click time) can be type-checked from Linux without
the other OS's SDK:

```bash
rustup target add aarch64-apple-darwin x86_64-pc-windows-msvc
PYO3_CROSS_PYTHON_VERSION=3.12 CARGO_TARGET_DIR=target/macos \
  cargo clippy --workspace --all-targets --target aarch64-apple-darwin -- -D warnings
PYO3_CROSS_PYTHON_VERSION=3.12 CARGO_TARGET_DIR=target/windows \
  cargo clippy --workspace --all-targets --target x86_64-pc-windows-msvc -- -D warnings
```

This only compiles; CI's macOS and Windows jobs run the tests.

Building on Linux needs a few system packages; see
[Installation](installation.md#option-3-build-from-source).

Every real engine capability has its own headless, GPU-backed pixel test
under `crates/engine-render/tests/`, proving it actually paints what it
claims to, not just that the code compiles.

## Building this documentation site

```bash
pip install mkdocs mkdocs-material
mkdocs serve   # live preview at http://127.0.0.1:8000
mkdocs build   # static site in site/
```

!!! note
    Don't use `pip install '.[docs]'` for this — installing extras from a
    local `pyproject.toml` always builds the base package too (Tesserae Engine's
    real Rust extension), which needs a full Rust toolchain and
    `fontconfig` dev headers you don't need just to build docs. The
    plain install above only ever runs `mkdocs`, never imports `tre`.

## Project discipline

This project holds itself to one discipline end to end for every real
change: investigate → plan → implement → test → document → commit. See
the GitHub project [Tesserae Rendering Engine](https://github.com/users/mindderivative/projects/3) (0.5.0 on; its
README describes how a step is recorded),
[`BUILD_TRACKER_ARCHIVE_0.4.md`](https://github.com/mindderivative/tre/blob/main/BUILD_TRACKER_ARCHIVE_0.4.md), and
[`BUILD_TRACKER_ARCHIVE_0.3.md`](https://github.com/mindderivative/tre/blob/main/BUILD_TRACKER_ARCHIVE_0.3.md) for the
milestone-by-milestone history this discipline has produced, and
[Architecture](architecture.md) for the design principles that shape it.

Commit messages match the existing history's style (see `git log`); real,
empirically-verified findings are preferred over assumptions — a claim
about behavior is checked by actually running the code, not inferred
from reading it alone.
