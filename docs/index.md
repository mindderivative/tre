# tre

**Python-facing GUI framework backend, Rust-native rendering engine.**

`tre` is the rendering, layout, animation, and accessibility engine behind
a Material Design 3 desktop GUI framework. Application authors write
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
- **Material Design 3 visual language** — dynamic color (full HCT/
  tonal-palette scheme resolution), elevation shadows, hover/press state
  layers with real ripple, shape morphing, and MD3 motion curves.
- **Real MD3 components** — [`Checkbox`, `Slider`](guide/components.md),
  [`TextField`](guide/components.md) (real keyboard editing, mouse
  click-to-position and drag-to-select, IME composition, clipboard),
  [`Image`](guide/components.md) (GPU-texture-backed, with
  cover/contain/fill content fit), and [`Icon`](guide/components.md) (a
  curated set of real Material Symbols icons, rendered as vector fills).
- **Desktop shell primitives** — multi-window apps, a
  [fixed-zone docking system](guide/docking-and-shell.md), splitters,
  [virtualized/variable-height lists](guide/canvas-and-lists.md), context
  menus and other overlays, an `AppShell` navigation pattern, and MD3's
  container-transform choreography.
- **Two authoring paths, one engine** — build a UI
  [imperatively from Python](guide/imperative-api.md), or
  [declaratively from YAML view files](guide/declarative-views.md), with
  `include:`-based composition and one- and two-way data binding against
  a plain Python `ViewModel`.
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
- **[Getting Started](getting-started.md)** — build and run a first
  window in a few lines of Python.
- **[Guide](guide/imperative-api.md)** — walkthroughs of the imperative
  API, declarative YAML views, MD3 components, docking, canvas drawing,
  and theming.
- **[Python API Reference](api/python/index.md)** — every public class
  and method, with real signatures pulled from the source.
- **[Architecture](architecture.md)** — the engine's crate layout and
  design principles.

Looking for a single, all-in-one tour instead of the piece-by-piece
guide? Run
[`demo/showcase.py`](https://github.com/mindderivative/tre/blob/main/demo/showcase.py)
from a source checkout — one running app combining MD3 components and
live theming, real animation and custom `Canvas` drawing, a
virtualized list, docking, and a declarative `View` panel, all
keyboard-navigable.

## Project status

27 milestones have been built against [`ARCHITECTURE.md`](architecture.md)
as of `v0.2.0` — see
[`BUILD_TRACKER.md`](https://github.com/mindderivative/tre/blob/main/BUILD_TRACKER.md)
in the repository for the complete phase-by-phase build history. Every
real engine capability has its own headless, GPU-backed pixel test
proving it actually paints what it claims to, not just that the code
compiles.
