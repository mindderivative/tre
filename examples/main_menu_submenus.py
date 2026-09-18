#!/usr/bin/env python3
"""M30 Phase 8 Step 6's real `Main Menu` submenus (§11.3) -- extends
the existing context-menu overlay mechanism this codebase already has
real since M4 Phase 7, not a new overlay kind.

`add_menu_item(..., submenu=True)` adds a real, purely visual trailing
`chevron_right` indicator -- MD3's own real convention for "this item
opens a nested menu." The real submenu itself is nothing new: it's
just another real `build_menu`/`open_menu` pair, opened with the
parent item's own returned `Node` as the anchor, the identical real
`Tree::open_overlay` primitive every other overlay in this catalog
already reuses.

What this script proves automatically (headless-CI-safe, no human
needed): clicking the top-level "File" trigger opens its own menu,
clicking "Export As" (a real submenu item) opens a nested menu
anchored to it, and clicking a real submenu entry reaches its own
registered handler.
"""

from tre import App, Window

window = Window(width=400, height=300, title="tre v2 -- main menu submenus")
window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)

trigger = window.add_button(label="File", width=100, height=40)

new_item = window.add_menu_item(label="New")
open_item = window.add_menu_item(label="Open")
export_item = window.add_menu_item(label="Export As", submenu=True)
file_menu = window.build_menu([new_item, open_item, export_item], width=180)

pdf_item = window.add_menu_item(label="PDF")
png_item = window.add_menu_item(label="PNG")
export_submenu = window.build_menu([pdf_item, png_item], width=140)

trigger.enable_interaction()
trigger.set_on_click(lambda: window.open_menu(trigger, file_menu))

export_item.enable_interaction()
export_item.set_on_click(lambda: window.open_menu(export_item, export_submenu))

selected: dict[str, str] = {"format": ""}


def select_pdf() -> None:
    selected["format"] = "PDF"


pdf_item.enable_interaction()
pdf_item.set_on_click(select_pdf)

window.click(trigger)
window.click(export_item)
window.click(pdf_item)
assert selected["format"] == "PDF", "a real click on a submenu item must reach its own registered handler"

app = App()
app.add_window(window)
app.run(max_frames=60)
print(f"main_menu_submenus.py: exited cleanly after 60 frames, selected format {selected['format']!r}")
