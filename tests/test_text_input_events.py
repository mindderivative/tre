"""0.5.6 (#162, #165): the text input's `max_length`, `read_only`, `input_mode`,
`submit`, `selection_start`/`selection_end`, IME composition events, the
caret's style and blink, `caret_rect()`, `text_rects()` and `caret_move`."""

import pytest

from tre import Window
from helpers import add


def field(**props):
    window = Window(width=300, height=120)
    props.setdefault("height", 28)
    node = add(window, "text_input", width=240, font_size=16, **props)
    node.focus()
    return window, node


def type_text(window, text):
    window.simulate("input", text=text)


# --- max_length --------------------------------------------------------------

def test_max_length_cuts_typing_short_and_counts_characters():
    window, node = field(max_length=4)
    type_text(window, "abcdef")
    assert node.get("text") == "abcd"
    type_text(window, "x")
    assert node.get("text") == "abcd"
    node.set(text="", max_length=2)
    type_text(window, "a😀b")
    assert node.get("text") == "a😀"                     # a character, not four bytes


def test_a_rejected_edit_fires_no_change():
    window, node = field(max_length=2, text="ab")
    seen = []
    node.on("change", lambda e: seen.append(node.get("text")))
    type_text(window, "c")
    window.simulate("key_down", key="space")
    assert seen == [] and node.get("text") == "ab"


def test_typing_over_a_selection_still_fits():
    window, node = field(max_length=3, text="abc")
    node.set(selection=(0, 3))
    type_text(window, "xyz!")
    assert node.get("text") == "xyz"


def test_a_limit_below_the_text_keeps_it_and_allows_deleting():
    window, node = field(text="hello", max_length=2)
    type_text(window, "x")
    assert node.get("text") == "hello"
    window.simulate("key_down", key="backspace")
    assert node.get("text") == "hell"


def test_max_length_none_removes_the_limit():
    window, node = field(max_length=1)
    node.set(max_length=None)
    type_text(window, "abc")
    assert node.get("text") == "abc" and node.get("max_length") is None


# --- read_only ---------------------------------------------------------------

def test_read_only_takes_no_edits_but_moves_and_selects():
    window, node = field(text="hello", read_only=True)
    type_text(window, "x")
    for key in ("backspace", "delete", "space"):
        window.simulate("key_down", key=key)
    assert node.get("text") == "hello"
    window.simulate("key_down", key="home")
    window.simulate("key_down", key="arrow_right", shift=True)
    assert node.get("selection") == (0, 1)
    assert node.get("focused") is True


def test_read_only_ignores_a_composition_and_leaves_tab_to_focus():
    window, node = field(text="hi", read_only=True, multiline=True)
    seen = []
    node.on("compose_start", lambda e: seen.append(1))
    window.simulate("ime_preedit", text="ni")
    assert seen == []
    window.simulate("key_down", key="tab")
    assert node.get("text") == "hi"                      # no tab was inserted


# --- input_mode ---------------------------------------------------------------

def test_input_mode_round_trips_and_rejects_unknown():
    _, node = field()
    assert node.get("input_mode") == "text"
    for mode in ("numeric", "decimal", "email", "phone", "url", "search", "text"):
        node.set(input_mode=mode)
        assert node.get("input_mode") == mode
    with pytest.raises(ValueError, match="input_mode"):
        node.set(input_mode="emoji")


# --- submit -------------------------------------------------------------------

def test_enter_submits_a_single_line_field_and_does_not_bubble():
    window, node = field(text="ok")
    seen, outer = [], []
    node.on("submit", lambda e: seen.append((e.target == node, node.get("text"))))
    window.root.on("submit", lambda e: outer.append(1))
    window.simulate("key_down", key="enter")
    assert seen == [(True, "ok")] and outer == []


def test_enter_in_a_multiline_field_is_a_newline_not_a_submit():
    window, node = field(text="a", multiline=True)
    seen = []
    node.on("submit", lambda e: seen.append(1))
    window.simulate("key_down", key="enter")
    assert node.get("text") == "a\n" and seen == []


def test_read_only_still_submits():
    window, node = field(text="a", read_only=True)
    seen = []
    node.on("submit", lambda e: seen.append(1))
    window.simulate("key_down", key="enter")
    assert seen == [1]


# --- selection_start / selection_end -------------------------------------------

def test_selection_ends_read_in_order_and_set_each_other_along():
    window, node = field(text="hello world")
    assert (node.get("selection_start"), node.get("selection_end")) == (11, 11)
    node.set(selection_start=2, selection_end=7)
    assert node.get("selection") == (2, 7)
    node.set(selection_end=9)
    assert (node.get("selection_start"), node.get("selection_end")) == (2, 9)
    node.set(selection_start=5)
    assert (node.get("selection_start"), node.get("selection_end")) == (5, 9)
    node.set(selection_start=10)                          # past the end: it moves along
    assert (node.get("selection_start"), node.get("selection_end")) == (10, 10)
    node.set(selection=(8, 8))
    for _ in range(3):                                    # shift-select leftwards: reversed
        window.simulate("key_down", key="arrow_left", shift=True)
    assert node.get("selection") == (8, 5)
    assert (node.get("selection_start"), node.get("selection_end")) == (5, 8)


