"""0.5.6.2 (#178): mixed-unit sizes, `width="calc(100% - 16)"`.

A percentage plus pixels, resolved against the parent's box after layout (taffy
cannot lay such a size out itself): the whole width except a strip, a third and
ten pixels. `x`/`left` and the other insets cover the stretched-between-edges
case; this covers the rest.
"""

import pytest

from tre import Window


def scene(parent_width=300, padding=0, **child):
    """A parent box with one child, laid out; returns (window, parent, child)."""
    window = Window(width=600, height=400)
    parent = window.create("box", width=parent_width, height=200, padding=padding)
    window.root.add_child(parent)
    child.setdefault("height", 10)
    node = window.create("box", **child)
    parent.add_child(node)
    window.advance(16)
    return window, parent, node


@pytest.mark.parametrize(
    "text, width",
    [
        ("calc(100% - 16)", 284),
        ("100% - 16", 284),
        ("calc(100% - 16px)", 284),
        ("100%   -   16px", 284),
        ("33% + 10", 109),  # 33% of 300 is 99
        ("calc(50% + 20)", 170),
        ("calc(100% - 10% - 5)", 265),  # several percentages add
    ],
)
def test_the_accepted_forms(text, width):
    _, _, node = scene(width=text)
    assert node.get("layout_width") == width


def test_height_is_resolved_against_the_parents_height():
    _, _, node = scene(width=10, height="50% + 20")
    assert node.get("layout_height") == 120


def test_the_example_from_the_issue_an_absolute_node_8_in_and_16_less_than_the_parent():
    _, parent, node = scene(position="absolute", x=8, y=0, width="calc(100% - 16)")
    assert node.get("layout_x") - parent.get("layout_x") == 8
    assert node.get("layout_width") == 284


def test_a_flow_node_uses_the_content_box_and_an_absolute_one_the_padding_box():
    # Taffy's own bases for a plain "50%": 280 inside padding=10, 300 for absolute.
    _, _, in_flow = scene(padding=10, width="100% - 16")
    _, _, absolute = scene(padding=10, position="absolute", width="100% - 16")
    assert in_flow.get("layout_width") == 264
    assert absolute.get("layout_width") == 284


def test_the_size_follows_the_parent_when_it_is_resized():
    window, parent, node = scene(width="100% - 16")
    parent.set(width=400)
    window.advance(16)
    assert node.get("layout_width") == 384


def test_the_size_follows_a_parent_that_is_a_percentage_of_the_window():
    window = Window(width=600, height=400)
    parent = window.create("box", width="50%", height=100)
    window.root.add_child(parent)
    child = window.create("box", width="100% - 20", height=10)
    parent.add_child(child)
    window.advance(16)
    first = child.get("layout_width")
    assert first == parent.get("layout_width") - 20
    window.resize(800, 400)
    window.advance(16)
    assert child.get("layout_width") == parent.get("layout_width") - 20 != first


def test_get_reads_a_calc_back_in_one_canonical_form():
    window = Window(width=100, height=100)
    node = window.create("box", width="100% - 16", height="33% + 10")
    assert node.get("width") == "calc(100% - 16px)"
    assert node.get("height") == "calc(33% + 10px)"


def test_a_lone_percentage_or_pixel_count_is_the_plain_size_it_says():
    window = Window(width=100, height=100)
    node = window.create("box", width="calc(50%)", height="10 + 5")
    assert node.get("width") == "50%"
    assert node.get("height") == 15


def test_any_other_value_takes_the_calc_off():
    window, parent, node = scene(width="100% - 16")
    node.set(width=50)
    window.advance(16)
    assert node.get("width") == 50 and node.get("layout_width") == 50
    parent.set(width=400)
    window.advance(16)
    assert node.get("layout_width") == 50
    node.set(width="100% - 16")
    window.advance(16)
    assert node.get("layout_width") == 384
    node.set(width="auto")
    assert node.get("width") == "auto"


def test_fit_content_and_calc_replace_each_other():
    window = Window(width=300, height=100)
    label = window.create("text", text="Hi", font_size=14, width="fit-content")
    label.set(width="100% - 4")
    assert label.get("width") == "calc(100% - 4px)"
    label.set(width="fit-content")
    assert label.get("width") == "fit-content"


@pytest.mark.parametrize("bad", ["calc(100% -)", "100% * 2", "calc(100%+16)", "calc()", "10% 20%", "x% + 1"])
def test_a_malformed_calc_is_refused(bad):
    window = Window(width=100, height=100)
    with pytest.raises(ValueError, match="width"):
        window.create("box", width=bad)


def test_min_and_max_sizes_are_not_calc():
    window = Window(width=100, height=100)
    with pytest.raises(ValueError, match="min_width"):
        window.create("box", min_width="100% - 16")


def test_a_size_its_own_parent_depends_on_is_left_auto():
    # The row is as wide as a 100 px block plus the child, and the child is the
    # row less 16: no solution. It is given up on, not made to drift each frame.
    window = Window(width=600, height=400)
    outer = window.create("box", width=400, height=200, align_items="flex_start")
    window.root.add_child(outer)
    row = window.create("box", height=50, flex_direction="horizontal")
    outer.add_child(row)
    row.add_child(window.create("box", width=100, height=10))
    child = window.create("box", width="100% - 16", height=10)
    row.add_child(child)
    window.advance(16)
    assert child.get("width") == "auto"
    before = child.get("layout_width")
    window.advance(16)
    window.advance(16)
    assert child.get("layout_width") == before
