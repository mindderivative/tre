# Examples

Each script in
[`examples/`](https://github.com/mindderivative/tre/tree/main/examples) is
small and runnable. Most are self-checking: they drive their building
block headlessly with `window.simulate` and `window.advance` and assert
what should happen before opening a window for a few seconds. Run one with
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
| `hidpi.py` | `dpi_scaling`: a card laid out in logical pixels and drawn sharp at the display's scale; `--watch` keeps the window open to drag between monitors |
| `snapshot.py` | `window.snapshot()` and `tre.write_png`: a window's pixels at 1x and 2x, with no display |
| `gradients.py` | `Gradient.linear`, `.radial` and `.sweep` fills, one animating between two gradients |
| `effects.py` | `blur`, `backdrop_blur` (frosted glass, animating) and `blend_mode` over a striped background |
| `languages.py` | `set_system_fonts`: Latin, CJK, Hebrew, Indic, Thai and colour emoji in one window |
| `rich_text.py` | `spans` (colour, weight, italic, underline, strikethrough) and `selectable` text |
| `touch.py` | `tap`, `long_press`, `pan` and `pinch`, driven by simulated fingers (or real ones with `--watch`) |
| `file_drop.py` | `file_hover`, `file_hover_cancel` and `file_drop` for a drop zone (simulated, or real files with `--watch`) |
| `accessibility_preferences.py` | `get("reduced_motion")`, `get("high_contrast")` and their events: snap instead of ease, swap to a high-contrast palette |
| `frame_stats.py` | `window.frame_stats()`: stage times, redraw kind, fps and percentiles for a window that animates |
| `transparent_window.py` | `transparent=True`: a rounded, undecorated card on the desktop, with a close button |
| `spring.py` | `easing=("spring", bounce)`: overshoot, and a retarget that keeps its speed |
| `sticky_headers.py` | `sticky=0`: section headers that hold the top of a scroll view |
| `path_morph.py` | One path morphing into another |
| `canvas.py` | A chart drawn with a `Painter`, with a precise hit test |
| `node_graph.py` | Absolutely placed nodes over a canvas of edges, selected by click, zoomed by transform |
| `text_field.py` | A text input in a box that outlines on focus; `input` and `change` |
| `clipboard.py` | Copy, cut, paste, and select-all in a text input |
| `layers.py` | An anchored menu and a modal dialog |
| `scrollable_list.py`, `variable_height_list.py` | Virtual lists with fixed and per-row heights |
| `scroll_keys.py` | A scroll view driven by keys and focus, with the `scroll` event |
| `docking.py` | Docked panels dragged between zones |
| `two_windows.py` | One `App` driving two windows |
| `custom_titlebar.py` | An undecorated window drawing its own title bar: a drag region, minimize/maximize/close, a resize border, and room for macOS's traffic lights; `--watch` keeps it open, `--menu` adds the OS's window menu |
| `shader_shine.py` | Shining text: an effect shader over a text node, driven by `frame.time` (WGSL in `shaders/shine.wgsl`) |
| `shader_panel.py` | A fractal behind a panel: an animated fill shader painting a rounded box, with ordinary children on top (`shaders/julia.wgsl`) |
| `threadsafe_reload.py` | A background file watcher reloading settings inside `App.run()` |
