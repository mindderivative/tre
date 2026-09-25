# Target API (M93, proposed)

!!! warning "Design proposal — not implemented"
    This page specifies where `tre` is heading under the approved
    building-block program (M93–M103). Nothing here exists yet except
    where marked **(exists)**. It's published for review by the project
    owner and by Tesserae; M94 onward implements it only after approval.

## Purpose

`tre` becomes a small engine that provides **building blocks** — a node
tree, layout, painting, text, animation, input, accessibility, layers,
and the event loop — and nothing a framework can build from them.
Declarative views, the MD3 component catalog, and MD3 theming move to
the framework (Tesserae). Every name is chosen so a framework author can
tell at a glance what it is.

Decisions D1–D11 (approved) are in `BUILD_TRACKER.md`'s Program section.
This page adds the design choices marked **R1–R8** for review.

## Naming convention

| Rule | Examples |
| --- | --- |
| **Methods are verb phrases.** `create_` makes a detached node, `add_`/`insert_`/`remove` change structure, `set`/`get` read and write properties, `show_`/`hide_` change visibility of layers | `create_box`, `add_child`, `insert_child`, `set`, `get`, `show_layer` |
| **Properties are nouns**, spelled identically everywhere they appear: `set`, `get`, `animate`, and event payloads | `width`, `fill`, `corner_radius`, `opacity` |
| **Events are named for what happened**, without an `on_` prefix; they're registered with `on(event, handler)` | `click`, `pointer_down`, `key_down`, `focus`, `blur` |
| **One name per concept.** No synonyms across node kinds or APIs | one `fill`, not `background` on boxes and `foreground` on text (R1) |
| **Booleans read as adjectives or states** | `visible`, `focusable`, `clip_children`, `multiline` |
| **Units:** lengths are logical pixels with no suffix; durations end in `_ms`; angles end in `_deg`; fractions are `0.0`–`1.0` and say so in docs | `duration_ms`, `rotation_deg`, `trim_end` |
| **Enum values are lowercase `snake_case` strings** | `"horizontal"`, `"flex_start"`, `"cover"` |
| **No abbreviations** beyond universal ones | `rgba`, `id` |
| **Colors are `(r, g, b, a)` tuples of ints 0–255**; parsing hex or theme names is the framework's job (R4) | `(0x67, 0x50, 0xA4, 0xFF)` |

## Design choices for review

| | Choice | Recommendation | Why it matters |
| --- | --- | --- | --- |
| R1 | Paint naming | Every node paints `fill`, with an optional `stroke_color`/`stroke_width`. Replaces `background` (boxes), `foreground` (text, icons, paths), and `border_color`/`border_width` | One concept — "the color this node paints" — gets one name. Cost: renames names M90 introduced, in the same release as every other rename |
| R2 | Creating nodes | `window.create_*()` returns a detached node; the framework attaches it with `add_child`/`insert_child`. No `add_*`-and-attach shortcuts | One way to create, one way to attach; a keyed reconciler needs detached creation anyway |
| R3 | Event propagation | Pointer, wheel, and key events bubble from the target to its ancestors; a handler calls `event.stop()` to end it. `click`, `focus`, `blur`, `change`, `dismiss` don't bubble | Lets a framework put one handler on a composite widget instead of on every inner node |
| R4 | Color format | Tuples only | One format; hex/theme parsing is framework policy |
| R5 | Removing vs destroying | `node.remove()` detaches but keeps the node alive (reattachable, keeps handlers and state); `node.destroy()` frees it and its subtree | Kept-alive subtrees are what screen switching and keyed reordering need |
| R6 | Setting properties | One `node.set(**props)` for layout, paint, text, accessibility, and behavior. No per-group setters | Replaces `set_layout`, `set_text`, `set_checked`, `set_clip_children`, … — one entry point |
| R7 | Headless testing (D9) | One `window.simulate(event, node=None, **fields)` covering every event | Replaces 13 methods (`click`, `hover`, `press_key`, `type_text`, `copy`, …) |
| R8 | Explicit Tab order | A `tab_index` property; default is tree order | Framework-built forms often need a non-tree order |

