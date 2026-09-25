"""M97 Phase 2 Step 2: `docs/design/legacy-behavior.md` -- the page's
rebuild recipes, run against its own numbers, and its claims about the
legacy widgets checked against the widgets themselves.
"""

from __future__ import annotations

import re
from pathlib import Path
from typing import Any

import pytest

import tre

DOC = Path(__file__).resolve().parent.parent / "docs" / "design" / "legacy-behavior.md"
BLACK = (0, 0, 0, 255)


def recipes() -> dict[str, Any]:
    """The page's Python blocks, run -- the functions they define."""
    names: dict[str, Any] = {}
    for block in re.findall(r"```python\n(.*?)```", DOC.read_text(), re.S):
        exec(block, names)
    return names


def button(w: tre.Window) -> tre.Node:
    w.advance(0)
    node = w.create("box", width=100, height=40, corner_radius=20)
    w.root.add_child(node)
    return node


def test_the_state_layer_recipe_hovers_like_the_legacy_layer() -> None:
    w = tre.Window(200, 100, "hover")
    node = button(w)
    layer = recipes()["add_state_layer"](w, node, BLACK)
    assert node.children() == [layer]
    w.simulate("pointer_enter", node=node)
    w.advance(50)
    assert layer.get("opacity") == pytest.approx(0.04)
    w.advance(50)
    assert layer.get("opacity") == pytest.approx(0.08)
    w.simulate("pointer_leave")
    w.advance(100)
    assert layer.get("opacity") == pytest.approx(0.0)


def test_the_ripple_recipe_grows_and_fades_like_the_legacy_ripple() -> None:
    w = tre.Window(200, 100, "ripple")
    node = button(w)
    recipes()["add_state_layer"](w, node, BLACK)
    w.simulate("pointer_down", node=node, x=30, y=10)
    circle = node.children()[1]
    assert (circle.get("x"), circle.get("y")) == (-70, -90)
    assert (circle.get("scale"), circle.get("opacity")) == (0.0, 0.12)
    w.advance(150)  # half way: radius 50 of 100, opacity 0.06 of 0.12
    assert circle.get("scale") == pytest.approx(0.5)
    assert circle.get("opacity") == pytest.approx(0.06)
    w.advance(150)
    assert len(node.children()) == 1, "a finished ripple is gone"


def shadows(level: float) -> list[tuple[Any, ...]]:
    """engine-render's key and ambient shadow geometry, ported."""

    def c(v: float, hi: float = 1.0) -> float:
        return min(max(v, 0.0), hi)

    key_y = c(level) + c(level - 3) + 2 * c(level - 4)
    key_blur = 2 * c(level) + c(level - 2) + c(level - 4)
    amb_y = c(level) + c(level - 1) + 2 * c(level - 2, 3)
    amb_blur = 3 * c(level, 2) + 2 * c(level - 2, 3)
    amb_spread = c(level, 4) + 2 * c(level - 4)
    return [
        ((0, 0, 0, 77), 0, key_y, key_blur, 0),
        ((0, 0, 0, 38), 0, amb_y, amb_blur, amb_spread),
    ]


@pytest.mark.parametrize("level", [1, 2, 3, 4, 5])
def test_the_elevation_table_is_the_painters_formula(level: int) -> None:
    rows = [tuple(int(v) if isinstance(v, float) else v for v in s) for s in shadows(level)]
    assert f"| {level} | `{rows}` |" in DOC.read_text()


def test_the_engine_never_toggles_a_checkbox() -> None:
    w = tre.Window(100, 100, "checkbox")
    checkbox = w.add_checkbox(background=BLACK, width=18, height=18)
    w.simulate("click", node=checkbox)
    assert checkbox.get_checked() is False
    assert checkbox.get("check_progress") == 0.0


def test_slider_arrow_keys_step_a_twentieth() -> None:
    w = tre.Window(300, 100, "slider")
    slider = w.add_slider(background=BLACK, width=200, height=20, value=0.5)
    slider.focus()
    w.simulate("key_down", key="arrow_right")
    assert slider.get("value") == pytest.approx(0.55)
    w.simulate("key_down", key="arrow_left")
    w.simulate("key_down", key="arrow_left")
    assert slider.get("value") == pytest.approx(0.45)


def test_the_legacy_group_reflow_compounds_and_never_restores() -> None:
    """The page's stated bug: each layout pass while a child is held
    reflows again, and releasing leaves the widths where they ended."""
    w = tre.Window(600, 100, "group")
    _, children = w.add_button_group(labels=["A", "B", "C"], width=100, height=40)

    def widths() -> list[float]:
        return [c.get("layout_width") for c in children]

    assert widths() == [100, 100, 100]
    w.simulate("pointer_down", node=children[1])
    first = widths()
    second = widths()
    assert 100 < first[1] < second[1], "the pressed child keeps growing"
    assert sum(first) == sum(second) == 300, "the row keeps its width"
    w.simulate("pointer_up", node=children[1])
    assert widths() != [100, 100, 100]
