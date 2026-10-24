"""0.5.7 (#198): `LayerWindow`, a window whose Wayland role is a layer surface.

These need no compositor: a `LayerWindow` is built and driven headless, like a `Window`
(`simulate`, `advance`, `snapshot`), with no display in the environment. What a
compositor does with one is in `test_layer_window_live.py`.
"""

import subprocess
import sys

import pytest

import tre
from tre import LayerWindow, UnsupportedPlatform, Window

pytestmark = pytest.mark.skipif(
    not sys.platform.startswith("linux"), reason="layer surfaces are a Linux (Wayland) feature"
)


@pytest.fixture(autouse=True)
def headless(monkeypatch):
    """No display: availability is `Headless`, so a layer window can be built."""
    monkeypatch.delenv("WAYLAND_DISPLAY", raising=False)
    monkeypatch.delenv("DISPLAY", raising=False)


def bar(**overrides):
    settings = dict(
        layer="top",
        anchor=("top", "left", "right"),
        size=(0, 32),
        exclusive_zone=32,
    )
    settings.update(overrides)
    return LayerWindow("tesserae-shell.bar", **settings)


def test_a_layer_window_is_a_window():
    w = bar()
    assert isinstance(w, Window)
    box = w.create("box", width=60, height=20, fill=(200, 100, 50, 255))
    w.root.add_child(box)
    w.advance(16)
    pixels, width, height = w.snapshot()
    assert (width, height) == (1920, 32)  # a stretched axis is a placeholder until a compositor sizes it
    assert len(pixels) == width * height * 4


def test_the_settings_read_back():
    w = bar(margin=(1, 2, 3, 4), keyboard="on_demand")
    assert w.get("namespace") == "tesserae-shell.bar"
    assert w.get("layer") == "top"
    assert w.get("anchor") == ("top", "left", "right")
    assert w.get("size") == (0, 32)
    assert w.get("exclusive_zone") == 32
    assert w.get("exclusive_edge") is None
    assert w.get("margin") == (1, 2, 3, 4)
    assert w.get("keyboard") == "on_demand"
    assert w.get("output") is None


def test_the_defaults_and_the_short_forms():
    w = LayerWindow("x", anchor="top", size=(100, 40))
    assert w.get("layer") == "top" and w.get("keyboard") == "none"
    assert w.get("anchor") == ("top",) and w.get("exclusive_zone") == 0
    assert w.get("margin") == (0, 0, 0, 0)
    assert LayerWindow("x", anchor="top", size=(1, 1), margin=5).get("margin") == (5, 5, 5, 5)
    # Nothing anchored and a size: a floating widget the compositor centres.
    assert LayerWindow("x", size=(200, 100)).get("anchor") == ()


def test_the_root_has_no_padding_so_a_bar_can_fill_its_surface():
    w = bar()
    w.root.set(fill=(255, 0, 0, 255))
    w.advance(16)
    pixels, width, height = w.snapshot()
    assert tuple(pixels[:4]) == (255, 0, 0, 255)  # the corner is the root's fill, not padding


@pytest.mark.parametrize(
    "settings,key",
    [
        (dict(size=(0, 32), anchor=("top", "left")), "size"),  # a stretched width needs both sides
        (dict(size=(200, 0), anchor=("top", "left", "right")), "size"),  # and a stretched height both ends
        (dict(layer="above"), "layer"),
        (dict(keyboard="always"), "keyboard"),
        (dict(anchor=("middle",)), "anchor"),
        (dict(exclusive_zone=-2), "exclusive_zone"),
        (dict(size=(-1, 32)), "size"),
        (dict(margin=(1, 2, 3)), "margin"),
    ],
)
def test_the_protocol_errors_are_python_errors_naming_the_key(settings, key):
    with pytest.raises(ValueError, match=rf"LayerWindow {key}"):
        bar(**settings)


def test_a_namespace_is_required_and_not_empty():
    with pytest.raises(TypeError):
        LayerWindow()
    with pytest.raises(ValueError, match="namespace"):
        LayerWindow("", anchor="top", size=(10, 10))


def corner(**overrides):
    settings = dict(anchor=("top", "right"), size=(120, 28), exclusive_zone=28)
    settings.update(overrides)
    return LayerWindow("tesserae-shell.clock", **settings)