## Classes and functions

| Name | Role | Status |
| --- | --- | --- |
| `App` | Runs one or more windows: `App()`, `add_window(window)`, `run(max_frames=None)`, `thread_handle()` | exists, unchanged |
| `LoopHandle` | Thread-safe: `call_soon(fn)` | exists, unchanged |
| `Window` | Owns a node tree: creation, layers, window events, batching, text measurement, clipboard, docking, `simulate` | reshaped |
| `Node` | A handle to one node: `set`, `get`, `animate`, `on`/`off`, structure, focus, pointer capture | reshaped |
| `Event` | The payload every handler receives | extended |
| `Painter` | The drawing surface a `canvas` node's `draw` callback receives (was `CanvasContext`) | renamed, extended |
| `register_font(data)` | Registers font bytes; returns family names | exists, unchanged |
| `MONOSPACE_FONT_FAMILY` | The bundled monospace family name | exists, unchanged |

Removed: `View`, `Component`, `Theme`, `Signal`, `Computed`, `Effect`,
`ViewModel`, `batch`, `untrack` (D5, D7).

## Node kinds

| Kind | Created with | What it is |
| --- | --- | --- |
| `box` | `create_box(**props)` | A layout container that may paint a fill, stroke, rounded corners, and a shadow. Replaces `Rect` and `Container` (D3) |
| `text` | `create_text(text, **props)` | Shaped, non-editable text |
| `text_input` | `create_text_input(**props)` | Editable text: single or multi-line, selection, clipboard, IME, syntax spans, folding, whitespace glyphs (D2). Replaces `TextField` and `add_code_editor` |
| `image` | `create_image(rgba, pixel_width, pixel_height, **props)` | Decoded RGBA8 pixels; replace them any time with `set_pixels` (video is repeated `set_pixels`). Replaces `add_image_from_bytes`, `add_video`, `push_frame` (D6) |
| `path` | `create_path(data, **props)` | A vector path — fill, stroke, trim, morph. Replaces `Icon` (D4) |
| `canvas` | `create_canvas(draw, **props)` | Immediate-mode drawing through a `Painter`; `redraw()` asks for a new frame |
| `scroll_view` | `create_scroll_view(**props)` | Clips and scrolls exactly one child |
| `virtual_list` | `create_virtual_list(item_count, materialize, **props)` | Materializes only the visible items |
| `terminal` | `create_terminal(shell, cols, rows, **props)` | A PTY-backed terminal emulator (D1) |

`window.root` is the window's root `box` **(exists implicitly today)**.

## Node

### Properties — `set`, `get`, `animate`

`node.set(**props)` writes any number at once; `node.get(name)` reads
one; `node.animate(name, to, duration_ms, easing="linear",
on_complete=None)` eases an **animatable** (A) property.

