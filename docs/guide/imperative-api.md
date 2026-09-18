# Imperative API

The imperative path builds a UI directly from Python method calls —
`Window` creates nodes, and each returned [`Node`](../api/python/node.md)
is a handle for events, animation, and property reads/writes. This is the
lower-level of `tre`'s two authoring paths; see
[Declarative Views](declarative-views.md) for the YAML alternative.

## Windows and the app loop

```python
from tre import App, Window

win1 = Window(width=400, height=200, title="Main")
win2 = Window(width=300, height=150, title="Panel")

app = App()
app.add_window(win1)
app.add_window(win2)
app.run()
```

`App` collects one or more `Window`s and drives them all together in one
blocking `run()` call; each `Window` owns its own node tree and size.
`run()` returns once every window has closed. Pass `max_frames=N` to
`run()` to cap each window at `N` frames — useful for headless/CI runs
that need a real exit condition with no interactive close.

Every `Window` starts with one implicit root node: a flex row with
16px padding and 16px gaps between children. Every `add_*` method below
attaches its new node as a direct child of that root, in call order.

## Creating nodes

| Method | Creates |
| --- | --- |
| `add_rect(background, width, height, x=None, y=None)` | A plain colored rectangle |
| `add_text(content, background, width, height, font_family="Roboto", font_weight=400.0, font_size=16.0, x=None, y=None)` | A plain, non-editable text label |
| `add_checkbox(background, width, height, checked=False, x=None, y=None)` | An MD3 checkbox |
| `add_slider(background, width, height, value=0.0, x=None, y=None)` | An MD3 slider (drag-to-set built in) |
| `add_text_field(background, width, height, content="", font_family="Roboto", font_weight=400.0, font_size=16.0, x=None, y=None)` | An MD3 text field |
| `add_image(path, width, height, fit="fill", x=None, y=None)` | A GPU-texture-backed image loaded from disk |
| `add_icon(name, color, size, x=None, y=None)` | A curated Material Symbols vector icon |
| `add_splitter(background, width, height, initial_position=0.5)` | A drag-resizable pane divider |
| `add_canvas(width, height, draw, x=None, y=None)` | A custom-drawn surface — see [Canvas & Virtualized Lists](canvas-and-lists.md) |
| `add_virtual_list(item_count, materialize, item_extent=None, size_hint=None, width=None, height=None)` | A virtualized list — see [Canvas & Virtualized Lists](canvas-and-lists.md) |

`x`/`y` are independently optional: give either to absolutely-position
the node (relative to the window's own root padding box), or omit both to
use the default flex-row flow. See [MD3 Components](components.md) for a
deeper look at `Checkbox`/`Slider`/`TextField`/`Image`/`Icon`, and
[Docking & Shell Layout](docking-and-shell.md) for `build_shell` and the
docking methods.

## Events

Every `Node` supports these handler registrations:

```python
node.set_on_click(lambda: ...)
node.set_on_hover_enter(lambda: ...)
node.set_on_hover_exit(lambda: ...)
node.set_on_change(lambda: ...)   # a Slider drag ending, or set_checked/set_text
```

Handlers are called with no arguments. An exception raised inside a
handler is caught, logged, and non-fatal — it never crashes the app.

`set_on_click` also makes the node keyboard-Tab-reachable (it adds a
`Focus`/`Click` accessibility action), so a node only becomes part of the
Tab order once it's actually given behavior — see
[Theming & Accessibility](theming-and-accessibility.md).

Call `node.enable_interaction()` to opt a node into the default MD3
ripple/hover state-layer animation — this is independent of whether the
node has any handlers at all; a purely-hoverable, non-clickable node is a
real, supported case.

### Driving events without a live window

`Window` exposes direct, synthetic dispatch methods, useful for tests and
headless scripts — no live rendered window is needed:

```python
window.click(node)
window.hover(node)
window.scroll(node, delta_y=10.0)
window.right_click(node)          # opens a registered context menu, if any
window.press_key("tab")           # "tab"/"enter"/"space"/"escape"/"backspace"/
                                   # "delete"/"left"/"right"/"home"/"end"
window.type_text("hello")         # only affects the currently focused TextField
window.copy()                     # returns the focused TextField's selected text
window.cut()
window.paste("clipboard text")
```

`click`/`hover`/`scroll`/`right_click` each compute layout first, then
dispatch at the node's real, current center point — exactly what a real
mouse interaction there would produce.

## Animation

```python
node.animate(property, to, duration_ms=0, on_complete=None)
```

Starts (or retargets) an animation on one property. Returns immediately —
it never blocks waiting for the animation to finish. `on_complete`, when
given, is called with no arguments exactly once, the real frame the
animation genuinely finishes.

Universal properties (every node kind):

| Property | `to` type | Meaning |
| --- | --- | --- |
| `"opacity"` | `float` | 0.0–1.0 |
| `"corner_radius"` | `float` | pixels |
| `"elevation"` | `float` | shadow elevation |
| `"background"` | `(r, g, b, a)` tuple of ints 0–255 | fill color |
| `"transform"` | `(translate_x, translate_y, scale)` tuple of floats | pan + zoom |
| `"shape"` | `list[(x, y)]` float tuples | morphs to the closed polygon these vertices describe |

Component-specific properties:

| Property | Node kind | Meaning |
| --- | --- | --- |
| `"check_progress"` | `Checkbox` | the checkmark's own draw progress, 0.0–1.0 |
| `"thumb_position"` | `Slider` | 0.0–1.0 along the track |

Read a property's current (possibly still-animating) value back with
`node.get(property)`.

```python
def on_faded():
    print("done fading")

rect.animate("opacity", 0.0, duration_ms=300, on_complete=on_faded)
```

!!! note
    `on_complete` callbacks are drained and invoked by `App.run()`'s own
    per-frame loop. A node created via a `Window` sees its callbacks fire
    for real; a node created via a [`View`](declarative-views.md) does
    not, since `View` has no render loop of its own to drain them
    through.

## Tree structure

```python
child_node.add_child(other_node)  # attach other_node under child_node
node.remove()                     # remove this node and its whole subtree
```

`add_child` rejects (with `ValueError`) attaching a node as a child of its
own descendant, and rejects attaching a node that belongs to a different
`Window`'s tree.

## Context menus

```python
menu_content = window.add_rect(background=(0xFF, 0xFF, 0xFF, 0xFF), width=160, height=120)
anchor_node.set_context_menu(menu_content)
```

Registers `menu_content` as `anchor_node`'s right-click context menu,
opened by `window.right_click(anchor_node)` or a real right-click. The
content node must be unattached or will be detached from its current
parent first, and must belong to the same `Window`.
