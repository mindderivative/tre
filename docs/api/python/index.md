# Python API Reference

The `tre` package's public classes, all compiled pyo3 bindings
(`tre._core`). *Removed in 0.3.5:* `View`, `Component`, and the reactivity
layer (`Signal`, `Computed`, `Effect`, `ViewModel`, `batch`, `untrack`) —
declarative views and reactivity belong to a framework built on `tre`, such
as Tesserae.

| Class | Purpose |
| --- | --- |
| [`App`](app.md) | Opens and drives one or more `Window`s together in one blocking call |
| [`LoopHandle`](app.md#loophandle) | The one thread-safe object: queues a callable onto a running `App`'s event loop, via `App.thread_handle()` |
| [`Window`](window.md) | Owns a node tree, its size/title; creates nodes, dispatches input, docking, theming |
| [`Node`](node.md) | A handle to one node — events, animation, property reads/writes |
| [`Event`](node.md#the-event-payload) | The payload a handler receives when it takes one argument |
| [`CanvasContext`](canvas-context.md) | The draw surface passed to a `Canvas` node's `draw` callback |

*New in 0.3.4*, the [target API](../../design/target-api.md)'s building
blocks: [Nodes and Properties](properties.md) (`window.create` and every
property), [Events and Listeners](events.md), [Paint, Paths, and
Animation](paint.md), and [Layers](layers.md).

```python
from tre import App, Window
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
