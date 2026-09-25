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
- **A real, wide MD3 component catalog** — 60 `Window.add_*` factories spanning
  [buttons, selection controls, cards, chips, navigation, overlays
  (dialogs, menus, snackbars), a real PTY-backed `Terminal`, and a
  `CodeEditor` with folding and syntax highlighting](guide/components.md),
  not just a handful of samples.
- **Full theming, not just color** — MD3 dynamic color (HCT/tonal-
  palette resolution), a named shape/elevation token system
  (`corner_radius: small`, `elevation: level_2`), and a real,
  Flutter-sourced MD3 type scale apps can reference by role instead of
  literal font values — see [Theming & Accessibility](guide/theming-and-accessibility.md).
- **Desktop shell primitives** — multi-window apps, a
  [fixed-zone docking system](guide/docking-and-shell.md), splitters,
  [virtualized/variable-height lists](guide/canvas-and-lists.md), context
  menus and other overlays, an `AppShell` navigation pattern, and MD3's
  container-transform choreography.
- **A real, wide layout/styling surface** — per-side padding/margin,
  flex-grow/shrink/basis, align/justify, and border kwargs across the
  catalog, both from Python and from declarative `style:` blocks.
- **Two authoring paths, one engine** — build a UI
  [imperatively from Python](guide/imperative-api.md), or
  [declaratively as data](guide/declarative-views.md) (a tree of widget
  specs), with a stylesheet cascade, embeddable components, in-place
  reconciliation, and one- and two-way data binding against a plain
  Python `ViewModel`.
- **Data in, not files** — views, themes, and stylesheets are passed as
  plain `dict`s, images as decoded pixels, fonts as bytes, so a
  framework built on `tre` owns every file format and loading decision.
  Using `tre` directly? It will also [read files for you](guide/working-with-files.md)
  — YAML views with `include:`, theme files, PNG/JPEG images, and
  file-watching hot reload.
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
- **[Getting Started](getting-started.md)** — build and run a first
  window in a few lines of Python.
- **[Guide](guide/imperative-api.md)** — walkthroughs of the imperative
  API, declarative views, MD3 components, docking, canvas drawing, and
  theming.
- **[Working with Files](guide/working-with-files.md)** — for using
  `tre` directly: YAML view files, `include:`, theme and image files,
  and hot reload.
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

88 milestones have been built against [`ARCHITECTURE.md`](architecture.md)
as of `v0.3.2` — see
[`BUILD_TRACKER.md`](https://github.com/mindderivative/tre/blob/main/BUILD_TRACKER.md)
in the repository for the complete phase-by-phase build history. Every
real engine capability has its own headless, GPU-backed pixel test
proving it actually paints what it claims to, not just that the code
compiles.
