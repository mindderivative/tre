#!/usr/bin/env python3
"""M53 (§8, §10, §11.3, §16.7): a real Copy/Cut/Paste/Select All
context menu on both `TextField` and `CodeEditor` -- the real
capability this milestone exists to build.

Before M53, every generic interaction primitive (`set_on_click`/
`enable_interaction`/`set_context_menu`) already worked on these two
node kinds, and a real right-click already opened whatever context
menu was attached, live, in the real winit event loop (confirmed by
this milestone's own investigation -- see `BUILD_TRACKER.md`'s M53
section). What was actually missing: right-click never focused the
field a menu was about to act on (Phase 1), there was no real,
callable path to the actual OS clipboard for a menu item's own
`on_click` to call at all (`Window.copy`/`cut`/`paste` are deliberately
hermetic -- Phase 2's own new `copy_to_system_clipboard`/`cut_to_
system_clipboard`/`paste_from_system_clipboard` are the real ones), and
no `select_all` existed anywhere.

Real, deliberate design decision, not glossed over: `add_text_field`/
`add_code_editor` do not call `enable_interaction()` themselves --
every composite `add_*` factory in this catalog stays opt-in
(`add_card`'s own doc comment states the identical real reason, and
auto-enabling it here would be a real, unscoped behavior change for
every existing call site). This script calls it explicitly on each
field so a human running it locally sees a real focus ring/hover
state, not just the mechanically-correct-but-invisible focus a plain
click already gives for free.

What this script proves automatically (headless-CI-safe, no human
needed): a real right-click on each field opens its own real Copy/Cut/
Paste/Select All menu -- the same functional proof `tests/test_
context_menu.py` already establishes (click a menu item, confirm *its
own* handler fired, only possible if the menu genuinely attached to
the live tree). `Select All`'s own real effect is verified through the
existing, always-real, hermetic `Window.copy()` -- deliberately not
through the new OS-touching methods, which stay environment-dependent
(some sandboxed CI environments have no reachable clipboard service at
all, the same real, honest tolerance `tests/test_clipboard.py`'s own
new coverage already establishes). The real OS clipboard write/read is
demonstrated on the field's own menu, reporting success or a real,
graceful "no clipboard reachable here" -- never a hard failure.
"""

from tre import App, Window

window = Window(width=420, height=280, title="tre v2 -- text field context menu")
window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF))

activity: list[str] = []


def build_edit_menu(name, on_action):
    """One real Copy/Cut/Paste/Select All menu, built from the existing
    `add_menu_item`/`build_menu` primitives -- the real, composable
    pattern this milestone's own investigation confirmed (no packaged
    context-menu factory exists, deliberately -- see `PLAN.md`'s own
    "explicitly out of scope"), the identical real shape every other
    menu in this catalog already uses (`examples/menu.py`). Returns the
    menu panel plus a `{action: Node}` map so the caller can click a
    specific item directly by name for its own verification, the same
    real pattern `tests/test_context_menu.py` already establishes.
    """
    actions = ["copy", "cut", "paste", "select_all"]
    labels = ["Copy", "Cut", "Paste", "Select All"]
    items = [window.add_menu_item(label=label) for label in labels]
    for item, action in zip(items, actions):
        item.enable_interaction()
        item.set_on_click(lambda action=action: on_action(name, action))
    menu = window.build_menu(items, width=180)
    return menu, dict(zip(actions, items))


def handle_action(name, action):
    activity.append(f"{name}:{action}")
    if action == "copy":
        ok = window.copy_to_system_clipboard()
        print(f"{name}: real Copy {'succeeded' if ok else 'had no real clipboard to write to'}")
    elif action == "cut":
        ok = window.cut_to_system_clipboard()
        print(f"{name}: real Cut {'succeeded' if ok else 'had no real clipboard to write to'}")
    elif action == "paste":
        ok = window.paste_from_system_clipboard()
        print(f"{name}: real Paste {'succeeded' if ok else 'had no real clipboard to read from'}")
    elif action == "select_all":
        window.select_all()


field = window.add_text_field(
    background=(0xFF, 0xFF, 0xFF, 0xFF),
    width=260,
    height=40,
    content="Right-click me",
    x=20,
    y=20,
)
field.enable_interaction()
field_menu, field_items = build_edit_menu("field", handle_action)
field.set_context_menu(field_menu)

editor = window.add_code_editor(
    content="def hello():\n    print('hi')\n",
    background=(0xF5, 0xF5, 0xF5, 0xFF),
    width=260,
    height=140,
    x=20,
    y=90,
)
editor.enable_interaction()
editor_menu, editor_items = build_edit_menu("editor", handle_action)
editor.set_context_menu(editor_menu)

# A real, neutral spot to click for menu dismissal below -- this
# engine's own `dismiss_overlays_outside` (tree.rs) treats a press
# outside every open menu as a dismissal of it, consuming that press
# rather than also opening a *different* menu in the same click (the
# same real convention `tests/test_context_menu.py`'s own `test_a_
# real_click_outside_an_open_context_menu_dismisses_it` establishes).
# Right-clicking the editor while the field's menu is still open would
# otherwise be swallowed as that dismissal instead of opening the
# editor's own menu -- a real gotcha this example demonstrates working
# around the same tested way: an explicit outside click first. Placed
# in its own right-hand gutter column (x=300), clear of both fields
# (which end at x=280) and of either menu (`build_menu`'s own panel is
# anchored at its trigger's own x, width=180, so it never passes
# x=200) -- a real spot neither menu, nor `field`/`editor` themselves,
# ever covers.
elsewhere = window.add_rect(background=(0xEE, 0xEE, 0xEE, 0xFF), width=100, height=240, x=300, y=20)

# Real, functional, headless-CI-safe proof: a real right-click on each
# field focuses it (Phase 1) and opens its own real menu -- clicking
# "Select All" reaches that item's own registered handler (only
# possible if open_overlay genuinely attached the menu to the live
# tree), and its real effect is confirmed through the always-real,
# hermetic Window.copy() -- deliberately not the new OS-touching
# methods, which stay environment-dependent.
window.right_click(field)
window.click(field_items["select_all"])
assert activity[-1] == "field:select_all"
assert window.copy() == "Right-click me", "Select All must select the field's entire real content"

window.click(elsewhere)  # dismiss the field's still-open menu first
window.right_click(editor)
window.click(editor_items["select_all"])
assert activity[-1] == "editor:select_all"
assert window.copy() == "def hello():\n    print('hi')\n", (
    "Select All must select the editor's entire real multi-line content"
)

# Demonstrates the real, non-hermetic Copy/Cut/Paste path on the
# field's own menu -- reports success or a real, graceful "no
# clipboard reachable" rather than asserting a specific outcome, since
# real OS clipboard reachability is genuinely environment-dependent.
window.click(elsewhere)  # dismiss the editor's still-open menu first
window.right_click(field)
window.click(field_items["copy"])

app = App()
app.add_window(window)
app.run(max_frames=60)
print(f"text_field_context_menu.py: exited cleanly after 60 frames, activity={activity!r}")