| Group | Properties |
| --- | --- |
| **Size and position** | `width`, `height`, `min_width`, `min_height`, `max_width`, `max_height`; `position` (`"relative"`, `"absolute"`), `x`, `y` |
| **Flex layout** (boxes) | `flex_direction`, `flex_grow`, `flex_shrink`, `flex_basis`, `align_items`, `justify_content`, `gap`, `padding`, `padding_top`/`_right`/`_bottom`/`_left`, `margin`, `margin_top`/`_right`/`_bottom`/`_left` |
| **Paint** (R1) | `fill` (A), `stroke_color` (A), `stroke_width` (A), `corner_radius` (A), `opacity` (A) |
| **Transform** | `translate_x` (A), `translate_y` (A), `scale` (A), `rotation_deg` (A) |
| **Shadow** (M95, replaces `elevation`) | `shadow_color` (A), `shadow_offset_x` (A), `shadow_offset_y` (A), `shadow_blur` (A), `shadow_spread` (A) |
| **Visibility and clipping** | `visible`, `clip_children`, `z_index` |
| **Text** (`text`, `text_input`) | `text`, `font_family`, `font_weight`, `font_size`, `line_height`, `text_align` |
| **Text input** | `multiline`, `show_whitespace`, `syntax_spans`, `folded_ranges`, `selection` (`(start, end)` byte offsets) |
| **Path** | `data` (A — animating it morphs, M95), `trim_start` (A), `trim_end` (A) |
| **Image** | `fit` (`"cover"`, `"contain"`, `"fill"`) |
| **Scroll view** | `orientation`, `scroll_offset` (A — needed to animate carousel snapping) |
| **Virtual list** | `item_count`, `item_extent`, `size_hint` |
| **Terminal** | `cols`, `rows`, `scrollback_lines`, `font_size`, `selection` (`(start_row, start_col, end_row, end_col)`) |
| **Interaction** | `focusable`, `tab_index` (R8), `cursor` (`"default"`, `"pointer"`, `"text"`, `"grab"`, …), `hit_testable` |
| **Accessibility** | `role` (`"button"`, `"checkbox"`, `"slider"`, `"link"`, …), `label`, `value`, `checked`, `selected`, `expanded`, `disabled` |

Read-only via `get`: `focused`, `layout_x`, `layout_y`, `layout_width`,
`layout_height` (the computed box, for anchoring and drag math), and every
color property (M96 adds color readback).

Easing: `"linear"` or a cubic bezier `(x1, y1, x2, y2)`. The MD3 named
curves become bezier values in the framework.

### Structure

| Method | Does |
| --- | --- |
| `add_child(child)` | Appends `child`; moves it if already attached elsewhere |
| `insert_child(index, child)` | Inserts at `index`; moves it if already attached — the keyed-reorder primitive |
| `remove()` | Detaches this node; it stays alive and reattachable (R5) |
| `destroy()` | Frees this node and its subtree |
| `children()` | This node's children, in order |
| `parent()` | Its parent, or `None` |

### Events

`node.on(event, handler)` registers, `node.off(event)` removes. A handler
receives an `Event` (below), or nothing if it takes no parameters.

| Event | Fired when | Bubbles (R3) |
| --- | --- | --- |
| `pointer_enter`, `pointer_leave` | The pointer enters or leaves the node | no |
| `pointer_down`, `pointer_move`, `pointer_up` | Button press, pointer movement, button release | yes |
| `click` | Primary press and release on the same node, or keyboard activation | no |
| `secondary_click` | Secondary-button click (context menus) | no |
| `wheel` | Wheel or trackpad scroll | yes |
| `key_down`, `key_up` | A key, with modifiers, while the node or a descendant has focus | yes |
| `text_input` | Committed text for the focused node | no |
| `focus`, `blur` | The node gains or loses keyboard focus | no |
| `change` | A `text_input`'s text or a `scroll_view`'s offset changed by the user | no |
| `scroll` | A `scroll_view` scrolled | no |
| `dismiss` | A layer was dismissed by an outside click or Escape | no |

Pointer capture: `node.capture_pointer()` during `pointer_down` routes
every later pointer event to this node until `pointer_up` or
`release_pointer()`.

Focus: `node.focus()` moves focus to it.

### Event

| Field | Present for |
| --- | --- |
| `type` | every event (was `kind`) |
| `node` | every event — the target |
| `current` | bubbling events — the node whose handler is running |
| `x`, `y` | pointer and wheel events, node-local |
| `window_x`, `window_y` | pointer and wheel events |
| `button` | pointer events (`"primary"`, `"secondary"`, `"middle"`) |
| `delta_x`, `delta_y` | `wheel`, `scroll` |
| `key` | `key_down`, `key_up` — full key names (`"a"`, `"enter"`, `"arrow_left"`, `"f5"`, …), wider than today's set |
| `shift`, `ctrl`, `alt`, `meta` | pointer and key events |
| `text` | `text_input` |
| `old_value`, `new_value` | `change` |
| `stop()` | bubbling events — ends propagation |

