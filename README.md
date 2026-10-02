# Tesserae Engine

**A GPU-rendered retained-mode UI engine for Python, written in Rust.**

Tesserae Engine (imported as `tre`) gives Python the building blocks of a desktop UI — nodes, flexbox
and grid layout, paint, animation, input and events, text, accessibility, layers,
and threading — and renders them on the GPU, redrawing only what changed, with
[`vello_gpu`](https://github.com/linebender/vello), laid out by
[`taffy`](https://github.com/DioxusLabs/taffy), shaped by
[`parley`](https://github.com/linebender/parley), and exposed to screen
readers through [`AccessKit`](https://github.com/AccessKit/accesskit).

It has no widgets and no theme of its own. A UI library or application built on it turns
the blocks into buttons, dialogs, and design systems.

```python
from tre import App, Window

window = Window(width=320, height=120, title="Hello")
button = window.create("box", width=120, height=40, corner_radius=20,
                       fill=(0x67, 0x50, 0xA4, 0xFF), role="button",
                       label="Say hello", focusable=True)
button.on("click", lambda: print("hello"))
window.root.add_child(button)

app = App()
app.add_window(window)
app.run()
```

## Installing

```bash
pip install tesserae-engine
```

On PyPI it's **`tesserae-engine`** (the name `tre` there is another
project's); you still `import tre`. Wheels cover CPython 3.9+ on Linux
(x86_64), macOS (arm64), and Windows (x64); other platforms build from
source. See
[Installation](https://mindderivative.github.io/tre/installation/) for the
details.

## Documentation

The guide, API reference, and migration notes are at
[mindderivative.github.io/tre](https://mindderivative.github.io/tre/), and
their source is in [`docs/`](https://github.com/mindderivative/tre/tree/main/docs),
built with MkDocs:

```bash
pip install mkdocs mkdocs-material
mkdocs serve   # http://127.0.0.1:8000
```

Start with Getting Started, then the guide's page for each building block;
[Building a Widget](https://mindderivative.github.io/tre/guide/building-a-widget/) puts them together.
Upgrading from 0.4.x? See [Upgrading to 0.5.x](https://mindderivative.github.io/tre/migrating-0.5/);
from 0.3.5, [Upgrading to 0.4.x](https://mindderivative.github.io/tre/migrating-0.4/); from 0.3.4, [Migrating to 0.3.5](https://mindderivative.github.io/tre/migrating-0.3.5/).

## Building from source

```bash
python -m venv .venv
source .venv/bin/activate          # .venv\Scripts\activate on Windows
pip install maturin
maturin develop --release -m crates/engine-py/Cargo.toml
python examples/switch.py
```

Each script in [`examples/`](https://github.com/mindderivative/tre/tree/main/examples) is a small, runnable demonstration
of one building block.

## Development

```bash
cargo test --workspace --release
cargo clippy --workspace --all-targets --release -- -D warnings
cargo fmt --all --check
pytest tests/
mkdocs build --strict
mypy --strict python/tre
python -m mypy.stubtest tre --allowlist tools/stubtest_allowlist.txt
cargo +1.90 check --workspace --all-targets   # MSRV
```

Every rendering capability has a headless, GPU-backed pixel test under
`crates/engine-render/tests/`, and the Python suite drives input and time
headlessly through `window.simulate` and `window.advance`.

[`ARCHITECTURE.md`](https://github.com/mindderivative/tre/blob/main/ARCHITECTURE.md) is the design reference, and
the GitHub project [Tesserae Rendering Engine](https://github.com/users/mindderivative/projects/3) tracks the work from `0.5.0` on; the `0.4.x` line is in
[`BUILD_TRACKER_ARCHIVE_0.4.md`](https://github.com/mindderivative/tre/blob/main/BUILD_TRACKER_ARCHIVE_0.4.md), and the
milestone-by-milestone history through `v0.3.5.2` is in
[`BUILD_TRACKER_ARCHIVE_0.3.md`](https://github.com/mindderivative/tre/blob/main/BUILD_TRACKER_ARCHIVE_0.3.md).
Tesserae Engine is a from-scratch second iteration of an earlier Vulkan engine,
archived under [`archive/`](https://github.com/mindderivative/tre/tree/main/archive) with its lessons learned.