def test_exclusive_edge_names_which_edge_of_a_corner_the_zone_is_for():
    w = corner(exclusive_edge="top")
    assert w.get("exclusive_edge") == "top"
    w.set(exclusive_edge="right")
    assert w.get("exclusive_edge") == "right"


def test_exclusive_edge_must_be_an_anchored_edge():
    with pytest.raises(ValueError, match="exclusive_edge"):
        corner(exclusive_edge="bottom")
    w = corner(exclusive_edge="top")
    with pytest.raises(ValueError, match="exclusive_edge"):
        w.set(exclusive_edge="left")
    assert w.get("exclusive_edge") == "top"
    with pytest.raises(ValueError, match="edge"):
        corner(exclusive_edge="middle")
    with pytest.raises(ValueError, match="edge"):
        corner(exclusive_edge=3)


def test_an_edge_once_set_cannot_be_cleared_because_the_protocol_cannot():
    w = corner(exclusive_edge="top")
    with pytest.raises(ValueError, match="cannot be cleared"):
        w.set(exclusive_edge=None)
    assert w.get("exclusive_edge") == "top"
    # A surface that never had one may be given None (a no-op).
    none = corner()
    none.set(exclusive_edge=None)
    assert none.get("exclusive_edge") is None


def test_an_edge_and_a_bar_need_no_exclusive_edge_but_may_give_the_one_they_have():
    w = bar(exclusive_edge="top")  # a bar anchored top, left and right
    assert w.get("exclusive_edge") == "top"
    one = LayerWindow("tesserae-shell.dock", anchor="bottom", size=(300, 40), exclusive_zone=40,
                      exclusive_edge="bottom")
    assert one.get("exclusive_edge") == "bottom"


def test_a_corner_with_an_edge_has_no_zone_warning_and_one_without_has():
    seen = []
    w = corner()
    w.on("warning", lambda e: seen.append(e.key))
    w.set(exclusive_zone=29)  # re-validates: no edge, a corner, a zone
    assert seen == ["exclusive_zone"]
    seen.clear()
    w.set(exclusive_edge="right")
    w.set(exclusive_zone=30)
    assert seen == []


def test_set_changes_the_layer_settings_together_or_not_at_all():
    w = bar()
    w.set(exclusive_zone=48, margin=(0, 0, 0, 8), layer="overlay", keyboard="exclusive")
    assert (w.get("exclusive_zone"), w.get("margin"), w.get("layer"), w.get("keyboard")) == (
        48, (0, 0, 0, 8), "overlay", "exclusive",
    )
    with pytest.raises(ValueError, match="size"):
        w.set(exclusive_zone=10, size=(0, 32), anchor=("top",))  # the second is invalid
    assert w.get("exclusive_zone") == 48 and w.get("anchor") == ("top", "left", "right")
    # A bad shared key leaves the layer keys alone too.
    with pytest.raises(ValueError):
        w.set(exclusive_zone=10, partial_redraw="yes")
    assert w.get("exclusive_zone") == 48


@pytest.mark.parametrize(
    "key,value",
    [
        ("title", "bar"),
        ("decorations", False),
        ("maximized", True),
        ("fullscreen", True),
        ("resize_border", 4),
        ("system_menu", True),
        ("min_width", 100),
        ("x", 10),
        ("always_on_top", True),
    ],
)
def test_window_keys_that_mean_nothing_to_a_placed_surface_raise(key, value):
    with pytest.raises(ValueError, match=rf"LayerWindow {key}: means nothing"):
        bar().set(**{key: value})


def test_namespace_and_output_are_for_the_constructor_only():
    w = bar()
    for key in ("namespace", "output"):
        with pytest.raises(ValueError, match="constructor"):
            w.set(**{key: "x"})


def test_the_shared_window_keys_still_work():
    w = bar()
    w.set(touch_slop=14, partial_redraw=False)
    assert w.get("touch_slop") == 14.0 and w.get("partial_redraw") is False
    assert w.get("transparent") is True  # a bar can be rounded or floating without a backdrop


def test_a_zone_the_protocol_ignores_is_a_warning_event_not_an_error():
    w = bar()
    seen = []
    w.on("warning", lambda e: seen.append((e.key, e.message)))
    w.set(anchor=("top", "left"), size=(200, 32))  # no reservable edge set, zone 32
    assert len(seen) == 1 and seen[0][0] == "exclusive_zone"
    assert "treats it as 0" in seen[0][1]
    # Nothing to warn about now.
    w.set(exclusive_zone=0)
    assert len(seen) == 1


