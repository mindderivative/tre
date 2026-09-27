"""Style properties through `node.set`, `window.create`, and `animate`: layout
fields (size, padding, margin, gap, flex, alignment) are accepted and bad
alignment names rejected; `stroke_color`/`stroke_width` set, animate, and
default to zero; a text node takes `line_height` and rejects the removed
`typography_role`. `test_layout_props.py` checks the values read back.
"""

import pytest

from tre import Node, Window
from helpers import add


# --- layout fields through node.set ---------------------------------------


def test_set_with_no_arguments_does_not_raise():
    window = Window(width=200, height=200)
    node = add(window, "box", fill=(0, 0, 0, 255), width=50, height=50)
    node.set()


def test_set_accepts_each_layout_field_individually():
    window = Window(width=200, height=200)
    node = add(window, "box", fill=(0, 0, 0, 255), width=50, height=50)
    node.set(width=120.0)
    node.set(height=80.0)
    node.set(padding=8.0)
    node.set(gap=4.0)


def test_set_accepts_layout_fields_together():
    window = Window(width=200, height=200)
    node = add(window, "box", fill=(0, 0, 0, 255), width=50, height=50)
    node.set(width=100.0, height=60.0, padding=6.0, gap=2.0)


def test_set_can_be_called_repeatedly():
    window = Window(width=200, height=200)
    node = add(window, "box", fill=(0, 0, 0, 255), width=50, height=50)
    node.set(width=100.0)
    node.set(width=150.0)
    node.set(height=90.0)


def test_set_accepts_per_side_padding_and_margin_individually():
    window = Window(width=200, height=200)
    node = add(window, "box", fill=(0, 0, 0, 255), width=50, height=50)
    node.set(padding_top=4.0)
    node.set(padding_right=4.0)
    node.set(padding_bottom=4.0)
    node.set(padding_left=4.0)
    node.set(margin=2.0)
    node.set(margin_top=1.0)
    node.set(margin_right=1.0)
    node.set(margin_bottom=1.0)
    node.set(margin_left=1.0)


def test_set_per_side_padding_layers_on_top_of_the_uniform_value():
    """A per-side value alongside the uniform `padding`/`margin` is
    accepted (per-side wins for its side; `test_layout_props.py` reads
    that back)."""
    window = Window(width=200, height=200)
    node = add(window, "box", fill=(0, 0, 0, 255), width=50, height=50)
    node.set(padding=8.0, padding_top=2.0, margin=4.0, margin_left=1.0)


def test_set_accepts_flex_grow_shrink_and_basis():
    window = Window(width=200, height=200)
    node = add(window, "box", fill=(0, 0, 0, 255), width=50, height=50)
    node.set(flex_grow=1.0, flex_shrink=0.0, flex_basis=40.0)


@pytest.mark.parametrize(
    "value",
    ["start", "end", "flex_start", "flex_end", "center", "baseline", "stretch"],
)
def test_set_accepts_every_align_items_value(value):
    window = Window(width=200, height=200)
    node = add(window, "box", fill=(0, 0, 0, 255), width=50, height=50)
    node.set(align_items=value)


@pytest.mark.parametrize(
    "value",
    [
        "start",
        "end",
        "flex_start",
        "flex_end",
        "center",
        "stretch",
        "space_between",
        "space_around",
        "space_evenly",
    ],
)
def test_set_accepts_every_justify_content_value(value):
    window = Window(width=200, height=200)
    node = add(window, "box", fill=(0, 0, 0, 255), width=50, height=50)
    node.set(justify_content=value)


def test_set_rejects_an_unknown_align_items_value():
    window = Window(width=200, height=200)
    node = add(window, "box", fill=(0, 0, 0, 255), width=50, height=50)
    with pytest.raises(ValueError, match="align_items"):
        node.set(align_items="sideways")


def test_set_rejects_an_unknown_justify_content_value():
    window = Window(width=200, height=200)
    node = add(window, "box", fill=(0, 0, 0, 255), width=50, height=50)
    with pytest.raises(ValueError, match="justify_content"):
        node.set(justify_content="sideways")


# --- stroke via animate()/get() -----------------------------------------


def test_animate_accepts_stroke_color_and_stroke_width():
    window = Window(width=200, height=200)
    node = add(window, "box", fill=(0, 0, 0, 255), width=50, height=50)
    window.advance(0)
    node.animate("stroke_color", (255, 0, 0, 255), duration_ms=0)
    node.animate("stroke_width", 2.0, duration_ms=0)
    window.advance(1)
    assert node.get("stroke_color") == (255, 0, 0, 255)
    assert node.get("stroke_width") == pytest.approx(2.0)


def test_stroke_width_defaults_to_zero():
    window = Window(width=200, height=200)
    node = add(window, "box", fill=(0, 0, 0, 255), width=50, height=50)
    assert node.get("stroke_width") == pytest.approx(0.0)


def test_stroke_color_requires_a_four_tuple_not_a_float():
    window = Window(width=200, height=200)
    node = add(window, "box", fill=(0, 0, 0, 255), width=50, height=50)
    with pytest.raises(ValueError, match="must be an \\(r, g, b, a\\) tuple"):
        node.animate("stroke_color", 1.0)


# --- stroke at construction ---------------------------------------------


def test_create_box_accepts_stroke():
    window = Window(width=200, height=200)
    node = add(window, "box", fill=(0, 0, 0, 255), width=50, height=50, stroke_color=(255, 255, 255, 255), stroke_width=1.5)
    assert isinstance(node, Node)
    assert node.get("stroke_width") == pytest.approx(1.5)


# --- text line_height ------------------------------------------------------


def test_create_text_accepts_line_height():
    # `engine-render`'s text tests check that line_height widens the
    # per-line advance.
    window = Window(width=200, height=200)
    node = add(window, "text", text="Hello", fill=(0, 0, 0, 0), width=100, height=40, line_height=1.5)
    assert isinstance(node, Node)


def test_create_text_without_line_height():
    # Omitted, the font's natural metrics apply.
    window = Window(width=200, height=200)
    node = add(window, "text", text="Hello", fill=(0, 0, 0, 0), width=100, height=40)
    assert isinstance(node, Node)


# --- removed props -----------------------------------------------------------


def test_create_text_rejects_typography_role():
    window = Window(width=200, height=200)
    with pytest.raises(ValueError, match="typography_role"):
        add(window, "text", text="Heading", fill=(0, 0, 0, 0), width=200, height=40, typography_role="headline_small")
