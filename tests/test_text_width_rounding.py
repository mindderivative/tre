"""0.5.3 (#98): a text node's explicit `width` is laid out rounded **up**.

Layout rounds every box to whole pixels. A text node given the width
`measure_text` returns (66.43 for "Add a task", Roboto 500 at 14 px) was laid
out 66 wide, and the text, which needs 66.43, wrapped onto a second line.
"""

import pytest

from tre import Window

STYLE = dict(font_size=14, font_weight=500)
WHITE = (255, 255, 255, 255)


def laid_out(window, **props):
    node = window.create("text", text="Add a task", fill=WHITE, **STYLE, **props)
    window.root.add_child(node)
    window.advance(16)
    return node


def test_the_measured_width_fits_after_layout():
    window = Window(width=300, height=100)
    measured_width, line = window.measure_text("Add a task", **STYLE)
    node = laid_out(window, width=measured_width, height=20)
    assert node.get("layout_width") == 67.0
    # What fits on one line at the laid-out width: the text is not wrapped.
    assert window.measure_text("Add a task", max_width=node.get("layout_width"), **STYLE)[1] == line


@pytest.mark.parametrize(
    "width, laid_out_width",
    [(66.43, 67.0), (66.0, 66.0), (66.01, 67.0), (67.0, 67.0), (120.5, 121.0)],
)
def test_an_explicit_width_is_the_whole_pixel_at_or_above_it(width, laid_out_width):
    window = Window(width=300, height=100)
    assert laid_out(window, width=width, height=20).get("layout_width") == laid_out_width


def test_get_reads_back_the_width_that_was_set():
    window = Window(width=300, height=100)
    node = laid_out(window, width=66.43, height=20)
    assert node.get("width") == pytest.approx(66.43)
    node.set(width=70.2)
    window.advance(16)
    assert node.get("width") == pytest.approx(70.2)
    assert node.get("layout_width") == 71.0


def test_min_and_max_width_round_up_too():
    window = Window(width=300, height=100)
    assert laid_out(window, width=10, min_width=66.43, height=20).get("layout_width") == 67.0
    window2 = Window(width=300, height=100)
    capped = laid_out(window2, max_width=66.43, flex_grow=1, height=20)
    assert capped.get("layout_width") == 67.0


def test_a_box_is_rounded_as_before_and_percent_widths_are_untouched():
    window = Window(width=300, height=100)
    box = window.create("box", width=66.43, height=20)
    window.root.add_child(box)
    half = window.create("text", text="x", width="50%", height=20)
    window.root.add_child(half)
    window.advance(16)
    assert box.get("layout_width") == 66.0
    assert half.get("layout_width") == pytest.approx(round(half.get("layout_width")))
