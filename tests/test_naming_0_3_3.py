"""M90: one name per concept across the imperative and declarative APIs.

Each rename is tested both ways: the new name works, and the pre-0.3.3
name fails naming its replacement (or, for a removed keyword argument,
with pyo3's own TypeError naming the bad keyword).
"""

import pytest

from tre import Window
from helpers import add

BLACK = (0, 0, 0, 255)


# --- A: foreground is the glyph/text color ---------------------------------


def test_add_text_takes_foreground():
    add(Window(), "text", text="Hi", fill=BLACK, width=60, height=20)


def test_the_old_glyph_color_keywords_are_gone():
    with pytest.raises(ValueError, match="background"):
        Window().create("text", text="Hi", background=BLACK)


def test_node_animate_foreground_works_on_text_and_background_is_rejected():
    label = add(Window(), "text", text="Hi", fill=BLACK, width=60, height=20)
    label.animate("foreground", (255, 0, 0, 255), duration_ms=0)
    with pytest.raises(ValueError, match="'foreground'"):
        label.animate("background", (255, 0, 0, 255), duration_ms=0)


def test_foreground_is_not_a_property_of_a_fill_kind():
    rect = add(Window(), "box", fill=BLACK, width=10, height=10)
    with pytest.raises(ValueError, match="Rect has no property 'foreground'"):
        rect.animate("foreground", (255, 0, 0, 255), duration_ms=0)


# --- E: orientation ----------------------------------------------------------


def test_an_unknown_orientation_is_a_clear_error():
    with pytest.raises(ValueError, match="`orientation` must be one of"):
        add(Window(), "scroll_view", width=100, height=50, orientation="diagonal")
