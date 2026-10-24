# `LayerWindow`

A [`Window`](window.md) whose Wayland role is a **layer surface** (`zwlr_layer_shell_v1`):
a bar, a panel, a dock, an OSD or a wallpaper that the compositor places, not the user.
It anchors to screen edges, sits on a stacking layer, can reserve screen space so
maximised windows stop short of it, takes the keyboard in the way you choose, and can be
bound to one output. A `LayerWindow` is a `Window` in every way that makes sense (`root`,
`create`, `show_layer`, `after`, `every`, `on`, `simulate`, `advance`, `snapshot`,
`close`), and `App.run()` opens it like one. How to build one is in
[Bars and panels](../guide/bars-and-panels.md).

> **Two senses of "layer".** tre's [layers](layers.md) (`show_layer`) are nodes that
> float over a window's own content, drawn inside it. A layer *surface* is a separate
> window, with its own stacking position among everything on the desktop. A
> `LayerWindow`'s `layer` setting is the second kind; it keeps the protocol's word. A
> `LayerWindow` can use `show_layer` too, but its layers are clipped to the bar's own
> small surface.

```python
from tre import App, LayerWindow

bar = LayerWindow("tesserae-shell.bar", layer="top", anchor=("top", "left", "right"),
                  size=(0, 32), exclusive_zone=32)
app = App()
app.add_window(bar)
app.run()
```

## The settings

**`LayerWindow(namespace, *, layer="top", anchor=None, size=(0, 0), exclusive_zone=0,
margin=0, keyboard="none", output=None)`**

