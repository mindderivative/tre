# Layers

Menus, dialogs, tooltips, snackbars, and sheets all float over the window's
content. Tesserae Engine has one mechanism for all of them: build the content from
ordinary nodes and show it as a layer. Tesserae Engine stacks it, places it against
an anchor, keeps it on screen, routes input around it, scopes focus to it,
and tells you when the user dismisses it. What it looks like — and whether
there's a scrim — is yours.

```python
window.show_layer(node, anchor=None, placement="below", modal=False, dismissible=True)
window.hide_layer(node)
```

The reference is [Layers](../reference/layers.md).

## A menu

A menu is anchored to the button that opened it. It closes on a press
outside it, on Escape, or when an item is chosen:

```python
menu = window.create("box", width=200, padding_top=8, padding_bottom=8,
                     flex_direction="vertical", corner_radius=4,
                     fill=(0xF3, 0xED, 0xF7, 0xFF),
                     shadows=[((0, 0, 0, 77), 0, 1, 2, 0), ((0, 0, 0, 38), 0, 2, 6, 2)],
                     role="menu")
for name in ["Cut", "Copy", "Paste"]:
    item = window.create("box", height=48, padding_left=12, align_items="center",
                         role="menuitem", label=name, focusable=True)
    item.add_child(window.create("text", text=name, width=160, height=20))
    item.on("click", lambda e, name=name: choose(name))
    menu.add_child(item)

def choose(name):
    window.hide_layer(menu)
    run_command(name)

menu.on("dismiss", lambda: window.hide_layer(menu))
more_button.on("click", lambda: window.show_layer(menu, anchor=more_button))
```

`placement` picks the anchor's side: `"below"`, `"above"`, `"start"`, or
`"end"`. When that side is short of room and the opposite side has more,
the layer flips; then it shifts to stay inside the window. It re-places
itself at every layout, so it follows its anchor.
`menu.get("layer_placement")` says which side it landed on — for a menu
that grows upward, or an arrow that points the right way.

A press inside a layer never dismisses the layers under it, so a submenu is
just another layer, anchored to its item with `placement="end"`.

## A modal dialog

`modal=True` blocks input to everything beneath the layer and moves focus to
its first focusable node; Tab stays inside it. The scrim is an ordinary
window-sized box:

```python
scrim = window.create("box", width="100%", height="100%", fill=(0, 0, 0, 82),
                      align_items="center", justify_content="center")
dialog = window.create("box", width=312, padding=24, gap=16,
                       flex_direction="vertical", corner_radius=28,
                       fill=(0xEC, 0xE6, 0xF0, 0xFF),
                       role="dialog", label="Discard draft?")
scrim.add_child(dialog)
# ... the dialog's text and buttons ...
scrim.on("dismiss", lambda: window.hide_layer(scrim))
window.show_layer(scrim, modal=True)
```

A layer without an anchor sits at its own `x`/`y`, `(0, 0)` by default, so
a 100% box covers the window. `hide_layer` detaches the layer — it stays
alive while you hold it, ready to show again — and returns focus to
whatever held it when the layer opened.

## Tooltips and snackbars

`dismissible=False` leaves closing entirely to you. A tooltip shows on hover
and hides when the pointer leaves:

```python
icon.on("pointer_enter", lambda: window.show_layer(tip, anchor=icon,
                                                   placement="above",
                                                   dismissible=False))
icon.on("pointer_leave", lambda: window.hide_layer(tip))
```

A snackbar is a non-dismissible layer at the bottom of the window with
`live="polite"`, so screen readers announce it, hidden by your own timer
or its action button.

## How input flows

- Layers stack in the order they're shown; the newest is on top.
- Events inside a layer bubble up to the layer's node and stop there — they
  never reach the content beneath.
- Each layer is its own Tab scope.
- With `dismissible=True`, a press outside the layer — and outside its
  anchor — delivers `dismiss` to the layer and goes no further, so it
  doesn't also click whatever was under it. Escape delivers `dismiss` to the
  topmost dismissible layer.
- `dismiss` asks; it doesn't close. Hide the layer, or keep it open — for a
  dialog with unsaved changes.

A layer isn't part of the content tree: `window.root.children()` doesn't
list it and its `parent()` is `None`.
