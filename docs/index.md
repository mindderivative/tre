# tre

**Python-facing GUI framework backend, Rust-native rendering engine.**

`tre` is the rendering, layout, animation, and accessibility engine behind
a Python desktop GUI framework (Tesserae). Application authors write
Python — this project never asks them to touch Rust, WGPU, or Vello
directly. The engine itself is a purpose-built retained-tree renderer
exposed through a thin, stable [PyO3](https://pyo3.rs/) boundary:

- **GPU-accelerated rendering** via [`vello_hybrid`](https://github.com/linebender/vello)
- **Layout** via [`taffy`](https://github.com/DioxusLabs/taffy) (flexbox)
- **Text shaping** via [`parley`](https://github.com/linebender/parley)
- **Accessibility** via [`AccessKit`](https://github.com/AccessKit/accesskit)

This is a from-scratch second iteration of an earlier project (`TRE`, a
Vulkan-based 2D rendering engine), archived in full under
[`archive/`](https://github.com/mindderivative/tre/tree/main/archive)
along with its own lessons-learned document that shaped several of this
project's design decisions.

## What's built

- **A real, retained node tree** — layout via `taffy`, a uniform,
  centrally-ticked animation system (`Animated<T>` on every animatable
  property), and real per-frame GPU rendering.
- **Design-language-neutral paint** — fills, borders, per-corner radii,
  layered shadows, vector paths with trim and morph, and cubic-bezier
  easing. Material Design 3's components, theming, and motion belong to
  the framework; `tre` 0.3.5 removed its own.
- **Desktop shell primitives** — multi-window apps, a
  [fixed-zone docking mechanism](guide/docking-and-shell.md),
  [virtualized/variable-height lists](guide/canvas-and-lists.md), scroll
  views, and [layers](api/python/layers.md) for overlays.
- **A real, wide layout surface** — per-side padding/margin,
  flex-grow/shrink/basis, align/justify, absolute positioning.
- **Building blocks for a framework** — build a UI
  [from Python](guide/imperative-api.md) with nodes, properties,
  listeners, layers, and animation. Declarative views, data binding, and
  reactivity belong to a framework built on `tre` (Tesserae); `tre` 0.3.5
  removed its own.
- **Data in, not files** — images as decoded pixels, fonts as bytes, so
  a framework built on `tre` owns every file format and loading
  decision; `tre` 0.3.5 removed its own file loading.
- **Thread-safe updates into a running app** — `App.thread_handle()`
  lets a background thread (a file watcher, a network client) hand work
  to the event loop, waking it even when idle.
- **Accessibility from day one** — a real AccessKit tree built fresh
  every frame from the same node tree, keyboard focus/Tab order, and
  screen-reader-driven actions routed through the same input pipeline as
  pointer/keyboard events.
- **Real cross-platform packaging** — a manylinux-repaired, portable
  wheel, built and verified end-to-end, with CI producing Linux/macOS/
  Windows wheels across supported Python versions on every tagged
  release.

## Where to go next

- **[Installation](installation.md)** — install a released wheel, or
  build from source with `maturin`.
- **[Migrating to 0.3.5](migrating-0.3.5.md)** — upgrading from 0.3.4: what
  moved to the framework, every rename, and behavior changes.
- **[Migrating to 0.3.3](migrating-0.3.3.md)** — upgrading from 0.3.2:
  the property renames, a view-file migration script, and behavior
  changes.
- **[Getting Started](getting-started.md)** — build and run a first
  window in a few lines of Python.
- **[Guide](guide/imperative-api.md)** — walkthroughs of the imperative
  API, docking, canvas drawing, and accessibility.
- **[Python API Reference](api/python/index.md)** — every public class
  and method, with real signatures pulled from the source.
- **[Architecture](architecture.md)** — the engine's crate layout and
  design principles.

## Project status

88 milestones have been built against [`ARCHITECTURE.md`](architecture.md)
as of `v0.3.2` — see
[`BUILD_TRACKER.md`](https://github.com/mindderivative/tre/blob/main/BUILD_TRACKER.md)
in the repository for the complete phase-by-phase build history. Every
real engine capability has its own headless, GPU-backed pixel test
proving it actually paints what it claims to, not just that the code
compiles.
