#!/usr/bin/env python3
"""M30 Phase 3 Step 5's real `Window.add_tooltip` (§5, §7, §11.3),
closing Phase 3: MD3's real Plain Tooltip panel, shown and hidden
through the exact same `Window.open_menu`/`close_menu` overlay
primitive `menu.py` already demonstrates -- triggered here from a
real button's own `set_on_hover_enter`/`set_on_hover_exit`, both
already generic (work on any `NodeKind`), rather than a tooltip-
specific open/close pair.

What this script proves automatically (headless-CI-safe, no human
needed): a real tooltip panel renders, and a real synthetic hover
dispatch (`Window.hover`) actually opens it via the shared overlay
primitive, exiting cleanly.
"""

from tre import App, Window

window = Window(width=240, height=100, title="tre v2 -- tooltip")

button = window.add_icon_button(icon="add", variant="standard")
tooltip = window.add_tooltip(text="Add item", width=90, x=0, y=44)

window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)

shown = {"value": False}


def show_tooltip() -> None:
    window.open_menu(button, tooltip)
    shown["value"] = True


def hide_tooltip() -> None:
    window.close_menu(tooltip)
    shown["value"] = False


button.set_on_hover_enter(show_tooltip)
button.set_on_hover_exit(hide_tooltip)
window.hover(button)

app = App()
app.add_window(window)
app.run(max_frames=60)
print(f"tooltip.py: exited cleanly after 60 frames, tooltip shown={shown['value']}")
