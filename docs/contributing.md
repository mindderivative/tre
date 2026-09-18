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
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --check
python -m pytest tests/
```

Every real engine capability has its own headless, GPU-backed pixel test
under `crates/engine-render/tests/`, proving it actually paints what it
claims to, not just that the code compiles.

## Building this documentation site

```bash
pip install '.[docs]'
mkdocs serve   # live preview at http://127.0.0.1:8000
mkdocs build   # static site in site/
```

## Project discipline

This project holds itself to one discipline end to end for every real
change: investigate → plan → implement → test → document → commit. See
[`BUILD_TRACKER.md`](https://github.com/mindderivative/tre/blob/main/BUILD_TRACKER.md)
for the complete phase-by-phase history this discipline has produced, and
[`ARCHITECTURE.md`](architecture.md) for the design principles that
shape it.

Commit messages match the existing history's style (see `git log`); real,
empirically-verified findings are preferred over assumptions — a claim
about behavior is checked by actually running the code, not inferred
from reading it alone.
