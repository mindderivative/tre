"""M96 Phase 2: the layout, visibility, order, and transform properties of
`node.set`/`node.get`, and the read-only computed box (`layout_x`,
`layout_y`, `layout_width`, `layout_height`).
"""

from __future__ import annotations

from typing import Any

import pytest

import tre


def window() -> tre.Window:
    return tre.Window(400, 300, "layout")


def attached(w: tre.Window, **props: Any) -> tre.Node:
    node = w.create("box", **props)
    w.root.add_child(node)
    return node


def test_layout_props_read_back_as_set() -> None:
    w = window()
    values: dict[str, Any] = {
        "width": "50%",
        "height": 40.0,
        "min_width": 10.0,
        "max_width": "90%",
        "min_height": "auto",
        "max_height": 200.0,
        "aspect_ratio": 1.5,
        "position": "absolute",
        "x": 12.0,
        "y": "auto",
        "flex_direction": "vertical",
        "flex_wrap": "wrap",
        "align_items": "center",
        "justify_content": "space_between",
        "align_self": "end",
        "gap": 8.0,
        "padding": 4.0,
        "margin": "auto",
        "flex_grow": 1.0,
        "flex_shrink": 0.0,
        "flex_basis": "25%",
        "visible": False,
        "z_index": 3,
        "clip_children": True,
        "translate_x": 5.0,
        "translate_y": -2.0,
        "scale": 1.25,
        "rotation_deg": 45.0,
    }
    box = w.create("box", **values)
    assert {name: box.get(name) for name in values} == values


def test_sides_read_back_as_one_value_or_four() -> None:
    w = window()
    box = w.create("box", padding=4, padding_left=10)
    assert box.get("padding") == (4.0, 4.0, 4.0, 10.0)
    assert box.get("padding_left") == 10.0
    box.set(margin_top=2)
    assert box.get("margin") == (2.0, 0.0, 0.0, 0.0)


@pytest.mark.parametrize(
    ("props", "message"),
    [
        ({"flex_wrap": "yes"}, "one of: no_wrap, wrap"),
        ({"align_self": "middle"}, "one of: start, end"),
        ({"width": -5}, "a number, \"auto\", or a percentage"),
        ({"aspect_ratio": 0}, "a positive number"),
        ({"z_index": 1.5}, "an int"),
        ({"visible": 1}, "a bool"),
        ({"scale": -1}, "non-negative"),
    ],
)
def test_layout_props_reject_bad_values(props: dict[str, Any], message: str) -> None:
    box = window().create("box")
    with pytest.raises(ValueError, match=message):
        box.set(**props)


def test_unknown_property_lists_layout_and_other_names() -> None:
    with pytest.raises(ValueError, match="settable: width, height.*visible"):
        window().create("box").set(colour=1)


def test_layout_box_is_computed_on_read() -> None:
    w = window()
    row = attached(w, width=300, height=100, gap=10, padding=0)
    a = w.create("box", width=50, height=20)
    b = w.create("box", width="25%", height=20)
    row.add_child(a)
    row.add_child(b)
    assert a.get("layout_width") == 50.0
    assert b.get("layout_width") == 75.0
    assert b.get("layout_x") - a.get("layout_x") == 60.0
    assert a.get("layout_y") == row.get("layout_y")


def test_aspect_ratio_and_min_max_bound_the_size() -> None:
    w = window()
    # align_self: the root would otherwise stretch it to its own height.
    box = attached(w, width=100, aspect_ratio=2.0, align_self="start")
    assert box.get("layout_height") == 50.0
    box.set(max_width=80)
    assert box.get("layout_width") == 80.0


def test_a_detached_subtree_lays_out_at_its_content_size() -> None:
    w = window()
    card = w.create("box", padding=5)
    card.add_child(w.create("box", width=40, height=30))
    assert (card.get("layout_width"), card.get("layout_height")) == (50.0, 40.0)


def test_proof_wrapped_chip_row() -> None:
    """A chip row wrapping onto a second line when the chips don't fit."""
    w = window()
    row = attached(w, width=200, flex_wrap="wrap", gap=8, align_self="start")
    chips = [w.create("box", width=60, height=24) for _ in range(4)]
    for chip in chips:
        row.add_child(chip)
    tops = [chip.get("layout_y") for chip in chips]
    assert tops[0] == tops[1] == tops[2], "three chips fit on the first line"
    assert tops[3] == tops[0] + 24 + 8, "the fourth wraps to the next line"
    assert chips[3].get("layout_x") == chips[0].get("layout_x")
    assert row.get("layout_height") == 24 * 2 + 8


def test_hidden_node_takes_no_space_and_is_never_hit() -> None:
    w = window()
    row = attached(w, width=300, height=50, padding=0, gap=0)
    a = w.create("box", width=50, height=50)
    b = w.create("box", width=50, height=50)
    row.add_child(a)
    row.add_child(b)
    clicks: list[str] = []
    a.on("click", lambda: clicks.append("a"))
    x = a.get("layout_x") + 25
    y = a.get("layout_y") + 25
    a.set(visible=False)
    assert b.get("layout_x") == row.get("layout_x"), "b moves into a's place"
    w.simulate("click", x=x, y=y)
    assert clicks == []


def test_higher_z_index_is_hit_first() -> None:
    w = window()
    stack = attached(w, width=100, height=100)
    under = w.create("box", position="absolute", x=0, y=0, width=100, height=100, z_index=1)
    over = w.create("box", position="absolute", x=0, y=0, width=100, height=100)
    stack.add_child(under)
    stack.add_child(over)
    hits: list[str] = []
    under.on("click", lambda: hits.append("under"))
    over.on("click", lambda: hits.append("over"))
    w.simulate("click", node=stack)
    assert hits == ["under"], "z_index beats child order"


def test_scale_changes_what_is_hit_about_the_center() -> None:
    w = window()
    holder = attached(w, width=200, height=200, padding=0)
    box = w.create("box", width=100, height=100)
    holder.add_child(box)
    hits: list[str] = []
    box.on("click", lambda: hits.append("box"))
    left = box.get("layout_x")
    top = box.get("layout_y")
    w.simulate("click", x=left + 120, y=top + 50)
    assert hits == []
    box.set(scale=1.5)  # grows 25 px each side about its center
    w.simulate("click", x=left + 120, y=top + 50)
    assert hits == ["box"]


def test_transform_parts_animate_independently() -> None:
    w = window()
    w.advance(0)
    box = w.create("box")
    box.animate("scale", 2.0, 1000)
    box.animate("translate_x", 100.0, 100)
    w.advance(100)
    assert box.get("translate_x") == 100.0
    assert box.get("scale") == pytest.approx(1.1)
    assert box.get_target("scale") == 2.0
