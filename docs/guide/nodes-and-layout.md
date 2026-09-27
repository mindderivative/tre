# Nodes and Layout

Everything on screen is a node in a window's tree. You create nodes, attach
them under one another, and set their layout properties; `tre` lays the tree
out with flexbox ([taffy](https://github.com/DioxusLabs/taffy)) whenever it
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

The mouse wheel scrolls it, and so does setting `scroll_offset`, which also
animates — how a carousel eases to a snap point:

```python
view.animate("scroll_offset", 400, 300, easing=(0.2, 0.0, 0.0, 1.0))
```

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
