"""M96 Phase 4: layers -- `window.show_layer`/`hide_layer`, the one mechanism
dialogs, menus, tooltips, snackbars, and sheets are built from: stacking,
anchored placement that flips and shifts to fit, modal blocking with a focus
trap and focus restore, each layer its own focus scope, events stopping at
the layer, and `dismiss` on an outside press or Escape.
"""

from __future__ import annotations

from typing import Any

import pytest

import tre

W, H = 400, 300


def window() -> tre.Window:
    w = tre.Window(W, H, "layers")
    w.root.set(padding=0, gap=0)
    return w


def box(w: tre.Window, **props: Any) -> tre.Node:
    return w.create("box", **props)


def button(w: tre.Window, **props: Any) -> tre.Node:
    return w.create("box", **{"width": 60, "height": 30, "focusable": True, "role": "button", **props})


def clicks(node: tre.Node, log: list[str], name: str) -> None:
    node.on("click", lambda: log.append(name))


def test_a_layer_sits_over_the_content_and_out_of_its_structure() -> None:
    w = window()
    page = box(w, width=W, height=H)
    w.root.add_child(page)
    sheet = box(w, width=100, height=100, x=0, y=0)
    w.show_layer(sheet, dismissible=False)
    log: list[str] = []
    clicks(page, log, "page")
    clicks(sheet, log, "sheet")
    w.simulate("click", x=50, y=50)
    assert log == ["sheet"]
    assert w.root.children() == [page], "a layer isn't content"
    assert sheet.parent() is None
    later = box(w, width=W, height=H)
    w.root.insert_child(0, later)
    w.root.add_child(box(w, width=10, height=10))
    w.simulate("click", x=50, y=50)
    assert log == ["sheet", "sheet"], "content added later stays beneath"
    clicks(later, log, "later")
    w.hide_layer(sheet)
    w.simulate("click", x=50, y=50)
    assert log[-1] == "later", "hidden: the content beneath is hit again"


def test_layers_stack_in_show_order() -> None:
    w = window()
    first = box(w, width=100, height=100, x=0, y=0)
    second = box(w, width=100, height=100, x=50, y=50)
    w.show_layer(first, dismissible=False)
    w.show_layer(second, dismissible=False)
    log: list[str] = []
    clicks(first, log, "first")
    clicks(second, log, "second")
    w.simulate("click", x=75, y=75)
    assert log == ["second"]


def test_an_anchored_layer_flips_and_shifts_to_fit() -> None:
    w = window()
    trigger = button(w, position="absolute", x=350, y=260)
    w.root.add_child(trigger)
    menu = box(w, width=120, height=90)
    w.show_layer(menu, anchor=trigger, placement="below")
    assert menu.get("layer_placement") == "above", "no room below"
    assert menu.get("layout_y") == 260 - 90
    assert menu.get("layout_x") == W - 120, "shifted left to fit"
    trigger.set(y=20)
    assert menu.get("layer_placement") == "below", "placed again at every layout"
    assert menu.get("layout_y") == 50


def test_dismiss_on_outside_press_and_escape() -> None:
    w = window()
    background = box(w, width=W, height=H)
    w.root.add_child(background)
    menu = box(w, width=100, height=100, x=0, y=0)
    tip = box(w, width=50, height=50, x=300, y=0)
    w.show_layer(menu)
    w.show_layer(tip, dismissible=False)
    log: list[str] = []
    menu.on("dismiss", lambda e: log.append(f"menu {e.type}"))
    tip.on("dismiss", lambda: log.append("tip"))
    clicks(background, log, "background")
    clicks(menu, log, "menu click")
    w.simulate("click", x=50, y=50)
    assert log == ["menu click"], "inside the menu: no dismiss"
    w.simulate("click", x=200, y=200)
    assert log[1:] == ["menu dismiss"], "outside: dismissed, the press consumed"
    w.simulate("key_down", key="escape")
    assert log[2:] == ["menu dismiss"], "Escape: the topmost dismissible layer"
    w.simulate("click", x=50, y=50)
    assert log[-1] == "menu click", "still shown: closing is the framework's call"


