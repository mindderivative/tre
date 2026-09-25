"""M96 Phase 2: `window.create` for text, text input, image, scroll view, and
terminal nodes, and each kind's own `set`/`get` properties.
"""

from __future__ import annotations

from typing import Any

import pytest

import tre

RED = (255, 0, 0, 255)


def window() -> tre.Window:
    return tre.Window(400, 300, "kinds")


def test_create_lists_its_kinds_and_required_props() -> None:
    w = window()
    with pytest.raises(ValueError, match="create builds: box, text, text_input"):
        w.create("label")
    with pytest.raises(ValueError, match=r'create\("text"\) needs `text`'):
        w.create("text")
    with pytest.raises(ValueError, match=r'create\("image"\) needs `rgba`'):
        w.create("image")
    with pytest.raises(ValueError, match=r'create\("terminal"\) needs `shell`'):
        w.create("terminal", cols=10, rows=2)


def test_text_props_read_back_and_default_to_black() -> None:
    w = window()
    text = w.create("text", text="Hello")
    assert text.get("fill") == (0, 0, 0, 255)
    values: dict[str, Any] = {
        "text": "Bonjour",
        "font_family": "Roboto",
        "font_weight": 500.0,
        "font_size": 14.0,
        "line_height": 1.5,
        "text_align": "center",
    }
    text.set(**values)
    assert {name: text.get(name) for name in values} == values


def test_kind_props_are_checked_against_the_kind() -> None:
    w = window()
    with pytest.raises(ValueError, match="`text_align` applies only to a text node"):
        w.create("text_input").set(text_align="center")
    with pytest.raises(ValueError, match="`multiline` applies only to a text_input node"):
        w.create("text", text="x").set(multiline=True)
    with pytest.raises(ValueError, match="`text` applies only to a text or text_input node"):
        w.create("box").get("text")


def test_text_input_props() -> None:
    w = window()
    field = w.create("text_input", text="hello world", selection=(0, 5), multiline=True)
    assert field.get("text") == "hello world"
    assert field.get("selection") == (0, 5)
    assert field.get("multiline") is True
    field.set(
        show_whitespace=True,
        syntax_spans=[(0, 5, RED)],
        folded_ranges=[(6, 11)],
    )
    assert field.get("syntax_spans") == [(0, 5, RED)]
    assert field.get("folded_ranges") == [(6, 11)]
    field.set(text="hi")
    assert field.get("selection") == (2, 2), "new text puts the caret at its end"


def test_selection_is_checked_against_the_text_set_with_it() -> None:
    w = window()
    field = w.create("text_input", text="abc")
    field.set(text="abcdef", selection=(1, 6))
    assert field.get("selection") == (1, 6)
    with pytest.raises(ValueError, match="start <= end <= 3"):
        field.set(text="xyz", selection=(0, 4))
    assert field.get("text") == "abcdef", "a failed set changes nothing"


def test_a_text_inputs_fill_is_its_text_color_and_animates() -> None:
    w = window()
    w.advance(0)
    field = w.create("text_input")
    assert field.get("fill") == (0, 0, 0, 255)
    field.animate("fill", (200, 0, 0, 255), 100)
    w.advance(50)
    assert field.get("fill") == (100, 0, 0, 255)
    assert field.get_target("fill") == (200, 0, 0, 255)


def test_a_created_text_input_is_in_the_tab_order() -> None:
    w = window()
    a = w.create("text_input")
    b = w.create("text_input")
    w.root.add_child(a)
    w.root.add_child(b)
    w.simulate("focus", node=a)
    w.simulate("key_down", key="tab")
    assert b.get("focused") is True


def test_image_pixels_are_set_together() -> None:
    w = window()
    pixels = bytes([255, 0, 0, 255] * 4)
    image = w.create("image", rgba=pixels, pixel_width=2, pixel_height=2, fit="contain")
    assert (image.get("pixel_width"), image.get("pixel_height")) == (2, 2)
    assert image.get("rgba") == pixels
    assert image.get("fit") == "contain"
    with pytest.raises(ValueError, match="rgba has 4 bytes, but a 2x2 RGBA8 frame needs 16"):
        image.set(rgba=bytes(4), pixel_width=2, pixel_height=2)
    with pytest.raises(ValueError, match="given with `rgba`"):
        image.set(pixel_width=4)


