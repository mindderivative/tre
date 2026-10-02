# Python API Reference

The `tre` package's public classes and functions, all compiled pyo3
bindings (`tre._core`), with type stubs for IDEs and `mypy`.

| Class | Purpose |
| --- | --- |
| [`App`](app.md) | Opens and drives one or more `Window`s together in one blocking call |
| [`LoopHandle`](app.md#loophandle) | The one thread-safe object: queues a callable onto a running `App`'s event loop, via `App.thread_handle()` |
| [`Window`](window.md) | Owns a node tree and its size and title; creates nodes, shows layers, simulates input, docking |
| [`Node`](node.md) | A handle to one node — events, animation, property reads/writes |
| [`Event`](events.md#event) | What a listener receives when it takes one argument — see [Events and Listeners](events.md#event) |
| [`Shader`](shader.md) | WGSL that paints a node (0.5.1): checked when it is created, set with `node.set(shader=...)` |
| [`Painter`](painter.md) | The drawing surface a canvas node's `draw` callback receives |

Across classes: [Nodes and Properties](properties.md) lists every kind and
property, [Events and Listeners](events.md) every event and `Event` field,
[Paint, Paths, and Animation](paint.md) the paint model, and
[Layers](layers.md) overlays.

```python
from tre import App, Window
```

## Module functions and constants

| Name | Purpose |
| --- | --- |
| `MONOSPACE_FONT_FAMILY` | `"Hack Nerd Font Mono"`, the bundled monospace face terminals shape with, and a code editor's natural font — use it for sibling nodes (a gutter, line numbers) that must line up with their grid |
| `register_font(data: bytes) -> list[str]` | Registers a font the caller already loaded (a `.ttf`/`.otf`/`.ttc` file's raw bytes) with every current and future window; returns the family names it contains. See [Text → Fonts](../../guide/text.md#fonts) |

For a narrative walkthrough of how these fit together, start with
[Getting Started](../../getting-started.md) or the
[Guide](../../guide/nodes-and-layout.md) instead — this reference
is organized by class/method, not by task.
