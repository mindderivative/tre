"""0.5.6 (#160): more accessibility states on nodes -- `pressed`, `invalid`,
`description`, `describedby`, `controls`, `current`, `value_now`, `value_text`,
`busy` and the `separator` role. What reaches the platform tree is covered by
Rust tests (`access_states_0_5_6`); here, the Python surface."""

import pytest

from tre import Window


def make():
    window = Window(width=200, height=120)
    nodes = [window.create("box", width=20, height=20, fill=(255, 0, 0, 255)) for _ in range(3)]
    for node in nodes:
        window.root.add_child(node)
    return window, nodes


def test_defaults_are_unset():
    _, (a, *_) = make()
    assert a.get("pressed") is None
    assert a.get("invalid") is False and a.get("busy") is False
    assert a.get("current") is None and a.get("description") is None
    assert a.get("value_now") is None and a.get("value_text") is None
    assert a.get("describedby") == [] and a.get("controls") == []


@pytest.mark.parametrize("value", [True, False, "mixed", None])
def test_pressed_round_trips(value):
    _, (a, *_) = make()
    a.set(pressed=value)
    assert a.get("pressed") == value


@pytest.mark.parametrize("value", ["page", "step", "location", "date", "time", True, False, None])
def test_current_round_trips(value):
    _, (a, *_) = make()
    a.set(current=value)
    assert a.get("current") == value


def test_simple_states_round_trip_and_clear():
    _, (a, *_) = make()
    a.set(invalid=True, busy=True, description="Must be an email", value_now=0.4, value_text="40 percent")
    assert a.get("invalid") is True and a.get("busy") is True
    assert a.get("description") == "Must be an email"
    assert a.get("value_now") == 0.4 and a.get("value_text") == "40 percent"
    a.set(description=None, value_now=None, value_text=None, invalid=False, busy=False)
    assert a.get("description") is None and a.get("value_now") is None
    assert a.get("value_text") is None and a.get("invalid") is False


def test_value_now_is_the_number_of_value():
    _, (a, *_) = make()
    a.set(value=3.0)
    assert a.get("value_now") == 3.0
    a.set(value="three")
    assert a.get("value_now") is None          # a text value is not a number
    a.set(value_now=2.0)
    assert a.get("value") == 2.0


def test_relations_take_a_node_or_a_list_and_clear():
    _, (a, b, c) = make()
    a.set(describedby=b, controls=[b, c])
    assert a.get("describedby") == [b]
    assert a.get("controls") == [b, c]
    a.set(controls=(c, c))
    assert a.get("controls") == [c]
    a.set(describedby=None, controls=[])
    assert a.get("describedby") == [] and a.get("controls") == []


def test_a_related_node_that_was_removed_drops_out():
    window, (a, b, c) = make()
    a.set(controls=[b, c])
    b.destroy()
    assert a.get("controls") == [c]


def test_relations_to_another_window_are_rejected():
    _, (a, *_) = make()
    _, (other, *_) = make()
    with pytest.raises(ValueError, match="different Window"):
        a.set(describedby=other)


@pytest.mark.parametrize("name,value,message", [
    ("pressed", "yes", "pressed"),
    ("pressed", 1, "pressed"),
    ("current", "soon", "current"),
    ("invalid", "no", "a bool"),
    ("busy", 1, "a bool"),
    ("describedby", "x", "describedby"),
    ("controls", [1], "controls"),
    ("value_now", "x", "value_now"),
    ("description", 5, "description"),
])
def test_bad_values_are_rejected_and_change_nothing(name, value, message):
    _, (a, *_) = make()
    with pytest.raises(ValueError, match=message):
        a.set(**{name: value}, label="x")
    assert a.get("label") is None              # the set was atomic


def test_separator_is_a_role():
    _, (a, *_) = make()
    a.set(role="separator")
    assert a.get("role") == "separator"


def test_they_can_be_given_when_a_node_is_created():
    window = Window()
    box = window.create("box", width=10, height=10, pressed="mixed", busy=True, current="page")
    assert (box.get("pressed"), box.get("busy"), box.get("current")) == ("mixed", True, "page")
