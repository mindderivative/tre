"""0.5.4 (#112): `spans` (rich text) and `selectable` (static text selection)."""

import pytest

from tre import Window

WHITE = (255, 255, 255, 255)
RED = (255, 0, 0, 255)


def text_window(content="Hello world", **props):
    window = Window(width=300, height=60)
    window.root.set(fill=WHITE, padding=0)
    node = window.create("text", text=content, font_size=28, width=290, height=50,
                         fill=(0, 0, 0, 255), **props)
    window.root.add_child(node)
    return window, node


def ink(window, x0=0, x1=300):
    rgba, width, height = window.snapshot()
    return sum(
        1
        for y in range(height)
        for x in range(x0, x1)
        if rgba[(y * width + x) * 4 : (y * width + x) * 4 + 3] != bytes([255, 255, 255])
    )


def test_spans_default_to_none_and_read_back():
    _, node = text_window()
    assert node.get("spans") == []
    spans = [(0, 5, {"color": RED, "weight": 700.0, "italic": True,
                     "underline": True, "strikethrough": True}),
             (6, 11, {"underline": True})]
    node.set(spans=spans)
    assert node.get("spans") == spans
    node.set(spans=[])
    assert node.get("spans") == []


def test_a_colour_span_colours_its_range_when_drawn():
    window, node = text_window()
    plain = window.snapshot()
    node.set(spans=[(0, 5, {"color": RED})])
    rgba, width, _ = window.snapshot()
    assert rgba != plain[0]
    reddish = sum(
        1 for i in range(0, len(rgba), 4) if rgba[i] > 200 and rgba[i + 1] < 90 and rgba[i + 2] < 90
    )
    assert reddish > 100


def test_underline_and_strikethrough_add_ink_a_bold_span_adds_more():
    window, node = text_window()
    base = ink(window)
    node.set(spans=[(0, 11, {"underline": True})])
    assert ink(window) > base
    node.set(spans=[(0, 11, {"strikethrough": True})])
    assert ink(window) > base
    node.set(spans=[(0, 11, {"weight": 500})])
    assert ink(window) > base


@pytest.mark.parametrize(
    "value, message",
    [
        ("x", "spans"),
        ([(0, 5)], "spans"),
        ([(5, 0, {})], "spans"),
        ([(0, 5, {"colour": RED})], "colour"),
        ([(0, 5, {"color": "red"})], r"\(r, g, b, a\)"),
        ([(0, 5, {"weight": True})], "weight"),
        ([(0, 5, {"weight": 0})], "weight"),
        ([(0, 5, {"underline": 1})], "bool"),
    ],
)
def test_a_bad_span_is_a_value_error_and_changes_nothing(value, message):
    _, node = text_window()
    node.set(spans=[(0, 2, {"underline": True})])
    with pytest.raises(ValueError, match=message):
        node.set(spans=value)
    assert node.get("spans") == [(0, 2, {"underline": True})]


def test_spans_apply_only_to_a_text():
    window = Window(width=100, height=100)
    box = window.create("box", width=10, height=10)
    with pytest.raises(ValueError, match="text"):
        box.set(spans=[])


def test_a_span_past_the_text_or_inside_a_character_is_harmless():
    window, node = text_window("aébc")
    node.set(spans=[(0, 2, {"color": RED}), (3, 900, {"underline": True})])
    window.snapshot()


def test_selectable_defaults_off_and_selection_is_none():
    _, node = text_window()
    assert node.get("selectable") is False
    assert node.get("selection") is None


def test_a_selection_can_be_set_read_cleared_and_drawn():
    window, node = text_window()
    node.set(selectable=True)
    plain = window.snapshot()
    node.set(selection=(0, 5))
    assert node.get("selection") == (0, 5)
    assert window.snapshot() != plain, "a highlight"
    node.set(selection=None)
    assert node.get("selection") is None
    assert window.snapshot() == plain


def test_a_selection_is_checked_against_the_text():
    _, node = text_window("héllo")
    node.set(selectable=True)
    with pytest.raises(ValueError, match="byte offsets"):
        node.set(selection=(0, 99))
    with pytest.raises(ValueError, match="byte offsets"):
        node.set(selection=(2, 1))
    with pytest.raises(ValueError, match="byte offsets"):
        node.set(selection=(0, 2))  # inside the two-byte 'é'
    node.set(selection=(0, 3))
    assert node.get("selection") == (0, 3)


def test_turning_selectable_off_clears_the_selection():
    _, node = text_window()
    node.set(selectable=True, selection=(0, 4))
    node.set(selectable=False)
    assert node.get("selection") is None


def test_copy_takes_the_selected_static_text():
    window, node = text_window("Hello world")
    if not window.write_clipboard("probe") or window.read_clipboard() != "probe":
        pytest.skip("no OS clipboard service reachable in this environment")
    node.set(selectable=True, selection=(6, 11))
    window.simulate("key_down", key="c", ctrl=True)
    assert window.read_clipboard() == "world"


def test_only_one_text_holds_a_selection_at_a_time():
    window, first = text_window("first one")
    second = window.create("text", text="second one", font_size=28, width=290, height=50)
    window.root.add_child(second)
    first.set(selectable=True, selection=(0, 5))
    second.set(selectable=True, selection=(0, 6))
    assert first.get("selection") is None
    assert second.get("selection") == (0, 6)


def test_the_selection_and_spans_are_in_the_text_nodes_snapshot_repeatably():
    window, node = text_window()
    node.set(selectable=True, selection=(0, 5), spans=[(0, 5, {"color": RED, "underline": True})])
    assert window.snapshot() == window.snapshot()