| Setting | Meaning |
|---|---|
| `namespace` | What the surface is for (`"tesserae-shell.bar"`); the protocol's namespace. Required and not empty. Constructor only. |
| `layer` | Where it stacks: `"background"`, `"bottom"`, `"top"` or `"overlay"`. Windows sit between `"bottom"` and `"top"`; a fullscreen window may cover `"top"`. |
| `anchor` | The edges it is attached to: a sequence of `"top"`, `"bottom"`, `"left"`, `"right"` (or one name). Nothing anchored floats, centred by the compositor. |
| `size` | `(width, height)` in logical pixels. A `0` stretches between the two edges anchored on that axis, so a `0` width needs `"left"` and `"right"` in `anchor`, and a `0` height needs `"top"` and `"bottom"`. |
| `exclusive_zone` | Pixels to reserve along the anchored edge, so maximised windows stop short of it. `0` reserves none; `-1` ignores other surfaces' zones and takes the whole output. A positive zone counts only for a surface anchored to one edge, or to one edge and both edges across it (a bar anchored `"top"`, `"left"`, `"right"`); with any other `anchor` the compositor treats it as `0`, and a `warning` event says so. |
| `margin` | `(top, right, bottom, left)` in logical pixels, or one number for all four. Negative is accepted and moves the surface past the screen edge. |
| `keyboard` | `"none"`: never gets key events. `"on_demand"`: gets them when the user focuses it (a click). `"exclusive"`: takes the keyboard on opening, and nothing else gets keys until it closes. |
| `output` | An [`Output`](outputs.md) (from `App.outputs()`) or a connector name (`"DP-1"`, `"HDMI-A-1"`) to bind it to. `None` picks the output at position (0, 0), or the first by name if none is there, the same on every run (the compositor's own choice alternates). A surface meant for another screen names it. Constructor only. |

`exclusive_edge` (the edge a corner-anchored surface's zone is for) needs layer-shell
protocol version 5, which this build does not bind yet; passing it raises `ValueError`.

**`set(**props)`** changes `layer`, `anchor`, `size`, `exclusive_zone`, `margin` and
`keyboard` live, in one commit, and the keys `Window.set` has that mean something to a
placed surface (`touch_slop`, `partial_redraw`, `present_mode`, ...). A bad value raises
`ValueError` and changes nothing, the layer keys included. The window keys that mean
nothing to a surface the compositor places raise too, rather than being ignored:
`title`, `decorations`, `maximized`, `minimized`, `fullscreen`, `resize_border`,
`system_menu`, `min_width`, `min_height`, `icon`, `x`, `y`, `always_on_top`,
`resizable`, `skip_taskbar` and `blur_behind`. `namespace` and `output` can only be given
to the constructor.

**`get(name)`** reads those settings (`anchor` as a tuple of names, `margin` as a
4-tuple), and anything `Window.get` reads.

**`close()`** closes the surface on the loop's next turn: `close_requested` first (a
listener may keep it open with `event.cancel()`), then `closed` with reason `"app"`.

## Errors

The protocol's mistakes are `ValueError`s that name the key, raised before anything is
sent to the compositor (a bad request would end the connection): a `0` size with the two
anchors it needs missing, an unknown `layer`, `keyboard` or edge name, a `margin` that is
not four numbers, an `exclusive_zone` below `-1`. A node's `window_region="drag"` (a title bar that moves its window) raises on a `LayerWindow`'s nodes: the compositor places the surface, so there is no window to drag.

`LayerWindow(...)` raises **`UnsupportedPlatform`** (a `RuntimeError`) where there is no
layer shell: an X11 session, GNOME's Mutter, Windows and macOS. It does not quietly turn
into a plain window: a bar without its exclusive zone is worse than an error. An app that
wants a fallback catches it and builds one from `Window`. `tre.supports("layer_shell")`
answers the question without making a window.

With no display at all (a CI run) a `LayerWindow` can be built and driven headless, like
a `Window`, but not opened. A stretched axis is `1920` or `1080` logical pixels there
until `resize(width, height)` says otherwise.

If an `output` names no output, `App.run()` raises `RuntimeError` listing the ones the
compositor has.

## Events

Window events apply, with these additions (see [Events](events.md)):

| Event | Fires | Fields |
|---|---|---|
| `configure` | The compositor assigned a size: when the surface first has one, and whenever it changes (a repeat of the same size, which sway sends, is not a change). `resize` fires too. | `width`, `height` in logical pixels |
| `closed` | The surface is gone | `reason`: `"compositor"`, `"output_removed"` or `"app"` |
| `output_changed` | The surface entered an output | `output`: its connector name |
| `warning` | A setting the protocol accepts but ignores | `key` (the setting), `message` |

After `closed` with reason `"compositor"` or `"output_removed"` the window is dead; make a
new `LayerWindow` for a new output.

## What it is like to draw on

- **Layout is in logical pixels, drawn at the surface's scale** (`dpi_scaling` is on, and
  follows fractional scaling). The root has **no padding or gap**, unlike a `Window`'s,
  and the window is **transparent**, so a rounded or floating bar needs no backdrop.
- **Input** is as a window's: pointer, buttons and scroll; keys (named keys, text, the
  clipboard shortcuts, and key repeat from the compositor's own rate; copy and paste go through the
  compositor's data-control protocol, so they work on a session with no X server); touch where the
  seat has it; the cursor shape a node asks for. Text arrives as typed (compose and dead
  keys work through the keymap); input-method (IME) preedit is not wired for layer
  surfaces yet. A click on a bar does not focus another window's keyboard: with
  `"on_demand"` the bar keeps keys until the user clicks elsewhere.
- **Idle costs nothing.** A surface draws only when something changed, and an idle bar
  wakes the loop for nothing but its own timers.
- It shares the loop, and one `App`, with ordinary windows.

## Not yet

`input_region` (so a transparent
surface can let clicks through), animating a margin, dragging a surface by its margin,
and `PopupWindow` are later parts of the same work (issue #195). Until `input_region`
exists a surface takes pointer input over its whole area, so a full-screen transparent
surface would block the desktop. AccessKit (screen reader) support for layer surfaces
has not been checked.