def test_selection_ends_reject_bad_offsets():
    _, node = field(text="héllo")
    with pytest.raises(ValueError, match="selection_start"):
        node.set(selection_start=2)                       # inside the é
    with pytest.raises(ValueError, match="selection_end"):
        node.set(selection_end=99)
    with pytest.raises(ValueError):
        node.set(selection_end=-1)


# --- composition ----------------------------------------------------------------

def test_a_composition_fires_start_update_and_end_with_its_text_and_cursor():
    window, node = field()
    seen = []
    for name in ("compose_start", "compose_update", "compose_end"):
        node.on(name, lambda e, name=name: seen.append((name, e.text, e.preedit_cursor)))
    window.simulate("ime_preedit", text="ni", cursor=(0, 2))
    window.simulate("ime_preedit", text="nihao", cursor=(5, 5))
    window.simulate("ime_preedit", text="")
    assert seen == [
        ("compose_start", "ni", (0, 2)),
        ("compose_update", "nihao", (5, 5)),
        ("compose_end", None, None),
    ]
    assert node.get("text") == ""                        # nothing was typed yet


def test_a_commit_after_the_composition_is_ordinary_input():
    window, node = field()
    inputs = []
    node.on("change", lambda e: inputs.append(node.get("text")))
    window.simulate("ime_preedit", text="ni")
    window.simulate("ime_preedit", text="")
    type_text(window, "你")
    assert node.get("text") == "你" and inputs == ["你"]


def test_composition_events_name_the_valid_events():
    _, node = field()
    with pytest.raises(ValueError, match="compose_update"):
        node.on("nonsense", lambda e: None)


# --- caret style ------------------------------------------------------------------

def test_caret_style_round_trips():
    _, node = field()
    assert node.get("caret_visible") is True
    assert node.get("caret_shape") == "bar" and node.get("caret_width") == 1.5
    assert node.get("caret_blink") is None
    node.set(caret_visible=False, caret_width=3, caret_shape="block", caret_blink=500)
    assert node.get("caret_visible") is False and node.get("caret_width") == 3
    assert node.get("caret_shape") == "block" and node.get("caret_blink") == 500
    node.set(caret_blink=None)
    assert node.get("caret_blink") is None
    for bad in ({"caret_shape": "wavy"}, {"caret_width": -1}, {"caret_blink": -5}, {"caret_visible": 1}):
        with pytest.raises(ValueError):
            node.set(**bad)


def test_caret_color_animates():
    window, node = field()
    seen = []
    node.on("animation_end", lambda e: seen.append(e.property))
    node.set(caret_color=(255, 0, 0, 255))
    node.animate("caret_color", (0, 0, 255, 255), 100)
    window.advance(50)
    mid = node.get("caret_color")
    assert mid != (255, 0, 0, 255) and mid != (0, 0, 255, 255)
    window.advance(100)
    assert node.get("caret_color") == (0, 0, 255, 255) and seen == ["caret_color"]


def test_the_caret_blinks_on_the_windows_clock_and_a_keystroke_restarts_it():
    window, node = field(caret_blink=100)

    def on():
        return node._caret_on if hasattr(node, "_caret_on") else None

    # The phase is observed through pixels: the caret is a column of the text colour.
    def caret_pixels():
        rgba, w, h = window.snapshot()
        return sum(1 for x in range(4, 60) for y in range(8, 30)
                   if rgba[(y * w + x) * 4 + 3] and rgba[(y * w + x) * 4] < 80)

    window.advance(0)
    solid = caret_pixels()
    assert solid > 0
    window.advance(110)
    assert caret_pixels() == 0               # off phase
    window.advance(100)
    assert caret_pixels() == solid           # on again
    window.advance(100)                       # off
    type_text(window, "a")                    # typing makes it solid at once
    window.advance(0)
    assert caret_pixels() > 0


def test_caret_visible_false_draws_none_and_shapes_differ():
    window, node = field()
    window.advance(0)

    def dark_pixels():
        rgba, w, h = window.snapshot()
        return {(x, y) for x in range(0, 80) for y in range(0, 40)
                if rgba[(y * w + x) * 4 + 3] and rgba[(y * w + x) * 4] < 80}

    bar = dark_pixels()
    node.set(caret_visible=False)
    assert dark_pixels() == set() and bar
    node.set(caret_visible=True, caret_shape="block", text="ab")
    node.set(selection=(0, 0))
    block = dark_pixels()
    node.set(caret_shape="underline")
    underline = dark_pixels()
    assert block and underline and block != underline
    assert len(block) > len(bar)


