# MD3 Components

`tre` implements a real, wide Material Design 3 component catalog directly
in the engine — not as Python-side wrapper logic, but as native `NodeKind`
variants (or themed compositions of `Rect`/`Text`/`Container`) with their
own state, dispatch, and painting. As of this writing the catalog covers
56 `Window.add_*` factory methods, all listed below, organized by real
usage category rather than the order they were built in. Every factory
attaches its result to the window's own root and returns the created
`Node` (or, for compound widgets, a small tuple of nodes — noted where
that applies) — the same generic `Node` every `set_on_click`/
`enable_interaction`/`animate` call already works against, so nothing
below needs a second API surface to become interactive.

Most factories share the same trailing kwargs: `x`/`y` (absolute
position, left as `None` when a parent's own flex layout should place the
node instead) and, for `Rect`-backed factories, `border_color`/
`border_width` (both `None` by default — no border painted). Those are
omitted from the tables below where every row in a table has them, to
keep the tables scannable; assume they're present unless a factory's own
row says otherwise.

## Buttons & Actions

```python
btn = window.add_button("Save", width=120, height=40, variant="filled")
btn.set_on_click(lambda: print("saved"))
btn.enable_interaction()  # opts into ripple/hover state layers
```

- `variant` is one of `"filled"`, `"outlined"`, `"text"`, `"elevated"`,
  `"tonal"` — MD3's own real five-variant button anatomy, each resolving
  its own color pair via the theme.
- Returns the button's *container* node — `set_on_click`/
  `enable_interaction`/`animate` all work on it exactly like any other
  node. Deliberately does **not** auto-call `enable_interaction()`
  (Design Principle 6: only a node that opts in pays the cost).
- `add_icon_button`/`add_fab`/`add_extended_fab` follow the identical
  variant-resolves-color-and-shape pattern, just with MD3's own distinct
  anatomy per component (a FAB's shape is keyed by *size*, not variant;
  its elevation is flat regardless of size or color).

| Factory | Signature | Notes |
|---|---|---|
| `add_button` | `label, width, height, variant="filled"` | Five real MD3 variants. |
| `add_icon_button` | `icon, size=40.0, variant="standard"` | Square, icon-only. |
| `add_fab` | `icon, size="default", variant="surface"` | `size` is `"small"`/`"default"`/`"large"`. |
| `add_extended_fab` | `label, width, icon=None, variant="primary"` | FAB with a label. |
| `add_segmented_button` | `labels, width, selected=None, height=...` | `labels` is a list; `selected` the initially-active index. |
| `add_button_group` | `labels, width, height, variant="filled"` | A connected row of buttons sharing one outline; hover/press "tightens" the shape between neighbors. |
| `add_split_button` | `label, width, height, variant="filled"` | A button with a trailing dropdown-affordance split, same shape-morph behavior as `add_button_group`. |

## Selection & Input

```python
box = window.add_checkbox(background=(0x67, 0x50, 0xA4, 0xFF), width=24, height=24, checked=False)
box.set_on_click(lambda: box.set_checked(not box.get_checked()))
box.animate("check_progress", 1.0, duration_ms=150)
```

- `background` is the box's own fill color; `checked` seeds the initial
  state.
- The engine never flips `checked` itself on click — by design, checked
  state depends on what the app's data means, so the app's own
  `on_click` handler calls `set_checked` explicitly, typically alongside
  an `animate("check_progress", ...)` call for the visual consequence.
- `node.set_checked(value)` — plain, non-animated write; also fires
  `Change`. `node.get_checked()` reads it back. `node.animate
  ("check_progress", 0.0 | 1.0, duration_ms)` is the checkmark's own
  draw animation, independent of `checked` itself.

```python
slider = window.add_slider(background=(0x03, 0xDA, 0xC6, 0xFF), width=200, height=32, value=0.3)
slider.set_on_change(lambda: print("new value:", slider.get("thumb_position")))
```

