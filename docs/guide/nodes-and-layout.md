# Nodes and Layout

Everything on screen is a node in a window's tree. You create nodes, attach
them under one another, and set their layout properties; Tesserae Engine lays the tree
out with flexbox or CSS Grid ([taffy](https://github.com/DioxusLabs/taffy)) whenever it
changes, and paints it every frame.

## Windows and the app

```python
from tre import App, Window

window = Window(width=640, height=400, title="Inbox")
# ... build the tree under window.root ...

app = App()
app.add_window(window)
app.run()
```

A `Window` owns one tree. Its `root` is a flex row with 16px of padding and
16px gaps, sized to the window; restyle it like any node, with
`window.root.set(...)`. `App.run()` opens every added window and blocks until
all of them close. `run(max_frames=N)` stops each window after `N` frames,
which is how the examples exit on their own.

A window's size follows the OS window: listen for the `resize` window event
to react when the user resizes it. `window.resize(width, height)` changes it
from code.

## Creating and attaching nodes

`window.create(kind, **props)` makes a node and sets its properties in one
atomic step: a bad property raises `ValueError` and nothing is created.

| Kind | What it is |
| --- | --- |
| `"box"` | A layout container that can paint a fill, a stroke, rounded corners, and shadows |
| `"text"` | Shaped text — see [Text](text.md) |
| `"text_input"` | Editable text — see [Text](text.md#text-input) |
| `"image"` | Decoded RGBA8 pixels — see [Painting](painting.md#images) |
| `"path"` | A vector path — see [Painting](painting.md#paths) |
| `"canvas"` | Immediate-mode drawing — see [Painting](painting.md#canvas) |
| `"scroll_view"` | A clipped, scrollable viewport — see [below](#scrolling) |
| `"virtual_list"` | A list that builds only its visible rows — see [below](#virtual-lists) |
| `"terminal"` | A terminal emulator running a shell — see [Text](text.md#terminal) |

A new node is **detached**. Attach it with `add_child`, which appends, or
`insert_child(index, child)`:

```python
panel = window.create("box", flex_direction="vertical", gap=8, padding=12, width=240)
title = window.create("text", text="Inbox", font_size=22, width=200, height=28)
search = window.create("text_input", placeholder="Search", width=216, height=32)
panel.add_child(title)
panel.add_child(search)
window.root.add_child(panel)
```

Every property is on [Nodes and Properties](../api/python/properties.md).

## Flex layout

A node lays out its children along `flex_direction` — `"horizontal"` (the
default) or `"vertical"` — separated by `gap` and inset by `padding`.
`align_items` places them across that axis and `justify_content` along it:

```python
toolbar = window.create("box", width="100%", height=48, padding=8, gap=8,
                        align_items="center", justify_content="space_between")
```

Sizes are pixels, `"auto"`, or a percentage of the parent: `width="50%"`.
As a flex child, a node can grow into spare room with `flex_grow`, shrink
with `flex_shrink`, and start from `flex_basis`; `align_self` overrides its
parent's `align_items` for it alone. A sidebar that keeps its width while the
content takes the rest:

```python
sidebar = window.create("box", width=200, flex_shrink=0)
content = window.create("box", flex_grow=1)
```

`padding` and `margin` take one value for every side, or set sides one at a
time with `padding_top`, `margin_left`, and so on. A `margin` of `"auto"`
takes up the free space on that side, which pushes a node to the far end of
its row.

`min_width`, `max_width`, `min_height`, and `max_height` bound a size, and
`aspect_ratio` fixes width over height. `flex_wrap="wrap"` lets children
flow onto more lines, and `align_content` spaces those lines. The
[property reference](../api/python/properties.md) lists every layout
property and its values.

## Grid layout

`display="grid"` lays a node's children out in rows and columns instead of
one line, so their edges line up across rows — a settings form of labels and
fields, a gallery of equal cells, a dashboard of tiles. The columns and rows
are track lists: pixels, a percentage, `"auto"`, a share of the free space
(`"1fr"`), `"minmax(min, max)"`, and `"repeat(n, tracks)"`, as a string or a list;
a bare number is one track of that many pixels (`grid_auto_rows=96`):

```python
form = window.create("box", display="grid", width="100%",
                     grid_template_columns="auto 1fr", row_gap=8, column_gap=12)
for label, field in rows:
    form.add_child(label)   # column 1
    form.add_child(field)   # column 2, as wide as the rest of the form
```

Children fill the cells in order, a row at a time (`grid_auto_flow="column"`
fills columns instead). A child can be placed with `grid_column` and
`grid_row`: a start line (`2`; negative lines count from the end, so `-2`
is the last column's), a span (`"span 2"`), or
both ends (`"1 / 3"`). A gallery whose cells wrap to fit the width:

```python
gallery = window.create("box", display="grid", width="100%", gap=8,
                        grid_template_columns="repeat(auto_fill, minmax(96, 1fr))")
```

`row_gap` and `column_gap` space the rows and columns apart (`gap` sets both),
and `justify_items`/`justify_self` place a child across its cell's width the
way `align_items`/`align_self` do across its height. The full list is in
[Node properties](../api/python/properties.md#grid-layout).

## Absolute positioning

`position="absolute"` takes a node out of the flow and places it at `x`/`y`
from its parent's padding box — for a badge over a corner, a drag preview, or
a scrim that fills its parent:

```python
badge = window.create("box", position="absolute", x=28, y=-4, width=16, height=16,
                      corner_radius=8, fill=(0xB3, 0x26, 0x1E, 0xFF))
icon_button.add_child(badge)
```

Content that floats over the whole window — menus, dialogs, tooltips — goes
in a [layer](layers.md) instead, which also handles placement, input, and
focus.

## Reading the layout

`layout_x`, `layout_y`, `layout_width`, and `layout_height` read a node's
computed box in window coordinates. Reading one runs any pending layout first,
so it always matches the tree as it is now:

```python
window.root.add_child(panel)
print(panel.get("layout_x"), panel.get("layout_width"))
```

A detached subtree is laid out on its own at its content size, so you can
measure a node before attaching it. `layout_x` and `layout_y` include the
node's transforms (`translate_x`, `scale`, ...) and its ancestors', so they
say where it's drawn; `layout_width` and `layout_height` are its size before
scaling. A transform never moves any other node.

## Changing the tree

| Method | Does |
| --- | --- |
| `add_child(child)` / `insert_child(index, child)` | Attach `child`, moving it if it's attached elsewhere |
| `children()` / `parent()` | Walk the tree |
| `remove()` | Detach the node; it stays alive while you hold a handle to it |
| `destroy()` | Free the node and its subtree now |

Moving a node keeps everything about it — its handle, listeners, focus, and
any running animation — so a list reorders by moving rows rather than
rebuilding them:

```python
for index, key in enumerate(new_order):
    list_box.insert_child(index, rows[key])
```

Switching screens is `old.remove()` then `window.root.add_child(new)`. The
removed screen keeps its scroll offsets, text, and selection, so attaching it
again brings it back as it was. A detached node you no longer hold is freed
automatically; `destroy()` frees one now. See
[Lifetime](../api/python/node.md#lifetime).

## Visibility and stacking

`visible=False` hides a node and its subtree: it isn't painted, takes no
layout space, can't be hit or focused, and leaves the accessibility tree.
Siblings paint in order, later on top; `z_index` changes that without
reordering. `clip_children=True` clips a node's children to its rounded box.

## Scrolling

A `"scroll_view"` clips its content and scrolls it along its `orientation`,
`"vertical"` or `"horizontal"`. Give it one content child that holds the rest:

```python
view = window.create("scroll_view", width=300, height=200)
content = window.create("box", flex_direction="vertical", width=300, gap=4)
for name in names:
    content.add_child(window.create("text", text=name, width=280, height=20))
view.add_child(content)
```

The mouse wheel scrolls it — the nearest scroll view under the pointer that
scrolls along the wheel's direction, so a plain wheel over a horizontal
carousel scrolls the page around it, and Shift+wheel scrolls a horizontal
view (0.4.3). So does setting `scroll_offset`, which also
animates — how a carousel eases to a snap point:

```python
view.animate("scroll_offset", 400, 300, easing=(0.2, 0.0, 0.0, 1.0))
```

An offset past the end is the end: `set` and `animate` clamp it to how far
the content reaches past the view (0.4.3), so it reads back right at once
and an animation eases all the way there.

### Keyboard scrolling

Keys scroll the nearest scroll view around the focused node (0.4.2): the arrow
keys by 40 pixels, Page Up and Page Down by a viewport, Home and End to either
end. Up, Down, Page Up, and Page Down move a vertical view, Left and Right a
horizontal one; a view that doesn't scroll along a key's axis passes it to the
next one out. Keys the focused node uses itself don't scroll: a node with a
`key_down` listener between the focused node and the view keeps every key, and
a text input keeps all but Page Up and Page Down. With nothing focused, keys
scroll nothing. A key pressed with Ctrl, Alt, or Meta held is a shortcut, not a
scroll (0.4.3) — Ctrl+Page Down or Alt+Left stays yours — while Shift still
scrolls. Keyboard scrolls jump rather than ease.

Keys, `scroll_into_view`, and the `scroll` event below apply to
`"scroll_view"` nodes; a `"virtual_list"` scrolls by wheel only.

### Revealing a node

`node.scroll_into_view()` scrolls every scroll view around `node`, innermost
first, by the least that shows it; a node longer than a view is aligned to its
start. Focus does the same, so Tab never lands on a node out of sight, and so
does an assistive technology's `scroll_into_view` request.

```python
rows[40].scroll_into_view()
```

### The `scroll` event

A scroll view fires `scroll` whenever its offset changes, whatever changed it:
the wheel, a key, `scroll_into_view`, focus, `set`, or an animation, once a
frame. `event.old_value` and `event.new_value` are the offsets before and
after. It doesn't bubble.

```python
view.on("scroll", lambda e: status.set(text=f"{e.new_value:.0f} px down"))
```

See `examples/scroll_keys.py`.

## Virtual lists

A `"virtual_list"` has `item_count` rows but builds only the ones its viewport
shows. `materialize(index)` returns a node for a row as it scrolls into view;
rows scrolled away are released.

```python
def row(index):
    return window.create("text", text=items[index], height=32)

rows = window.create("virtual_list", item_count=len(items), item_extent=32,
                     materialize=row, width=320, height=400)
```

Every row is `item_extent` tall, or give `size_hint(index)` for rows of
different heights. A `materialize` or `size_hint` that raises is logged, and
its row stays empty. Changing `item_count` (or either callback) rebuilds the
visible rows. See `examples/scrollable_list.py` and
`examples/variable_height_list.py`.
