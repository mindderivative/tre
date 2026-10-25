# Menus from a bar

A bar is 32 pixels tall, and `show_layer` draws inside its own window's bounds, so a menu for
a button on the bar needs a surface of its own. The way to get one is **a `PopupWindow`**: a
compositor popup (`xdg_popup`) hanging off the bar, sized to the menu, placed by the compositor
beside the button and flipped above it when the bar is at the bottom edge (0.5.8, #223).

```python
def open_menu(e=None):
    menu = PopupWindow(bar, size=(160, 90), anchor_rect=(100, 0, 60, 32),   # the button, in bar coordinates
                       anchor="bottom_left", gravity="bottom_right",
                       constraint_adjustment=("flip_y", "slide_x"))
    menu.root.set(fill=(30, 140, 90, 255))
    item = menu.create("box", width=148, height=30, x=6, y=6, position="absolute")
    item.on("click", lambda e: (run_action(), menu.close()))
    menu.root.add_child(item)
    menu.on("closed", lambda e: print(e.reason))    # "dismissed" on an outside click or Escape
    app.add_window(menu)
```

- A click outside the application, or Escape, closes the popup with `closed`, reason
  `"dismissed"`; no overlay is needed to catch it. A click on another window of the same
  application is delivered to that window instead, so listen for it and `close()` the menu.
- A **submenu** is a `PopupWindow` whose parent is the menu: `PopupWindow(menu, ...,
  anchor_rect=<the item>, anchor="top_right", gravity="bottom_right",
  constraint_adjustment=("flip_x", "slide_y"))`. Closing the menu closes its submenus.
- `configure` carries the popup's final `x`, `y` in the parent's coordinates, so a flip is visible
  to the app.
- It costs a bar's worth of memory (tens of MiB with the default small GPU blocks, #219), not an
  output's. `examples/popup_menu.py` is a complete bar with a menu and a submenu.

See the [`PopupWindow` reference](../reference/layer-window.md#popupwindow) for every argument.

## The overlay route

Before `PopupWindow` a menu was drawn on **a second `LayerWindow`, transparent and covering the
output, that exists only while the menu is open**. It still works, and is the thing to reach for
when the menu must be drawn by the engine's own `show_layer` (one layer system with tooltips and
dismissible layers) or span several outputs. `examples/layer_menu.py` is the whole thing.

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

- **It is not a compositor popup.** A `PopupWindow` is placed by the compositor,
  which knows the screen edges and other panels' reserved areas; here the engine flips the menu
  inside the output, which does not know where another panel's zone is.
- **It covers the output while open.** Other surfaces below it keep drawing and updating, but
  receive no input until the menu closes (the dismissing click included).
- **One overlay per output.** A menu belongs to the output of the bar that opened it
  (`output=bar.get("output")`).
- A screen reader sees the overlay as one more window of the application while the menu is open
  (see [accessibility on layer surfaces](../reference/layer-window.md#accessibility)); the menu's
  nodes are in it like any other nodes.

If any of these matters, use a `PopupWindow` instead (above).
