# Bars and panels

A `LayerWindow` (0.5.8) is how an app makes a bar, a panel, a dock or an OSD: a window the
compositor places at a screen edge, above or below ordinary windows, optionally reserving
space so maximised windows stop short of it. It works only on a Wayland compositor that has
the layer shell (KDE's KWin and the wlroots compositors do; GNOME's Mutter, X11, Windows
and macOS do not), and raises `tre.UnsupportedPlatform` elsewhere. Every setting is in the
[`LayerWindow` reference](../reference/layer-window.md); this page is how they fit together.

## A bar

```python
from tre import App, LayerWindow

bar = LayerWindow(
    "example.bar",                       # what it is for
    layer="top",                         # above ordinary windows
    anchor=("top", "left", "right"),     # attached to the top edge, across the screen
    size=(0, 32),                        # 0 = stretch between the anchored sides
    exclusive_zone=32,                   # reserve 32 px: maximised windows stop below it
)
bar.root.set(fill=(0x1D, 0x1B, 0x20, 0xF0), flex_direction="horizontal",
             align_items="center", padding_left=12, padding_right=12, gap=10)
bar.root.add_child(bar.create("text", text="hello", font_size=14,
                              fill=(255, 255, 255, 255),
                              width="fit-content", height="fit-content"))

app = App()
app.add_window(bar)
app.run()
```

The bar is an ordinary tree: nodes, listeners, timers (`bar.every`), animation, text.
`examples/layer_bar.py` adds a clock.

## Where it goes

| You want | `anchor` | `size` | `exclusive_zone` |
|---|---|---|---|
| A bar across the top | `("top", "left", "right")` | `(0, 32)` | `32` |
| A bar across the bottom | `("bottom", "left", "right")` | `(0, 40)` | `40` |
| A sidebar on the left | `("left", "top", "bottom")` | `(64, 0)` | `64` |
| A dock, centred on the bottom | `("bottom",)` | `(480, 56)` | `56` |
| A clock in a corner, reserving nothing | `("top", "right")` | `(120, 28)` | `0` |
| An OSD in the middle of the screen | `()` | `(300, 80)` | `0` |

A `0` on an axis stretches between the two edges anchored on it, so it needs both. A
positive zone reserves space only for a surface anchored to one edge, or to one edge and
both edges across it; the clock row above shows the other case, where the zone is `0`. A
zone the protocol would ignore raises no error: a `warning` event says so.

`margin=(top, right, bottom, left)` moves it off the edge. `layer="overlay"` puts an OSD
above everything, fullscreen windows included; `"bottom"` and `"background"` are for
widgets and wallpapers under windows.

## Several screens

Leave `output` as `None` and the compositor picks one, and picks differently from run to
run. A shell that wants a bar on each screen reads the list and makes one `LayerWindow` per
[`Output`](../reference/outputs.md), and follows screens being plugged in and out:

```python
bars = {}

def add_bar(out):
    bar = make_bar(output=out)       # LayerWindow(..., output=out)
    bars[out.name] = bar
    app.add_window(bar)

for out in app.outputs():            # the screens as they are now, before run()
    add_bar(out)
app.on("output_added", add_bar)
app.on("output_removed", lambda out: bars.pop(out.name, None))   # its bar closes itself
app.run()
```

A bar bound to a screen that is unplugged is closed by the compositor and fires `closed` with
reason `"output_removed"`. `output=` also takes a connector name (`"DP-1"`); a name that is not
there makes `App.run()` raise `RuntimeError` listing the ones that are. Each bar's
`output_changed` event gives the name the compositor put it on.

## Changing it while it runs

`set()` changes the layer settings in one commit:

```python
bar.set(exclusive_zone=0)                    # stop reserving the space
bar.set(margin=(-32, 0, 0, 0), exclusive_zone=0)   # slide it off the top edge (hidden)
bar.set(margin=0, exclusive_zone=32)         # and back
```

A margin takes effect as a step; animating it (and the surface's own size) is not built
yet. Animating what is *inside* the surface (a node's `opacity`, `translate_y`, a width)
works as in any window and costs no protocol traffic.

## The keyboard

`keyboard="none"` (the default) means no key events: a bar that only shows things should
not take the keyboard from the window the user is typing in. `"on_demand"` lets the user
focus it with a click, and it keeps keys until they click elsewhere; give the node that
should receive them `focusable=True`. `"exclusive"` takes the keyboard when the surface
opens and keeps it until it closes: for a lock screen or a launcher, not a bar.

## In tests

A `LayerWindow` is a `Window`, so `simulate`, `advance` and `snapshot` work with no
compositor and no display, and the headless size of a stretched axis is `1920` x `1080`
unless `resize` says otherwise. The protocol's validation errors are raised in Python, so
they are testable the same way. Where there is a display but no layer shell the
constructor raises `UnsupportedPlatform`; `tre.supports("layer_shell")` tells a test
whether to skip.

## What is not there yet

A menu or flyout that leaves the bar's own surface is a `PopupWindow`, a compositor popup
parented to the bar: see [Menus from a bar](menus-from-a-bar.md). A screen frame (a painter surface plus reservers)
uses `input_region`, which is built; margin animation and drag are built too (see the
[LayerWindow reference](../reference/layer-window.md)).