## Window

| Group | API |
| --- | --- |
| **Construction** | `Window(width, height, title)` **(exists)** |
| **Nodes** | `root`; `create_box`, `create_text`, `create_text_input`, `create_image`, `create_path`, `create_canvas`, `create_scroll_view`, `create_virtual_list`, `create_terminal` |
| **Content** | `set_content(node)` — show any kept-alive subtree as the window's content (replaces `show_view`) |
| **Layers** (M96) | `show_layer(node, anchor=None, placement="below", modal=False, dismissible=True)`, `hide_layer(node)` — replaces the six open/close pairs, `build_menu`, and `set_context_menu` |
| **Window events** | `on("resize", h)` with `width`/`height`; `on("color_scheme", h)` with `dark` — `tre` no longer themes anything itself (D7) |
| **Batching** (M96) | `with window.batch():` — defers layout and paint until the block ends |
| **Text measurement** (M96) | `measure_text(text, font_family, font_size, font_weight=400, line_height=None, max_width=None)` → `(width, height)`; replaces `get_monospace_cell_size` |
| **Clipboard** | `read_clipboard()` → `str` or `None`; `write_clipboard(text)` → `bool`. Text inputs handle Ctrl+C/X/V/A themselves |
| **Docking** (D10) | below |
| **Testing** (R7) | `simulate(event, node=None, **fields)`; `resize(width, height)` **(exists)** |

### Docking (bare bones, D10)

| API | Does |
| --- | --- |
| `add_dock_zone(side, box, size)` | Makes `box` the container for `side` (`"left"`, `"right"`, `"top"`, `"bottom"`, `"center"`) **(exists)** |
| `dock_panel(side, panel)` | Docks `panel` into a zone **(exists)** |
| `set_active_panel(side, index)` | Which docked panel in a zone is shown (was `set_active_tab`) |
| `start_panel_drag(panel)` | Starts dragging `panel` — call from the framework's own drag handle's `pointer_down` |
| window event `dock_target` | While dragging: the zone under the pointer, or `None` — the framework draws its own highlight |
| window event `dock_drop` | The drag ended: `panel` and the `side` it was docked into, or `None` if cancelled |

Removed: `set_dock_handle`, `set_drop_zone_highlight` (presentation),
`drag_panel_over`, `drop_panel_at` (covered by `simulate`), `build_shell`.

## Painter (canvas)

`fill_rect`, `fill_circle`, `stroke_path` **(exist)**; `fill_path`,
`draw_text` (new); `set_hit_test_circle`, `set_hit_test_path` **(exist)**.
Same color rule (R4); `color` stays the parameter name here because a
painter call has no node to carry a `fill`.

## Rebuilding the MD3 widgets from building blocks

Every removed widget kind maps to building blocks; M94–M96 add exactly
the ones not yet present.

| Widget | Built from |
| --- | --- |
| Checkbox | `box` + check `path` with animated `trim_end`; `click`; `role="checkbox"`, `checked` |
| Radio button | `box` with `corner_radius`; inner dot animated by `scale`; `click`; `role="radio"`, `selected` |
| Switch | `box` track + thumb `box` animated by `translate_x`; `click`, and `pointer_down`/`move`/`up` with capture for dragging; `role="switch"`, `checked` |
| Slider | track and thumb `box`es; `pointer_down` + `capture_pointer` + `pointer_move`; `key_down` for arrows; `role="slider"`, `value` |
| Linear and circular progress | `path` with animated `trim_start`/`trim_end`; `role="progressbar"`, `value` |
| Loading indicator | `path` morphing between shapes by animating `data`, re-triggered from `on_complete` |
| Time picker dial | `path`s, `text`, pointer drag with capture |
| Carousel | `scroll_view` + drag + animated `scroll_offset` for snapping |
| Splitter | `box` handle + capture drag + setting neighbours' `width`/`flex_basis`; `cursor="grab"` |
| Link | `text` + `click`; `role="link"`, `cursor="pointer"` |
| Ripple / state layer (D8) | `box` with `clip_children` + a circle `path` animated by `scale` and `opacity`, positioned from `pointer_down`'s `x`/`y` |
| Elevation | `shadow_*` properties with MD3 values supplied by the framework |
| Dialogs, menus, tooltips, snackbars, sheets, drawers, context menus | `show_layer` with `modal`/`dismissible`/`anchor`; `dismiss` event; `secondary_click` for context menus |
| MD3 theme (D7) | the framework computes colors (HCT) and `set`s them; `color_scheme` window event for OS light/dark |

