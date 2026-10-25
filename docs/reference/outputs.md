# `Output`

A screen the compositor has (0.5.8). A shell makes one bar per screen, a launcher opens on the
screen the pointer is on, an app remembers which screen a window was on: all start from the list
of outputs and from being told when it changes.

```python
from tre import App, LayerWindow

app = App()
for out in app.outputs():          # before run(): the screens as they are now
    print(out.name, out.width, out.height, "at", out.x, out.y, "scale", out.scale_factor)
    app.add_window(LayerWindow("shell.bar", anchor=("top", "left", "right"), size=(0, 32),
                               exclusive_zone=32, output=out))
app.run()
```

**`App.outputs()`** reads them now, on a connection of its own (a few milliseconds), so it
works before `run()` and agrees with the compositor at the moment of the call. Top to bottom, then
left to right. Empty on anything but Wayland, and on a compositor that does not report connector
names.

## `Output`

A read-only value. Two `Output`s are equal, and hash alike, when their connector names are, so one
read before a change and one read after are the same screen.

| Property | Meaning |
|---|---|
| `name` | The connector name (`"DP-1"`, `"HDMI-A-1"`): stable across runs, unlike the compositor's own numbering. |
| `description` | The compositor's description, usually make, model and serial. |
| `x`, `y` | The output's top-left corner in the desktop's logical layout. |
| `width`, `height` | The output's size in logical pixels. |
| `scale_factor` | Physical over logical pixels, as a float: `1.5` on a 1.5x output. Not the integer scale `wl_output` reports. |
| `transform` | `"normal"`, `"90"`, `"180"`, `"270"`, or one of those after `"flipped_"`. |

`LayerWindow(output=...)` takes an `Output` or its connector name. An output that is not there
makes `App.run()` raise `RuntimeError` listing the ones that are.

## Events

```python
app.on("output_added", lambda out: app.add_window(make_bar(output=out)))
app.on("output_removed", lambda out: print(out.name, "went away"))
app.on("output_updated", lambda out: relayout(out))
```

| Event | Fires |
|---|---|
| `output_added` | A screen was plugged in or switched on. |
| `output_removed` | A screen was unplugged or switched off. A `LayerWindow` bound to it is closed by the compositor, and fires `closed` with reason `"output_removed"`. |
| `output_updated` | A screen's position, size, scale or rotation changed. |

The handler gets the `Output` as it is now (for a removal, as it last was). One handler per event;
a second `on` replaces the first, and `off(event)` removes it. A handler that raises is logged and
the run goes on. The outputs that exist when the app starts are not "added": read them with
`outputs()`.

Output events arrive on the same Wayland connection a `LayerWindow` uses, so they need a
compositor with the layer shell (KDE's KWin and wlroots' do; GNOME's Mutter does not). Registering a
handler starts that connection even with no layer window; before `run()` or while it runs.

**`App.simulate(event, **fields)`** delivers an event as the compositor would, for tests with no
display: `app.simulate("output_added", name="DP-2", width=2560, height=1440)`. The fields are
`name` (required), `description`, `x`, `y`, `width`, `height`, `scale_factor` and `transform`.

## What it does not do

It never changes a screen: mode, scale and position belong to the compositor's own settings. On
Windows and macOS there are no outputs to list yet (`outputs()` is empty), since a layer surface,
the thing that needs them, exists only on Wayland.
