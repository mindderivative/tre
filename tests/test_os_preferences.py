"""0.5.4 (#115): the OS's reduced-motion and increased-contrast preferences.

`window.get("reduced_motion")` and `get("high_contrast")` read them (`None`
where the platform can't say), and the window's `reduced_motion` and
`high_contrast` events report a change. `simulate` delivers the events; the
real values are whatever the machine running the tests has set.
"""

import pytest

from tre import Window


@pytest.mark.parametrize("name", ["reduced_motion", "high_contrast"])
def test_the_preference_reads_as_a_bool_or_none(name):
    assert Window().get(name) in (True, False, None)


@pytest.mark.parametrize("name", ["reduced_motion", "high_contrast"])
def test_reading_it_twice_agrees(name):
    window = Window()
    assert window.get(name) == window.get(name)


def test_the_events_carry_the_new_value():
    window = Window()
    heard = []
    window.on("reduced_motion", lambda e: heard.append(("motion", e.type, e.reduced_motion)))
    window.on("high_contrast", lambda e: heard.append(("contrast", e.type, e.high_contrast)))
    window.simulate("reduced_motion", value=True)
    window.simulate("high_contrast", value=True)
    window.simulate("reduced_motion", value=False)
    window.simulate("high_contrast", value=False)
    assert heard == [
        ("motion", "reduced_motion", True),
        ("contrast", "high_contrast", True),
        ("motion", "reduced_motion", False),
        ("contrast", "high_contrast", False),
    ]


def test_an_event_carries_only_its_own_field():
    window = Window()
    seen = []
    window.on("reduced_motion", lambda e: seen.append((e.reduced_motion, e.high_contrast, e.dark)))
    window.simulate("reduced_motion", value=True)
    assert seen == [(True, None, None)]


def test_a_missing_or_bad_value_is_an_error():
    window = Window()
    with pytest.raises(ValueError, match="value"):
        window.simulate("reduced_motion")
    with pytest.raises(ValueError, match="bool"):
        window.simulate("high_contrast", value="yes")


def test_the_new_names_are_listed_among_the_valid_ones():
    window = Window()
    with pytest.raises(ValueError, match="reduced_motion"):
        window.on("nonsense", lambda: None)
    with pytest.raises(ValueError, match="high_contrast"):
        window.get("nonsense")
