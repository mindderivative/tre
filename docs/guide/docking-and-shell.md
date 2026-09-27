# Docking

*0.3.5 removed `build_shell` and the MD3 shell chrome:* a framework builds
its own app shell from the building blocks. It also reduced docking to its
bare bones: `tre` docks panels, drags them, and reports; what a drag looks
like — the handle, the highlight over the target zone — is the framework's.

## Zones and panels

A zone is an ordinary node you build and register for a side. Docking a
panel attaches it under that node and makes it the zone's shown panel.

```python
left_zone = window.create("box", fill=(0xF5, 0xF5, 0xF5, 0xFF), width=200, height=400)
window.root.add_child(left_zone)
window.add_dock_zone("left", left_zone, size=200.0)

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
| `dock_panel(side, panel)` | Docks `panel` into `side`'s zone and shows it |
| `set_active_panel(side, index)` | Shows the zone's `index`th panel |
| `start_panel_drag(panel)` | Starts dragging `panel`, which must be docked |

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

*0.3.5 removed* `set_active_tab` (now `set_active_panel`),
`set_dock_handle`, `set_drop_zone_highlight`, `drag_panel_over`, and
`drop_panel_at`; `start_panel_drag` now takes the panel, not a handle.