# --- geometry -----------------------------------------------------------------------

def test_caret_rect_follows_the_text_and_the_cursor():
    _, node = field(text="")
    x0, y0, w0, h0 = node.caret_rect()
    assert w0 == pytest.approx(1.5) and h0 > 10
    node.set(text="hello", selection=(5, 5))
    x1, y1, _, h1 = node.caret_rect()
    assert x1 > x0 + 20 and y1 == y0 and h1 == h0
    node.set(selection=(2, 2))
    x2, *_ = node.caret_rect()
    assert x0 < x2 < x1
    node.set(caret_width=4)
    assert node.caret_rect()[2] == pytest.approx(4)


def test_caret_rect_is_at_the_end_of_a_composition():
    window, node = field(text="ab")
    node.set(selection=(2, 2))
    before = node.caret_rect()
    window.simulate("ime_preedit", text="xyz", cursor=(3, 3))
    after = node.caret_rect()
    assert after[0] > before[0] + 10


def test_caret_rect_moves_down_a_line_in_a_multiline_field():
    _, node = field(text="one\ntwo", multiline=True, height=80)
    node.set(selection=(0, 0))
    top = node.caret_rect()
    node.set(selection=(4, 4))
    below = node.caret_rect()
    assert below[1] > top[1] + 8 and below[0] == pytest.approx(top[0])


def test_text_rects_cover_a_range_per_line():
    _, node = field(text="hello world")
    rects = node.text_rects(0, 5)
    assert len(rects) == 1
    x, y, w, h = rects[0]
    assert w > 20 and h > 10
    whole = node.text_rects(0, 11)[0]
    assert whole[2] > w
    assert node.text_rects(3, 3) == [] or node.text_rects(3, 3)[0][2] == 0
    _, multi = field(text="ab\ncd", multiline=True, height=80)
    assert len(multi.text_rects(0, 5)) == 2


def test_geometry_rejects_bad_offsets_and_other_kinds():
    window, node = field(text="héllo")
    for args in ((2, 3), (0, 99), (4, 2)):
        with pytest.raises(ValueError, match="text_rects"):
            node.text_rects(*args)
    box = add(window, "box", width=10, height=10)
    with pytest.raises(ValueError, match="text_input"):
        box.caret_rect()


# --- caret_move ------------------------------------------------------------------------

def test_caret_move_reports_the_caret_and_the_inserted_bytes():
    window, node = field()
    seen = []
    node.on("caret_move", lambda e: seen.append((e.caret, e.inserted)))
    type_text(window, "héy")
    assert len(seen) == 1
    caret, inserted = seen[0]
    assert inserted == (0, 4) and caret == node.caret_rect()
    window.simulate("key_down", key="arrow_left")
    assert len(seen) == 2 and seen[1][1] is None
    assert seen[1][0][0] < caret[0]
    window.simulate("key_down", key="arrow_left")
    window.simulate("key_down", key="arrow_right")
    assert len(seen) == 4


def test_caret_move_does_not_fire_without_a_move_or_for_programmatic_changes():
    window, node = field(text="abc")
    seen = []
    node.on("caret_move", lambda e: seen.append(1))
    window.simulate("key_down", key="end")                # already at the end
    node.set(text="xyz")
    assert seen == []


def test_caret_move_fires_for_a_click_that_places_the_caret():
    window, node = field(text="hello world")
    seen = []
    node.on("caret_move", lambda e: seen.append(e.caret))
    window.simulate("pointer_down", x=60, y=30)
    window.simulate("pointer_up", x=60, y=30)
    assert seen and node.get("selection_start") < 11


LIVE = """
import json
from tre import App, Window

window = Window(width=300, height=120)
field = window.create("text_input", width=240, height=28, text="hello", caret_blink=200)
window.root.add_child(field)
field.focus()
moves = []
field.on("caret_move", lambda e: moves.append(e.caret))
app = App()
app.add_window(window)
window.after(1100, window.close)
app.run()
print(json.dumps({"frames": window.frame_stats()["frames"], "moves": len(moves)}))
"""


def test_a_live_blinking_caret_wakes_the_window_at_each_edge_not_every_frame():
    import json
    import os
    import subprocess
    import sys

    if not (os.environ.get("DISPLAY") or os.environ.get("WAYLAND_DISPLAY")):
        pytest.skip("needs a display")
    result = subprocess.run([sys.executable, "-c", LIVE], capture_output=True, text=True, timeout=60)
    assert result.returncode == 0, result.stderr
    data = json.loads(result.stdout.strip().splitlines()[-1])
    # 1.1 s at a 200 ms blink: about 6 edges, a few startup frames. Not ~66 at 60 Hz.
    assert 3 <= data["frames"] < 30, data
