"""M94 Phase 1: `node.on`/`window.on` listeners, the M93 propagation model,
pointer capture, keys, text, and window events -- all driven headlessly
through `Window.simulate`, which shares `App.run()`'s own input pipeline.
"""

from __future__ import annotations

from typing import Any

import pytest

import tre

BLACK = (0, 0, 0, 255)
WHITE = (255, 255, 255, 255)


def window() -> tre.Window:
    return tre.Window(400, 300, "listeners")


def nested(w: tre.Window) -> tuple[tre.Node, tre.Node]:
    outer = w.add_rect(BLACK, 200, 150)
    inner = w.add_rect(WHITE, 50, 50)
    outer.add_child(inner)
    return outer, inner


# --- registration ---------------------------------------------------------


def test_on_rejects_an_unknown_event_listing_the_valid_ones() -> None:
    w = window()
    node = w.add_rect(BLACK, 10, 10)
    with pytest.raises(ValueError, match="valid events: pointer_enter"):
        node.on("tap", lambda: None)


def test_off_removes_a_listener() -> None:
    w = window()
    node = w.add_rect(BLACK, 40, 40)
    hits: list[str] = []
    node.on("click", lambda: hits.append("click"))
    node.off("click")
    w.simulate("click", node=node)
    assert hits == []


def test_a_zero_argument_listener_is_called_without_an_event() -> None:
    w = window()
    node = w.add_rect(BLACK, 40, 40)
    hits: list[str] = []
    node.on("click", lambda: hits.append("click"))
    w.simulate("click", node=node)
    assert hits == ["click"]


def test_legacy_handlers_keep_their_non_bubbling_behavior() -> None:
    w = window()
    outer, inner = nested(w)
    legacy: list[str] = []
    outer.set_on_click(lambda: legacy.append("outer"))
    w.simulate("click", node=inner)
    assert legacy == []


# --- bubbling ---------------------------------------------------------------


def test_click_bubbles_with_target_and_current() -> None:
    w = window()
    outer, inner = nested(w)
    seen: list[tuple[str, str, bool, bool]] = []

    def record(tag: str) -> Any:
        def listener(e: tre.Event) -> None:
            assert e.target is not None and e.current is not None
            seen.append((tag, e.type, e.target == inner, e.current == inner))

        return listener

    inner.on("click", record("inner"))
    outer.on("click", record("outer"))
    w.simulate("click", node=inner)
    assert seen == [
        ("inner", "click", True, True),
        ("outer", "click", True, False),
    ]


def test_stop_ends_propagation() -> None:
    w = window()
    outer, inner = nested(w)
    seen: list[str] = []
    def stop_here(e: tre.Event) -> None:
        seen.append("inner")
        e.stop()

    inner.on("click", stop_here)
    outer.on("click", lambda e: seen.append("outer"))
    w.simulate("click", node=inner)
    assert seen == ["inner"]


def test_pointer_events_report_coordinates_local_to_the_current_node() -> None:
    w = window()
    outer, inner = nested(w)
    seen: list[tuple[str, float, float, float, float]] = []
    for tag, node in (("inner", inner), ("outer", outer)):
        node.on(
            "pointer_down",
            lambda e, tag=tag: seen.append(
                (tag, e.x, e.y, e.window_x - e.x, e.window_y - e.y)
            ),
        )
    w.simulate("pointer_down", node=inner, x=10, y=20)
    (_, ix, iy, *_), (_, ox, oy, *_) = seen
    assert (ix, iy) == (10, 20)
    inner_origin = (seen[0][3], seen[0][4])
    outer_origin = (seen[1][3], seen[1][4])
    assert (ox, oy) == (
        10 + inner_origin[0] - outer_origin[0],
        20 + inner_origin[1] - outer_origin[1],
    )


def test_pointer_events_carry_button_and_modifiers() -> None:
    w = window()
    node = w.add_rect(BLACK, 40, 40)
    seen: list[tuple[str | None, bool | None, bool | None]] = []
    node.on("pointer_down", lambda e: seen.append((e.button, e.shift, e.ctrl)))
    w.simulate("pointer_down", node=node, button="secondary", shift=True)
    assert seen == [("secondary", True, False)]


