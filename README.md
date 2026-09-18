# tre

**Python-facing GUI framework backend, Rust-native rendering engine.**

`tre` is the rendering, layout, animation, and accessibility engine
behind a Material Design 3 desktop GUI framework. Application authors
write Python — this project never asks them to touch Rust, WGPU, or
Vello directly. The engine itself is a purpose-built retained-tree
renderer (GPU-accelerated via [`vello_hybrid`](https://github.com/linebender/vello),
layout via [`taffy`](https://github.com/DioxusLabs/taffy), text via
[`parley`](https://github.com/linebender/parley), accessibility via
[`AccessKit`](https://github.com/AccessKit/accesskit)) exposed through
a thin, stable [PyO3](https://pyo3.rs/) boundary.

This is a from-scratch second iteration of an earlier project (`TRE`,
a Vulkan-based 2D rendering engine), archived in full under
[`archive/`](archive/) along with its own lessons-learned document
that shaped several of this project's own design decisions.

See [`ARCHITECTURE.md`](ARCHITECTURE.md) for the full design
reference and [`BUILD_TRACKER.md`](BUILD_TRACKER.md) for a complete,
phase-by-phase build history.

## What's built

- **A real, retained node tree** — layout via `taffy`, a uniform,
  centrally-ticked animation system (`Animated<T>` on every animatable
  property), and real per-frame GPU rendering.
- **Material Design 3 visual language** — dynamic color (full HCT/
  tonal-palette scheme resolution), elevation shadows, hover/press
  state layers with real ripple, shape morphing, and MD3 motion
  curves.
- **Real MD3 components** — `Checkbox`, `Slider` (with keyboard
  arrow-key increments), `TextField` (real keyboard editing, mouse
  click-to-position and drag-to-select, IME composition, clipboard),
  `Image` (GPU-texture-backed, with `Cover`/`Contain`/`Fill` content
  fit), and `Icon` (a curated set of real Material Symbols icons,
  rendered as vector fills).
- **Desktop shell primitives** — multi-window apps, a fixed-zone
  docking system, splitters, virtualized/variable-height lists,
  context menus and other overlays, an `AppShell` navigation pattern,
  and MD3's container-transform choreography.
- **Two authoring paths, one engine** — build a UI imperatively from
  Python, or declaratively from YAML view files (`engine-spec`), with
  a real stylesheet cascade, hot-reload, `include:`-based composition,
  and one- and two-way data binding against a plain Python
  `ViewModel`.
- **Accessibility from day one** — a real AccessKit tree built fresh
  every frame from the same node tree, keyboard focus/Tab order, and
  screen-reader-driven actions routed through the same input pipeline
  as pointer/keyboard events.
- **Real cross-platform packaging** — a manylinux-repaired, portable
  wheel, built and verified end-to-end (not just `maturin develop`),
  with CI producing Linux/macOS/Windows wheels across supported Python
  versions on every tagged release.

## Getting started

```bash
python -m venv .venv
source .venv/bin/activate  # or .venv\Scripts\activate on Windows
pip install maturin
maturin develop --release
```

Then run any script under [`examples/`](examples/) — each one is a
small, self-contained, real proof of one real mechanism (e.g.
`examples/slider.py` for keyboard-driven `Slider` control,
`examples/docking.py` for the docking system, `examples/
view_composition.py` for declarative YAML composition):

```bash
python examples/checkbox.py
```

A minimal imperative app looks like this:

```python
from tre import App, Window

window = Window(width=400, height=200, title="tre")
window.add_text_field(background=(0xEE, 0xEE, 0xEE, 0xFF), width=300, height=48)

app = App()
app.add_window(window)
app.run()
```

## Development

```bash
cargo test --workspace --release
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --check
```

Every real engine capability has its own headless, GPU-backed pixel
test under `crates/engine-render/tests/` proving it actually paints
what it claims to, not just that the code compiles — see
`BUILD_TRACKER.md` for the discipline this project holds itself to
end to end (investigate → plan → implement → test → document →
commit, every phase).
