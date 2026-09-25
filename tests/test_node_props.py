"""M94 Phase 2: `node.set`/`node.get` for accessibility, focus, and
interaction properties; `node.focus()`; Tab order; click-to-focus; and the
`a11y_action` event.
"""

from __future__ import annotations

from typing import Any

import pytest

import tre

BLACK = (0, 0, 0, 255)


def window() -> tre.Window:
    return tre.Window(400, 300, "props")


def test_set_and_get_round_trip_every_property() -> None:
    w = window()
    node = w.add_rect(BLACK, 100, 20)
    values: dict[str, Any] = {
        "role": "slider",
        "label": "Volume",
        "value": 3.0,
        "value_min": 0.0,
        "value_max": 10.0,
        "value_step": 0.5,
        "checked": True,
        "selected": False,
        "expanded": True,
        "disabled": True,
        "level": 2,
        "live": "polite",
        "a11y_hidden": True,
        "focusable": True,
        "tab_index": 3,
        "cursor": "grab",
        "hit_testable": False,
    }
    node.set(**values)
    assert {name: node.get(name) for name in values} == values


def test_optional_properties_clear_with_none() -> None:
    w = window()
    node = w.add_rect(BLACK, 40, 40)
    node.set(label="Hi", value="text", checked=True, cursor="pointer", live="assertive")
    node.set(label=None, value=None, checked=None, cursor=None, live=None)
    assert [node.get(n) for n in ("label", "value", "checked", "cursor", "live")] == [
        None
    ] * 5


def test_set_is_atomic() -> None:
    w = window()
    node = w.add_rect(BLACK, 40, 40)
    node.set(label="Before")
    with pytest.raises(ValueError, match="`level` must be a positive int"):
        node.set(label="After", level=0)
    assert node.get("label") == "Before"


@pytest.mark.parametrize(
    ("props", "message"),
    [
        ({"colour": 1}, "settable: role, label"),
        ({"role": "widget"}, "one of: button, checkbox"),
        ({"cursor": "hand"}, "one of: default, pointer"),
        ({"live": "loud"}, "one of: off, polite, assertive"),
        ({"focusable": 1}, "`focusable` must be a bool"),
        ({"value": True}, "a str, a number, or None"),
        ({"tab_index": "first"}, "`tab_index` must be an int"),
    ],
)
def test_set_rejects_bad_names_and_values(props: dict[str, Any], message: str) -> None:
    w = window()
    node = w.add_rect(BLACK, 40, 40)
    with pytest.raises(ValueError, match=message):
        node.set(**props)


def test_get_still_reads_animatable_numbers() -> None:
    w = window()
    node = w.add_rect(BLACK, 40, 40)
    assert node.get("opacity") == 1.0


def test_value_on_a_built_in_slider_stays_its_numeric_value() -> None:
    w = window()
    slider = w.add_slider(BLACK, 200, 24, value=0.25)
    assert slider.get("value") == pytest.approx(0.25)


def test_focus_moves_focus_and_fires_listeners() -> None:
    w = window()
    node = w.add_rect(BLACK, 40, 40)
    node.set(focusable=True)
    seen: list[str] = []
    node.on("focus", lambda e: seen.append(e.type))
    node.focus()
    assert node.get("focused") is True
    assert seen == ["focus"]


def test_tab_order_follows_tab_index() -> None:
    w = window()
    a, b, c = (w.add_rect(BLACK, 40, 40) for _ in range(3))
    for node in (a, b, c):
        node.set(focusable=True)
    c.set(tab_index=1)
    b.set(tab_index=-1)
    visited: list[str] = []
    for tag, node in (("a", a), ("b", b), ("c", c)):
        node.on("focus", lambda e, tag=tag: visited.append(tag))
    for _ in range(3):
        w.simulate("key_down", key="tab")
    assert visited == ["c", "a", "c"]


def test_a_click_focuses_the_nearest_focusable_ancestor() -> None:
    w = window()
    button = w.add_rect(BLACK, 100, 40)
    inner = w.add_rect(BLACK, 20, 20)
    button.add_child(inner)
    button.set(focusable=True, tab_index=-1)
    w.simulate("click", node=inner)
    assert button.get("focused") is True


def test_a_plain_box_is_not_focusable_by_default() -> None:
    w = window()
    node = w.add_rect(BLACK, 40, 40)
    assert node.get("focusable") is False
    w.simulate("click", node=node)
    assert node.get("focused") is False


def test_a11y_actions_bubble_with_their_value() -> None:
    w = window()
    group = w.add_rect(BLACK, 200, 100)
    slider = w.add_rect(BLACK, 100, 20)
    group.add_child(slider)
    slider.set(role="slider", value=1.0, value_min=0.0, value_max=10.0)
    seen: list[tuple[str | None, Any, bool]] = []
    group.on("a11y_action", lambda e: seen.append((e.action, e.value, e.target == slider)))
    w.simulate("a11y_action", node=slider, action="increment")
    w.simulate("a11y_action", node=slider, action="set_value", value=7.5)
    assert seen == [("increment", None, True), ("set_value", 7.5, True)]


def test_simulate_rejects_an_unknown_a11y_action() -> None:
    w = window()
    node = w.add_rect(BLACK, 40, 40)
    with pytest.raises(ValueError, match="valid actions: increment"):
        w.simulate("a11y_action", node=node, action="activate")
