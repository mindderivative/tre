"""0.5.6 (#159): `Window.set(x=, y=, always_on_top=, resizable=, skip_taskbar=)`
and `Window.center()`. Placement itself needs a display and a compositor that
lets apps place windows (not Wayland), so these cover what a window reports
and rejects before it opens."""

import sys

import pytest

from tre import Window


def test_defaults():
    window = Window()
    assert window.get("always_on_top") is False
    assert window.get("resizable") is True
    assert window.get("skip_taskbar") is False
    assert window.get("x") is None and window.get("y") is None


def test_position_is_remembered_until_the_window_opens():
    window = Window()
    window.set(x=120, y=80)
    assert (window.get("x"), window.get("y")) == (120.0, 80.0)
    window.set(x=10)
    assert (window.get("x"), window.get("y")) == (10.0, 80.0)


def test_flags_round_trip():
    window = Window()
    window.set(always_on_top=True, resizable=False)
    assert window.get("always_on_top") is True
    assert window.get("resizable") is False


def test_bad_values_are_rejected():
    window = Window()
    with pytest.raises(ValueError, match="`x` must be a number"):
        window.set(x="left")
    with pytest.raises(ValueError, match="`x` must be a number"):
        window.set(x=float("nan"))
    with pytest.raises(ValueError, match="`resizable` must be a bool"):
        window.set(resizable="yes")


@pytest.mark.skipif(sys.platform == "win32", reason="Windows supports it")
def test_skip_taskbar_is_refused_where_it_is_unsupported():
    window = Window()
    with pytest.raises(ValueError, match="only supported on Windows"):
        window.set(skip_taskbar=True)
    assert window.get("skip_taskbar") is False
    window.set(skip_taskbar=False)


def test_center_before_the_window_is_open_is_false():
    assert Window().center() is False
