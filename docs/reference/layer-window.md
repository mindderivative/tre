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
margin=0, keyboard="none", output=None, input_region=None)`**

| Setting | Meaning |
|---|---|
| `namespace` | What the surface is for (`"tesserae-shell.bar"`); the protocol's namespace. Required and not empty. Constructor only. |
| `layer` | Where it stacks: `"background"`, `"bottom"`, `"top"` or `"overlay"`. Windows sit between `"bottom"` and `"top"`; a fullscreen window may cover `"top"`. |
| `anchor` | The edges it is attached to: a sequence of `"top"`, `"bottom"`, `"left"`, `"right"` (or one name). Nothing anchored floats, centred by the compositor. |
| `size` | `(width, height)` in logical pixels. A `0` stretches between the two edges anchored on that axis, so a `0` width needs `"left"` and `"right"` in `anchor`, and a `0` height needs `"top"` and `"bottom"`. |
| `exclusive_zone` | Pixels to reserve along the anchored edge (for a corner, the `exclusive_edge`), so maximised windows stop short of it. `0` reserves none; `-1` ignores other surfaces' zones and takes the whole output. A positive zone counts only for a surface anchored to one edge, or to one edge and both edges across it (a bar anchored `"top"`, `"left"`, `"right"`); with any other `anchor` the compositor treats it as `0`, and a `warning` event says so. |
| `margin` | `(top, right, bottom, left)` in logical pixels, or one number for all four. Negative is accepted and moves the surface past the screen edge. |
| `keyboard` | `"none"`: never gets key events. `"on_demand"`: gets them when the user focuses it (a click). `"exclusive"`: takes the keyboard on opening, and nothing else gets keys until it closes. |
| `output` | An [`Output`](outputs.md) (from `App.outputs()`) or a connector name (`"DP-1"`, `"HDMI-A-1"`) to bind it to. `None` picks the output at position (0, 0), or the first by name if none is there, the same on every run (the compositor's own choice alternates). A surface meant for another screen names it. Constructor only. |
| `input_region` | Where the compositor delivers pointer and touch input. `None`: the whole surface. `[]`: nothing, so clicks fall through to the window below. A list of `(x, y, width, height)` rectangles in logical pixels from the surface's top-left, rounded outward, at most 256: input lands only inside them. Live through `set()`. `click_through=True` is `input_region=[]` on a `LayerWindow` (and `False` gives the whole surface back); giving both with different meanings raises `ValueError`. `"auto"`: the union of the shapes of drawn, hit-testable nodes (a box with a fill, gradient or border; any image, path, svg, canvas, text field or terminal; a selectable or linked text), with a box's rounded corners and a canvas's circle hit shape counted. A transparent root or a node with `hit_testable=False` adds nothing, so clicks there fall through. It follows layout, transforms and animation: while something animates it only grows, then snaps to the exact shape. A shape is cut into horizontal bands of 2 px or more (at most 256 rectangles, coarser if a shape needs more); it may cover a pixel or two beyond a curve, never less. The region reaches the compositor with a commit of its own, so an idle surface needs no frame. |

**`exclusive_edge`** says which edge a corner-anchored surface's `exclusive_zone` is for: a clock
in the top-right corner (`anchor=("top", "right")`) that reserves 28 pixels from the top takes
`exclusive_edge="top"`; with `"right"` it reserves them from the right instead. Without it a
corner surface reserves nothing (the compositor cannot tell which of the two edges was meant, and
a `warning` event says so). It must be one of the anchored edges. A surface anchored to one edge,
or to one edge and both edges across it (a bar), needs none: the compositor deduces it, and giving
the same edge is allowed. It is live through `set()`; the protocol has no request that clears an
edge once set, so `set(exclusive_edge=None)` on a surface that has one raises `ValueError` (move it
to the other edge, or make a new window). It needs layer-shell version 5 (KWin and recent wlroots
compositors have it); on a compositor with version 4 the surface still opens and a `warning` event
(`key` `"exclusive_edge"`) says the edge is ignored. tre binds the layer shell itself, at the
highest version the compositor offers.

**Compositor support.** sway (wlroots) reserves the zone on the named edge of a corner surface.
KDE's KWin 6.8.1 offers version 5 and accepts the request, but does not reserve space for a
corner surface (a maximised window keeps its size); on KWin make the reserving surface a bar
(one edge, or three) for now.

**`set(**props)`** changes `layer`, `anchor`, `size`, `exclusive_zone`, `margin` and
`keyboard` live, in one commit, and the keys `Window.set` has that mean something to a
placed surface (`touch_slop`, `partial_redraw`, `present_mode`, ...). A bad value raises
`ValueError` and changes nothing, the layer keys included. The window keys that mean
nothing to a surface the compositor places raise too, rather than being ignored:
`title`, `decorations`, `maximized`, `minimized`, `fullscreen`, `resize_border`,
`system_menu`, `min_width`, `min_height`, `icon`, `x`, `y`, `always_on_top`,
`resizable`, `skip_taskbar` and `blur_behind`. `namespace` and `output` can only be given
to the constructor.

**`animate(property, to, duration_ms=0, easing=None, on_complete=None)`** moves one margin
(`"margin_top"`, `"margin_right"`, `"margin_bottom"` or `"margin_left"`) to `to` pixels from where
it is now, with the same `duration_ms`, `easing` (linear, a cubic bezier, or a spring) and
`on_complete` as `Node.animate`. The engine runs it: one small commit per step, no buffer
change, and no Python until `on_complete` (called once, when the duration has elapsed). The
window's root gets `animation_end` (`property`, `finished`). Starting another animation on the
same margin retargets from the current value (the first one's `on_complete` is not called);
`set(margin=...)` while one runs cancels it and snaps (`finished=False`). Negative values slide
a surface past the screen edge. Any other property raises `ValueError`: surface size and
`exclusive_zone` are step changes, not animations. Headless, `advance(ms)` runs margin
animations and their callbacks with no compositor. Every window's animations in one loop turn
read the same time, but Wayland cannot commit two surfaces atomically, so surfaces animating
together can differ by up to a frame on screen. A margin takes whole pixels (the animation is
rounded to them).

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
| `moved` | A margin drag ended (see below) | `margin`: the final `(top, right, bottom, left)` |

After `closed` with reason `"compositor"` or `"output_removed"` the window is dead; make a
new `LayerWindow` for a new output.

## Dragging a surface by its margin

A movable widget (a desktop clock, a floating panel) anchors to an edge or a corner and moves
by changing its margins. Give a node `window_region="drag_margin"` and a primary press on it,
or on a non-interactive node inside it, starts a drag: the engine follows the pointer, one
small commit per input frame, with no Python in the loop, and when the button goes up the
window gets `moved` with the final `margin`, which the app can save. The usual exclusions apply
as for `"drag"`: a press on a focusable node, a text input or terminal, a node with a `click`
listener, or one holding the pointer capture, met before the drag node, is an ordinary press;
`window_region="none"` on a node keeps a part of the handle from dragging.

- Only the anchored edges move: a surface anchored `"top"` and `"left"` changes `margin_top`
  and `margin_left`; one anchored `"bottom"` and `"right"` changes the other two, in the
  opposite direction. An axis anchored to both edges (a bar across the top) or to neither does
  not move on that axis.
- The margins are clamped so the surface stays on its output: never negative, never past the
  far edge. (Other surfaces' exclusive zones do not limit it.)
- The deltas come from the compositor's relative-pointer protocol, so they are exact however
  the surface lags behind the pointer. An absolute pointer (a tablet, a virtual machine's,
  a remote desktop's, and KWin's reading of a `/dev/uinput` absolute device) sends no relative
  motion; the drag then follows the pointer's surface-local position, which can overshoot a
  little while a commit is still in flight. The same fallback runs where the compositor has no
  relative-pointer protocol.
- A running margin animation is stopped where it is when a drag starts. A node's
  `window_region="drag_margin"` raises on an ordinary `Window`, and `"drag"` still raises here.

## What it is like to draw on

- **Layout is in logical pixels, drawn at the surface's scale** (`dpi_scaling` is on, and
  follows fractional scaling). The root has **no padding or gap**, unlike a `Window`'s,
  and the window is **transparent**, so a rounded or floating bar needs no backdrop.
- **Input** is as a window's: pointer, buttons and scroll; keys (named keys, text, the
  clipboard shortcuts, and key repeat from the compositor's own rate; copy and paste go through the
  compositor's data-control protocol, so they work on a session with no X server); touch where the
  seat has it; the cursor shape a node asks for. Text arrives as typed (compose and dead
  keys work through the keymap); an **input method** (CJK composition, a candidate window) works
  too, through `zwp_text_input_v3`: while a text input of the surface has keyboard focus the
  input method is enabled, the candidate window is placed at the caret, a password field says
  so, and the node gets `compose_start`, `compose_update` and `compose_end` and the committed
  text as `input` and `change`, as in an ordinary window. It needs a compositor with the
  text-input protocol (KWin, sway) and an input method running; otherwise the surface types
  through the keymap alone. The surface needs keyboard focus (`"on_demand"` or `"exclusive"`). A click on a bar does not focus another window's keyboard: with
  `"on_demand"` the bar keeps keys until the user clicks elsewhere.
- **Idle costs nothing.** A surface draws only when something changed, and an idle bar
  wakes the loop for nothing but its own timers.
- It shares the loop, and one `App`, with ordinary windows.

## Accessibility

A layer surface is published to screen readers like a window (0.5.7). Linux has no toolkit-wide
accessibility layer to hook, so tre gives each surface an AT-SPI adapter of its own (AccessKit's
`accesskit_unix`, the backend a window's adapter wraps): the surface appears as one more window of
the application, its nodes carry the `role`, `label` and state you set on them, the tree follows the
nodes as they change, a screen reader's action on a node (a click, a focus) reaches the app as the
usual `a11y_action` and activation events, and keyboard focus entering or leaving the surface tells
the reader whether the window is active. Nothing is built until a screen reader asks. Under Wayland
an application cannot know where its surfaces are on screen, so the reader gets the tree but no
screen position for a layer surface. See [Accessibility](../guide/accessibility.md) for what a node
should declare.

## Not yet

`PopupWindow` (a compositor popup) is a later part of the same work (issue #195); menus from a bar
work today with an overlay `LayerWindow`, see [Menus from a bar](../guide/menus-from-a-bar.md).