def test_the_layer_window_events_are_registrable():
    w = bar()
    for name in ("configure", "closed", "output_changed", "warning", "close_requested", "resize"):
        w.on(name, lambda e: None)
    with pytest.raises(ValueError, match="unknown window event"):
        w.on("opened", lambda e: None)


def test_close_before_the_window_opens_does_nothing():
    bar().close()


def test_it_has_a_clock_and_timers_like_a_window():
    w = bar()
    ticks = []
    w.every(100, lambda: ticks.append(1))
    w.advance(350)
    assert len(ticks) == 3


def test_simulate_drives_it_like_a_window():
    w = bar()
    button = w.create("box", width=60, height=24, fill=(1, 2, 3, 255), x=10, y=4, position="absolute")
    w.root.add_child(button)
    clicks = []
    button.on("click", lambda e: clicks.append(1))
    w.advance(16)
    w.simulate("click", node=button)
    assert clicks == [1]


def test_resize_changes_the_headless_size_for_tests():
    w = bar()
    w.resize(800, 32)
    w.advance(16)
    assert w.snapshot()[1:] == (800, 32)


def test_supports_says_what_this_process_can_do():
    assert tre.supports("no_such_feature") is False
    # No display in this environment (the fixture): no layer shell to offer.
    assert tre.supports("layer_shell") is False


@pytest.mark.parametrize(
    "env,expect",
    [
        ({"DISPLAY": ":0"}, "X11"),  # an X11 session: no layer shell
    ],
)
def test_without_a_layer_shell_the_constructor_raises(env, expect):
    code = "import tre\ntry:\n    tre.LayerWindow('x', anchor='top', size=(10, 10))\nexcept tre.UnsupportedPlatform as e:\n    print('UNSUPPORTED', e)\n"
    import os

    clean = {k: v for k, v in os.environ.items() if k not in ("WAYLAND_DISPLAY", "DISPLAY")}
    clean.update(env)
    out = subprocess.run([sys.executable, "-I", "-c", code], capture_output=True, text=True, env=clean)
    assert "UNSUPPORTED" in out.stdout and expect in out.stdout, out.stdout + out.stderr


def test_unsupported_platform_is_a_runtime_error():
    assert issubclass(UnsupportedPlatform, RuntimeError)


def test_a_node_cannot_be_a_window_drag_region_on_a_layer_window():
    # The compositor places the surface: there is no window to drag.
    w = bar()
    node = w.create("box", width=10, height=10)
    with pytest.raises(ValueError, match="window_region"):
        node.set(window_region="drag")
    with pytest.raises(ValueError, match="window_region"):
        w.create("box", window_region="drag")
    node.set(window_region="none")  # "none" and None are fine
    node.set(window_region=None)
    # An ordinary window still takes it.
    ordinary = Window(width=100, height=40)
    ordinary.create("box", window_region="drag")


# ---- input_region (#204) ----


def test_input_region_defaults_to_the_whole_surface():
    w = bar()
    assert w.get("input_region") is None
    assert w.get("click_through") is False


def test_input_region_takes_rectangles_and_rounds_them_outward():
    w = bar(input_region=[(0, 0, 100, 32), (0.5, 1.2, 10, 2)])
    assert w.get("input_region") == [(0, 0, 100, 32), (0, 1, 11, 3)]
    w.set(input_region=[(10, 10, 5, 5)])
    assert w.get("input_region") == [(10, 10, 5, 5)]
    w.set(input_region=None)
    assert w.get("input_region") is None


def test_an_empty_input_region_is_click_through():
    w = bar(input_region=[])
    assert w.get("input_region") == []
    assert w.get("click_through") is True


def test_click_through_is_an_alias_for_an_empty_region():
    w = bar(click_through=True)
    assert w.get("input_region") == []
    w.set(click_through=False)
    assert w.get("input_region") is None
    w.set(click_through=True)
    assert w.get("input_region") == []
    w.set(input_region=[(0, 0, 4, 4)])
    assert w.get("click_through") is False


def test_click_through_and_input_region_must_agree():
    with pytest.raises(ValueError, match="disagree"):
        bar(click_through=True, input_region=[(0, 0, 4, 4)])
    with pytest.raises(ValueError, match="disagree"):
        bar(click_through=False, input_region=[])
    bar(click_through=True, input_region=[])  # the same thing said twice is fine


