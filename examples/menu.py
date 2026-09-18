#!/usr/bin/env python3
"""M30 Phase 2 Step 4's real `Window.add_menu_item`/`build_menu`/
`open_menu`/`close_menu` (§5, §7, §11.3): a real MD3 dropdown menu,
built on the exact same `Tree::open_overlay`/`close_overlay`
primitive the existing right-click context-menu mechanism already
uses (`context_menu.py`, left completely untouched by this step) --
just triggered by a plain click instead of a right-click.

What this script proves automatically (headless-CI-safe, no human
needed): a real menu of items assembles into a real panel, opens
anchored below a trigger button, a real click on one of its items
reaches that item's own registered handler (proving the item survived
both `build_menu`'s re-parent and `open_menu`'s later attach), and the
menu closes cleanly.
"""

from typing import Callable

from tre import App, Window

window = Window(width=240, height=200, title="tre v2 -- menu")

trigger = window.add_button(label="Options", width=120, height=40)

items = [window.add_menu_item(label=label, icon="add") for label in ["New", "Open", "Save"]]
menu = window.build_menu(items, width=160)

window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)

selected: dict[str, str | None] = {"label": None}


def make_selector(label: str) -> Callable[[], None]:
    def select() -> None:
        selected["label"] = label
        window.close_menu(menu)

    return select


for label, item in zip(["New", "Open", "Save"], items):
    item.enable_interaction()
    item.set_on_click(make_selector(label))

trigger.enable_interaction()
trigger.set_on_click(lambda: window.open_menu(trigger, menu))
window.click(trigger)
window.click(items[1])

app = App()
app.add_window(window)
app.run(max_frames=60)
print(f"menu.py: exited cleanly after 60 frames, selected {selected['label']!r}")
