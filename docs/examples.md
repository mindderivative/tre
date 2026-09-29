# Examples

Each script in
[`examples/`](https://github.com/mindderivative/tre/tree/main/examples) is
small, runnable, and self-checking: it drives its building block
headlessly with `window.simulate` and `window.advance`, asserts what
should happen, then opens a window for a few seconds. Run one with
`python examples/<name>.py`.

| Script | Shows |
| --- | --- |
| `showcase.py` | A settings window combining the widgets below: switches, a slider, a text field, a sorted list, a menu, and a dialog |
| `switch.py` | A complete widget from two boxes — the [Building a Widget](guide/building-a-widget.md) walkthrough |
| `slider.py` | Pointer capture, arrow keys, and assistive-technology actions in one widget |
| `ripple.py` | A press ripple: a clipped, scaling round box that destroys itself when done |
| `reorder.py` | A keyed list reordered with `insert_child`, keeping identity, focus, and animations |
| `layout.py` | An app frame from flex rows, a growing content area, and an absolute badge |
| `grid.py` | A form whose labels and fields line up in grid columns, over a gallery whose cells wrap to fit |
| `animation.py` | Linear and bezier easing, retargeting mid-flight, and chained `on_complete` |
| `pan_zoom.py` | The transform parts animating independently |
| `shadows.py` | Layered shadows easing between elevation levels |
| `show_damage.py` | What partial redraw repaints each frame, tinted; `--watch` keeps the window open for checking on a real desktop |
| `path_morph.py` | One path morphing into another |
| `canvas.py` | A chart drawn with a `Painter`, with a precise hit test |
| `node_graph.py` | Absolutely placed nodes over a canvas of edges, selected by click, zoomed by transform |
| `text_field.py` | A text input in a box that outlines on focus; `input` and `change` |
| `clipboard.py` | Copy, cut, paste, and select-all in a text input |
| `layers.py` | An anchored menu and a modal dialog |
| `scrollable_list.py`, `variable_height_list.py` | Virtual lists with fixed and per-row heights |
| `docking.py` | Docked panels dragged between zones |
| `two_windows.py` | One `App` driving two windows |
| `threadsafe_reload.py` | A background file watcher reloading settings inside `App.run()` |
