# Nodes and Properties

*New in 0.3.4.* Every node the [target API](../../design/target-api.md) builds
comes from `window.create(kind, **props)`, and every property is read and
written through [`node.set`, `node.get`, and `node.animate`](node.md#set-get-and-focus).
This page lists them all. Paint, paths, shadows, and easing have their own page,
[Paint, Paths, and Animation](paint.md); events are in
[Events and Listeners](events.md).

## Kinds

| Kind | Required | What it is |
| --- | --- | --- |
| `"box"` | — | A layout container with an optional fill, stroke, corners, and shadows |
| `"text"` | `text` | Shaped, non-editable text |
| `"text_input"` | — | Editable text: selection, clipboard, IME, syntax spans, folding, obscuring |
| `"image"` | `rgba`, `pixel_width`, `pixel_height` | Decoded RGBA8 pixels; video is repeated `set(rgba=..., ...)` |
| `"path"` | `data` | A vector path — see [Paths](paint.md#paths) |
| `"canvas"` | `draw` | Immediate-mode drawing through a [painter](canvas-context.md) |
| `"scroll_view"` | — | Clips and scrolls one child |
| `"virtual_list"` | `item_count`, `materialize`, and `item_extent` or `size_hint` | Builds only the rows its viewport shows |
| `"terminal"` | `shell`, `cols`, `rows` | A PTY-backed terminal emulator |

```python
title = window.create("text", text="Inbox", font_size=22, font_weight=500)
field = window.create("text_input", placeholder="Search", width=240)
```

`create` makes a **detached** node: attach it with `add_child` or
`insert_child`. Until it's attached, it's freed once no handle points into it
(see [Lifetime](node.md#lifetime)). A text's fill defaults to opaque black, and
so does a text input's; every other color defaults to transparent.

## Size and position

Every node.

| Property | Value |
| --- | --- |
| `width`, `height`, `min_width`, `min_height`, `max_width`, `max_height` | A number of pixels, `"auto"`, or a percentage such as `"50%"` |
| `aspect_ratio` | Width over height, or `None` |
| `position` | `"relative"` (in the flow) or `"absolute"` |
| `x`, `y` | The left and top offset: a number, a percentage, or `"auto"`/`None` |

## Flex layout

On a node with children:

| Property | Value |
| --- | --- |
| `flex_direction` | `"horizontal"` or `"vertical"` |
| `flex_wrap` | `"no_wrap"` or `"wrap"` |
| `align_items` | `"start"`, `"end"`, `"flex_start"`, `"flex_end"`, `"center"`, `"baseline"`, `"stretch"` |
| `justify_content` | `align_items`' values, plus `"space_between"`, `"space_around"`, `"space_evenly"` |
| `gap` | Between children: a number or a percentage |
| `padding`, `padding_top`, `padding_right`, `padding_bottom`, `padding_left` | A number or a percentage |

As a flex child, any node:

| Property | Value |
| --- | --- |
| `flex_grow`, `flex_shrink` | Non-negative numbers |
| `flex_basis` | Like `width` |
| `align_self` | One of `align_items`' values, or `None` to follow the parent |
| `margin`, `margin_top`, `margin_right`, `margin_bottom`, `margin_left` | A number, a percentage, or `"auto"` |

`padding` and `margin` read back as one value while all four sides agree, and as
`(top, right, bottom, left)` once they don't.

## Transform, visibility, and order

Every node.

| Property | Value |
| --- | --- |
| `translate_x`, `translate_y` | Pixels (animatable) |
| `scale` | A factor about the node's center (animatable) |
| `rotation_deg` | Degrees clockwise about the node's center (animatable) |
| `visible` | `False` hides the node and its subtree: not painted, not hit, no layout space, not in the accessibility tree or tab order |
| `z_index` | Paint and hit order among siblings: higher is on top; equal values keep child order |
| `clip_children` | `True` clips children to the node's rounded box |

Each transform part animates on its own, so easing one never disturbs another.

## Text

On `text` and `text_input` unless noted.

| Property | Value |
| --- | --- |
| `text` | The content. Setting it on a text input puts the caret at the end, and fires no `change` event: that's for edits the user makes |
| `font_family` | A family name — see [`register_font`](index.md) |
| `font_weight` | 1–1000 |
| `font_size` | Pixels (also on a terminal) |
| `line_height` | A multiple of the font size, or `None` (`text` only) |
| `text_align` | `"start"`, `"center"`, or `"end"` (`text` only) |
| `font_style` | `"normal"` or `"italic"` — slanted when the family has no italic face (`text` only) |
| `letter_spacing` | Extra pixels after each character (`text` only) |
| `wrap` | `"word"` wraps within the node's width; `"none"` keeps each paragraph on one line (`text` only) |
| `max_lines` | The most lines shown, or `None` (`text` only) |
| `overflow` | `"clip"` cuts text past the box or the line limit; `"ellipsis"` ends each cut line with "…" (`text` only) |
| `fill` | The glyph color (animatable) |

A text node has no size of its own: size it with
[`window.measure_text`](window.md#measure_text), which lays text out exactly as
a text node with the same properties paints it:

```python
style = dict(font_size=14, max_lines=1, overflow="ellipsis")
width, height = window.measure_text("A long list item title", max_width=180, **style)
title = window.create("text", text="A long list item title", width=180, height=height, **style)
```

## Text input

| Property | Value |
| --- | --- |
| `multiline` | `True` for several lines |
| `selection` | `(start, end)` byte offsets; `start == end` is a caret. Given with `text`, it selects in the new text |
| `show_whitespace` | Draws spaces and tabs as visible marks |
| `syntax_spans` | A list of `(start, end, color)` byte ranges |
| `folded_ranges` | A list of `(start, end)` byte ranges drawn as an ellipsis |

Plus the colors on [Paint, Paths, and Animation](paint.md#text-inputs-scroll-views-and-terminals).

## Image

| Property | Value |
| --- | --- |
| `rgba`, `pixel_width`, `pixel_height` | Straight-alpha RGBA8 pixels and their size — always set together |
| `fit` | `"fill"`, `"contain"`, or `"cover"` |

## Canvas

| Property | Value |
| --- | --- |
| `draw` | `draw(painter)`, called when the canvas is created, when `draw` is set, and on `node.redraw()` |

## Scroll view

| Property | Value |
| --- | --- |
| `orientation` | `"vertical"` or `"horizontal"` |
| `scroll_offset` | Pixels scrolled (animatable, so a carousel can ease to a snap point) |

## Virtual list

```python
def row(index):
    return window.create("text", text=items[index])

rows = window.create("virtual_list", item_count=len(items), item_extent=32,
                     materialize=row, width=320, height=400)
```

| Property | Value |
| --- | --- |
| `item_count` | How many rows |
| `item_extent` | Every row's height, or `None` once `size_hint` is set |
| `size_hint` | `size_hint(index)` → that row's height, for rows of different heights |
| `materialize` | `materialize(index)` → a node this window made, for that row |

`tre` keeps exactly the rows the viewport shows built: whenever layout runs —
each frame, `window.advance`, `window.simulate`, and any `layout_*` read — it
calls `materialize` for each newly visible row, gives the row the list's width
and its own height, and detaches rows scrolled away. A detached row is freed
unless you keep a handle to it to reuse. Changing `item_count`, `item_extent`,
`size_hint`, or `materialize` rebuilds every row.

## Terminal

| Property | Value |
| --- | --- |
| `cols`, `rows` | The grid size, 1–1000 each; the terminal's box follows its grid and font |
| `selection` | `(start_row, start_col, end_row, end_col)`, or `None` |
| `shell`, `scrollback_lines` | Given to `create` only |

Plus `font_size` and the `palette` on
[Paint, Paths, and Animation](paint.md#text-inputs-scroll-views-and-terminals).

## Read-only

| Property | Value |
| --- | --- |
| `focused` | Whether the node has keyboard focus |
| `layout_x`, `layout_y` | The node's computed position in the window |
| `layout_width`, `layout_height` | Its computed size |

Reading a `layout_*` value runs any pending layout first, so it always matches
the tree as it is now. A detached subtree is laid out on its own, at its content
size — so you can measure one before attaching it.
