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
        ("add_loading_indicator", {"color": BLACK}),
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


# --- B: toolbar vibrant ------------------------------------------------------


def test_add_toolbar_takes_vibrant():
    Window(width=800, height=600).add_toolbar(vibrant=True)


# --- C: selected for Switch and RadioButton ---------------------------------


def test_add_switch_takes_selected_and_node_reads_it_back():
    switch = Window().add_switch(selected=True)
    assert switch.get_selected() is True
    switch.set_selected(False)
    assert switch.get_selected() is False


def test_add_switch_rejects_the_old_on_keyword():
    with pytest.raises(TypeError, match="on"):
        Window().add_switch(on=True)


# --- D: a slider's value ----------------------------------------------------


def test_slider_value_is_readable_and_animatable_as_value():
    slider = Window().add_slider(background=BLACK, width=100, height=20, value=0.25)
    assert slider.get("value") == pytest.approx(0.25)
    slider.animate("value", 0.75, duration_ms=0)


def test_the_old_thumb_position_property_names_its_replacement():
    slider = Window().add_slider(background=BLACK, width=100, height=20)
    with pytest.raises(ValueError, match="renamed to \"value\""):
        slider.get("thumb_position")
    with pytest.raises(ValueError, match="renamed to \"value\""):
        slider.animate("thumb_position", 0.5)


# --- E: orientation ----------------------------------------------------------


def test_divider_and_scroll_view_take_orientation():
    window = Window()
    window.add_divider(length=100, orientation="vertical")
    window.add_scroll_view(width=100, height=50, orientation="horizontal")


@pytest.mark.parametrize("factory", ["add_divider", "add_scroll_view"])
def test_an_unknown_orientation_is_a_clear_error(factory):
    window = Window()
    kwargs = {"length": 100} if factory == "add_divider" else {"width": 100, "height": 50}
    with pytest.raises(ValueError, match="unknown orientation"):
        getattr(window, factory)(orientation="diagonal", **kwargs)


def test_the_old_orientation_booleans_are_gone():
    window = Window()
    with pytest.raises(TypeError, match="vertical"):
        window.add_divider(length=100, vertical=True)
    with pytest.raises(TypeError, match="horizontal"):
        window.add_scroll_view(width=100, height=50, horizontal=True)


# --- F: lowercase enum values ------------------------------------------------


# --- G: text-string parameters -----------------------------------------------


def test_link_dialog_and_popover_take_their_new_text_parameters():
    window = Window()
    window.add_link(content="Docs", width=80)
    window.add_dialog(headline="Title", supporting_text="Body", width=200, height=120)
    window.add_popover(subhead="Title", supporting_text="Body", width=200, height=100)


# --- H: typography_role ------------------------------------------------------