def test_secondary_click_bubbles() -> None:
    w = window()
    outer, inner = nested(w)
    seen: list[str] = []
    outer.on("secondary_click", lambda e: seen.append(e.type))
    w.simulate("secondary_click", node=inner)
    assert seen == ["secondary_click"]


def test_pointer_up_precedes_click() -> None:
    w = window()
    node = w.add_rect(BLACK, 40, 40)
    order: list[str] = []
    for name in ("pointer_down", "pointer_up", "click"):
        node.on(name, lambda e: order.append(e.type))
    w.simulate("click", node=node)
    assert order == ["pointer_down", "pointer_up", "click"]


# --- pointer_enter / pointer_leave -----------------------------------------------


def test_pointer_enter_and_leave_are_subtree_events() -> None:
    w = window()
    outer, inner = nested(w)
    seen: list[tuple[str, str]] = []
    for tag, node in (("outer", outer), ("inner", inner)):
        for name in ("pointer_enter", "pointer_leave"):
            node.on(name, lambda e, tag=tag: seen.append((tag, e.type)))

    w.simulate("pointer_move", node=inner)
    assert seen == [("outer", "pointer_enter"), ("inner", "pointer_enter")]
    seen.clear()

    # Still inside `outer`'s subtree: only `inner` is left.
    w.simulate("pointer_move", node=outer, x=150, y=120)
    assert seen == [("inner", "pointer_leave")]
    seen.clear()

    # Leaving the window leaves everything still hovered.
    w.simulate("pointer_leave")
    assert seen == [("outer", "pointer_leave")]


def test_pointer_leaving_the_window_fires_the_legacy_hover_exit() -> None:
    w = window()
    node = w.add_rect(BLACK, 40, 40)
    exits: list[object] = []
    node.set_on_hover_exit(lambda e: exits.append(e.position))
    w.simulate("pointer_move", node=node)
    w.simulate("pointer_leave")
    assert exits == [None]


# --- pointer capture ----------------------------------------------------------


def test_captured_pointer_events_stay_on_the_capturing_node() -> None:
    w = window()
    a = w.add_rect(BLACK, 40, 40)
    b = w.add_rect(WHITE, 40, 40)
    seen: list[str] = []
    a.on("pointer_down", lambda e: a.capture_pointer())
    a.on("pointer_move", lambda e: seen.append("a move"))
    a.on("pointer_up", lambda e: seen.append("a up"))
    b.on("pointer_move", lambda e: seen.append("b move"))

    w.simulate("pointer_down", node=a)
    w.simulate("pointer_move", node=b)
    w.simulate("pointer_up", node=b)
    # `pointer_up` released the capture.
    w.simulate("pointer_move", node=b)
    assert seen == ["a move", "a up", "b move"]


def test_release_pointer_ends_capture() -> None:
    w = window()
    a = w.add_rect(BLACK, 40, 40)
    b = w.add_rect(WHITE, 40, 40)
    seen: list[str] = []
    a.on("pointer_down", lambda e: a.capture_pointer())
    b.on("pointer_move", lambda e: seen.append("b move"))
    w.simulate("pointer_down", node=a)
    a.release_pointer()
    w.simulate("pointer_move", node=b)
    assert seen == ["b move"]


# --- wheel --------------------------------------------------------------------


def test_wheel_bubbles_in_pixels_positive_down() -> None:
    w = window()
    outer, inner = nested(w)
    seen: list[tuple[float | None, float | None]] = []
    outer.on("wheel", lambda e: seen.append((e.delta_x, e.delta_y)))
    w.simulate("wheel", node=inner, delta_y=40)
    assert seen == [(0.0, 40.0)]


# --- keys, text, change, focus --------------------------------------------------


def test_key_events_go_to_the_focused_node_and_bubble() -> None:
    w = window()
    box = w.add_rect(BLACK, 200, 150)
    field = w.add_text_field(WHITE, 150, 30)
    box.add_child(field)
    seen: list[tuple[str, str | None, bool | None, bool | None]] = []
    box.on("key_down", lambda e: seen.append((e.type, e.key, e.shift, e.repeat)))
    box.on("key_up", lambda e: seen.append((e.type, e.key, e.shift, e.repeat)))
    w.simulate("focus", node=field)
    w.simulate("key_down", key="f5", shift=True, repeat=True)
    w.simulate("key_up", key="f5")
    assert seen == [("key_down", "f5", True, True), ("key_up", "f5", False, False)]


