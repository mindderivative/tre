# TRE -- Tesserae Render Engine

TRE is a GPU-accelerated 2D rendering engine backend, built in Rust and exposed to Python via PyO3. It renders vector paths, dynamic typography, and SVG; drives animation and custom shaders; and integrates with the native desktop (clipboard, file dialogs, tray, accessibility, focus).

This site is the curated, developer-facing documentation for the project -- distinct from the raw, chronological engineering log kept in `documentation/` in the repository, which records the phase-by-phase build history.

## Where to start

- **Getting Started** -- install `tre-python` and render your first frame.
- **Architecture** -- how the engine is put together: the Canvas API, the IR, the RHI, the sort/batch pipeline.
- **Python API** -- the full `tre` module reference.
- **Platform Integration** -- windowing, input, clipboard, tray, accessibility, focus.

## Project links

- Source: [github.com/mindderivative/tre](https://github.com/mindderivative/tre)