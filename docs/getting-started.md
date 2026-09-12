# Getting Started

This page gets you from a fresh checkout to a rendered frame, using `tre-python` -- the way almost everyone should approach this project. If you're working on the Rust engine itself rather than consuming it from Python, see the note at the bottom.

## Prerequisites

**Rust toolchain.** The workspace pins an exact toolchain in [`rust-toolchain.toml`](https://github.com/mindderivative/tre/blob/main/rust-toolchain.toml) (stable `1.98.0`, with `rustfmt`/`clippy`) -- `rustup` installs it automatically the first time you build in this directory. You don't need to install Rust separately if `rustup` is already on your machine.

**Python 3.9+.** `tre-python` is built with [PyO3](https://pyo3.rs/) 0.27, **without** the `abi3` stable-ABI feature -- the compiled extension is tied to the exact Python minor version it was built against (e.g. a build against Python 3.12 only loads under Python 3.12), so you rebuild it with `maturin develop` whenever you switch Python versions. This project has been built and exercised against Python 3.14.

**A Vulkan 1.2+ GPU and driver**, with `VK_KHR_dynamic_rendering` and `VK_EXT_descriptor_indexing` (bindless textures) support -- required by every real GPU feature in this engine, including headless rendering. Even `HeadlessRenderer` needs a reachable display-server connection (X11 or Wayland) to probe a surface during device setup, so this doesn't run in a true no-display environment (a bare CI container with no compositor at all needs a software Vulkan implementation such as `lavapipe`/`mesa-vulkan-drivers` plus `xvfb-run`, the same way this project's own CI does it).

**Linux system libraries.** `tre-python` pulls in windowing (`winit`), clipboard (`arboard`), native dialogs (`rfd`), system tray (`tray-icon`), and accessibility (`tre-a11y`/`accesskit`) -- confirmed, on this project's own development machine, to need the Vulkan loader, GTK 3, D-Bus, fontconfig, and the Wayland/X11 client libraries. On Debian/Ubuntu, the equivalent packages are typically:

```bash
sudo apt install libvulkan1 libgtk-3-dev libdbus-1-dev libfontconfig1-dev \
                  libwayland-dev libxkbcommon-dev libx11-dev libxcb1-dev
```

Exact package names vary by distribution -- this list reflects what's genuinely required, not a tested minimal set per distro. Windows and macOS support for `tre-platform`'s windowing layer is not yet implemented (see [Architecture](architecture.md)).

## Building `tre-python`

```bash
python3 -m venv .venv
.venv/bin/pip install maturin
.venv/bin/maturin develop --release -m crates/tre-python/Cargo.toml
```

This compiles the extension and installs it into `.venv` as an editable package, under the module name **`tre_python`** -- every example on this site imports it aliased as `tre`:

```bash
.venv/bin/python -c "import tre_python as tre; print(tre.rgba8(255, 0, 0, 255))"
```

Re-run `maturin develop` after any change to Rust source under `crates/` -- there's no separate "watch" mode.

## Your first render

`HeadlessRenderer` needs no window -- it renders into an in-memory pixel buffer, which is the fastest way to confirm your setup works and is also this project's own correctness oracle for every feature (every real capability is proven headless first).

```python
import tre_python as tre

WIDTH, HEIGHT = 200, 150

registry = tre.ShapeRegistry()
registry.insert_rectangle(
    tre.Rectangle(50, 40, 100, 70, tre.rgba8(200, 60, 60, 255))
)

renderer = tre.HeadlessRenderer(WIDTH, HEIGHT)
frame = renderer.render(registry)  # tightly-packed BGRA8 bytes
assert len(frame) == WIDTH * HEIGHT * 4

# Save it to look at -- Pillow expects RGBA, so swap B and R first.
from PIL import Image

rgba = bytearray(frame)
for i in range(0, len(rgba), 4):
    rgba[i], rgba[i + 2] = rgba[i + 2], rgba[i]
Image.frombytes("RGBA", (WIDTH, HEIGHT), bytes(rgba)).save("first_render.png")
print("wrote first_render.png")
```

Run it (`.venv/bin/python first_render.py`) and open `first_render.png` -- you should see a dark red rectangle on the renderer's dark slate clear color. See [Canvas & Shapes](python-api/canvas-and-shapes.md) for every shape primitive, and [Fonts, Gradients & Textures](python-api/fonts-gradients-textures.md)/[SVG](python-api/svg.md) for text and vector art.

## Rendering to a real window

`WindowedRenderer` owns one or more real OS windows and their event queues. It **must stay on the thread that constructed it** (see the [Python API overview](python-api/index.md)).

```python
import time

import tre_python as tre

renderer = tre.WindowedRenderer("My First TRE Window", 400, 300)
window = renderer.main_window

registry = tre.ShapeRegistry()
registry.insert_rectangle(
    tre.Rectangle(50, 40, 200, 120, tre.rgba8(60, 120, 200, 255))
)

while True:
    for event in renderer.poll_events():
        if isinstance(event, tre.InputEvent.CloseRequested):
            raise SystemExit
    renderer.render(window, registry)
    time.sleep(1 / 60)
```

A plain rectangle in a resizable window, closable normally. From here: [Rendering](python-api/rendering.md) covers custom shaders and multi-window setups, [Input Events](python-api/input-events.md) covers everything `poll_events()` can return, and [Text Editing](python-api/text-editing.md) covers building an actual text field on top of this loop.

## Where to go next

- **[Python API](python-api/index.md)** -- the complete reference for everything shown above and much more.
- **[Architecture](architecture.md)** -- how the Canvas API, the intermediate representation, and the RHI fit together underneath what you just called.
- **[Platform Integration](platform-integration.md)** -- clipboard, file dialogs, tray icons, accessibility, and OS-specific quirks.

## Building the Rust engine itself

If you're contributing to `tre-engine`/`tre-platform`/the RHI backends rather than just consuming `tre-python`, the whole Cargo workspace builds and tests independently of Python:

```bash
cargo build --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
cargo test --workspace
```

See the top-level [README.md](https://github.com/mindderivative/tre/blob/main/README.md) for the crate layout, and `documentation/` in the repository (DESIGN.md, ARCHITECTURE.md, TECHNICAL.md) for the engine's own design documentation and full phase-by-phase build history.