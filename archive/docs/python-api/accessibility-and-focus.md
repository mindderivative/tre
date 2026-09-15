# Accessibility & Focus

Two related but distinct features: publishing an accessibility tree to the OS (for screen readers), and tracking in-app keyboard focus with Tab-order traversal. Both start with tagging nodes on a `Canvas`; see [Canvas & Shapes](canvas-and-shapes.md) for `tag_accessibility_node`/`tag_focusable`.

## Accessibility

### `AccessibilityRole`

Variants: `Generic`, `Button`, `TextLabel`, `Image`.

### `AccessibilityNode`

A read-only record returned by `canvas.accessibility_nodes()`. Fields: `node_id: int`, `x: float`, `y: float`, `width: float`, `height: float`, `role: AccessibilityRole`.

### `A11yBridge`

Publishes tagged nodes to the OS accessibility layer (AT-SPI2 on Linux). **Must stay on its constructing thread.**

```python
bridge = tre.A11yBridge(app_name="My App", toolkit_name="my-toolkit", toolkit_version="1.0")

# once per rendered frame:
nodes = canvas.accessibility_nodes()  # read BEFORE render_canvas() consumes the canvas
renderer.render_canvas(window, canvas)
bridge.publish(nodes)
```

| Method | Signature | Notes |
|---|---|---|
| `__init__` | `(app_name: str, toolkit_name: str, toolkit_version: str)` | Infallible -- never raises. If no AT-SPI2 registry is reachable, the bridge gracefully degrades to a permanently-inactive adapter and `publish()` becomes a real no-op |
| `publish` | `(nodes: list[AccessibilityNode])` | Call once per rendered frame, with the current frame's tagged nodes. Never blocks on D-Bus I/O |

Works in the same process that does the real GPU rendering -- no second process or handoff file is needed for a Python application (that split is a real, separate concern for *testing* an app's own AT-SPI2 output, not a requirement for shipping one).

## Focus and Tab order

### `FocusableNode`

A read-only record returned by `canvas.focusable_nodes()`. Fields: `node_id: int`, `x: float`, `y: float`, `width: float`, `height: float`, `tab_index: int | None`.

### `FocusManager`

Persistent, pure in-memory widget-focus state -- no OS handle, no thread affinity, safe to use from any thread.

```python
focus = tre.FocusManager()

nodes = canvas.focusable_nodes()
focus.focus_next(nodes)      # Tab
focus.focus_previous(nodes)  # Shift+Tab
focus.set_focus(some_node_id)  # e.g. a mouse click on that widget
focus.focused()  # -> int | None, the currently focused node_id
```

| Method | Signature |
|---|---|
| `__init__` | `()` |
| `focused` | `() -> int \| None` |
| `set_focus` | `(node_id: int \| None)` |
| `focus_next` | `(nodes: list[FocusableNode]) -> int \| None` |
| `focus_previous` | `(nodes: list[FocusableNode]) -> int \| None` |

**Tab order follows the HTML `tabindex` convention:** nodes with a positive `tab_index` come first, ascending, ties broken by the order they appear in `nodes`; then nodes with `tab_index=None` or `0`, in the same order; nodes with a **negative** `tab_index` are excluded from `focus_next`/`focus_previous` entirely, but remain directly targetable via `set_focus` -- the standard "focusable, but skip me during Tab navigation" pattern.

**`tre-engine` does not recognize the Tab key itself or track Shift state.** You (or your UI framework) recognize `InputEvent.KeyboardKey` with the Tab/Shift+Tab key codes and modifier state, and call `focus_next()`/`focus_previous()` once you've decided a tab navigation should happen:

```python
for event in renderer.poll_events():
    if isinstance(event, tre.InputEvent.KeyboardKey) and event.state == tre.ElementState.Pressed:
        if event.key_code == KEY_TAB:
            nodes = canvas.focusable_nodes()
            if shift_held:
                focus.focus_previous(nodes)
            else:
                focus.focus_next(nodes)
```

This is distinct from `InputEvent.WindowFocused` (OS-level window focus -- see [Input Events](input-events.md)), which fires even for an app with no concept of a focused widget at all.