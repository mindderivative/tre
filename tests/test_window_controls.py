"""0.5.0 M2 (issue #28, #37): undecorated windows and the window's own
properties and controls -- what a framework drawing its own title bar needs
from `tre`. Everything here runs without a display: a window that isn't open
keeps each setting and applies it when `App.run()` opens it.
"""

import pytest

from tre import Window


# --- Step 1: decorations -------------------------------------------------------


def test_a_window_is_decorated_by_default():
    assert Window().get("decorations") is True


def test_decorations_can_be_turned_off_at_creation():
    window = Window(width=320, height=200, title="Notes", decorations=False)
    assert window.get("decorations") is False


def test_decorations_can_be_set_either_way():
    window = Window()
    window.set(decorations=False)
    assert window.get("decorations") is False
    window.set(decorations=True)
    assert window.get("decorations") is True


def test_decorations_must_be_a_bool():
    with pytest.raises(ValueError, match="`decorations` must be a bool"):
        Window().set(decorations="no")


def test_the_unknown_property_error_names_decorations():
    with pytest.raises(ValueError, match="decorations"):
        Window().set(colour=1)
    with pytest.raises(ValueError, match="decorations"):
        Window().get("colour")
