"""0.4.2 M11 (issue #23): CSS Grid through `node.set`/`node.get` -- the
container's tracks and the children's placements, read back as set, and the
layout Taffy computes from them.
"""

from __future__ import annotations

from typing import Any

import pytest

import tre


def window() -> tre.Window:
    return tre.Window(400, 300, "grid")


def grid(w: tre.Window, **props: Any) -> tre.Node:
    # align_self: the root would otherwise stretch it to its own height.
    props.setdefault("width", 300)
    node = w.create("box", display="grid", align_self="start", **props)
    w.root.add_child(node)
    return node


def cells(w: tre.Window, parent: tre.Node, count: int, **props: Any) -> list[tre.Node]:
    made = [w.create("box", height=20, **props) for _ in range(count)]
    for cell in made:
        parent.add_child(cell)
    return made


def x(node: tre.Node) -> float:
    return float(node.get("layout_x"))


def test_grid_properties_read_back_as_set() -> None:
    w = window()
    g = grid(
        w,
        grid_template_columns="200 1fr auto",
        grid_template_rows="repeat(2, minmax(40, 1fr))",
        grid_auto_rows="min_content",
        grid_auto_flow="column dense",
        row_gap=4,
        column_gap="5%",
        justify_items="center",
        align_content="space_between",
    )
    assert g.get("display") == "grid"
    assert g.get("grid_template_columns") == "200 1fr auto"
    assert g.get("grid_template_rows") == "repeat(2, minmax(40, 1fr))"
    assert g.get("grid_auto_rows") == "min_content"
    assert g.get("grid_auto_flow") == "column dense"
    assert (g.get("row_gap"), g.get("column_gap")) == (4.0, "5%")
    assert g.get("justify_items") == "center"
    assert g.get("align_content") == "space_between"

    child = w.create("box", grid_column="1 / 3", grid_row="span 2", justify_self="end")
    assert child.get("grid_column") == "1 / 3"
    assert child.get("grid_row") == "span 2"
    assert child.get("justify_self") == "end"
    child.set(grid_row=2)
    assert child.get("grid_row") == "2"


def test_a_track_list_can_be_a_list() -> None:
    g = grid(window(), grid_template_columns=[120, "1fr", "repeat(2, 40)"])
    assert g.get("grid_template_columns") == "120 1fr repeat(2, 40)"


def test_columns_split_the_width_as_their_tracks_say() -> None:
    w = window()
    g = grid(w, grid_template_columns="100 1fr")
    a, b = cells(w, g, 2)
    assert (a.get("layout_width"), b.get("layout_width")) == (100.0, 200.0)
    assert x(b) - x(a) == 100.0


def test_repeat_and_column_gap() -> None:
    w = window()
    g = grid(w, width=320, grid_template_columns="repeat(3, 1fr)", column_gap=10)
    made = cells(w, g, 3)
    assert [c.get("layout_width") for c in made] == [100.0, 100.0, 100.0]
    assert x(made[2]) - x(made[0]) == 220.0


def test_items_flow_into_rows_and_span_columns() -> None:
    w = window()
    g = grid(w, grid_template_columns="1fr 1fr", row_gap=6)
    first, second, third = cells(w, g, 3)
    assert first.get("layout_y") == second.get("layout_y")
    assert third.get("layout_y") == first.get("layout_y") + 20 + 6
    assert x(third) == x(first), "the third item starts the second row"

    third.set(grid_column="1 / 3")
    assert third.get("layout_width") == 300.0, "spans both columns"
    third.set(grid_column="span 2")
    assert third.get("layout_width") == 300.0


def test_an_explicit_line_places_a_child() -> None:
    w = window()
    g = grid(w, grid_template_columns="50 50 200")
    (child,) = cells(w, g, 1, grid_column=3)
    assert x(child) - x(g) == 100.0
    child.set(grid_column=-2)
    assert x(child) - x(g) == 100.0, "-2 is the last column's start line"


@pytest.mark.parametrize(
    ("props", "message"),
    [
        ({"display": "table"}, "one of: flex, grid"),
        ({"grid_template_columns": "1em"}, "isn't a track size"),
        ({"grid_template_columns": "minmax(1fr, 2fr)"}, "can't be in `fr`"),
        ({"grid_template_columns": [10, True]}, "track list"),
        ({"grid_auto_rows": "repeat(2, 1fr)"}, "isn't a track size"),
        ({"grid_column": 0}, "isn't a line"),
        ({"grid_row": "span 0"}, "at least 1"),
        ({"grid_auto_flow": "diagonal"}, "one of: row, column"),
    ],
)
def test_bad_grid_values_say_why(props: dict[str, Any], message: str) -> None:
    with pytest.raises(ValueError, match=message):
        window().create("box", **props)


def test_the_unknown_property_error_lists_the_grid_properties() -> None:
    with pytest.raises(ValueError, match="display, grid_template_columns"):
        window().create("box").set(colour=1)
