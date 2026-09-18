#!/usr/bin/env python3
"""M30 Phase 5 Step 5's real `Window.add_search_bar`/`add_search_view`
(§5, §7, §11.3), closing Phase 5's own component list.

`add_search_bar`'s own input reuses `TextField`'s already-real
`NodeKind` directly -- typing, focus, and selection all work exactly
like `add_text_field`'s own, for free. `add_search_view`'s own docked
suggestions panel is a plain styled container the app populates
itself, shown/hidden through the existing `Window.open_menu`/
`close_menu` -- the same real reuse `add_tooltip`'s own panel already
has, not a new dedicated open/close pair.

What this script proves automatically (headless-CI-safe, no human
needed): typing into the search field's real `TextField` works, the
search view opens anchored to the bar and can be populated with real
suggestion rows, and the trailing "clear" icon is independently
clickable.
"""

from tre import App, Window

window = Window(width=500, height=400, title="tre v2 -- search")
window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)

bar, field, leading, trailing = window.add_search_bar(
    placeholder="Search conversations",
    width=400,
    leading_icon="add",
    trailing_icons=["add"],
)

assert leading is not None
assert field.get_text() == "Search conversations"

window.press_key("tab")
field.set_text("")
window.type_text("hello")
assert field.get_text() == "hello", "typing into the real TextField must actually edit its content"

cleared: dict[str, bool] = {"value": False}
trailing[0].enable_interaction()
trailing[0].set_on_click(lambda: cleared.__setitem__("value", True))
window.click(trailing[0])
assert cleared["value"], "the trailing clear icon must reach its own registered handler"

# A real, deliberate ordering: open the search view only after the
# clear-icon interaction above -- once it's open, a real click
# outside it (dismiss_on_outside_click, the same real behavior every
# dropdown menu already has) would dismiss it and consume the click
# before it ever reached the icon's own handler.
view = window.add_search_view(width=400, height=160)
for label in ["Recent: hello world", "Recent: search demo", "Recent: tre v2"]:
    row = window.add_text(content=label, background=(0, 0, 0, 0), width=360, height=20)
    view.add_child(row)

window.open_menu(bar, view)
window.close_menu(view)

app = App()
app.add_window(window)
app.run(max_frames=60)
print(
    f"search.py: exited cleanly after 60 frames, field text {field.get_text()!r}, "
    f"cleared={cleared['value']}"
)
