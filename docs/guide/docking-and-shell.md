# Docking & Shell Layout

## App shell

```python
menu_bar = window.add_rect(background=(0xEE, 0xEE, 0xEE, 0xFF), width=400, height=32)
status_bar = window.add_rect(background=(0xEE, 0xEE, 0xEE, 0xFF), width=400, height=24)

content = window.build_shell(menu_bar=menu_bar, toolbar=None, status_bar=status_bar)
content.add_child(window.add_rect(background=(0xFF, 0xFF, 0xFF, 0xFF), width=400, height=144))
```

`build_shell(menu_bar=None, toolbar=None, status_bar=None)` builds a
`Column`-flex shell container sized to the window's full width/height,
re-parenting whichever named chrome regions you pass (each must already
be a node you built) into it in `menu_bar` → `toolbar` → *(content)* →
`status_bar` order. It returns the empty `content` node — a
`flex_grow: 1.0` container that fills whatever space the given chrome
regions don't take. `build_shell` is a composition convenience; it
doesn't build the chrome nodes' own content, only names which node plays
which role.

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

## Container-transform navigation

```python
window.begin_container_transform(
    trigger=list_item,
    destination=detail_view,
    duration_ms=300,
    content_stagger_ms=90,
    on_complete=lambda: window.end_container_transform(list_item),
)
```

Starts MD3's container-transform choreography between `trigger` and
`destination` — `destination` must already be attached to the tree,
laid out, and carry its own real target appearance (this call captures
that as the animation's target, then morphs from `trigger`'s captured
from-state). `curve` is fixed at `MotionCurve::Emphasized`, the curve
MD3 names as typical for this transition. `on_complete`, when given,
fires once the whole transition genuinely finishes — a common pattern is
to call `end_container_transform(trigger)` from it, which detaches
`trigger` from its own parent (the "trigger node is hidden or removed"
step of the pattern). See `examples/navigation.py` for a complete
list-to-detail navigation built this way.
