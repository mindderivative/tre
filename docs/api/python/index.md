# Python API Reference

The `tre` package's public classes. `App`, `Window`, `Node`, `View`,
`Component`, `Theme`, `LoopHandle`, `Event`, and `CanvasContext` are
compiled pyo3 bindings (`tre._core`); `Signal`, `Computed`, `Effect`,
and `ViewModel` are plain Python, layered on top.

| Class | Purpose |
| --- | --- |
| [`App`](app.md) | Opens and drives one or more `Window`s together in one blocking call |
| [`LoopHandle`](app.md#loophandle) | The one thread-safe object: queues a callable onto a running `App`'s event loop, via `App.thread_handle()` |
| [`Window`](window.md) | Owns a node tree, its size/title; creates nodes, dispatches input, docking, theming |
| [`Node`](node.md) | A handle to one node — events, animation, property reads/writes |
| [`View`](view.md) | A declarative view, built from a spec `dict` (or a YAML file) into its own node tree |
| [`Component`](view.md#component) | An embedded, independent instance of another view spec, created via `View.instantiate` |
| [`Theme`](window.md#theme) | Read-only access to a `Window`'s live MD3 theme resolution, via `window.theme` |
| [`Event`](node.md#the-event-payload) | The payload a handler receives when it takes one argument |
| [`Signal`](signal-and-viewmodel.md#signal) / [`Computed`](signal-and-viewmodel.md#computed) / [`Effect`](signal-and-viewmodel.md#effect) | Reactive values, derived values, and side effects (plus `batch`/`untrack`) |
| [`ViewModel`](signal-and-viewmodel.md#viewmodel) | Wires a `View`'s declared bindings and handlers to Python state |
| [`CanvasContext`](canvas-context.md) | The draw surface passed to a `Canvas` node's `draw` callback |

```python
from tre import App, Computed, Effect, Signal, View, ViewModel, Window
```

## Module functions and constants

| Name | Purpose |
| --- | --- |
| `MONOSPACE_FONT_FAMILY` | `"Hack Nerd Font Mono"`, the bundled monospace face `add_terminal`/`add_code_editor` shape with — use it for sibling nodes (a gutter, line numbers) that must line up with their grid |
| `register_font(data: bytes) -> list[str]` | Registers a font the caller already loaded (a `.ttf`/`.otf`/`.ttc` file's raw bytes) with every current and future window; returns the family names it contains. See [Theming & Accessibility → Custom fonts](../../guide/theming-and-accessibility.md#custom-fonts) |

For a narrative walkthrough of how these fit together, start with
[Getting Started](../../getting-started.md) or the
[Guide](../../guide/imperative-api.md) section instead — this reference
is organized by class/method, not by task.