@pytest.mark.parametrize(
    "bad",
    ["everything", "AUTO", (0, 0, 4, 4), [(0, 0, 0, 4)], [(0, 0, 4, -1)], [(0, 0, float("nan"), 4)], [(0, 0, 4)], 7],
)
def test_a_bad_input_region_raises_and_changes_nothing(bad):
    w = bar(input_region=[(0, 0, 8, 8)])
    with pytest.raises(ValueError, match="input_region"):
        w.set(input_region=bad)
    assert w.get("input_region") == [(0, 0, 8, 8)]


def test_input_region_is_capped_at_256_rectangles():
    bar(input_region=[(0, 0, 1, 1)] * 256)
    with pytest.raises(ValueError, match="at most 256"):
        bar(input_region=[(0, 0, 1, 1)] * 257)


def test_an_ordinary_window_keeps_its_own_click_through():
    w = Window(width=100, height=100)
    w.set(click_through=True)
    assert w.get("click_through") is True


def test_auto_is_a_region_that_follows_what_is_drawn():
    w = bar(input_region="auto")
    assert w.get("input_region") == "auto"
    assert w.get("click_through") is False
    w.set(input_region=[(0, 0, 4, 4)])
    assert w.get("input_region") == [(0, 0, 4, 4)]
    w.set(input_region="auto")
    assert w.get("input_region") == "auto"
    w.set(click_through=True)
    assert w.get("input_region") == []
    with pytest.raises(ValueError, match="disagree"):
        bar(input_region="auto", click_through=True)


def test_a_rectangle_may_be_a_list_as_well_as_a_tuple():
    w = bar(input_region=[[0, 0, 10, 10], (5, 5, 2, 2)])
    assert w.get("input_region") == [(0, 0, 10, 10), (5, 5, 2, 2)]


# ---- margin animation (#205) ----


def margins(w):
    return w.get("margin")


def test_a_margin_animates_with_the_clock_and_ends_exactly():
    w = bar(margin=(0, 0, 0, 0))
    done = []
    w.animate("margin_top", -32, 200, on_complete=lambda: done.append(1))
    assert margins(w) == (0, 0, 0, 0)
    w.advance(100)
    assert margins(w)[0] == -16  # linear, half way
    assert done == []
    w.advance(100)
    assert margins(w) == (-32, 0, 0, 0)
    assert done == [1]
    w.advance(500)
    assert done == [1], "on_complete is called once"


def test_the_four_margins_animate_independently():
    w = bar(margin=(0, 0, 0, 0))
    w.animate("margin_top", 10, 100)
    w.animate("margin_right", 20, 200)
    w.animate("margin_bottom", 30, 100)
    w.animate("margin_left", 40, 400)
    w.advance(100)
    assert margins(w) == (10, 10, 30, 10)
    w.advance(300)
    assert margins(w) == (10, 20, 30, 40)


def test_an_easing_curve_applies():
    w = bar(margin=(0, 0, 0, 0))
    w.animate("margin_left", 100, 100, easing=(0.42, 0, 1, 1))  # ease-in
    w.advance(50)
    assert 0 < margins(w)[3] < 50
    w.advance(50)
    assert margins(w)[3] == 100
    w.animate("margin_left", 0, 100, easing="spring")
    w.advance(5000)
    assert margins(w)[3] == 0


def test_retargeting_starts_from_where_it_is_and_cancels_the_first_callback():
    w = bar(margin=(0, 0, 0, 0))
    first, second = [], []
    w.animate("margin_top", 100, 100, on_complete=lambda: first.append(1))
    w.advance(50)
    here = margins(w)[0]
    assert here == 50
    w.animate("margin_top", 0, 100, on_complete=lambda: second.append(1))
    w.advance(1)
    assert margins(w)[0] <= here
    w.advance(100)
    assert margins(w)[0] == 0
    assert first == [] and second == [1]


def test_set_margin_cancels_a_running_animation_with_no_callback():
    w = bar(margin=(0, 0, 0, 0))
    seen, done = [], []
    w.root.on("animation_end", lambda e: seen.append((e.property, e.finished)))
    w.animate("margin_top", 100, 100, on_complete=lambda: done.append(1))
    w.advance(30)
    w.set(margin=(5, 5, 5, 5))
    assert margins(w) == (5, 5, 5, 5)
    w.advance(500)
    assert margins(w) == (5, 5, 5, 5)
    assert done == []
    assert ("margin_top", False) in seen