def test_key_events_reach_the_root_when_nothing_is_focused() -> None:
    w = window()
    seen: list[str | None] = []
    w.root.on("key_down", lambda e: seen.append(e.key))
    w.simulate("key_down", key="escape")
    assert seen == ["escape"]


def test_input_and_change_for_typing() -> None:
    w = window()
    box = w.add_rect(BLACK, 200, 150)
    field = w.add_text_field(WHITE, 150, 30)
    box.add_child(field)
    seen: list[tuple[Any, ...]] = []
    box.on("input", lambda e: seen.append(("input", e.text)))
    field.on("change", lambda e: seen.append(("change", e.old_value, e.new_value)))
    box.on("change", lambda e: seen.append(("change bubbled",)))
    w.simulate("focus", node=field)
    w.simulate("key_down", key="h")
    w.simulate("input", text="i!")
    w.simulate("key_down", key="backspace")
    assert seen == [
        ("input", "h"),
        ("change", "", "h"),
        ("input", "i!"),
        ("change", "h", "hi!"),
        ("change", "hi!", "hi"),
    ]
    assert field.get_text() == "hi"


def test_focus_and_unfocus_bubble_for_focus_within() -> None:
    w = window()
    box = w.add_rect(BLACK, 200, 150)
    field = w.add_text_field(WHITE, 150, 30)
    box.add_child(field)
    seen: list[tuple[str, bool]] = []

    def record(e: tre.Event) -> None:
        assert e.target is not None and e.current is not None
        seen.append((e.type, e.target == field))

    box.on("focus", record)
    box.on("unfocus", record)
    w.simulate("focus", node=field)
    w.simulate("unfocus", node=field)
    assert seen == [("focus", True), ("unfocus", True)]


def test_tab_moves_focus_and_fires_focus_listeners() -> None:
    w = window()
    a = w.add_text_field(WHITE, 100, 30)
    b = w.add_text_field(WHITE, 100, 30)
    seen: list[str] = []
    a.on("unfocus", lambda e: seen.append("a unfocus"))
    b.on("focus", lambda e: seen.append("b focus"))
    w.simulate("focus", node=a)
    w.simulate("key_down", key="tab")
    assert seen == ["a unfocus", "b focus"]


def test_focus_and_unfocus_name_the_node_on_the_other_side() -> None:
    w = window()
    bar = w.add_rect(BLACK, 300, 40)
    field = w.add_text_field(WHITE, 200, 30)
    clear = w.add_text_field(WHITE, 30, 30)
    bar.add_child(field)
    bar.add_child(clear)
    seen: list[tuple[str, tre.Node | None]] = []
    bar.on("unfocus", lambda e: seen.append(("unfocus", e.related_target)))
    bar.on("focus", lambda e: seen.append(("focus", e.related_target)))
    w.simulate("focus", node=field)
    w.simulate("key_down", key="tab")
    w.simulate("unfocus", node=clear)
    assert seen == [("focus", None), ("unfocus", clear), ("focus", field), ("unfocus", None)]


def test_focus_visible_follows_the_input_that_moved_focus() -> None:
    w = window()
    a = w.add_text_field(WHITE, 100, 30)
    b = w.add_text_field(WHITE, 100, 30)
    visible: list[bool | None] = []
    a.on("focus", lambda e: visible.append(e.focus_visible))
    b.on("focus", lambda e: visible.append(e.focus_visible))
    a.on("unfocus", lambda e: visible.append(e.focus_visible))
    w.simulate("pointer_down", node=a)
    w.simulate("key_down", key="tab")
    # Programmatic focus follows the last interaction -- here the keyboard.
    w.simulate("focus", node=a)
    w.simulate("pointer_down", node=b)
    w.simulate("focus", node=a)
    assert visible == [False, None, True, True, None, False, False]


def test_a_shortcut_doesnt_count_as_keyboard_navigation() -> None:
    w = window()
    field = w.add_text_field(WHITE, 100, 30)
    visible: list[bool | None] = []
    field.on("focus", lambda e: visible.append(e.focus_visible))
    w.simulate("pointer_down", node=field)
    w.simulate("key_down", key="a", ctrl=True)
    w.simulate("unfocus", node=field)
    w.simulate("focus", node=field)
    assert visible == [False, False]


