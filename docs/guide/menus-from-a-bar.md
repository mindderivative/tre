# Menus from a bar

A bar is 32 pixels tall, and `show_layer` draws inside its own window's bounds, so a menu for
a button on the bar needs a surface of its own. The pattern that works today needs no new
API: **a second `LayerWindow`, transparent and covering the output, that exists only while
the menu is open** (0.5.7). `examples/layer_menu.py` is the whole thing.

```python
def show(e=None):
    overlay = LayerWindow("shell.menu", layer="overlay", anchor=("top", "bottom", "left", "right"),
                          size=(0, 0), exclusive_zone=-1, keyboard="exclusive",
                          input_region="auto", output=bar.get("output"))
    overlay.root.set(fill=(0, 0, 0, 0))
    anchor = overlay.create("box", width=64, height=32, x=12, y=0, position="absolute")
    anchor.set(hit_testable=False)          # an invisible marker where the button is
    overlay.root.add_child(anchor)
    overlay.show_layer(menu, anchor=anchor, placement="below", dismissible=True, flip=True)
    menu.on("dismiss", hide)                # a click outside the menu, or Escape
    app.add_window(overlay)

def hide():
    overlay.close()                         # unmaps it: nothing is held while no menu is open
```

What each part does:

- **`layer="overlay"`, all four edges, `size=(0, 0)`** covers the output above the windows and the
  bar. `exclusive_zone=-1` means it reserves nothing and ignores the bar's zone.
- **`input_region="auto"`** lets every click where nothing is drawn through to the windows below,
  so the overlay never blocks the desktop; only the menu takes input.
- **`dismissible=True`** makes a click outside the menu, or Escape, deliver `dismiss` to the
  menu's node; you close it. While a dismissible or modal layer is open the overlay takes the
  *whole* surface, so the dismissing click reaches the engine; as for any dismissible layer, that
  click is consumed and does not also reach the window below.
- **`keyboard="exclusive"`** gives the overlay the keyboard the moment it opens, so Escape and
  arrow keys work without a click. It holds the keyboard until the overlay closes, which is what a
  menu wants. (`"on_demand"` would need a click first.)
- **`placement="below"` with `flip=True`** puts the menu under the button, and above it when the
  bar is at the bottom edge and there is no room, using the overlay's bounds (the output).
- A **submenu** is another `show_layer` anchored to an item with `placement="end"` and
  `dismissible=False`: a press inside it never dismisses the menu beneath.
- A **tooltip** is a `show_layer` with `dismissible=False`: only its own shape takes input.

## What it costs

Measured on a headless sway (an AMD RADV GPU), by reading `mem_info_vram_used` with the menu
closed, open, and closed again, and by reading the screen back with `grim`:

| | 1280 x 720 output | 3840 x 2160 output |
|---|---|---|
| VRAM while the overlay is mapped | about 212 MB | about 333 MB |
| VRAM after it closes | back to where it was | back to where it was (within 40 MB) |
| Frames drawn by an open menu while nothing moves | 0 | 0 |
| Click to menu on screen | | about 140 ms (measured by polling `grim`, so within 70 ms) |

About 210 MB of that is a fixed cost every tre window pays (the bar does too), and the rest
grows with the output's pixels, roughly 16 bytes each. So **keeping a full-screen overlay mapped
all the time costs a few hundred MB per output; creating it when the menu opens and closing it
on dismiss costs nothing in between**, at the price of about a tenth of a second to open. Create
it on demand.

## What it does not do

- **It is not a compositor popup.** A menu from an `xdg_popup` is placed by the compositor,
  which knows the screen edges and other panels' reserved areas; here the engine flips the menu
  inside the output, which does not know where another panel's zone is.
- **It covers the output while open.** Other surfaces below it keep drawing and updating, but
  receive no input until the menu closes (the dismissing click included).
- **One overlay per output.** A menu belongs to the output of the bar that opened it
  (`output=bar.get("output")`).
- A screen reader sees the overlay as one more window of the application while the menu is open
  (see [accessibility on layer surfaces](../reference/layer-window.md#accessibility)); the menu's
  nodes are in it like any other nodes.

If any of these matters for the shell, a real `PopupWindow` (an `xdg_popup` parented to the layer
surface) is the follow-up; it is a larger piece of platform work, tracked in issue #206.