def test_animate_reports_its_end_as_an_animation_end_event():
    w = bar(margin=(0, 0, 0, 0))
    seen = []
    w.root.on("animation_end", lambda e: seen.append((e.property, e.finished)))
    w.animate("margin_bottom", 8, 50)
    w.advance(60)
    assert seen == [("margin_bottom", True)]


def test_zero_duration_snaps_on_the_next_tick():
    w = bar(margin=(0, 0, 0, 0))
    w.animate("margin_top", 7)
    w.advance(1)
    assert margins(w)[0] == 7


@pytest.mark.parametrize(
    "args, message",
    [
        (("margin", 5), "only the margins"),
        (("size", 5), "only the margins"),
        (("margin_top", "far"), "must be a number"),
        (("margin_top", float("nan")), "finite"),
        (("margin_top", float("inf")), "finite"),
    ],
)
def test_a_bad_margin_animation_raises_and_changes_nothing(args, message):
    w = bar(margin=(1, 2, 3, 4))
    with pytest.raises(ValueError, match=message):
        w.animate(*args)
    assert margins(w) == (1, 2, 3, 4)


def test_a_bad_easing_or_callback_raises():
    w = bar()
    with pytest.raises(ValueError):
        w.animate("margin_top", 5, 100, easing="bounce")
    with pytest.raises(ValueError, match="callable"):
        w.animate("margin_top", 5, 100, on_complete=7)


def test_two_windows_animating_together_stay_in_step():
    a, b = bar(margin=(0, 0, 0, 0)), bar(margin=(0, 0, 0, 0))
    a.animate("margin_top", 100, 100)
    b.animate("margin_top", 100, 100)
    a.advance(40)
    b.advance(40)
    assert margins(a) == margins(b) == (40, 0, 0, 0)


# ---- window_region="drag_margin" (#205) ----


def test_drag_margin_is_a_layer_window_region_and_reads_back():
    w = bar()
    node = w.create("box", width=40, height=20)
    w.root.add_child(node)
    node.set(window_region="drag_margin")
    assert node.get("window_region") == "drag_margin"
    node.set(window_region=None)
    assert node.get("window_region") is None


def test_an_ordinary_window_refuses_drag_margin():
    w = Window(width=100, height=100)
    node = w.create("box", width=40, height=20)
    w.root.add_child(node)
    with pytest.raises(ValueError, match="LayerWindow"):
        node.set(window_region="drag_margin")


def test_a_layer_window_still_refuses_drag_and_names_the_choices_for_a_bad_value():
    w = bar()
    node = w.create("box", width=40, height=20)
    w.root.add_child(node)
    with pytest.raises(ValueError, match="LayerWindow"):
        node.set(window_region="drag")
    with pytest.raises(ValueError, match="drag_margin"):
        node.set(window_region="slide")


def _press(w, x, y):
    w.simulate("pointer_move", x=x, y=y)
    w.simulate("pointer_down", x=x, y=y)
    w.simulate("pointer_up", x=x, y=y)


def test_an_interactive_child_of_a_drag_margin_node_is_not_a_drag_handle():
    w = bar(size=(300, 40), anchor=("top", "left"))
    handle = w.create("box", width=200, height=30, fill=(1, 2, 3, 255))
    handle.set(window_region="drag_margin")
    button = w.create("box", width=40, height=20, fill=(9, 9, 9, 255))
    handle.add_child(button)
    w.root.add_child(handle)
    seen = []
    button.on("click", lambda e: seen.append("click"))
    handle.on("pointer_cancel", lambda e: seen.append("cancel"))
    button.on("pointer_cancel", lambda e: seen.append("cancel"))
    _press(w, 10, 10)  # on the button: its own click wins
    assert seen == ["click"]


def test_a_non_interactive_drag_margin_node_cancels_the_press():
    w = bar(size=(300, 40), anchor=("top", "left"))
    handle = w.create("box", width=100, height=30, fill=(1, 2, 3, 255))
    handle.set(window_region="drag_margin")
    w.root.add_child(handle)
    seen = []
    handle.on("pointer_cancel", lambda e: seen.append("cancel"))
    handle.on("pointer_down", lambda e: seen.append("down"))
    _press(w, 20, 10)
    assert seen == ["down", "cancel"]


def test_moved_is_a_window_event():
    w = bar()
    w.on("moved", lambda e: None)
