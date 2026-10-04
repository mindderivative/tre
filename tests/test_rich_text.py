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


# 0.5.4 (#131): links, per-span size and family, keyboard selection.


def test_size_family_and_link_spans_read_back():
    _, node = text_window()
    spans = [
        (0, 5, {"font_size": 40.0, "font_family": "Hack Nerd Font Mono"}),
        (6, 11, {"link": "https://example.com", "underline": True}),
    ]
    node.set(spans=spans)
    assert node.get("spans") == spans


def test_bad_size_family_and_link_values_are_refused():
    _, node = text_window()
    for style in ({"font_size": 0}, {"font_size": -3}, {"font_size": True},
                  {"font_size": "big"}, {"font_family": 3}, {"link": 3}):
        with pytest.raises(ValueError):
            node.set(spans=[(0, 5, style)])
    assert node.get("spans") == []
    with pytest.raises(ValueError, match="font_size, font_family or link"):
        node.set(spans=[(0, 5, {"sparkle": True})])


def test_a_size_span_changes_the_picture_and_a_link_alone_does_not():
    window, node = text_window()
    before = window.snapshot()
    node.set(spans=[(0, 5, {"link": "x"})])
    assert window.snapshot() == before
    node.set(spans=[(0, 11, {"font_size": 14})])
    assert ink(window) < ink(text_window()[0]) // 2


def test_a_size_span_makes_its_line_taller():
    window = Window(width=300, height=200)
    window.root.set(fill=WHITE, padding=0, align_items="start")
    node = window.create("text", text="Hello world", font_size=14, fill=(0, 0, 0, 255))
    window.root.add_child(node)

    def lowest_ink():
        rgba, width, height = window.snapshot()
        rows = [y for y in range(height) if any(
            rgba[(y * width + x) * 4] < 128 for x in range(width))]
        return max(rows)

    small = lowest_ink()
    node.set(spans=[(0, 5, {"font_size": 60})])
    assert lowest_ink() > small + 20


def test_a_link_event_reaches_listeners_with_its_href():
    window, node = text_window()
    node.set(spans=[(6, 11, {"link": "https://example.com/docs"})])
    seen = []
    node.on("link", lambda event: seen.append((event.type, event.href, event.target)))
    window.simulate("link", node=node, href="https://example.com/docs")
    assert len(seen) == 1
    assert seen[0][:2] == ("link", "https://example.com/docs")
    assert seen[0][2] == node


def test_a_link_event_bubbles_to_a_container_and_needs_an_href():
    window, node = text_window()
    heard = []
    window.root.on("link", lambda event: heard.append(event.href))
    window.simulate("link", node=node, href="a")
    assert heard == ["a"]
    with pytest.raises(ValueError):
        window.simulate("link", node=node)


def test_href_is_none_on_other_events():
    window = Window(width=100, height=100)
    node = window.create("box", width=50, height=50)
    window.root.add_child(node)
    seen = []
    node.on("click", lambda event: seen.append(event.href))
    window.simulate("click", node=node)
    assert seen == [None]


def test_ctrl_a_and_shift_arrows_work_on_selected_static_text():
    window, node = text_window("abcdef", selectable=True)
    node.set(selection=(2, 2))
    window.simulate("key_down", key="arrow_right", shift=True)
    window.simulate("key_down", key="arrow_right", shift=True)
    assert node.get("selection") == (2, 4)
    window.simulate("key_down", key="arrow_left", shift=True)
    assert node.get("selection") == (2, 3)
    window.simulate("key_down", key="end", shift=True)
    assert node.get("selection") == (2, 6)
    window.simulate("key_down", key="home", shift=True)
    assert node.get("selection") == (2, 0)
    window.simulate("key_down", key="a", ctrl=True)
    assert node.get("selection") == (0, 6)


def test_selecting_in_one_text_clears_the_selection_in_another():
    window = Window(width=300, height=120)
    window.root.set(fill=WHITE, padding=0)
    first = window.create("text", text="alpha", width=100, height=30, selectable=True)
    second = window.create("text", text="bravo", width=100, height=30, selectable=True)
    window.root.add_child(first)
    window.root.add_child(second)
    first.set(selection=(0, 3))
    second.set(selection=(1, 4))
    assert first.get("selection") is None
    assert second.get("selection") == (1, 4)


def test_ctrl_shift_arrows_move_the_selection_by_word_and_shift_arrows_cross_texts():
    window = Window(width=300, height=120)
    window.root.set(fill=WHITE, padding=0)
    first = window.create("text", text="one  two, three", width=200, height=30, selectable=True)
    second = window.create("text", text="next", width=200, height=30, selectable=True)
    window.root.add_child(first)
    window.root.add_child(second)
    first.set(selection=(0, 0))
    window.simulate("key_down", key="arrow_right", shift=True, ctrl=True)
    assert first.get("selection") == (0, 5)
    window.simulate("key_down", key="arrow_right", shift=True, ctrl=True)
    assert first.get("selection") == (0, 8)
    window.simulate("key_down", key="arrow_left", shift=True, ctrl=True)
    assert first.get("selection") == (0, 5)
    # Without Ctrl it is one character; at the end it goes into the next text.
    window.simulate("key_down", key="end", shift=True)
    window.simulate("key_down", key="arrow_right", shift=True)
    assert first.get("selection") == (0, 15)
    assert second.get("selection") == (0, 0)
    window.simulate("key_down", key="arrow_right", shift=True)
    assert second.get("selection") == (0, 1)


def test_ctrl_a_selects_the_selected_static_text_even_when_a_button_has_focus():
    window = Window(width=300, height=120)
    window.root.set(fill=WHITE, padding=0)
    label = window.create("text", text="alpha beta", width=200, height=30, selectable=True)
    button = window.create("box", width=60, height=30, focusable=True)
    window.root.add_child(label)
    window.root.add_child(button)
    label.set(selection=(2, 2))
    button.focus()
    window.simulate("key_down", key="a", ctrl=True)
    assert label.get("selection") == (0, 10)
