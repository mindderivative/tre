"""0.5.7 (#223): `PopupWindow`, a menu the compositor places beside a rectangle of its parent.

These need no compositor: a popup is built and driven headless (its parent is a headless
`LayerWindow`). What a compositor does with one (placement, flipping, the grab, dismissal) is in
`test_layer_window_live.py`.
"""

import sys

import pytest

from tre import LayerWindow, PopupWindow, UnsupportedPlatform, Window

pytestmark = pytest.mark.skipif(
    not sys.platform.startswith("linux"), reason="popups are a Linux (Wayland) feature"
)


@pytest.fixture(autouse=True)
def headless(monkeypatch):
    monkeypatch.delenv("WAYLAND_DISPLAY", raising=False)
    monkeypatch.delenv("DISPLAY", raising=False)


def bar():
    return LayerWindow("tesserae-shell.bar", anchor=("top", "left", "right"), size=(0, 32), exclusive_zone=32)


def popup(parent=None, **overrides):
    settings = dict(size=(160, 90), anchor_rect=(100, 0, 60, 32))
    settings.update(overrides)
    return PopupWindow(parent or bar(), **settings)


def test_a_popup_is_a_window_with_sensible_defaults():
    p = popup()
    assert isinstance(p, Window)
    assert p.get("size") == (160, 90)
    assert p.get("anchor_rect") == (100, 0, 60, 32)
    assert p.get("anchor") == "bottom_left"
    assert p.get("gravity") == "bottom_right"
    assert p.get("constraint_adjustment") == ("slide_x", "flip_y")
    assert p.get("offset") == (0, 0)
    assert p.get("grab") is True
    assert p.get("position") is None, "not placed until a compositor does it"


def test_every_setting_reads_back():
    p = popup(anchor="top_right", gravity="top_left", constraint_adjustment="slide_y", offset=(3, -4), grab=False)
    assert p.get("anchor") == "top_right"
    assert p.get("gravity") == "top_left"
    assert p.get("constraint_adjustment") == ("slide_y",)
    assert p.get("offset") == (3, -4)
    assert p.get("grab") is False
    assert popup(constraint_adjustment=()).get("constraint_adjustment") == (), "an empty sequence is none"
    assert popup(constraint_adjustment=["flip_x", "flip_y", "resize_x"]).get("constraint_adjustment") == (
        "flip_x", "flip_y", "resize_x")


def test_the_parent_may_be_another_popup_for_a_submenu():
    first = popup()
    second = PopupWindow(first, size=(100, 60), anchor_rect=(0, 0, 160, 30), anchor="top_right", gravity="bottom_right")
    assert second.get("size") == (100, 60)


@pytest.mark.parametrize(
    "overrides, key",
    [
        (dict(size=(0, 90)), "size"),
        (dict(size=(160, 0)), "size"),
        (dict(size="big"), "size"),
        (dict(anchor_rect=(0, 0, -1, 10)), "anchor_rect"),
        (dict(anchor_rect=(0, 0, 10)), "anchor_rect"),
        (dict(anchor="middle"), "anchor"),
        (dict(gravity="sideways"), "gravity"),
        (dict(constraint_adjustment="wobble"), "constraint_adjustment"),
        (dict(constraint_adjustment=["flip_y", "wobble"]), "constraint_adjustment"),
        (dict(constraint_adjustment=7), "constraint_adjustment"),
        (dict(offset=(1,)), "offset"),
    ],
)
def test_a_bad_setting_raises_naming_the_key(overrides, key):
    with pytest.raises(ValueError, match=key):
        popup(**overrides)


def test_the_parent_must_be_a_layer_window_or_a_popup():
    with pytest.raises(ValueError, match="parent"):
        PopupWindow(Window(width=100, height=100), size=(10, 10), anchor_rect=(0, 0, 1, 1))
    with pytest.raises(ValueError, match="parent"):
        PopupWindow("the bar", size=(10, 10), anchor_rect=(0, 0, 1, 1))


def test_size_and_anchor_rect_are_required():
    with pytest.raises(TypeError):
        PopupWindow(bar())
    with pytest.raises(TypeError):
        PopupWindow(bar(), size=(10, 10))


def test_the_popups_own_settings_are_fixed_and_window_only_keys_raise():
    p = popup()
    for key in ("size", "anchor_rect", "anchor", "gravity", "constraint_adjustment", "offset", "grab", "parent"):
        with pytest.raises(ValueError, match="constructor"):
            p.set(**{key: 1})
    for key in ("title", "decorations", "maximized", "x", "y", "always_on_top"):
        with pytest.raises(ValueError, match="popup"):
            p.set(**{key: 1})
    p.set(touch_slop=14)
    assert p.get("touch_slop") == 14.0


def test_a_popup_is_drawn_and_driven_headless_like_any_window():
    p = popup()
    p.root.set(fill=(40, 40, 60, 255))
    item = p.create("box", width=120, height=30, fill=(200, 200, 220, 255))
    p.root.add_child(item)
    seen = []
    item.on("click", lambda e: seen.append("click"))
    p.simulate("click", node=item)
    assert seen == ["click"]
    pixels, width, height = p.snapshot()
    assert (width, height) == (160, 90) and len(pixels) == width * height * 4


def test_the_popup_events_are_registrable():
    p = popup()
    for name in ("configure", "closed", "close_requested", "resize"):
        p.on(name, lambda e: None)


def test_without_a_layer_shell_the_constructor_raises(monkeypatch):
    parent = bar()
    monkeypatch.setenv("DISPLAY", ":99")  # X11 only: no layer shell
    with pytest.raises(UnsupportedPlatform):
        popup(parent)
