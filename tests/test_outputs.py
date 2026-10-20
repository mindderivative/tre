"""0.5.7 (#202): `Output`, `App.outputs()`, and the output events.

Needs no compositor: handlers are exercised with `App.simulate`, and a real listing is
compared with `wayland-info` in `test_layer_window_live.py`.
"""

import pytest

import tre
from tre import App, LayerWindow, Output


@pytest.fixture
def app():
    return App()


def captured(app, event, **fields):
    got = []
    app.on(event, got.append)
    app.simulate(event, **fields)
    return got[0]


def test_an_output_event_hands_the_handler_an_output(app):
    out = captured(app, "output_added", name="DP-2", description="a monitor", x=1920, y=-200,
                   width=2560, height=1440, scale_factor=1.5, transform="90")
    assert isinstance(out, Output)
    assert (out.name, out.description, out.x, out.y) == ("DP-2", "a monitor", 1920, -200)
    assert (out.width, out.height, out.scale_factor, out.transform) == (2560, 1440, 1.5, "90")


def test_the_defaults_of_a_simulated_output(app):
    out = captured(app, "output_updated", name="DP-3")
    assert (out.x, out.y, out.width, out.height, out.scale_factor, out.transform) == (
        0, 0, 1920, 1080, 1.0, "normal",
    )
    assert out.description == ""


def test_outputs_are_equal_by_name_and_hashable(app):
    a = captured(app, "output_added", name="DP-1", width=1000)
    b = captured(app, "output_added", name="DP-1", width=2000)
    c = captured(app, "output_added", name="DP-2")
    assert a == b and a != c and a != "DP-1"
    assert len({a, b, c}) == 2
    assert "DP-1" in repr(a) and "1000x1080" in repr(a)


def test_an_output_is_read_only(app):
    out = captured(app, "output_added", name="DP-1")
    with pytest.raises(AttributeError):
        out.name = "x"


def test_the_three_events_go_to_their_own_handlers(app):
    seen = []
    for event in ("output_added", "output_removed", "output_updated"):
        app.on(event, lambda o, event=event: seen.append((event, o.name)))
    app.simulate("output_removed", name="A")
    app.simulate("output_added", name="B")
    app.simulate("output_updated", name="C")
    assert seen == [("output_removed", "A"), ("output_added", "B"), ("output_updated", "C")]


def test_a_second_handler_replaces_the_first_and_off_removes_it(app):
    seen = []
    app.on("output_added", lambda o: seen.append("first"))
    app.on("output_added", lambda o: seen.append("second"))
    app.simulate("output_added", name="A")
    assert seen == ["second"]
    app.off("output_added")
    app.simulate("output_added", name="A")
    assert seen == ["second"]


def test_a_handler_may_take_no_argument(app):
    seen = []
    app.on("output_added", lambda: seen.append(1))
    app.simulate("output_added", name="A")
    assert seen == [1]


def test_a_handler_that_raises_is_logged_not_raised(app):
    def broken(output):
        raise RuntimeError("boom")

    app.on("output_added", broken)
    app.simulate("output_added", name="A")  # does not raise


@pytest.mark.parametrize("method", ["on", "off", "simulate"])
def test_an_unknown_event_lists_the_valid_ones(app, method):
    args = ("no_such_event", lambda o: None) if method == "on" else ("no_such_event",)
    with pytest.raises(ValueError, match="output_added, output_removed, output_updated"):
        getattr(app, method)(*args)


def test_simulate_needs_a_name_and_known_fields(app):
    with pytest.raises(ValueError, match="needs a `name`"):
        app.simulate("output_added")
    with pytest.raises(ValueError, match="unknown field `colour`"):
        app.simulate("output_added", name="A", colour="red")
    with pytest.raises(ValueError, match="wrong type"):
        app.simulate("output_added", name="A", width="wide")


def test_with_no_display_there_are_no_outputs(app, monkeypatch):
    monkeypatch.delenv("WAYLAND_DISPLAY", raising=False)
    monkeypatch.delenv("DISPLAY", raising=False)
    assert app.outputs() == []


def test_a_layer_window_takes_an_output_or_its_name(app, monkeypatch):
    monkeypatch.delenv("WAYLAND_DISPLAY", raising=False)
    monkeypatch.delenv("DISPLAY", raising=False)
    out = captured(app, "output_added", name="DP-7")
    kwargs = dict(anchor=("top", "left", "right"), size=(0, 24))
    assert LayerWindow("x", output=out, **kwargs).get("output") == "DP-7"
    assert LayerWindow("x", output="DP-8", **kwargs).get("output") == "DP-8"
    assert LayerWindow("x", **kwargs).get("output") is None
    with pytest.raises(ValueError, match="output"):
        LayerWindow("x", output=3, **kwargs)


def test_a_handler_that_refers_to_its_app_is_collectable():
    import gc
    import weakref

    class Probe:
        pass

    app = App()
    probe = Probe()
    app.on("output_added", lambda o, app=app, probe=probe: (app, probe))  # app -> handler -> app
    ref = weakref.ref(probe)
    del app, probe
    gc.collect()
    assert ref() is None
