# Docking

*0.3.5 removed `build_shell` and the MD3 shell chrome:* a framework builds
its own app shell from the building blocks.

## Docking

```python
left_zone = window.add_rect(background=(0xF5, 0xF5, 0xF5, 0xFF), width=200, height=400)
window.add_dock_zone("left", left_zone, size=200.0)

panel = window.add_rect(background=(0xFF, 0xFF, 0xFF, 0xFF), width=180, height=380)
window.dock_panel("left", panel)
```

Accepted `side` values: `"left"`, `"right"`, `"top"`, `"bottom"`,
`"center"` — anything else raises `ValueError`.

| Method | Purpose |
| --- | --- |
| `add_dock_zone(side, container, size)` | Registers `container` as `side`'s dock zone, seeding its extent |
| `dock_panel(side, panel)` | Attaches `panel` as `side`'s active tab |
| `set_active_tab(side, index)` | Switches which docked panel is active by index |
| `set_dock_handle(handle, panel)` | Makes `handle` the real drag grip for dragging `panel` |
| `set_drop_zone_highlight(content)` | Registers the node shown over whichever zone is under the pointer during a drag |
| `start_panel_drag(handle)` | Starts tracking a drag as if `handle` had just been pressed — returns whether it did |
| `drag_panel_over(x, y)` | Call while dragging — shows/hides/repositions the highlight over the enclosing zone, if any |
| `drop_panel_at(x, y)` | Ends the drag — reparents the panel into whichever zone encloses `(x, y)`, if different from its current one |

None of these need a live rendered window — `drag_panel_over`/
`drop_panel_at` recompute layout and hit-test purely in terms of the
window's own coordinate space, so docking can be driven and tested
headlessly the same way `click`/`hover` can.

A typical real drag sequence:

```python
window.start_panel_drag(handle_node)
window.drag_panel_over(mouse_x, mouse_y)   # called repeatedly while dragging
window.drop_panel_at(mouse_x, mouse_y)     # called once on release
```