def test_simulating_change_directly_is_refused() -> None:
    w = window()
    with pytest.raises(ValueError, match="simulate `input`"):
        w.simulate("change")


# --- window events and properties ------------------------------------------------


def test_window_resize_event_and_properties() -> None:
    w = window()
    seen: list[tuple[str, float | None, float | None]] = []
    w.on("resize", lambda e: seen.append((e.type, e.width, e.height)))
    w.simulate("resize", width=640, height=480)
    assert seen == [("resize", 640.0, 480.0)]
    assert (w.get("width"), w.get("height")) == (640.0, 480.0)


def test_window_color_scheme_and_scale_factor_events() -> None:
    w = window()
    seen: list[tuple[str, Any]] = []
    w.on("color_scheme", lambda e: seen.append((e.type, e.dark)))
    w.on("scale_factor", lambda e: seen.append((e.type, e.scale_factor)))
    w.simulate("color_scheme", dark=True)
    w.simulate("scale_factor", scale_factor=2.0)
    assert seen == [("color_scheme", True), ("scale_factor", 2.0)]


def test_close_requested_is_cancellable_and_other_events_are_not() -> None:
    w = window()
    cancelled: list[bool] = []

    def on_close(e: tre.Event) -> None:
        e.cancel()
        cancelled.append(True)

    w.on("close_requested", on_close)
    w.simulate("close_requested")
    assert cancelled == [True]

    errors: list[str] = []

    def on_resize(e: tre.Event) -> None:
        try:
            e.cancel()
        except ValueError as err:
            errors.append(str(err))

    w.on("resize", on_resize)
    w.simulate("resize", width=100, height=100)
    assert errors and "can't be cancelled" in errors[0]


def test_a_window_event_has_no_node() -> None:
    w = window()
    seen: list[tre.Event] = []
    w.on("closed", lambda e: seen.append(e))
    w.simulate("closed")
    (event,) = seen
    assert event.target is None
    with pytest.raises(AttributeError, match="window events have no target"):
        event.node  # noqa: B018


def test_window_title_is_settable_and_the_rest_read_only() -> None:
    w = window()
    w.set(title="Renamed")
    assert w.get("title") == "Renamed"
    assert w.get("scale_factor") == 1.0
    with pytest.raises(ValueError, match="read-only"):
        w.set(width=10)  # type: ignore[call-arg]
    with pytest.raises(ValueError, match="settable: title"):
        w.set(colour="red")  # type: ignore[call-arg]
    with pytest.raises(ValueError, match="valid: width, height, title, scale_factor"):
        w.get("depth")


def test_window_on_rejects_node_events() -> None:
    w = window()
    with pytest.raises(ValueError, match="valid events: resize"):
        w.on("click", lambda: None)


# --- simulate's own validation -------------------------------------------------


def test_simulate_reports_unknown_events_and_fields() -> None:
    w = window()
    node = w.add_rect(BLACK, 40, 40)
    with pytest.raises(ValueError, match="valid events: pointer_down"):
        w.simulate("tap")
    with pytest.raises(ValueError, match="unexpected field"):
        w.simulate("click", node=node, pressure=1)
    with pytest.raises(ValueError, match="needs `key`"):
        w.simulate("key_down")
    with pytest.raises(ValueError, match="needs `node`"):
        w.simulate("pointer_down")


def test_node_handles_compare_and_hash_by_identity() -> None:
    w = window()
    a = w.add_rect(BLACK, 40, 40)
    b = w.add_rect(BLACK, 40, 40)
    targets: list[tre.Node] = []
    a.on("click", lambda e: targets.append(e.target))
    w.simulate("click", node=a)
    (target,) = targets
    assert target == a and target != b
    assert {a: "a"}[target] == "a"
    assert w.root == w.root


def test_simulate_rejects_a_node_from_another_window() -> None:
    w = window()
    other = window()
    stranger = other.add_rect(BLACK, 40, 40)
    with pytest.raises(Exception, match="(?i)another|foreign|window"):
        w.simulate("click", node=stranger)
