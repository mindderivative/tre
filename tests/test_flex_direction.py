"""The `flex_direction` property: `"horizontal"` and `"vertical"` are accepted
(alone or with other layout props), anything else -- including taffy's
`"row"` -- is a `ValueError` naming the valid values.
"""

import pytest

from tre import Window
from helpers import add


def test_flex_direction_horizontal_does_not_raise():
    window = Window(width=200, height=100)
    node = add(window, "box", fill=(0, 0, 0, 255), width=100, height=50)
    node.set(flex_direction="horizontal")


def test_flex_direction_vertical_does_not_raise():
    window = Window(width=200, height=100)
    node = add(window, "box", fill=(0, 0, 0, 255), width=100, height=50)
    node.set(flex_direction="vertical")


def test_flex_direction_unknown_value_raises_naming_the_valid_ones():
    window = Window(width=200, height=100)
    node = add(window, "box", fill=(0, 0, 0, 255), width=100, height=50)
    with pytest.raises(ValueError, match="`flex_direction` must be one of: horizontal, vertical"):
        node.set(flex_direction="row")  # taffy's vocabulary, deliberately rejected


def test_flex_direction_composes_with_other_layout_props():
    window = Window(width=200, height=100)
    node = add(window, "box", fill=(0, 0, 0, 255), width=100, height=50)
    node.set(flex_direction="vertical", align_items="center", justify_content="center", gap=8)


def test_set_without_flex_direction_still_works():
    window = Window(width=200, height=100)
    node = add(window, "box", fill=(0, 0, 0, 255), width=100, height=50)
    node.set(width=120, height=60)
