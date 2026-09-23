# Installation

`tre` ships as a compiled Python extension module (`tre._core`, built with
[PyO3](https://pyo3.rs/)/[maturin](https://www.maturin.rs/)) plus a thin
pure-Python package (`tre`) on top of it. There are two ways to get it:
install a pre-built wheel, or build one yourself from source.

## Requirements

- Python 3.9 or newer (CPython; on Linux also free-threaded `3.14t`/
  `3.15t` builds and PyPy 3.11 — see the supported set below)
- Linux, macOS (arm64), or Windows

## Option 1: install a released wheel

Every [tagged release](https://github.com/mindderivative/tre/releases)
publishes real, verified wheels — a source distribution, plus:

- **Linux** — portable `manylinux_2_17`/`manylinux2014` wheels, built
  inside the real manylinux Docker container (not just locally
  repaired). The container ships several real CPython interpreters
  plus PyPy, and the build picks up every one it finds
  (`maturin-action`'s own `--find-interpreter`) rather than a
  hand-maintained list — as of `v0.3.0` that's CPython 3.9–3.15
  (including the free-threaded `3.14t`/`3.15t` builds) and PyPy 3.11.
- **macOS** (`arm64`) and **Windows** — one wheel per CPython version
  in an explicit, hand-maintained matrix (currently 3.9–3.14) via
  `actions/setup-python` — no free-threaded or PyPy builds on these
  two platforms; neither ships a container with several interpreters
  pre-installed the way the Linux build's own container does.

The GitHub Release also carries a standalone, non-wheel-packaged
Linux/CPython 3.14 extension module (`_core.cpython-314-x86_64-linux-
gnu.so`) for tooling that consumes the compiled module directly rather
than through `pip`.

Download the wheel matching your platform and interpreter from the
[latest release](https://github.com/mindderivative/tre/releases/latest)
and install it directly:

```bash
pip install ./tre-0.3.0-cp312-cp312-manylinux_2_17_x86_64.manylinux2014_x86_64.whl
```

!!! note
    `tre` is not yet published to PyPI, so `pip install tre` won't resolve
    it — install the downloaded wheel file directly, as shown above.

## Option 2: build from source

You'll need a Rust toolchain (see
[`rust-toolchain.toml`](https://github.com/mindderivative/tre/blob/main/rust-toolchain.toml)
for the exact pinned version — `rustup` picks it up automatically) and
[`maturin`](https://www.maturin.rs/):

```bash
git clone https://github.com/mindderivative/tre.git
cd tre
python -m venv .venv
source .venv/bin/activate  # or .venv\Scripts\activate on Windows
pip install maturin
maturin develop --release
```

`maturin develop` compiles the Rust extension and installs it directly
into your active virtual environment in editable form — the fastest loop
for local development. Run any script under
[`examples/`](https://github.com/mindderivative/tre/tree/main/examples)
to confirm it worked:

```bash
python examples/checkbox.py
```

### Building a portable wheel yourself

`maturin build --release` alone does **not** produce a portable wheel on
most Linux dev machines — without an explicit target it silently falls
back to a bare, non-redistributable `linux` platform tag. Building a real,
portable `manylinux` wheel from an ordinary Linux dev host needs either:

- **`maturin[zig]`**, cross-compiling with an explicit compatibility
  target:

  ```bash
  pip install 'maturin[zig]'
  maturin build --release --zig --compatibility manylinux_2_38
  ```

  (Zig itself also needs to be on `PATH` as `zig` — the `ziglang` PyPI
  package installs a `python-zig` entry point, not a plain `zig` binary,
  so a one-line shim script is needed on top of it.)

- **A real manylinux container**, the same way this project's own release
  CI does it (see
  [`.github/workflows/wheels.yml`](https://github.com/mindderivative/tre/blob/main/.github/workflows/wheels.yml)):
  a genuinely older glibc baseline inside the container achieves a
  substantially broader, more portable tag automatically, with no zig
  cross-compilation needed at all.

## Verifying the install

```python
import tre
print(tre.App, tre.Window, tre.Node, tre.View, tre.Signal, tre.ViewModel)
```

If that import succeeds, you're ready for [Getting Started](getting-started.md).
