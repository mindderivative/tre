# Migrating to 0.3.5

`tre` 0.3.5 makes `tre` a small building-block engine: nodes, layout, paint,
animation, events, text, accessibility, layers, and threading. Everything built
*from* those blocks — declarative views, data binding and reactivity, the
Material Design 3 widgets, theming, and file loading — now belongs to a framework
built on `tre`, such as Tesserae. Every surviving name also takes one consistent
form: `window.create`, `node.set`/`get`/`on`, and `window.simulate`.

It's a breaking release. Nothing is kept for compatibility; a removed name fails
with Python's own `AttributeError` or `ImportError`.

## Check a codebase first

Run your tests with `TRE_FORBID_REMOVED=1` in the environment:

```bash
TRE_FORBID_REMOVED=1 python -m pytest
```

On 0.3.5, every name below that it removes or renames then raises an
`AttributeError` naming its replacement, rather than Python's bare "has no
attribute" — old property names too, in `animate`, `get`, `get_target`, and
`stop_animation`, and `animate`'s old positional `on_complete`. `from tre
import View` fails with `ImportError`.

To find every use *before* upgrading, while the old names still work, copy
[`tre/_removed.py`](https://github.com/mindderivative/tre/blob/0.3.5/python/tre/_removed.py)
into a project on 0.3.4 — it uses only names 0.3.4 has — and call its
`install()` from `conftest.py` when `TRE_FORBID_REMOVED=1` is set.

## Moved to the framework

| 0.3.4 | 0.3.5 |
| --- | --- |
| `View` (`node`, `poll_reload`, `reconcile`, `set_stylesheet`, `set_theme`, `instantiate`, `click`, `hover`, `focus`, `right_click`), `Component`, `Window.from_view`, `Window.show_view` | the framework's declarative layer; a screen is `window.root.add_child(screen)` |
| `Signal`, `Computed`, `Effect`, `ViewModel`, `batch`, `untrack` | the framework's reactivity |
| `Theme`, `Window.theme`, `Window.set_theme` | the framework's theming |
| The MD3 widget factories — `add_button`, `add_icon_button`, `add_fab`, `add_extended_fab`, `add_segmented_button`, `add_chip`, `add_menu_item`, `add_badge`, `add_linear_progress`, `add_circular_progress`, `add_loading_indicator`, `add_time_picker_dial`, `add_card`, `add_divider`, `add_tooltip`, `add_dialog`, `add_snackbar`, `add_side_sheet`, `add_navigation_rail`, `add_navigation_drawer`, `add_top_app_bar`, `add_toolbar`, `add_split_button`, `add_button_group`, `add_tabs`, `add_search_bar`, `add_search_view`, `add_list_item`, `add_list`, `add_accordion_header`, `add_tree_node`, `add_date_picker_day`, `add_time_input_field`, `add_period_selector`, `add_popover`, `add_link`, `add_spin_box`, `add_pagination`, `add_status_bar`, `add_checkbox`, `add_radio_button`, `add_switch`, `add_slider`, `add_carousel`, `add_splitter`, `add_node_graph`, `add_graph_node` | the framework's widgets, built from boxes, text, and paths |
| Their state — `set_checked`, `get_checked`, `set_selected`, `get_selected`, `set_carousel_index`, `get_carousel_index`, `get_carousel_position`, `set_carousel_scroll`, `get_carousel_scroll`, `set_time_picker_dial_time`, `get_time_picker_dial_time`, `set_time_picker_dial_mode`, `get_time_picker_dial_mode` — and their properties `check_progress`, `select_progress`, `toggle_progress` | the framework's own state and animation |
| `build_menu`, `open_menu`, `close_menu`, `open_dialog`, `close_dialog`, `open_snackbar`, `close_snackbar`, `open_side_sheet`, `close_side_sheet`, `open_navigation_drawer`, `close_navigation_drawer`, `Node.set_context_menu` | [`show_layer`/`hide_layer`](api/python/layers.md), and `on("secondary_click")` for a context menu |
| `build_shell`, `begin_container_transform`, `end_container_transform` | the framework's shell and transitions, with `animate` |
| `Node.enable_interaction` (the ripple and state layer) | the framework's hover and press feedback, from `pointer_enter`/`pointer_leave`/`pointer_down` |
| `add_icon` | a `"path"` node from the icon's SVG data |
| `add_image(path)` | decode the file yourself; an `"image"` node takes RGBA8 pixels |
| The MD3 named motion curves | cubic bezier values passed as `animate(..., easing=(x1, y1, x2, y2))` |

## Renamed

### Creating nodes

`window.create(kind, **props)` makes a detached node; attach it with
`add_child` — `window.root.add_child(node)` for a top-level one.

| 0.3.4 | 0.3.5 |
| --- | --- |
| `add_rect(background, width, height, x, y, border_color, border_width)` | `create("box", fill=, width=, height=, stroke_color=, stroke_width=)`; `x`/`y` with `position="absolute"` |
| `add_text(content, foreground, ...)` | `create("text", text=, fill=, ...)` |
| `add_text_field(background, width, height, content, ...)` | `create("text_input", text=, ...)` — a text input paints no box of its own: put it in a `"box"` |
| `add_code_editor(content, ...)` | `create("text_input", text=, multiline=True, show_whitespace=True, font_family=MONOSPACE_FONT_FAMILY)` |
| `add_image_from_bytes(rgba, pixel_width, pixel_height, ...)` | `create("image", rgba=, pixel_width=, pixel_height=, ...)` |
| `add_video(...)` | an `"image"` node; each frame is `set(rgba=, pixel_width=, pixel_height=)` |
| `add_canvas(width, height, draw)` | `create("canvas", draw=, ...)` — it draws when created |
| `add_scroll_view(...)` | `create("scroll_view", ...)` |
| `add_virtual_list(...)` | `create("virtual_list", ...)` — `materialize(index)` returns a node, and the list builds its visible rows itself |
| `add_terminal(shell, cols, rows, background, ...)` | `create("terminal", shell=, cols=, rows=, palette={"background": ...})` |

### `Window`

| 0.3.4 | 0.3.5 |
| --- | --- |
| `get_monospace_cell_size(font_size)` | `measure_text("M", font_family=MONOSPACE_FONT_FAMILY, font_size=)` |
| `resize_terminal(node, cols, rows)` | `terminal.set(cols=, rows=)` |
| `copy_terminal_selection()` | `terminal.get("selection")`; Ctrl+Shift+C copies it |
| `set_virtual_list_window(list, start, end)` | nothing: a virtual list builds its visible rows |
| `redraw_canvas(canvas)` | `canvas.redraw()` |
| `click`, `hover`, `focus`, `scroll`, `right_click`, `press_key`, `type_text`, `press_ctrl`, `copy`, `cut`, `paste`, `select_all` | `simulate` — `simulate("click", node=)`, `("pointer_move", node=)`, `("focus", node=)`, `("wheel", node=, delta_y=)`, `("secondary_click", node=)`, `("key_down", key=)`, `("input", text=)`, `("key_down", key="c", ctrl=True)`, ... |
| `copy_to_system_clipboard`, `cut_to_system_clipboard`, `paste_from_system_clipboard` | `read_clipboard()`, `write_clipboard(text)`; a text input handles Ctrl+C/X/V/A itself |
| `set_active_tab(side, index)` | `set_active_panel(side, index)` |
| `start_panel_drag(handle)` | `start_panel_drag(panel)` |
| `set_dock_handle`, `set_drop_zone_highlight`, `drag_panel_over`, `drop_panel_at` | the framework draws the handle and highlight from the `dock_target`/`dock_drop` window events; `simulate` drives a drag |

### `Node`

| 0.3.4 | 0.3.5 |
| --- | --- |
| `set_layout(...)` | `set(...)` |
| `set_text(t)` | `set(text=t)` |
| `get_text()` | `get("text")` |
| `is_focused()` | `get("focused")` |
| `set_clip_children(b)`, `set_syntax_spans(s)`, `set_folded_ranges(r)` | `set(clip_children=)`, `set(syntax_spans=)`, `set(folded_ranges=)` |
| `set_terminal_selection(a, b, c, d)` | `set(selection=(a, b, c, d))` |
| `push_frame(rgba, width, height)` | `set(rgba=, pixel_width=, pixel_height=)` |
| `set_on_click`, `set_on_hover_enter`, `set_on_hover_exit`, `set_on_change`, `set_on_focus_enter`, `set_on_focus_exit` | `on("click")`, `on("pointer_enter")`, `on("pointer_leave")`, `on("change")`, `on("focus")`, `on("unfocus")` |

### Properties

In `animate`, `get`, `get_target`, `stop_animation`, and `set`:

| 0.3.4 | 0.3.5 |
| --- | --- |
| `background`, `foreground` | `fill` |
| `border_color`, `border_width` | `stroke_color`, `stroke_width` |
| `corner_radii_override` | `corner_radius` as a 4-tuple |
| `elevation` | `shadows` |
| `transform` | `translate_x`, `translate_y`, `scale` |
| `rotation` | `rotation_deg` |
| `shape` | a `"path"` node's `data` |
| `value` on a slider or progress indicator | the framework's own state; `value` stays the accessibility value |

### Events and the rest

| 0.3.4 | 0.3.5 |
| --- | --- |
| `Event.kind` | `Event.type` |
| `Event.node`, `Event.source` | `Event.target` |
| `Event.position` | `Event.window_x`, `Event.window_y` |
| `CanvasContext` | `Painter` |

## Behavior changes

These keep their names but act differently:

- **`on("click")` bubbles**, to every ancestor's `click` listener, and **doesn't
  make a node focusable**: set `focusable=True` to put a node in the Tab order,
  where Enter and Space then fire `click`. `set_on_click` did both implicitly and
  fired on its own node only.
- **`set(text=...)` fires no `change`** — that event is for edits the user
  makes. `set_text` fired one.
- **A canvas draws when it's created** and when its `draw` is set, as well as on
  `redraw()`; `add_canvas` waited for the first `redraw_canvas`.
- **A virtual list builds its own rows** whenever layout runs, and releases the
  ones scrolled away; a `materialize` or `size_hint` that raises is logged and
  leaves its row empty rather than raising.
- **A wheel over a terminal**: a negative `delta_y` scrolls up into its history,
  as for any other scrollable node; `window.scroll(terminal, 400)` scrolled up.
- **Ctrl+A in a text input selects all** — it did nothing before.
- **What an app copies stays on the clipboard** while the app runs, with or
  without a clipboard manager.
- **`start_panel_drag` raises** `ValueError` for a node that isn't docked,
  instead of returning `False`.
- **`remove()` keeps a node alive** for reattaching; `destroy()` frees it — the
  0.3.4 change, in case you're coming from 0.3.3.
