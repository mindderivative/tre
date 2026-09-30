# Custom Title Bars

A window can drop the operating system's title bar and borders, so your
framework draws its own: an icon, a title, minimize, maximize, and close
buttons, and anything else an app puts up there — tabs, a search field, a
menu. Tesserae Engine does what only the platform can do — move, resize,
maximize, report the window's state — and leaves the look to you (0.5.0).

The whole of this page runs in
[`examples/custom_titlebar.py`](https://github.com/mindderivative/tre/blob/main/examples/custom_titlebar.py):
`python examples/custom_titlebar.py --watch` keeps the window open to try.

## An undecorated window

```python
window = Window(width=640, height=400, title="Notes", decorations=False)
window.set(min_width=360, min_height=220)  # so the bar's buttons never crush
window.root.set(flex_direction="vertical", padding=0, gap=0)
```

`decorations=False` — or `window.set(decorations=False)` on an open window —
removes the title bar and borders. The title still names the window in the
taskbar and window switcher. On Windows the window keeps its shadow. On
macOS the title bar stays as a transparent overlay instead; see
[macOS](#macos) below.

## The bar is a drag region

```python
bar = window.create("box", width="100%", height=40, flex_shrink=0,
                    align_items="center", gap=10, fill=BAR,
                    window_region="drag")
title = window.create("text", text="Notes", font_size=13, height=18, flex_grow=1)
bar.add_child(title)
window.root.add_child(bar)
```

A primary press on a `window_region="drag"` node moves the window — and so
does a press on anything inside it that isn't interactive, like the title.
The press still reaches your `pointer_down` listeners first; then the OS
takes the pointer, and the pressed node gets **`pointer_cancel`** instead of
`pointer_up` or `click`. Use it to clear a pressed look, as you would for a
drag that ends somewhere you can't see.

A **double-click** on the bar maximizes or restores the window, as a native
title bar's does, timed by the system's double-click setting; on macOS it
does what the user chose in System Settings.

## Buttons stay buttons

A node inside the bar is interactive — pressed, not dragged — when it's
`focusable`, has a `click` listener, holds the pointer capture, or is a text
input. So an ordinary button just works:

```python
def button(label, icon, action):
    node = window.create("box", width=46, height=40, align_items="center",
                         justify_content="center", role="button", label=label,
                         focusable=True)
    node.add_child(window.create("path", data=icon, view_box=(0, 0, 10, 10),
                                 width=10, height=10, stroke_color=TEXT,
                                 stroke_width=1, hit_testable=False))
    node.on("click", action)
    bar.add_child(node)
    return node

button("Minimize", "M1 5 H9", window.minimize)
maximize = button("Maximize", "M1.5 1.5 H8.5 V8.5 H1.5 Z",
                  lambda: window.restore() if window.get("maximized") else window.maximize())
button("Close", "M1.5 1.5 L8.5 8.5 M8.5 1.5 L1.5 8.5", window.close)
```

Two overrides cover the rest: `window_region="none"` keeps a node that
isn't interactive from dragging (a tab strip's empty space, say), and
`window_region="drag"` on a node inside a button makes that part drag
again.

`window.close()` closes the window the way the user's close would:
`close_requested` fires first, so an app's "save changes?" check still
runs, and a listener can cancel it.

## Following the window's state

```python
window.on("maximized", lambda e: maximize_icon.set(data=RESTORE if e.maximized else MAXIMIZE))
window.on("active", lambda e: title.animate("fill", TEXT if e.active else DIMMED, 150))
```

`maximized` fires when the window is maximized or restored — by your button,
a double-click, or the OS's own snapping — and `active` when it gains or
loses focus; each carries the new value and fires only on a change.
`window.get("maximized")`, `"minimized"`, `"active"`, and `"fullscreen"` read
the current state.

## Resizing

```python
window.set(resize_border=6)
```

A press within 6 pixels of an edge resizes the window from that edge, or
from a corner where two meet, and the pointer shows the matching resize
cursor over it. Those presses never reach your nodes. The border is off
while the window is maximized or fullscreen, and on macOS, where the OS
resizes the window itself.

## The window menu

```python
window.set(system_menu=True)
```

Off by default, so a right-click on your bar is yours — for your own
context menu. Turned on, a secondary press on the drag region opens the
OS's window menu (Restore, Move, Size, Minimize, Maximize, Close) where the
platform has one: Windows, and Wayland compositors that offer it. On
Windows, Alt+Space opens it too.

## macOS

`winit` can't let the user resize an undecorated macOS window, so on macOS
`decorations=False` keeps the title bar as a transparent overlay: your
content runs up under it, and the traffic lights stay in the top-left
corner, over your bar. Leave room for them, and hide your own buttons:

```python
def fit(inset=None):
    _, width = inset or window.get("titlebar_inset")
    bar.set(padding_left=max(12.0, width))
    for node in own_buttons:
        node.set(visible=not window.get("native_controls"))

window.on("titlebar_inset", lambda e: fit(e.titlebar_inset))
fit()
```

`titlebar_inset` is `(height, width)`: the title-bar strip and the traffic
lights with their margins. It's `(0, 0)` in fullscreen, where the traffic
lights hide, for a decorated window, and on every other platform, so the
same code runs everywhere. `native_controls` says whether the OS's controls
are showing.

## What each platform does

| | Windows | Linux (Wayland, X11) | macOS |
| --- | --- | --- | --- |
| `decorations=False` | No title bar or border; the shadow stays | No title bar or border | A transparent title bar over the content; the traffic lights stay |
| Drag region | Moves the window | Moves the window | Moves the window |
| `resize_border` | Resizes | Resizes | Off: the OS resizes |
| Double-click | Maximize, at the system's double-click time | Maximize, within 500 ms | The user's setting: zoom, minimize, or nothing |
| `system_menu=True` | The system menu, and Alt+Space | The compositor's window menu, where it has one | Nothing |
| `window.get("platform")` | `"windows"` | `"wayland"` or `"x11"` | `"macos"` |

## Testing without a display

Everything but the OS's own move, resize, and menu runs headless:

```python
window.advance(0)  # pin the clock, for double-click timing
window.simulate("pointer_down", node=title)
window.simulate("pointer_up", node=title)             # the bar dragged: pointer_cancel, no click
window.simulate("click", node=close_button)           # buttons click
window.simulate("maximized", maximized=True)          # as the OS would report it
window.simulate("titlebar_inset", height=28, width=78)  # as macOS would
```

See [Window controls and state](../api/python/window.md#window-controls-and-state),
[macOS: the overlay title bar](../api/python/window.md#macos-the-overlay-title-bar),
and [Title bar and borders](../api/python/window.md#title-bar-and-borders) for
the reference, and the [design](../design/custom-windowing.md) for why each
behaves as it does.