def test_a_press_in_a_submenu_keeps_its_parent_open() -> None:
    w = window()
    menu = box(w, width=100, height=100, x=0, y=0)
    submenu = box(w, width=100, height=100, x=150, y=0)
    w.show_layer(menu)
    w.show_layer(submenu)
    log: list[str] = []
    menu.on("dismiss", lambda: log.append("menu"))
    submenu.on("dismiss", lambda: log.append("submenu"))
    w.simulate("click", x=200, y=50)
    assert log == []
    w.simulate("click", x=50, y=50)
    assert log == ["submenu"], "a press in the parent dismisses only the child"


def test_keys_bubble_to_the_layer_and_stop() -> None:
    w = window()
    field = w.create("text_input", width=100, height=30)
    panel = box(w, width=200, height=100, x=0, y=0)
    panel.add_child(field)
    w.show_layer(panel, dismissible=False)
    log: list[str] = []
    w.root.on("key_down", lambda: log.append("root"))
    panel.on("key_down", lambda: log.append("panel"))
    w.simulate("focus", node=field)
    w.simulate("key_down", key="a")
    assert log == ["panel"]


def test_proof_modal_dialog() -> None:
    """A confirmation dialog from primitives: a window-sized scrim box with
    a centered panel and two buttons, shown modal."""
    w = window()
    trigger = button(w)
    behind = button(w)
    w.root.add_child(trigger)
    w.root.add_child(behind)
    log: list[str] = []
    clicks(behind, log, "behind")

    scrim = box(w, width="100%", height="100%", fill=(0, 0, 0, 82),
                align_items="center", justify_content="center")
    panel = box(w, width=240, height=120, fill=(255, 251, 254, 255),
                corner_radius=28, role="dialog", label="Discard draft?")
    cancel, discard = button(w), button(w)
    panel.add_child(cancel)
    panel.add_child(discard)
    scrim.add_child(panel)

    def close() -> None:
        w.hide_layer(scrim)
        log.append("closed")

    scrim.on("dismiss", close)
    w.simulate("focus", node=trigger)
    w.show_layer(scrim, modal=True)
    assert cancel.get("focused") is True, "focus moves into the dialog"

    w.simulate("key_down", key="tab")
    assert discard.get("focused") is True
    w.simulate("key_down", key="tab")
    assert cancel.get("focused") is True, "Tab stays inside"

    w.simulate("click", node=behind)
    assert "behind" not in log, "the page beneath is blocked"

    w.simulate("key_down", key="escape")
    assert log[-1] == "closed"
    assert trigger.get("focused") is True, "focus returns to the trigger"
    assert scrim.parent() is None


def test_proof_anchored_menu() -> None:
    """A dropdown menu from primitives: three items under an anchor button,
    dismissed by an outside press, an item's own click still working."""
    w = window()
    anchor = button(w)
    w.root.add_child(anchor)
    menu = box(w, width=160, flex_direction="vertical", role="menu")
    items = [button(w, width=160, height=40, role="menuitem") for _ in range(3)]
    for item in items:
        menu.add_child(item)
    log: list[str] = []
    clicks(items[1], log, "item 1")

    def dismissed() -> None:
        w.hide_layer(menu)
        log.append("dismissed")

    menu.on("dismiss", dismissed)
    w.show_layer(menu, anchor=anchor, placement="below")
    assert menu.get("layer_placement") == "below"
    assert menu.get("layout_y") == anchor.get("layout_y") + anchor.get("layout_height")
    w.simulate("click", node=items[1])
    assert log == ["item 1"]
    w.simulate("click", x=W - 5, y=H - 5)
    assert log == ["item 1", "dismissed"]
    assert menu.parent() is None


def test_show_layer_rejects_bad_arguments() -> None:
    w = window()
    with pytest.raises(ValueError, match="`placement` must be one of: below, above"):
        w.show_layer(box(w), placement="left")
    with pytest.raises(ValueError, match="isn't a shown layer"):
        w.hide_layer(box(w))
