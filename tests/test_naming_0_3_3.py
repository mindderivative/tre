"""M90: one name per concept across the imperative and declarative APIs.

Each rename is tested both ways: the new name works, and the pre-0.3.3
name fails naming its replacement (or, for a removed keyword argument,
with pyo3's own TypeError naming the bad keyword).
"""

import pytest

from tre import Window

BLACK = (0, 0, 0, 255)


# --- A: foreground is the glyph/text color ---------------------------------


def test_add_text_takes_foreground():
    Window().add_text("Hi", foreground=BLACK, width=60, height=20)


@pytest.mark.parametrize(
    ("factory", "kwargs"),
    [
        ("add_text", {"content": "Hi", "background": BLACK, "width": 60, "height": 20}),
        ("add_icon", {"name": "home", "color": BLACK, "size": 24}),
    ],
)
def test_the_old_glyph_color_keywords_are_gone(factory, kwargs):
    with pytest.raises(TypeError, match=next(k for k in kwargs if k in ("background", "color"))):
        getattr(Window(), factory)(**kwargs)


def test_node_animate_foreground_works_on_text_and_background_is_rejected():
    label = Window().add_text("Hi", foreground=BLACK, width=60, height=20)
    label.animate("foreground", (255, 0, 0, 255), duration_ms=0)
    with pytest.raises(ValueError, match="'foreground'"):
        label.animate("background", (255, 0, 0, 255), duration_ms=0)


def test_node_animate_foreground_works_on_an_icon():
    icon = Window().add_icon("home", foreground=BLACK, size=24)
    icon.animate("foreground", (255, 0, 0, 255), duration_ms=0)
    # M92: an Icon's color eases like the other glyph kinds', with
    # on_complete accepted -- the Rust tick test proves it interpolates.
    icon.animate("foreground", (0, 0, 255, 255), duration_ms=200, on_complete=lambda: None)


def test_foreground_is_not_a_property_of_a_fill_kind():
    rect = Window().add_rect(background=BLACK, width=10, height=10)
    with pytest.raises(ValueError, match="Rect has no property 'foreground'"):
        rect.animate("foreground", (255, 0, 0, 255), duration_ms=0)


# --- E: orientation ----------------------------------------------------------


def test_an_unknown_orientation_is_a_clear_error():
    with pytest.raises(ValueError, match="unknown orientation"):
        Window().add_scroll_view(width=100, height=50, orientation="diagonal")
