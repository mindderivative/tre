# Python API Reference

The `tre` package exposes six public classes. `App`, `Window`, `Node`,
`View`, and `CanvasContext` are compiled pyo3 bindings (`tre._core`);
`Signal` and `ViewModel` are plain Python, layered on top.

| Class | Purpose |
| --- | --- |
| [`App`](app.md) | Opens and drives one or more `Window`s together in one blocking call |
| [`Window`](window.md) | Owns a node tree, its size/title; creates nodes, dispatches input, docking, theming |
| [`Node`](node.md) | A handle to one node — events, animation, property reads/writes |
| [`View`](view.md) | Loads a declarative `view.yaml` file into its own node tree |
| [`Signal`](signal-and-viewmodel.md) / [`ViewModel`](signal-and-viewmodel.md) | Reactive data binding for declarative views |
| [`CanvasContext`](canvas-context.md) | The draw surface passed to a `Canvas` node's `draw` callback |

```python
from tre import App, Node, Signal, View, ViewModel, Window
```

For a narrative walkthrough of how these fit together, start with
[Getting Started](../../getting-started.md) or the
[Guide](../../guide/imperative-api.md) section instead — this reference
is organized by class/method, not by task.