## Tesserae's needs

| Need | Covered by |
| --- | --- |
| Declarative layer, reconciler, binding evaluator, reactivity | Move to Tesserae (D5, M98) |
| Reorder and move children | `insert_child` |
| Show a kept-alive subtree | `set_content`, `remove` (R5) |
| Set and read every property | `set`/`get` (R6), color readback |
| Batched updates | `window.batch()` |
| Resize and OS light/dark | window events `resize`, `color_scheme` |
| Focus and blur, Tab order | `focus`/`blur` events, `tab_index` (R8) |
| Flex layout stays | Flex layout properties |
| Text measurement | `measure_text` |
| Stable node handles | `Node` holds an id and shared references **(exists)** |
| Clip, z-order, opacity | `clip_children`, `z_index`, `opacity` |
| Fonts, images, threads | `register_font`, `create_image`/`set_pixels`, `App.thread_handle` **(exist)** |

## Migration table

Every current public name, and what it becomes.

### `Window` — creation

| Today | Target |
| --- | --- |
| `add_rect`, `Container` kind | `create_box` |
| `add_text` | `create_text` |
| `add_text_field`, `add_code_editor` | `create_text_input` |
| `add_image_from_bytes`, `add_video` | `create_image`; frames via `set_pixels` |
| `add_image(path)` | removed (D6) |
| `add_icon` | `create_path` (icon data from the framework) |
| `add_canvas` | `create_canvas` |
| `add_scroll_view` | `create_scroll_view` |
| `add_virtual_list` | `create_virtual_list` |
| `add_terminal` | `create_terminal` |
| `add_dock_zone` | unchanged |
| `add_button`, `add_icon_button`, `add_fab`, `add_extended_fab`, `add_segmented_button`, `add_chip`, `add_menu_item`, `add_badge`, `add_linear_progress`, `add_circular_progress`, `add_loading_indicator`, `add_time_picker_dial`, `add_card`, `add_divider`, `add_tooltip`, `add_dialog`, `add_snackbar`, `add_side_sheet`, `add_navigation_rail`, `add_navigation_drawer`, `add_top_app_bar`, `add_toolbar`, `add_split_button`, `add_button_group`, `add_tabs`, `add_search_bar`, `add_search_view`, `add_list_item`, `add_list`, `add_accordion_header`, `add_tree_node`, `add_date_picker_day`, `add_time_input_field`, `add_period_selector`, `add_popover`, `add_link`, `add_spin_box`, `add_pagination`, `add_status_bar`, `add_checkbox`, `add_radio_button`, `add_switch`, `add_slider`, `add_carousel`, `add_splitter`, `add_node_graph`, `add_graph_node` | framework |

### `Window` — everything else