def test_scroll_view_orientation_and_animated_offset() -> None:
    w = window()
    w.advance(0)
    view = w.create("scroll_view", orientation="horizontal", scroll_offset=10)
    assert (view.get("orientation"), view.get("scroll_offset")) == ("horizontal", 10.0)
    view.animate("scroll_offset", 110.0, 100)
    w.advance(50)
    assert view.get("scroll_offset") == pytest.approx(60.0)


def test_terminal_grid_props_and_its_box() -> None:
    w = window()
    term = w.create("terminal", shell="/bin/sh", cols=20, rows=4)
    w.root.add_child(term)
    assert (term.get("cols"), term.get("rows")) == (20, 4)
    width = term.get("layout_width")
    term.set(cols=40)
    # Layout rounds to whole pixels; a cell is a fractional width.
    assert term.get("layout_width") == pytest.approx(width * 2, abs=1), "the box follows the grid"
    term.set(selection=(0, 0, 1, 3))
    assert term.get("selection") == (0, 0, 1, 3)
    with pytest.raises(ValueError, match="inside the grid"):
        term.set(selection=(0, 0, 9, 0))
    with pytest.raises(ValueError, match="an int from 1 to 1000"):
        term.set(rows=0)


def test_a_canvas_draws_when_created_and_on_redraw() -> None:
    w = window()
    calls: list[str] = []

    def draw(painter: Any) -> None:
        calls.append("draw")
        painter.fill_rect(0, 0, 10, 10, RED)

    canvas = w.create("canvas", draw=draw, width=50, height=50)
    assert calls == ["draw"]
    canvas.redraw()
    assert calls == ["draw", "draw"]
    canvas.set(draw=lambda painter: calls.append("other"))
    assert calls[-1] == "other", "a new draw callback draws at once"
    assert callable(canvas.get("draw"))
    with pytest.raises(ValueError, match="`draw` must be a callable"):
        canvas.set(draw=3)
    with pytest.raises(ValueError, match="`draw` applies only to a canvas node"):
        w.create("box").set(draw=draw)


def list_window(**props: Any) -> tuple[tre.Window, tre.Node, list[int]]:
    w = window()
    built: list[int] = []

    def materialize(index: int) -> tre.Node:
        built.append(index)
        return w.create("text", text=f"row {index}")

    rows = w.create("virtual_list", materialize=materialize, width=200, height=100, **props)
    w.root.add_child(rows)
    w.advance(0)  # layout runs, and builds the visible rows
    return w, rows, built


def test_a_virtual_list_builds_only_its_visible_rows() -> None:
    w, rows, built = list_window(item_count=1000, item_extent=20)
    assert built == [0, 1, 2, 3, 4]
    assert len(rows.children()) == 5
    assert rows.children()[4].get("text") == "row 4"
    assert rows.children()[4].get("layout_width") == rows.get("layout_width")


def test_scrolling_a_virtual_list_builds_new_rows_and_releases_old_ones() -> None:
    w, rows, built = list_window(item_count=1000, item_extent=20)
    w.simulate("wheel", node=rows, delta_y=50)
    w.advance(0)
    assert built == [0, 1, 2, 3, 4, 5, 6, 7]
    assert [child.get("text") for child in rows.children()] == [
        "row 2", "row 3", "row 4", "row 5", "row 6", "row 7"
    ]


def test_a_virtual_list_sizes_rows_from_size_hint() -> None:
    w, rows, built = list_window(item_count=10, size_hint=lambda index: 30 + index * 10)
    # 30 + 40 + 50 = 120 > 100: rows 0-2 meet the viewport.
    assert built == [0, 1, 2]
    assert [child.get("layout_height") for child in rows.children()] == [30.0, 40.0, 50.0]


def test_changing_item_count_rebuilds_the_rows() -> None:
    w, rows, built = list_window(item_count=1000, item_extent=20)
    rows.set(item_count=2)
    w.advance(0)
    assert built == [0, 1, 2, 3, 4, 0, 1]
    assert rows.get("item_count") == 2
    assert len(rows.children()) == 2


def test_create_virtual_list_needs_one_extent() -> None:
    w = window()
    with pytest.raises(ValueError, match="exactly one of `item_extent` or `size_hint`"):
        w.create("virtual_list", item_count=3, materialize=lambda i: None)