- `value` seeds `thumb_position`, clamped to `0.0..=1.0`. Drag-to-set is
  entirely built into the engine's own input dispatch — no Python wiring
  needed for that half. Arrow-key increments work once the slider has
  keyboard focus (it opts into `Role::Slider` + `Action::Focus` at
  construction, so it's Tab-reachable from the start).
- `node.animate("thumb_position", value, duration_ms)` for an
  app-triggered eased move (e.g. a keyboard nudge), distinct from the
  drag path above. `node.set_on_change(...)` fires when a drag genuinely
  ends.

| Factory | Signature | Notes |
|---|---|---|
| `add_radio_button` | `size=20.0, selected=false` | Single-select affordance — grouping/exclusivity is the app's own concern, same as `pyCopper`'s precedent. |
| `add_switch` | `width=52.0, height=32.0, on=false` | `node.set_checked`/`get_checked` shared with `Checkbox`'s own accessor names. |
| `add_spin_box` | `value, x=None, y=None` | Numeric stepper; no `width`/`height` — sized from its own content. |

## Text Fields, Code Editor & Terminal

```python
field = window.add_text_field(
    background=(0xEE, 0xEE, 0xEE, 0xFF), width=280, height=48,
    content="", font_family="Roboto", font_weight=400.0, font_size=16.0,
)
```

- Real keyboard editing, mouse click-to-position and drag-to-select, IME
  composition, and clipboard integration. Opts into `Role::TextInput` +
  `Action::Focus` at construction.
- `node.set_text(content)` — plain write, resets the cursor to the new
  content's end, fires `Change`. `node.get_text()` reads it back.
  `node.set_on_change(...)` fires on any edit (typing, cut, paste,
  backspace/delete).
- `add_time_input_field(value, x=None, y=None)` is a small, fixed-format
  sibling (`HH:MM`) built on the same real text-editing machinery, sized
  from its own content.

```python
editor = window.add_code_editor(
    content="fn main() {}\n", background=(0x1E, 0x1E, 0x1E, 0xFF),
    width=600, height=400, font_weight=400.0, font_size=14.0,
)
editor.set_syntax_spans([(0, 2, (0x56, 0x9C, 0xD6, 0xFF))])  # (start, end, rgba) per span
editor.set_folded_ranges([(20, 80)])
```

- Always shapes with the engine's own bundled monospace face ("Hack
  Nerd Font Mono") — a genuinely monospace editor, not an approximation;
  `font_family` isn't exposed as a param (a proportional face would
  defeat the point of this widget class).
- Real code folding (`node.set_folded_ranges`, a list of
  `(start_offset, end_offset)` byte ranges collapsed to one marker
  line each), real syntax highlighting (`node.set_syntax_spans`, a list
  of `(start, end, rgba)` color spans over the raw content — the app
  owns tokenization, the engine just paints the spans), and real visible
  whitespace glyphs. Real vertical *and* horizontal scroll+clip for
  content larger than the box, with caret-follow on both axes.
- Deliberately out of scope, matching the sibling `pyCopper` project's
  own identical choice: multi-cursor editing, a minimap, bracket
  auto-closing/matching, and any language-server (LSP) integration — an
  LSP client is an application concern, not a widget's hard dependency.

```python
term = window.add_terminal(shell="/bin/bash", cols=80, rows=24, background=(0x00, 0x00, 0x00, 0xFF))
```

- Spawns `shell` on a real pseudo-terminal (`portable-pty`) and parses
  its real byte stream with a real VT100/ANSI parser (`vt100`) — a
  genuine terminal emulator, not a styled text box. POSIX only.
- `width`/`height` aren't parameters — they're computed from `cols`/
  `rows` against the bundled monospace face's own measured cell size,
  so the node's box always exactly fits its real character grid.
  `scrollback_lines` (default `1000`) is a real retained history length;
  `0` disables it.
- A real mouse wheel over a focused terminal moves its live scrollback
  viewport directly (no separate Python-callable scroll method — driven
  by the same input dispatch every other scrollable surface uses).
  `Window.resize_terminal(node, cols, rows)`,
  `Window.get_monospace_cell_size(font_size)`, and
  `Window.copy_terminal_selection()` are the real `Window`-level (not
  `Node`-level) methods for resizing, measuring, and reading a live
  terminal session — see [`Window`](../api/python/window.md).

## Progress & Status

| Factory | Signature | Notes |
|---|---|---|
| `add_circular_progress` | `size=48.0, value=0.0` | `value` in `0.0..=1.0`; animate it directly with `node.animate("value", ...)`. |
| `add_linear_progress` | `width, height=4.0, value=0.0` | Same `value` contract as circular. |
| `add_loading_indicator` | `size=48.0, color=None` | MD3's newer indeterminate spinner shape; `color` falls back to the theme's primary. |

## Navigation & Shell Composition

Tabs, navigation rails/drawers, and app bars are shell-composition
pieces as much as they are components — see
[Docking & Shell Layout](docking-and-shell.md) for how they combine with
`build_shell`/docking to form a full window chrome. Quick reference:

| Factory | Signature | Notes |
|---|---|---|
| `add_tabs` | `labels, icons=None, selected=None, width=None` | A real MD3 tab row with an animated active-indicator. |
| `add_navigation_rail` | `labels, icons, selected=None` | The compact, icon-first side rail. |
| `add_navigation_drawer` | `labels, icons, selected=None, modal=false, width=..., height=None` | `modal=True` opens/closes via `Window.open_navigation_drawer`/`close_navigation_drawer` as a real dismissable overlay; non-modal is a permanent layout child. |
| `add_toolbar` | `variant="docked", orientation=None, color=None, width=None, height=None` | A floating or docked action-icon bar. |
| `add_top_app_bar` | `title, leading_icon=None, trailing_icons=None, width=None` | The window's own top title bar. |
| `add_status_bar` | `text, width=None` | A window-bottom status strip, typically passed to `build_shell(status_bar=...)`. |

## Overlays

Dialogs, menus, snackbars, and the side sheet all share one real
underlying primitive — `Tree::open_overlay`/`close_overlay` (the same
mechanism `Node.set_context_menu`'s right-click path already uses) —
exposed as a matched `open_*`/`close_*` pair per component instead of a
generic call, since each has its own real dismiss-on-outside-click/
dismiss-on-escape/modal defaults:

```python
dialog = window.add_dialog("Delete file?", "This can't be undone.", width=360, height=180)
window.open_dialog(dialog)
# ... later, e.g. from a button's own on_click:
window.close_dialog(dialog)
```

| Factory | Open / Close | Notes |
|---|---|---|
| `add_dialog` | `open_dialog(dialog)` / `close_dialog(dialog)` | `headline, text, width, height`. Modal by default. |
| `add_snackbar` | `open_snackbar(snackbar)` / `close_snackbar(snackbar)` | `text, width, action_label=None, closable=false`. Non-modal, auto-dismiss is the app's own timer. |
| `add_side_sheet` | `open_side_sheet(sheet)` / `close_side_sheet(sheet)` | `width=..., height=None, modal=false`. |
| `build_menu` | `open_menu(anchor, menu)` / `close_menu(menu)` | `items: list[Node], width` — a panel of `add_menu_item(...)` rows; anchored below `anchor`. `add_tooltip`'s and `add_search_view`'s own panels reuse this identical `open_menu`/`close_menu` pair rather than getting dedicated ones. |
| `add_navigation_drawer(modal=True)` | `open_navigation_drawer(drawer)` / `close_navigation_drawer(drawer)` | See the Navigation table above. |

`add_menu_item(label, icon=None, submenu=false, width=200.0)` builds one
row for `build_menu`; `submenu=True` paints the row with a trailing
disclosure affordance (the app still owns opening a nested `build_menu`
on click — no automatic nesting).

## Cards, Lists, Chips & Structural Rows

| Factory | Signature | Notes |
|---|---|---|
| `add_card` | `width, height, variant="elevated"` | `variant` is `"elevated"`/`"filled"`/`"outlined"`. Content-free — populate via `Node.add_child` (`Card` establishes the "engine gives primitives, app composes content" contract several other overlays reuse, e.g. `add_search_view`). |
| `add_list` | `items, width=360.0` | `items` is a list of already-built `add_list_item(...)` nodes, laid out as a column. |
| `add_list_item` | `headline, leading_icon=None, trailing_icon=None, supporting_text=None, width=360.0` | One real MD3 list row. |
| `add_chip` | `label, width, variant="assist", icon=None, selected=false, removable=false` | `variant` one of `"assist"`/`"filter"`/`"input"`/`"suggestion"`. |
| `add_badge` | `label=None, width=None` | No `height` — square (a dot) when `label=None`, a pill sized to `width` otherwise; caller positions it over another node's corner via `x`/`y`, same contract as `add_rect`. |
| `add_divider` | `length, vertical=false` | A single hairline. |
| `add_link` | `text, width` | Styled, clickable text — `set_on_click` for navigation. |
| `add_accordion_header` | `title, expanded=false, width=360.0` | Pairs with app-managed content shown/hidden on click. |
| `add_tree_node` | `title, depth=0, expanded=false, leaf=false, width=360.0` | `depth` drives indentation; the app owns real tree structure/recursion. |

## Search

```python
bar, text_field, leading, trailing = window.add_search_bar(
    placeholder="Search…", width=360, leading_icon="search",
)
text_field.set_on_change(lambda: print(text_field.get_text()))
```

- `add_search_bar(placeholder, width, leading_icon=None, trailing_icons=None)`
  returns a **4-tuple** — `(bar, text_field, leading_icon_node,
  trailing_icon_nodes)` — not a single `Node`, unlike every other
  factory on this page, since the caller needs the inner `TextField`
  node directly to wire `set_on_change`/read `get_text()`.
- `add_search_view(width, height)` is the docked suggestions/results
  panel — a plain styled `Rect` with no fixed content anatomy (the app
  populates it via `Node.add_child`, the same `Card` contract), shown/
  hidden via `Window.open_menu`/`close_menu` directly rather than a
  dedicated pair.

## Media & Graphics

```python
picture = window.add_image("logo.png", width=200, height=120, fit="cover")
```

- Loads and decodes a real file from disk (`png`/`jpeg`) at call time
  and uploads it as a GPU texture. `fit` is `"cover"`/`"contain"`/
  `"fill"` (default `"fill"`). Raises `OSError` if the file can't be
  read or decoded. No `background` param — there's no meaningful
  "behind the content" color for a node whose entire content is a
  loaded image.
- `add_image_from_bytes(rgba, pixel_width, pixel_height, width, height, fit="fill")`
  is `add_image`'s decode-free sibling — already-decoded RGBA8 pixels
  instead of a file path, for content decoded elsewhere (a network
  fetch, a different image library). See
  [`Window` API reference](../api/python/window.md#add_image_from_bytes).
- `add_video(width, height, fit="fill")` shares the same `fit` contract;
  frames are pushed at runtime via `node.push_frame(...)`, not loaded
  from a path at construction.

```python
icon = window.add_icon("settings", color=(0x1C, 0x1B, 0x1F, 0xFF), size=24)
```

- One square `size` (Material Symbols icons are uniformly square by
  design), rendered as a vector fill in `color`. Currently curated icon
  names: `home`, `search`, `menu`, `close`, `check`, `arrow_back`,
  `add`, `settings` — a small, deliberately additive starter set, not
  the full Material Symbols library. An unknown name raises
  `ValueError` listing the real known set. No `background` param, same
  reasoning as `Image`.

| Factory | Signature | Notes |
|---|---|---|
| `add_graph_node` | `graph, label, x, y, width, height` | One node inside an `add_node_graph` canvas; `x`/`y` are graph coordinates, not window-absolute. |
| `add_node_graph` | `width, height` | A `Canvas`-backed container for `add_graph_node` children plus app-drawn edges between them. |

## Date & Time Pickers

| Factory | Signature | Notes |
|---|---|---|
| `add_date_picker_day` | `day, selected=false, today=false, outside_month=false` | One real calendar cell; the app arranges a grid of them. |
| `add_time_picker_dial` | `hour=0, minute=0, size=256.0` | The analog clock-face dial. |
| `add_period_selector` | `selected="AM"` | The AM/PM toggle pairing with the dial. |

## Layout & Structure

```python
left = window.add_rect(background=(0xEE, 0xEE, 0xEE, 0xFF), width=200, height=300)
splitter = window.add_splitter(background=(0xCC, 0xCC, 0xCC, 0xFF), width=8, height=300, initial_position=0.5)
right = window.add_rect(background=(0xDD, 0xDD, 0xDD, 0xFF), width=200, height=300)
```

Add a splitter between two panes (in call order — left pane, splitter,
right pane) inside a `Window`'s own root row to get a resizable-pane
layout, with no separate "pane container" concept needed. Drag handling
is entirely built into the engine's own input dispatch.

`add_carousel(layout, width, height, background)` — `layout` is one of
`"uncontained"`, `"hero"`, `"multi_browse"`, MD3's own three real
carousel item-arrangement modes; populate it the same `Node.add_child`
way as `Card`/`List`.

`add_rect`/`add_text`/`add_scroll_view` are the primitive building
blocks every composite factory above is itself built from — see the
[Imperative API guide](imperative-api.md) for those, plus every
component's shared `x`/`y`/`border_color`/`border_width` contract.

## Theming components together

`window.set_theme(seed, dark=False)` builds a full MD3 dynamic color
scheme from one seed color and immediately re-tints every already-
`enable_interaction()`-enabled node's ripple/hover state layer, every
themed component's shape/elevation (corner radius, drop shadow), and
every checkbox mark / slider track / text field's default text color
created afterward. Corner radius and elevation can also be overridden
per-component by name (`"button"`, `"card"`, `"icon_button"`, …) via
`ThemeSpec`'s `components:` section, and text can reference a named MD3
type-scale role instead of literal font values. See
[Theming & Accessibility](theming-and-accessibility.md) for both.
