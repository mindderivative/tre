# Docking

Docking arranges panels in zones around a window's content — a file tree on
the left, an inspector on the right, a terminal at the bottom — and lets the
user drag a panel from one zone to another. `tre` docks panels, drags them,
and reports what's happening; what a drag looks like — the handle you grab
and the highlight over the target zone — is yours to draw.

## Zones and panels

A zone is an ordinary node you build and register for a side. Docking a
panel attaches it under that node and makes it the zone's shown panel.

```python
left_zone = window.create("box", fill=(0xF5, 0xF5, 0xF5, 0xFF), width=200, height=400)
window.root.add_child(left_zone)
window.add_dock_zone("left", left_zone, size=200.0)

right_zone = window.create("box", fill=(0xF5, 0xF5, 0xF5, 0xFF), width=200, height=400)
window.root.add_child(right_zone)
window.add_dock_zone("right", right_zone, size=200.0)

panel = window.create("box", fill=(0xFF, 0xFF, 0xFF, 0xFF), width=180, height=380)
window.dock_panel("left", panel)
```

Accepted `side` values: `"left"`, `"right"`, `"top"`, `"bottom"`,
`"center"` — anything else raises `ValueError`.

A zone holds any number of panels and shows one at a time.
`set_active_panel(side, index)` picks which, counting in docking order.

| Method | Purpose |
| --- | --- |
| `add_dock_zone(side, container, size)` | Registers `container` as `side`'s dock zone, seeding its extent |
| `dock_panel(side, panel)` | Docks `panel` into `side`'s zone and shows it — moving it, if it's docked in another zone |
| `set_active_panel(side, index)` | Shows the zone's `index`th panel |
| `start_panel_drag(panel)` | Starts dragging `panel`, which must be docked |
| `undock_panel(panel)` | Takes `panel` out of its zone and off the tree; it can be docked again |

## Moving a panel without a drag

`dock_panel` on a panel that's docked in another zone moves it, exactly as
a drop would: the old zone stops listing it and shows another of its
panels. That's how a "Move to" menu moves a panel for someone who can't
drag:

```python
window.dock_panel("right", panel)   # from wherever it's docked now
```

## Closing a panel

`undock_panel(panel)` takes a panel out of docking for good: out of its
zone's list, whose later panels move down one index, and off the tree. If
it was the one shown, the zone shows the next panel, or else the previous,
as closing a tab does — or nothing, if it was the last. A drag of it in
progress is cancelled.

```python
window.undock_panel(panel)          # a closed tab, or a panel a hot-reloaded
                                    # layout no longer names
window.dock_panel("left", panel)    # it can come back while you hold it
```

Use it rather than `panel.remove()`, which takes the panel off the tree but
leaves it in its zone's list, where `set_active_panel` would show it again.
A panel that is destroyed, or freed after `remove()`, leaves its zone on the
next docking call.

## Dragging a panel

Start a drag from your own handle's `pointer_down`. While the pointer moves,
the `dock_target` window event reports the zone under it (`event.side`, or
`None` over no zone) each time that changes. Releasing the primary button
moves the panel into the zone there — nowhere, if it's over no zone — and
reports `dock_drop` (`event.panel`, `event.side`).

```python
handle.on("pointer_down", lambda: window.start_panel_drag(panel))

highlight = window.create("box", fill=(0, 0x80, 0xFF, 0x60),
                          position="absolute", width="100%", height="100%")
zones = {"left": left_zone, "right": right_zone}

def show_target(event):
    highlight.remove()
    if event.side is not None:
        zones[event.side].add_child(highlight)

window.on("dock_target", show_target)
window.on("dock_drop", lambda: highlight.remove())
```

The drag runs in the same input pipeline as every other pointer event, so a
headless test drives it with `window.simulate`:

```python
window.start_panel_drag(panel)
window.simulate("pointer_move", node=right_zone)
window.simulate("pointer_up", node=right_zone)
assert right_zone.children() == [panel]
```
