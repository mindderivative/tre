# Layers

*New in 0.3.4.* One mechanism for everything that floats over a window's
content — dialogs, menus, tooltips, snackbars, side sheets, drawers, context
menus. You build the content from ordinary nodes; `tre` stacks it, places it,
routes input around it, and tells you when it's dismissed. `tre` draws no scrim:
a modal's scrim is a window-sized `box` inside the layer.

```python
menu = window.create("box", width=200, flex_direction="vertical", role="menu")
# ... add item nodes to menu ...
menu.on("dismiss", lambda: window.hide_layer(menu))
window.show_layer(menu, anchor=button, placement="below")
```

## `show_layer`

**`show_layer(node, anchor=None, placement="below", modal=False,
dismissible=True)`**

Shows `node` over the window's content, above every layer already open —
layers stack in the order they're shown. A node attached elsewhere moves.

- **Placement.** With `anchor`, the layer sits against that node on the
  `placement` side: `"below"`, `"above"`, `"start"` (left), or `"end"`
  (right), aligned with the anchor's start. When that side lacks the room and
  the opposite side has more, it flips; then it shifts to stay inside the
  window. This happens at every layout, so it follows its anchor.
  `node.get("layer_placement")` reports the side it's on. Without an anchor, a
  layer sits at its own `x`/`y` (`0, 0` by default) — size a scrim with
  `width="100%", height="100%"`.
- **Modal.** Input beneath the layer is blocked: presses, hovers, and wheel
  events outside it reach nothing. Focus moves to the layer's first focusable
  node, and Tab stays inside it.
- **Dismissal.** With `dismissible=True`, a press outside the layer — and
  outside its anchor — delivers `dismiss` to the layer's node, and the press
  goes no further. Escape delivers `dismiss` to the topmost dismissible layer.
  A press inside a layer never dismisses the layers beneath it, so a submenu
  keeps its parent menu open. Closing is your call: call `hide_layer`, or not.

Every open layer is its own focus scope: Tab moves among the nodes of the layer
holding focus. Events inside a layer bubble up to the layer's node and stop
there, never reaching the content beneath.

A layer isn't part of the content: `window.root.children()` doesn't include it,
and its `parent()` is `None`. Content added while a layer is open still goes
beneath every layer.

## `hide_layer`

**`hide_layer(node)`**

Hides the layer. The node is detached — alive while you hold it, ready to show
again — and focus inside it returns to the node that held focus when the layer
opened. Raises `ValueError` if `node` isn't a shown layer.

## Building one

A modal dialog:

```python
scrim = window.create("box", width="100%", height="100%", fill=(0, 0, 0, 82),
                      align_items="center", justify_content="center")
panel = window.create("box", width=312, padding=24, fill=(255, 251, 254, 255),
                      corner_radius=28, role="dialog", label="Discard draft?")
scrim.add_child(panel)
scrim.on("dismiss", lambda: window.hide_layer(scrim))
window.show_layer(scrim, modal=True)
```

A tooltip that nothing dismisses but your own timer:

```python
window.show_layer(tip, anchor=icon, placement="above", dismissible=False)
```
