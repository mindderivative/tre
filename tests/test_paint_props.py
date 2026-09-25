"""M95 Phase 2: the target API's paint names on every node, per-corner
radii, shadows, easing, `get_target`/`stop_animation`, and the
theme-free text-input, scroll-view, and terminal colors -- through
`set`, `get`, and `animate`. What they paint is covered by
`engine-render`'s own pixel tests.
"""

from __future__ import annotations

from typing import Any

import pytest

import tre

RED = (255, 0, 0, 255)


def window() -> tre.Window:
    return tre.Window(300, 200, "paint")


def test_paint_props_read_back_exactly() -> None:
    w = window()
    values: dict[str, Any] = {
        "fill": (10, 20, 30, 128),
        "stroke_color": (1, 2, 3, 4),
        "stroke_width": 2.0,
        "opacity": 0.5,
        "corner_radius": (1.0, 2.0, 3.0, 4.0),
        "shadows": [((0, 0, 0, 60), 0.0, 2.0, 4.0, 1.0), ((0, 0, 0, 30), 1.0, 1.0, 0.0, 0.0)],
    }
    box = w.create("box", **values)
    assert {name: box.get(name) for name in values} == values


def test_corner_radius_is_one_number_or_four() -> None:
    w = window()
    box = w.create("box", corner_radius=(1, 2, 3, 4))
    box.set(corner_radius=8)
    assert box.get("corner_radius") == 8.0


def test_fill_on_an_icon_is_its_tint() -> None:
    w = window()
    icon = w.add_icon("home", (0, 0, 0, 255), 24)
    icon.set(fill=RED)
    assert icon.get("fill") == RED


def test_animate_targets_and_stop() -> None:
    w = window()
    box = w.create("box", fill=(0, 0, 0, 255))
    box.animate("fill", RED, 300, easing=(0.2, 0.0, 0.0, 1.0))
    assert box.get_target("fill") == RED
    assert box.get("fill") == (0, 0, 0, 255)
    box.stop_animation("fill")
    assert box.get_target("fill") == (0, 0, 0, 255)

    box.animate("corner_radius", (4, 4, 0, 0), 100)
    assert box.get_target("corner_radius") == (4.0, 4.0, 0.0, 0.0)
    box.animate("shadows", [((0, 0, 0, 80), 0, 4, 8, 0)], 100)
    assert box.get_target("shadows") == [((0, 0, 0, 80), 0.0, 4.0, 8.0, 0.0)]


def test_animate_completion_is_positional_after_easing() -> None:
    w = window()
    box = w.create("box")
    box.animate("opacity", 0.5, 100, "linear", lambda: None)


@pytest.mark.parametrize(
    "easing", ["ease", (2.0, 0.0, 0.0, 1.0), (0.1, 0.2, 0.3)]
)
def test_animate_rejects_bad_easing(easing: Any) -> None:
    w = window()
    box = w.create("box")
    with pytest.raises(ValueError, match="easing must be"):
        box.animate("fill", RED, 100, easing=easing)


def test_get_target_and_stop_only_take_animatable_names() -> None:
    w = window()
    box = w.create("box")
    with pytest.raises(ValueError, match="isn't animatable"):
        box.get_target("role")
    with pytest.raises(ValueError, match="isn't animatable"):
        box.stop_animation("label")


@pytest.mark.parametrize(
    ("props", "message"),
    [
        ({"fill": (1, 2, 3)}, "an \\(r, g, b, a\\) tuple"),
        ({"stroke_width": -1}, "non-negative"),
        ({"opacity": 1.5}, "from 0.0 to 1.0"),
        ({"corner_radius": (1, 2)}, "top_left, top_right"),
        ({"shadows": [(1, 2)]}, "list of \\(color, offset_x"),
        ({"placeholder": "x"}, "applies only to a text_input node"),
        ({"scrollbar_width": 3}, "applies only to a scroll_view node"),
        ({"palette": {}}, "applies only to a terminal node"),
    ],
)
def test_paint_props_reject_bad_values(props: dict[str, Any], message: str) -> None:
    w = window()
    box = w.create("box")
    with pytest.raises(ValueError, match=message):
        box.set(**props)


def test_text_input_colors_placeholder_and_obscured() -> None:
    w = window()
    field = w.add_text_field((255, 255, 255, 255), 150, 30)
    field.set(
        placeholder="Search",
        placeholder_fill=(1, 1, 1, 100),
        caret_color=(200, 0, 0, 255),
        selection_fill=(0, 0, 255, 80),
        obscured=True,
    )
    assert [
        field.get(n)
        for n in ("placeholder", "placeholder_fill", "caret_color", "selection_fill", "obscured")
    ] == ["Search", (1, 1, 1, 100), (200, 0, 0, 255), (0, 0, 255, 80), True]
    field.set(placeholder_fill=None, caret_color=None, selection_fill=None)
    assert field.get("caret_color") is None


def test_an_obscured_field_never_copies_or_cuts() -> None:
    w = window()
    field = w.add_text_field((255, 255, 255, 255), 150, 30)
    field.set(obscured=True)
    w.simulate("focus", node=field)
    w.simulate("input", text="secret")
    w.select_all()
    w.cut()
    assert field.get_text() == "secret"


def test_scroll_view_scrollbar_props() -> None:
    w = window()
    view = w.add_scroll_view(100, 100)
    view.set(scrollbar_fill=RED, scrollbar_width=8)
    assert (view.get("scrollbar_fill"), view.get("scrollbar_width")) == (RED, 8.0)
    view.set(scrollbar_fill=None)
    assert view.get("scrollbar_fill") is None


def test_terminal_palette_merges_the_given_keys() -> None:
    w = window()
    terminal = w.add_terminal("/bin/sh", 20, 5, (0, 0, 0, 255))
    before = terminal.get("palette")
    assert len(before["ansi"]) == 16
    terminal.set(palette={"foreground": RED, "ansi": [RED] * 16})
    after = terminal.get("palette")
    assert after["foreground"] == RED
    assert after["ansi"] == [RED] * 16
    assert after["cursor"] == before["cursor"]
    with pytest.raises(ValueError, match="exactly 16 colors"):
        terminal.set(palette={"ansi": [RED]})
    with pytest.raises(ValueError, match="a dict with any of"):
        terminal.set(palette={"accent": RED})