| Today | Target |
| --- | --- |
| `from_view`, `show_view` | `set_content` |
| `theme`, `set_theme` | framework (D7) |
| `build_menu`, `open_menu`, `close_menu`, `open_dialog`, `close_dialog`, `open_snackbar`, `close_snackbar`, `open_side_sheet`, `close_side_sheet`, `open_navigation_drawer`, `close_navigation_drawer` | `show_layer`, `hide_layer` |
| `build_shell`, `begin_container_transform`, `end_container_transform` | framework |
| `get_monospace_cell_size` | `measure_text` |
| `resize_terminal` | `terminal.set(cols=, rows=)` |
| `copy_terminal_selection` | `terminal.get("selection")` + `read`/`write_clipboard` |
| `click`, `hover`, `focus`, `scroll`, `right_click`, `press_key`, `type_text`, `press_ctrl`, `copy`, `cut`, `paste`, `select_all` | `simulate` |
| `copy_to_system_clipboard`, `cut_to_system_clipboard`, `paste_from_system_clipboard` | `read_clipboard`, `write_clipboard`; text inputs handle the keys themselves |
| `resize` | unchanged |
| `dock_panel` | unchanged |
| `set_active_tab` | `set_active_panel` |
| `start_panel_drag(handle)` | `start_panel_drag(panel)` |
| `set_dock_handle`, `set_drop_zone_highlight`, `drag_panel_over`, `drop_panel_at` | removed; `dock_target`/`dock_drop` events and `simulate` |
| `set_virtual_list_window` | `virtual_list.set(...)` — exact property decided in M94 |
| `redraw_canvas(canvas)` | `canvas.redraw()` |

### `Node`

| Today | Target |
| --- | --- |
| `animate(property, to, duration_ms, on_complete)` | `animate(name, to, duration_ms, easing, on_complete)` |
| `get(property)` | `get(name)` |
| `set_layout(...)`, `set_text`, `set_checked`, `set_selected`, `set_clip_children`, `set_syntax_spans`, `set_folded_ranges`, `set_terminal_selection` | `set(...)` |
| `get_text`, `get_checked`, `get_selected`, `is_focused` | `get("text")`, `get("focused")`; checked/selected become framework state |
| `set_on_click`, `set_on_hover_enter`, `set_on_hover_exit`, `set_on_change`, `set_on_focus_enter`, `set_on_focus_exit` | `on("click")`, `on("pointer_enter")`, `on("pointer_leave")`, `on("change")`, `on("focus")`, `on("blur")` |
| `set_context_menu` | `on("secondary_click")` + `show_layer` |
| `enable_interaction` | framework (D8) |
| `add_child` | unchanged |
| `remove` | `remove` (now keeps the node alive, R5); `destroy` to free |
| `push_frame` | `set_pixels` |
| `set_carousel_index`, `get_carousel_index`, `get_carousel_position`, `set_carousel_scroll`, `get_carousel_scroll`, `set_time_picker_dial_time`, `get_time_picker_dial_time`, `set_time_picker_dial_mode`, `get_time_picker_dial_mode` | framework |

### Properties

| Today | Target |
| --- | --- |
| `background`, `foreground` | `fill` (R1) |
| `border_color`, `border_width` | `stroke_color`, `stroke_width` (R1) |
| `elevation` | `shadow_*` |
| `transform` (tuple) | `translate_x`, `translate_y`, `scale` |
| `rotation` (icons) | `rotation_deg` (every node) |
| `shape` (MD3 shape library) | a `path`'s `data` |
| `check_progress`, `select_progress`, `toggle_progress`, `value` | framework-owned animation state |
| `opacity`, `corner_radius`, layout names | unchanged |

### Other

| Today | Target |
| --- | --- |
| `View` (`node`, `poll_reload`, `reconcile`, `set_stylesheet`, `set_theme`, `instantiate`, `click`, `hover`, `focus`, `right_click`), `Component` (`node`, `instantiate`, `remove`) | removed; the declarative layer moves to Tesserae (M98) |
| `Theme` (`role`, `is_set`, `shape`, `elevation`, `typography`) | removed; theming is the framework's (D7) |
| `Signal`, `Computed`, `Effect`, `ViewModel`, `batch`, `untrack` | Tesserae (D5) |
| `CanvasContext` | `Painter` |
| `Event.kind` | `Event.type` |
| MD3 named motion curves | cubic bezier values (framework) |
| `App`, `LoopHandle`, `register_font`, `MONOSPACE_FONT_FAMILY` | unchanged |
