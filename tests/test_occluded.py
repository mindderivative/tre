"""0.5.10 (#246): the window's `occluded` state, event and `pause_when_occluded`."""

import pytest

from tre import Window


def test_occluded_is_a_read_only_state_that_fires_once_per_change():
    window = Window(width=100, height=100)
    seen = []
    window.on("occluded", lambda e: seen.append(e.occluded))
    assert window.get("occluded") is False
    window.simulate("occluded", occluded=True)
    window.simulate("occluded", occluded=True)
    assert window.get("occluded") is True
    window.simulate("occluded", occluded=False)
    assert seen == [True, False]
    with pytest.raises(ValueError, match="read-only"):
        window.set(occluded=True)


def test_a_minimized_window_reads_as_occluded_without_an_event():
    window = Window(width=100, height=100)
    seen = []
    window.on("occluded", lambda e: seen.append(e.occluded))
    window.minimize()
    assert window.get("occluded") is True
    assert seen == []


def test_pause_when_occluded_defaults_on_and_can_be_turned_off():
    window = Window(width=100, height=100)
    assert window.get("pause_when_occluded") is True
    window.set(pause_when_occluded=False)
    assert window.get("pause_when_occluded") is False
    with pytest.raises(ValueError, match="bool"):
        window.set(pause_when_occluded="no")
